use crate::bytecode_override::override_bytecodes;
use crate::cli::{Cli, CliReportingData, Command, ForkUrl, PeriodicStateDumper};
use crate::utils::update_with_fork_details;
use anvil_zksync_api_server::NodeServerBuilder;
use anvil_zksync_config::constants::{
    DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    DEFAULT_FAIR_PUBDATA_PRICE, DEFAULT_L1_GAS_PRICE, DEFAULT_L2_GAS_PRICE, LEGACY_RICH_WALLETS,
    RICH_WALLETS, TEST_NODE_NETWORK_ID,
};
use anvil_zksync_config::types::SystemContractsOptions;
use anvil_zksync_config::ForkPrintInfo;
use anvil_zksync_core::filters::EthFilters;
use anvil_zksync_core::node::fork::ForkClient;
use anvil_zksync_core::node::{
    BlockSealer, BlockSealerMode, ImpersonationManager, InMemoryNode, InMemoryNodeInner,
    NodeExecutor, StorageKeyLayout, TestNodeFeeInputProvider, TxPool,
};
use anvil_zksync_core::observability::Observability;
use anvil_zksync_core::system_contracts::SystemContracts;
use anyhow::{anyhow, Context};
use clap::Parser;
use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::time::Duration;
use std::{env, net::SocketAddr, str::FromStr};
use tokio::sync::OnceCell;
use tokio::sync::RwLock;
use tower_http::cors::AllowOrigin;
use tracing_subscriber::filter::LevelFilter;
use zksync_telemetry::Telemetry;
use zksync_types::fee_model::{FeeModelConfigV2, FeeParams};
use zksync_types::{L2BlockNumber, H160};

mod bytecode_override;
mod cli;
mod utils;

const POSTHOG_API_KEY: &str = "phc_TsD52JxwkT2OXPHA2oKX2Lc3mf30hItCBrE9s9g1MKe";
const TELEMETRY_CONFIG_NAME: &str = "zksync-tooling";

