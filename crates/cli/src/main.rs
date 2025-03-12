use crate::bytecode_override::override_bytecodes;
use crate::cli::{Cli, Command, ForkUrl, PeriodicStateDumper};
use crate::utils::update_with_fork_details;
use anvil_zksync_api_server::NodeServerBuilder;
use anvil_zksync_common::{sh_eprintln, sh_err, sh_warn};
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
use anvil_zksync_l1_sidecar::L1Sidecar;
use anyhow::Context;
use clap::Parser;
use std::fs::File;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::{env, net::SocketAddr, str::FromStr};
use tokio::sync::RwLock;
use tower_http::cors::AllowOrigin;
use tracing_subscriber::filter::LevelFilter;
use zksync_error::anvil_zksync::gen::{generic_error, to_domain};
use zksync_error::anvil_zksync::AnvilZksyncError;
use zksync_error::{ICustomError, IError as _};
use zksync_telemetry::{get_telemetry, init_telemetry, TelemetryProps};
use zksync_types::fee_model::{FeeModelConfigV2, FeeParams};
use zksync_types::{L2BlockNumber, H160};

mod bytecode_override;
mod cli;
mod utils;

const POSTHOG_API_KEY: &str = "phc_TsD52JxwkT2OXPHA2oKX2Lc3mf30hItCBrE9s9g1MKe";
const TELEMETRY_CONFIG_NAME: &str = "zksync-tooling";

