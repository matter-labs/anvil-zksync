use crate::observability::Observability;
use anyhow::anyhow;
use bytecode_override::override_bytecodes;
use clap::Parser;
use config::cli::{Cli, Command};
use config::constants::{
    DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    LEGACY_RICH_WALLETS, RICH_WALLETS,
};
use config::ForkPrintInfo;
use fork::{ForkDetails, ForkSource};
use http_fork_source::HttpForkSource;
use logging_middleware::LoggingMiddleware;
use tracing_subscriber::filter::LevelFilter;

mod bootloader_debug;
mod bytecode_override;
mod cache;
mod config;
mod console_log;
mod deps;
mod filters;
mod fork;
mod formatter;
mod http_fork_source;
mod logging_middleware;
mod namespaces;
mod node;
mod observability;
mod resolver;
mod system_contracts;
mod testing;
mod utils;

use node::InMemoryNode;
use std::fs::File;
use std::{env, net::SocketAddr, str::FromStr};
use zksync_types::fee_model::{FeeModelConfigV2, FeeParams};
use zksync_web3_decl::namespaces::ZksNamespaceClient;

use futures::{
    channel::oneshot,
    future::{self},
    FutureExt,
};
use jsonrpc_core::MetaIoHandler;
use zksync_types::H160;

use crate::namespaces::{
    AnvilNamespaceT, ConfigurationApiNamespaceT, DebugNamespaceT, EthNamespaceT,
    EthTestNodeNamespaceT, EvmNamespaceT, HardhatNamespaceT, NetNamespaceT, Web3NamespaceT,
    ZksNamespaceT,
};

#[allow(clippy::too_many_arguments)]
async fn build_json_http<
    S: std::marker::Sync + std::marker::Send + 'static + ForkSource + std::fmt::Debug + Clone,