async fn start_program(telemetry: &'static Telemetry) -> anyhow::Result<()> {
    // Check for deprecated options
    Cli::deprecated_config_option();

    let opt = Cli::parse();
    let command = opt.command.clone();

    // Track node start
    let cli_reporting_data: CliReportingData = opt.clone().into();
    let mut event_props = HashMap::new();
    event_props.insert(
        "params".to_string(),
        serde_json::to_value(cli_reporting_data).expect("Failed to parse cli reporting data"),
    );
    let _ = telemetry.track_event("node_started", event_props).await;

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
    let (fork_client, transactions_to_replay) = match command {
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
                (None, Vec::new())
            } else {
                // Initialize the client to get the fee params
                let client =
                    ForkClient::at_block_number(ForkUrl::Mainnet.to_config(), None).await?;
                let fee = client.get_fee_params().await?;

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
                            .with_chain_id(config.chain_id.or(Some(TEST_NODE_NETWORK_ID)));
                    }
                    FeeParams::V1(_) => {
                        return Err(anyhow!("Unsupported FeeParams::V1 in this context"));
                    }
                }

                (None, Vec::new())
            }
        }
        Command::Fork(fork) => {
            let (fork_client, earlier_txs) = if let Some(tx_hash) = fork.fork_transaction_hash {
                // If transaction hash is provided, we fork at the parent of block containing tx
                ForkClient::at_before_tx(fork.fork_url.to_config(), tx_hash).await?
            } else {
                // Otherwise, we fork at the provided block
                (
                    ForkClient::at_block_number(
                        fork.fork_url.to_config(),
                        fork.fork_block_number.map(|bn| L2BlockNumber(bn as u32)),
                    )
                    .await?,
                    Vec::new(),
                )
            };

            update_with_fork_details(&mut config, &fork_client.details).await;
            (Some(fork_client), earlier_txs)
        }
        Command::ReplayTx(replay_tx) => {
            let (fork_client, earlier_txs) =
                ForkClient::at_before_tx(replay_tx.fork_url.to_config(), replay_tx.tx).await?;

            update_with_fork_details(&mut config, &fork_client.details).await;
            (Some(fork_client), earlier_txs)
        }
    };

    if matches!(
        config.system_contracts_options,
        SystemContractsOptions::Local
    ) {
        if let Some(path) = env::var_os("ZKSYNC_HOME") {
            tracing::info!("+++++ Reading local contracts from {:?} +++++", path);
        }
    }

    let fork_print_info = if let Some(fork_client) = &fork_client {
        let fee_model_config_v2 = match &fork_client.details.fee_params {
            FeeParams::V2(fee_params_v2) => {
                let config = fee_params_v2.config();
                FeeModelConfigV2 {
                    minimal_l2_gas_price: config.minimal_l2_gas_price,
                    compute_overhead_part: config.compute_overhead_part,
                    pubdata_overhead_part: config.pubdata_overhead_part,
                    batch_overhead_l1_gas: config.batch_overhead_l1_gas,
                    max_gas_per_batch: config.max_gas_per_batch,
                    max_pubdata_per_batch: config.max_pubdata_per_batch,
                }
            }
            _ => anyhow::bail!(
                "fork is using unsupported fee parameters: {:?}",
                fork_client.details.fee_params
            ),
        };

        Some(ForkPrintInfo {
            network_rpc: fork_client.url.to_string(),
            l1_block: fork_client.details.batch_number.to_string(),
            l2_block: fork_client.details.block_number.to_string(),
            block_timestamp: fork_client.details.block_timestamp.to_string(),
            fork_block_hash: format!("{:#x}", fork_client.details.block_hash),
            fee_model_config_v2,
        })
    } else {
        None
    };

    let impersonation = ImpersonationManager::default();
    if config.enable_auto_impersonate {
        // Enable auto impersonation if configured
        impersonation.set_auto_impersonation(true);
    }
    let pool = TxPool::new(impersonation.clone(), config.transaction_order);

    let fee_input_provider =
        TestNodeFeeInputProvider::from_fork(fork_client.as_ref().map(|f| &f.details));
    let filters = Arc::new(RwLock::new(EthFilters::default()));
    let system_contracts = SystemContracts::from_options(
        &config.system_contracts_options,
        config.use_evm_emulator,
        config.use_zkos,
    );
    let storage_key_layout = if config.use_zkos {
        StorageKeyLayout::ZkOs
    } else {
        StorageKeyLayout::ZkEra
    };

    let (node_inner, storage, blockchain, time, fork) = InMemoryNodeInner::init(
        fork_client,
        fee_input_provider.clone(),
        filters,
        config.clone(),
        impersonation.clone(),
        system_contracts.clone(),
        storage_key_layout,
    );

    let (node_executor, node_handle) = NodeExecutor::new(
        node_inner.clone(),
        system_contracts.clone(),
        storage_key_layout,
    );
    let sealing_mode = if config.no_mining {
        BlockSealerMode::noop()
    } else if let Some(block_time) = config.block_time {
        BlockSealerMode::fixed_time(config.max_transactions, block_time)
    } else {
        BlockSealerMode::immediate(config.max_transactions, pool.add_tx_listener())
    };
    let (block_sealer, block_sealer_state) =
        BlockSealer::new(sealing_mode, pool.clone(), node_handle.clone());

    let node: InMemoryNode = InMemoryNode::new(
        node_inner,
        blockchain,
        storage,
        fork,
        node_handle,
        Some(observability),
        time,
        impersonation,
        pool,
        block_sealer_state,
        system_contracts,
        storage_key_layout,
    );

    // We start the node executor now so it can receive and handle commands
    // during replay. Otherwise, replay would send commands and hang.
    tokio::spawn(async move {
        if let Err(err) = node_executor.run().await {
            let error = err.context("Node executor ended with error");
            tracing::error!("{:?}", error);

            let boxed_error: Box<&(dyn std::error::Error + Send + Sync)> = Box::new(error.as_ref());
            let _ = telemetry.track_error(boxed_error).await;
        }
    });

    if let Some(ref bytecodes_dir) = config.override_bytecodes_dir {
        override_bytecodes(&node, bytecodes_dir.to_string())
            .await
            .unwrap();
    }

    if !transactions_to_replay.is_empty() {
        node.apply_txs(transactions_to_replay, config.max_transactions)
            .await?;
    }

    for signer in config.genesis_accounts.iter() {
        let address = H160::from_slice(signer.address().as_ref());
        node.set_rich_account(address, config.genesis_balance).await;
    }
    for signer in config.signer_accounts.iter() {
        let address = H160::from_slice(signer.address().as_ref());
        node.set_rich_account(address, config.genesis_balance).await;
    }
    // sets legacy rich wallets
    for wallet in LEGACY_RICH_WALLETS.iter() {
        let address = wallet.0;
        node.set_rich_account(H160::from_str(address).unwrap(), config.genesis_balance)
            .await;
    }
    // sets additional legacy rich wallets
    for wallet in RICH_WALLETS.iter() {
        let address = wallet.0;
        node.set_rich_account(H160::from_str(address).unwrap(), config.genesis_balance)
            .await;
    }

    let mut server_builder = NodeServerBuilder::new(
        node.clone(),
        AllowOrigin::exact(
            config
                .allow_origin
                .parse()
                .context("allow origin is malformed")?,
        ),
        telemetry,
    );
    if config.health_check_endpoint {
        server_builder.enable_health_api()
    }
    if !config.no_cors {
        server_builder.enable_cors();
    }
    let mut server_handles = Vec::with_capacity(config.host.len());
    for host in &config.host {
        let mut addr = SocketAddr::new(*host, config.port);

        match server_builder.clone().build(addr).await {
            Ok(server) => {
                config.port = server.local_addr().port();
                server_handles.push(server.run());
            }
            Err(err) => {
                tracing::info!(
                    "Failed to bind to address {}:{}: {}. Retrying with a different port...",
                    host,
                    config.port,
                    err
                );

                // Attempt to bind to a dynamic port
                addr.set_port(0);
                match server_builder.clone().build(addr).await {
                    Ok(server) => {
                        config.port = server.local_addr().port();
                        tracing::info!(
                            "Successfully started server on port {} for host {}",
                            config.port,
                            host
                        );
                        server_handles.push(server.run());
                    }
                    Err(err) => {
                        return Err(anyhow!(
                            "Failed to start server on host {} with port: {}",
                            host,
                            err
                        ));
                    }
                }
            }
        }
    }
    let any_server_stopped =
        futures::future::select_all(server_handles.into_iter().map(|h| Box::pin(h.stopped())));

    // Load state from `--load-state` if provided
    if let Some(ref load_state_path) = config.load_state {
        let bytes = std::fs::read(load_state_path).expect("Failed to read load state file");
        node.load_state(zksync_types::web3::Bytes(bytes)).await?;
    }
    if let Some(ref state_path) = config.state {
        let bytes = std::fs::read(state_path).expect("Failed to read load state file");
        node.load_state(zksync_types::web3::Bytes(bytes)).await?;
    }

    let state_path = config.dump_state.clone().or_else(|| config.state.clone());
    let dump_interval = config
        .state_interval
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(60)); // Default to 60 seconds
    let preserve_historical_states = config.preserve_historical_states;
    let node_for_dumper = node.clone();
    let state_dumper = PeriodicStateDumper::new(
        node_for_dumper,
        state_path,
        dump_interval,
        preserve_historical_states,
    );

    config.print(fork_print_info.as_ref());

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::trace!("received shutdown signal, shutting down");
        },
        _ = any_server_stopped => {
            tracing::trace!("node server was stopped")
        },
        _ = block_sealer.run() => {
            tracing::trace!("block sealer was stopped")
        },
        _ = state_dumper => {
            tracing::trace!("state dumper was stopped")
        },
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    static TELEMETRY: OnceCell<Telemetry> = OnceCell::const_new();
    let telemetry = TELEMETRY
        .get_or_init(|| async {
            Telemetry::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                TELEMETRY_CONFIG_NAME,
                Some(POSTHOG_API_KEY.into()),
                None,
                None,
            )
            .await
            .expect("Failed to initialize telemetry")
        })
        .await;

    if let Err(err) = start_program(telemetry).await {
        let _ = telemetry.track_error(Box::new(err.as_ref())).await;
        anyhow::bail!(err)
    }
    Ok(())
}
