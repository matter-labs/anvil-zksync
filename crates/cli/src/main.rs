use crate::bytecode_override::override_bytecodes;
use crate::cli::{Cli, Command};
use crate::utils::update_with_fork_details;
use anvil_zksync_api_decl::{
    AnvilNamespaceServer, ConfigNamespaceServer, DebugNamespaceServer, EthNamespaceServer,
    EthTestNamespaceServer, EvmNamespaceServer, NetNamespaceServer, Web3NamespaceServer,
    ZksNamespaceServer,
};
use anvil_zksync_api_server::{
    AnvilNamespace, ConfigNamespace, DebugNamespace, EthNamespace, EthTestNamespace, EvmNamespace,
    NetNamespace, Web3Namespace, ZksNamespace,
};
use anvil_zksync_config::constants::{
    DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    DEFAULT_FAIR_PUBDATA_PRICE, DEFAULT_L1_GAS_PRICE, DEFAULT_L2_GAS_PRICE, LEGACY_RICH_WALLETS,
    RICH_WALLETS, TEST_NODE_NETWORK_ID,
};
use anvil_zksync_config::types::SystemContractsOptions;
use anvil_zksync_config::ForkPrintInfo;
use anvil_zksync_core::fork::ForkDetails;
use anvil_zksync_core::node::{
    BlockProducer, BlockSealer, BlockSealerMode, ImpersonationManager, InMemoryNode,
    TimestampManager, TxPool,
};
use anvil_zksync_core::observability::Observability;
use anvil_zksync_core::system_contracts::SystemContracts;
use anyhow::anyhow;
use clap::Parser;
use futures::{
    channel::oneshot,
    future::{self},
    FutureExt,
};
use http::Method;
use jsonrpsee::server::middleware::http::ProxyGetRequestLayer;
use jsonrpsee::server::{RpcServiceBuilder, ServerBuilder};
use std::fs::File;
use std::{env, net::SocketAddr, str::FromStr};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber::filter::LevelFilter;
use zksync_types::fee_model::{FeeModelConfigV2, FeeParams};
use zksync_types::H160;
use zksync_web3_decl::jsonrpsee::RpcModule;
use zksync_web3_decl::namespaces::ZksNamespaceClient;

mod bytecode_override;
mod cli;
mod utils;

