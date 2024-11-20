use crate::observability::Observability;
use anyhow::anyhow;
use bytecode_override::override_bytecodes;
use clap::Parser;
use colored::Colorize;
use config::cli::{Cli, Command};
use config::constants::{
    DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    LEGACY_RICH_WALLETS, RICH_WALLETS,
};
use config::TestNodeConfig;
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
use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};
use zksync_types::fee_model::FeeParams;
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
use crate::node::{BlockProducer, BlockSealer, ImpersonationManager, TimestampManager, TxPool};
use crate::system_contracts::SystemContracts;

#[allow(clippy::too_many_arguments)]
async fn build_json_http<
    S: std::marker::Sync + std::marker::Send + 'static + ForkSource + std::fmt::Debug + Clone,
>(
    addr: SocketAddr,
    log_level_filter: LevelFilter,
    node: InMemoryNode<S>,
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

        let server = jsonrpc_http_server::ServerBuilder::new(io_handler)
            .threads(1)
            .event_loop_executor(runtime.handle().clone())
            .start_http(&addr)
            .unwrap();

        server.wait();
        let _ = sender;
    });

    tokio::spawn(recv.map(drop))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Cli::parse();

    // Try to read the [`TestNodeConfig`] file if supplied as an argument.
    let mut config = TestNodeConfig::try_load(&opt.config).unwrap_or_default();
    config.override_with_opts(&opt);

    let log_level_filter = LevelFilter::from(config.log_level);
    let log_file = File::create(&config.log_file_path)?;

    // Initialize the tracing subscriber
    let observability =
        Observability::init(vec!["era_test_node".into()], log_level_filter, log_file)?;

    // Use `Command::Run` as default.
    let command = opt.command.as_ref().unwrap_or(&Command::Run);
    let fork_details = match command {
        Command::Run => {
            if opt.offline {
                tracing::warn!(
                    "Running in offline mode: default fee parameters will be used. \
        To override, specify values in `config.toml` and use the `--config` flag."
                );
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
                            .with_l1_gas_price(Some(fee_v2.l1_gas_price()))
                            .with_l2_gas_price(Some(fee_v2.config().minimal_l2_gas_price))
                            .with_price_scale(Some(DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR))
                            .with_gas_limit_scale(Some(DEFAULT_ESTIMATE_GAS_SCALE_FACTOR))
                            .with_l1_pubdata_price(Some(fee_v2.l1_pubdata_price()));
                    }
                    FeeParams::V1(_) => {
                        return Err(anyhow!("Unsupported FeeParams::V1 in this context"));
                    }
                }

                None
            }
        }
        Command::Fork(fork) => {
            match ForkDetails::from_network(
                &fork.network,
                fork.fork_block_number,
                &config.cache_config,
            )
            .await
            {
                Ok(fd) => Some(fd),
                Err(error) => {
                    tracing::error!("cannot fork: {:?}", error);
                    return Err(anyhow!(error));
                }
            }
        }
        Command::ReplayTx(replay_tx) => {
            match ForkDetails::from_network_tx(
                &replay_tx.network,
                replay_tx.tx,
                &config.cache_config,
            )
            .await
            {
                Ok(fd) => Some(fd),
                Err(error) => {
                    tracing::error!("cannot replay: {:?}", error);
                    return Err(anyhow!(error));
                }
            }
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

    let time = TimestampManager::default();
    let impersonation = ImpersonationManager::default();
    let pool = TxPool::new(impersonation.clone());
    let node: InMemoryNode<HttpForkSource> = InMemoryNode::new(
        fork_details,
        Some(observability),
        &config,
        time.clone(),
        impersonation,
        pool.clone(),
    );

    if let Some(bytecodes_dir) = opt.override_bytecodes_dir {
        override_bytecodes(&node, bytecodes_dir).unwrap();
    }

    if !transactions_to_replay.is_empty() {
        let _ = node.apply_txs(transactions_to_replay);
    }

    tracing::info!("");
    tracing::info!("Rich Accounts");
    tracing::info!("=============");
    for wallet in LEGACY_RICH_WALLETS.iter() {
        let address = wallet.0;
        node.set_rich_account(H160::from_str(address).unwrap());
    }
    for (index, wallet) in RICH_WALLETS.iter().enumerate() {
        let address = wallet.0;
        let private_key = wallet.1;
        let mnemonic_phrase = wallet.2;
        node.set_rich_account(H160::from_str(address).unwrap());
        tracing::info!(
            "Account #{}: {} ({})",
            index,
            address,
            "1_000_000_000_000 ETH".cyan()
        );
        tracing::info!("Private Key: {}", private_key);
        tracing::info!("Mnemonic: {}", &mnemonic_phrase.truecolor(128, 128, 128));
        tracing::info!("");
    }

    let threads = build_json_http(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), config.port),
        log_level_filter,
        node.clone(),
    )
    .await;

    let block_sealer = if let Some(block_time) = opt.block_time {
        BlockSealer::fixed_time(config.max_transactions, block_time)
    } else {
        BlockSealer::immediate(config.max_transactions)
    };
    let system_contracts =
        SystemContracts::from_options(&config.system_contracts_options, config.use_evm_emulator);
    let block_producer_handle = tokio::task::spawn(BlockProducer::new(
        node,
        pool,
        block_sealer,
        system_contracts,
    ));

    tracing::info!("========================================");
    tracing::info!("  Node is ready at 127.0.0.1:{}", config.port);
    tracing::info!("========================================");

    future::select_all(vec![threads, block_producer_handle])
        .await
        .0
        .unwrap();

    Ok(())
}