>(
    addr: SocketAddr,
    log_level_filter: LevelFilter,
    node: InMemoryNode<S>,
    enable_health_api: bool,
) -> tokio::task::JoinHandle<()> {
    let (sender, recv) = oneshot::channel::<()>();

    let io_handler = {
        let mut io = MetaIoHandler::with_middleware(LoggingMiddleware::new(log_level_filter));

        io.extend_with(NetNamespaceT::to_delegate(node.clone()));
        io.extend_with(Web3NamespaceT::to_delegate(node.clone()));
        io.extend_with(ConfigurationApiNamespaceT::to_delegate(node.clone()));
        io.extend_with(DebugNamespaceT::to_delegate(node.clone()));
        io.extend_with(EthNamespaceT::to_delegate(node.clone()));
        io.extend_with(EthTestNodeNamespaceT::to_delegate(node.clone()));
        io.extend_with(AnvilNamespaceT::to_delegate(node.clone()));
        io.extend_with(EvmNamespaceT::to_delegate(node.clone()));
        io.extend_with(HardhatNamespaceT::to_delegate(node.clone()));
        io.extend_with(ZksNamespaceT::to_delegate(node));
        io
    };

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .build()
            .unwrap();

        let mut builder = jsonrpc_http_server::ServerBuilder::new(io_handler)
            .threads(1)
            .event_loop_executor(runtime.handle().clone());

        if enable_health_api {
            builder = builder.health_api(("/health", "web3_clientVersion"));
        }

        let server = builder.start_http(&addr).unwrap();

        server.wait();
        let _ = sender;
    });

    tokio::spawn(recv.map(drop))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Check for deprecated options
    Cli::deprecated_config_option();

    let opt = Cli::parse();
    let command = opt.command.clone();

    let mut config = opt.into_test_node_config().map_err(|e| anyhow!(e))?;

    let log_level_filter = LevelFilter::from(config.log_level);
    let log_file = File::create(&config.log_file_path)?;

    // Initialize the tracing subscriber
    let observability =
        Observability::init(vec!["era_test_node".into()], log_level_filter, log_file)?;

    // Use `Command::Run` as default.
    let command = command.as_ref().unwrap_or(&Command::Run);
    let fork_details = match command {
        Command::Run => {
            if config.offline {
                tracing::warn!("Running in offline mode: default fee parameters will be used.");
                None
            } else {
                // Initialize the client to get the fee params
                let (_, client) = ForkDetails::fork_network_and_client("mainnet")
                    .map_err(|e| anyhow!("Failed to initialize client: {:?}", e))?;

                let fee = client.get_fee_params().await.map_err(|e| {
                    tracing::error!("Failed to fetch fee params: {:?}", e);
                    anyhow!(e)
                })?;

                match fee {
                    FeeParams::V2(fee_v2) => {
                        config = config
                            .clone()
                            .with_l1_gas_price(config.l1_gas_price.or(Some(fee_v2.l1_gas_price())))
                            .with_l2_gas_price(
                                config
                                    .l2_gas_price
                                    .or(Some(fee_v2.config().minimal_l2_gas_price)),
                            )
                            .with_price_scale(
                                config
                                    .price_scale_factor
                                    .or(Some(DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR)),
                            )
                            .with_gas_limit_scale(
                                config
                                    .limit_scale_factor
                                    .or(Some(DEFAULT_ESTIMATE_GAS_SCALE_FACTOR)),
                            )
                            .with_l1_pubdata_price(
                                config.l1_pubdata_price.or(Some(fee_v2.l1_pubdata_price())),
                            );
                    }
                    FeeParams::V1(_) => {
                        return Err(anyhow!("Unsupported FeeParams::V1 in this context"));
                    }
                }

                None
            }
        }
        Command::Fork(fork) => {
            let fork_details_result = if let Some(tx_hash) = fork.fork_transaction_hash {
                // If fork_transaction_hash is provided, use from_network_tx
                ForkDetails::from_network_tx(&fork.fork_url, tx_hash, &config.cache_config).await
            } else {
                // Otherwise, use from_network
                ForkDetails::from_network(
                    &fork.fork_url,
                    fork.fork_block_number,
                    &config.cache_config,
                )
                .await
            };

            config.update_with_fork_details(fork_details_result).await?
        }
        Command::ReplayTx(replay_tx) => {
            let fork_details_result = ForkDetails::from_network_tx(
                &replay_tx.fork_url,
                replay_tx.tx,
                &config.cache_config,
            )
            .await;

            config.update_with_fork_details(fork_details_result).await?
        }
    };

    // If we're replaying the transaction, we need to sync to the previous block
    // and then replay all the transactions that happened in
    let transactions_to_replay = if let Command::ReplayTx(replay_tx) = command {
        match fork_details
            .as_ref()
            .unwrap()
            .get_earlier_transactions_in_same_block(replay_tx.tx)
        {
            Ok(txs) => txs,
            Err(error) => {
                tracing::error!(
                    "failed to get earlier transactions in the same block for replay tx: {:?}",
                    error
                );
                return Err(anyhow!(error));
            }
        }
    } else {
        vec![]
    };

    if matches!(
        config.system_contracts_options,
        system_contracts::Options::Local
    ) {
        if let Some(path) = env::var_os("ZKSYNC_HOME") {
            tracing::info!("+++++ Reading local contracts from {:?} +++++", path);
        }
    }

    let fork_print_info = if let Some(fd) = fork_details.as_ref() {
        let fee_model_config_v2 = match fd.fee_params {
            Some(FeeParams::V2(fee_params_v2)) => {
                let config = fee_params_v2.config();
                Some(FeeModelConfigV2 {
                    minimal_l2_gas_price: config.minimal_l2_gas_price,
                    compute_overhead_part: config.compute_overhead_part,
                    pubdata_overhead_part: config.pubdata_overhead_part,
                    batch_overhead_l1_gas: config.batch_overhead_l1_gas,
                    max_gas_per_batch: config.max_gas_per_batch,
                    max_pubdata_per_batch: config.max_pubdata_per_batch,
                })
            }
            _ => None,
        };

        Some(ForkPrintInfo {
            network_rpc: fd.fork_source.get_fork_url().unwrap_or_default(),
            l1_block: fd.l1_block.to_string(),
            l2_block: fd.l2_miniblock.to_string(),
            block_timestamp: fd.block_timestamp.to_string(),
            fork_block_hash: format!("{:#x}", fd.l2_block.hash),
            fee_model_config_v2,
        })
    } else {
        None
    };

    let node: InMemoryNode<HttpForkSource> =
        InMemoryNode::new(fork_details, Some(observability), &config);

    if let Some(ref bytecodes_dir) = config.override_bytecodes_dir {
        override_bytecodes(&node, bytecodes_dir.to_string()).unwrap();
    }

    if !transactions_to_replay.is_empty() {
        let _ = node.apply_txs(transactions_to_replay);
    }

    for signer in config.genesis_accounts.iter() {
        let address = H160::from_slice(signer.address().as_ref());
        node.set_rich_account(address, config.genesis_balance);
    }
    for signer in config.signer_accounts.iter() {
        let address = H160::from_slice(signer.address().as_ref());
        node.set_rich_account(address, config.genesis_balance);
    }
    // sets legacy rich wallets
    for wallet in LEGACY_RICH_WALLETS.iter() {
        let address = wallet.0;
        node.set_rich_account(H160::from_str(address).unwrap(), config.genesis_balance);
    }
    // sets additional legacy rich wallets
    for wallet in RICH_WALLETS.iter() {
        let address = wallet.0;
        node.set_rich_account(H160::from_str(address).unwrap(), config.genesis_balance);
    }

    let threads = future::join_all(config.host.iter().map(|host| {
        let addr = SocketAddr::new(*host, config.port);
        build_json_http(
            addr,
            log_level_filter,
            node.clone(),
            config.health_check_endpoint,
        )
    }))
    .await;

    config.print(fork_print_info.as_ref());

    future::select_all(threads).await.0.unwrap();

    Ok(())
}