#[allow(clippy::too_many_arguments)]
async fn build_json_http(
    addr: SocketAddr,
    node: InMemoryNode,
    enable_health_api: bool,
    cors_allow_origin: String,
    enable_cors: bool,
) -> tokio::task::JoinHandle<()> {
    let (sender, recv) = oneshot::channel::<()>();

    let mut rpc = RpcModule::new(());
    rpc.merge(EthNamespace::new(node.clone()).into_rpc())
        .unwrap();
    rpc.merge(EthTestNamespace::new(node.clone()).into_rpc())
        .unwrap();
    rpc.merge(AnvilNamespace::new(node.clone()).into_rpc())
        .unwrap();
    rpc.merge(EvmNamespace::new(node.clone()).into_rpc())
        .unwrap();
    rpc.merge(DebugNamespace::new(node.clone()).into_rpc())
        .unwrap();
    rpc.merge(NetNamespace::new(node.clone()).into_rpc())
        .unwrap();
    rpc.merge(ConfigNamespace::new(node.clone()).into_rpc())
        .unwrap();
    rpc.merge(ZksNamespace::new(node.clone()).into_rpc())
        .unwrap();
    rpc.merge(Web3Namespace.into_rpc()).unwrap();

    let cors_layers = tower::util::option_layer(enable_cors.then(|| {
        // `CorsLayer` adds CORS-specific headers to responses but does not do filtering by itself.
        // CORS relies on browsers respecting server's access list response headers.
        // See [`tower_http::cors`](https://docs.rs/tower-http/latest/tower_http/cors/index.html)
        // for more details.
        let cors_layer = CorsLayer::new()
            .allow_origin(AllowOrigin::exact(
                cors_allow_origin.parse().expect("malformed allow origin"),
            ))
            .allow_headers([http::header::CONTENT_TYPE])
            .allow_methods([Method::GET, Method::POST]);
        cors_layer
    }));
    let health_api_layer = tower::util::option_layer(if enable_health_api {
        Some(ProxyGetRequestLayer::new("/health", "web3_clientVersion").unwrap())
    } else {
        None
    });
    let server_builder = ServerBuilder::default()
        .http_only()
        .set_http_middleware(
            tower::ServiceBuilder::new()
                .layer(cors_layers)
                .layer(health_api_layer),
        )
        .set_rpc_middleware(RpcServiceBuilder::new().rpc_logger(1024));

    let server = server_builder.build(addr).await.unwrap();

    tokio::spawn(async move {
        let server_handle = server.start(rpc);

        server_handle.stopped().await;
        drop(sender);
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
    let observability = Observability::init(
        vec!["anvil_zksync".into()],
        log_level_filter,
        log_file,
        config.silent,
    )?;

    // Use `Command::Run` as default.
    let command = command.as_ref().unwrap_or(&Command::Run);
    let fork_details = match command {
        Command::Run => {
            if config.offline {
                tracing::warn!("Running in offline mode: default fee parameters will be used.");
                config = config
                    .clone()
                    .with_l1_gas_price(config.l1_gas_price.or(Some(DEFAULT_L1_GAS_PRICE)))
                    .with_l2_gas_price(config.l2_gas_price.or(Some(DEFAULT_L2_GAS_PRICE)))
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
                        config.l1_pubdata_price.or(Some(DEFAULT_FAIR_PUBDATA_PRICE)),
                    )
                    .with_chain_id(config.chain_id.or(Some(TEST_NODE_NETWORK_ID)));
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
                            )
                            .with_chain_id(Some(TEST_NODE_NETWORK_ID));
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

            update_with_fork_details(&mut config, fork_details_result).await?
        }
        Command::ReplayTx(replay_tx) => {
            let fork_details_result = ForkDetails::from_network_tx(
                &replay_tx.fork_url,
                replay_tx.tx,
                &config.cache_config,
            )
            .await;

            update_with_fork_details(&mut config, fork_details_result).await?
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
        SystemContractsOptions::Local
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

    let time = TimestampManager::default();
    let impersonation = ImpersonationManager::default();
    let pool = TxPool::new(impersonation.clone());
    let sealing_mode = if config.no_mining {
        BlockSealerMode::noop()
    } else if let Some(block_time) = config.block_time {
        BlockSealerMode::fixed_time(config.max_transactions, block_time)
    } else {
        BlockSealerMode::immediate(config.max_transactions, pool.add_tx_listener())
    };
    let block_sealer = BlockSealer::new(sealing_mode);

    let node: InMemoryNode = InMemoryNode::new(
        fork_details,
        Some(observability),
        &config,
        time.clone(),
        impersonation,
        pool.clone(),
        block_sealer.clone(),
    );

    if let Some(ref bytecodes_dir) = config.override_bytecodes_dir {
        override_bytecodes(&node, bytecodes_dir.to_string()).unwrap();
    }

    if !transactions_to_replay.is_empty() {
        let _ = node.apply_txs(transactions_to_replay, config.max_transactions);
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

    let mut threads = future::join_all(config.host.iter().map(|host| {
        let addr = SocketAddr::new(*host, config.port);
        build_json_http(
            addr,
            node.clone(),
            config.health_check_endpoint,
            config.allow_origin.clone(),
            !config.no_cors,
        )
    }))
    .await;

    let system_contracts =
        SystemContracts::from_options(&config.system_contracts_options, config.use_evm_emulator);
    let block_producer_handle = tokio::task::spawn(BlockProducer::new(
        node.clone(),
        pool,
        block_sealer,
        system_contracts,
    ));
    threads.push(block_producer_handle);

    config.print(fork_print_info.as_ref());

    future::select_all(threads).await.0.unwrap();

    Ok(())
}