async fn start_program() -> Result<(), AnvilZksyncError> {
    // Check for deprecated options
    Cli::deprecated_config_option();

    let opt = Cli::parse();
    // We keep a serialized version of the provided arguments to communicate them later if the arguments were incorrect.
    let debug_opt_string_repr = format!("{opt:#?}");

    let command = opt.command.clone();

    // Track node start
    let telemetry = get_telemetry().expect("telemetry is not initialized");
    let cli_telemetry_props = opt.clone().into_telemetry_props();
    let _ = telemetry
        .track_event(
            "node_started",
            TelemetryProps::new()
                .insert("params", Some(cli_telemetry_props))
                .take(),
        )
        .await;

    let mut config = opt.into_test_node_config()?;
    let log_level_filter = LevelFilter::from(config.log_level);
    let log_file = File::create(&config.log_file_path).map_err(|inner| {
        zksync_error::anvil_zksync::env::LogFileAccessFailed {
            log_file_path: config.log_file_path.to_string(),
            wrapped_error: inner.to_string(),
        }
    })?;

    // Initialize the tracing subscriber
    let observability = Observability::init(
        vec!["anvil_zksync".into()],
        log_level_filter,
        log_file,
        config.silent,
    )
    .map_err(|error| zksync_error::anvil_zksync::env::GenericError {
        message: format!(
            "Internal error: Unable to set up observability. Please report. \n{error:#?}"
        ),
    })?;

    // Use `Command::Run` as default.
    let command = command.as_ref().unwrap_or(&Command::Run);
    let (fork_client, transactions_to_replay) = match command {
        Command::Run => {
            if config.offline {
                sh_warn!("Running in offline mode: default fee parameters will be used.");
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
                let client = ForkClient::at_block_number(ForkUrl::Mainnet.to_config(), None)
                    .await
                    .map_err(to_domain)?;
                let fee = client.get_fee_params().await.map_err(to_domain)?;

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
                        return Err(
                            generic_error!("Unsupported FeeParams::V1 in this context").into()
                        );
                    }
                }

                (None, Vec::new())
            }
        }
        Command::Fork(fork) => {
            let (fork_client, earlier_txs) = if let Some(tx_hash) = fork.fork_transaction_hash {
                // If transaction hash is provided, we fork at the parent of block containing tx
                ForkClient::at_before_tx(fork.fork_url.to_config(), tx_hash)
                    .await
                    .map_err(to_domain)?
            } else {
                // Otherwise, we fork at the provided block
                (
                    ForkClient::at_block_number(
                        fork.fork_url.to_config(),
                        fork.fork_block_number.map(|bn| L2BlockNumber(bn as u32)),
                    )
                    .await
                    .map_err(to_domain)?,
                    Vec::new(),
                )
            };

            update_with_fork_details(&mut config, &fork_client.details).await;
            (Some(fork_client), earlier_txs)
        }
        Command::ReplayTx(replay_tx) => {
            let (fork_client, earlier_txs) =
                ForkClient::at_before_tx(replay_tx.fork_url.to_config(), replay_tx.tx)
                    .await
                    .map_err(to_domain)?;

            update_with_fork_details(&mut config, &fork_client.details).await;
            (Some(fork_client), earlier_txs)
        }
    };

    if matches!(
        config.system_contracts_options,
        SystemContractsOptions::Local
    ) {
        if let Some(path) = env::var_os("ZKSYNC_HOME") {
            tracing::debug!("Reading local contracts from {:?}", path);
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
            _ => {
                return Err(to_domain(generic_error!(
                    "fork is using unsupported fee parameters: {:?}",
                    fork_client.details.fee_params
                )))
            }
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

    let mut node_service_tasks: Vec<Pin<Box<dyn Future<Output = anyhow::Result<()>>>>> = Vec::new();
    let l1_sidecar = match config.l1_config.as_ref() {
        Some(_) if fork_print_info.is_some() => {
            return Err(zksync_error::anvil_zksync::env::InvalidArguments {
                details: "Running L1 in forking mode is unsupported".into(),
                arguments: debug_opt_string_repr,
            }
            .into())
        }
        Some(l1_config) => {
            let (l1_sidecar, l1_sidecar_runner) =
                L1Sidecar::builtin(l1_config.port, blockchain.clone())
                    .await
                    .map_err(to_domain)?;
            node_service_tasks.push(Box::pin(l1_sidecar_runner.run()));
            l1_sidecar
        }
        None => L1Sidecar::none(),
    };

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
    node_service_tasks.push(Box::pin(block_sealer.run()));

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
            sh_err!("{:?}", error);

            let _ = telemetry.track_error(Box::new(error.as_ref())).await;
        }
    });

    if let Some(ref bytecodes_dir) = config.override_bytecodes_dir {
        override_bytecodes(&node, bytecodes_dir.to_string())
            .await
            .unwrap();
    }

    if !transactions_to_replay.is_empty() {
        node.apply_txs(transactions_to_replay, config.max_transactions)
            .await
            .map_err(to_domain)?;

        // If we are in replay mode, we don't start the server
        return Ok(());
    }

    // TODO: Consider moving to `InMemoryNodeInner::init`
    let rich_addresses = itertools::chain!(
        config
            .genesis_accounts
            .iter()
            .map(|acc| H160::from_slice(acc.address().as_ref())),
        config
            .signer_accounts
            .iter()
            .map(|acc| H160::from_slice(acc.address().as_ref())),
        LEGACY_RICH_WALLETS
            .iter()
            .map(|(address, _)| H160::from_str(address).unwrap()),
        RICH_WALLETS
            .iter()
            .map(|(address, _, _)| H160::from_str(address).unwrap()),
    )
    .collect::<Vec<_>>();
    for address in rich_addresses {
        node.set_rich_account(address, config.genesis_balance).await;
    }

    let mut server_builder = NodeServerBuilder::new(
        node.clone(),
        l1_sidecar,
        AllowOrigin::exact(
            config
                .allow_origin
                .parse()
                .context("allow origin is malformed")
                .map_err(to_domain)?,
        ),
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
                let port_requested = config.port;
                sh_eprintln!(
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
                        return Err(zksync_error::anvil_zksync::env::ServerStartupFailed {
                            host_requested: host.to_string(),
                            port_requested: port_requested.into(),
                            details: err.to_string(),
                        }
                        .into());
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
        node.load_state(zksync_types::web3::Bytes(bytes))
            .await
            .map_err(to_domain)?;
    }
    if let Some(ref state_path) = config.state {
        let bytes = std::fs::read(state_path).expect("Failed to read load state file");
        node.load_state(zksync_types::web3::Bytes(bytes))
            .await
            .map_err(to_domain)?;
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
    node_service_tasks.push(Box::pin(state_dumper));

    config.print(fork_print_info.as_ref());
    let node_service_stopped = futures::future::select_all(node_service_tasks);

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::trace!("received shutdown signal, shutting down");
        },
        _ = any_server_stopped => {
            tracing::trace!("node server was stopped")
        },
        _ = node_service_stopped => {
            tracing::trace!("node service was stopped")
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), AnvilZksyncError> {
    init_telemetry(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        TELEMETRY_CONFIG_NAME,
        Some(POSTHOG_API_KEY.into()),
        None,
        None,
    )
    .await
    .map_err(
        |inner| zksync_error::anvil_zksync::env::GenericError {
            message: format!("Failed to initialize telemetry collection subsystem: {inner}."),
        },
    )?;

    if let Err(err) = start_program().await {
        let telemetry = get_telemetry().expect("telemetry is not initialized");
        let _ = telemetry.track_error(Box::new(&err.to_unified())).await;
        sh_eprintln!("{}", err.to_unified().get_message());
        return Err(err);
    }
    Ok(())
}
