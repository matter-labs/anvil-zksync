use crate::bytecode_override::override_bytecodes;
use crate::cli::{Cli, Command, PeriodicStateDumper};
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
use anvil_zksync_core::node::fork::ForkDetails;
use anvil_zksync_core::node::{
    BlockSealer, BlockSealerMode, ImpersonationManager, InMemoryNode, InMemoryNodeInner,
    NodeExecutor, StorageKeyLayout, TestNodeFeeInputProvider, TxPool,
};
use anvil_zksync_core::observability::Observability;
use anvil_zksync_core::system_contracts::SystemContracts;
use anyhow::Context;
use clap::Parser;
use std::fs::File;
use std::sync::Arc;
use std::time::Duration;
use std::{env, net::SocketAddr, str::FromStr};
use tokio::sync::RwLock;
use tower_http::cors::AllowOrigin;
use tracing_subscriber::filter::LevelFilter;
use zksync_types::fee_model::{FeeModelConfigV2, FeeParams};
use zksync_types::H160;
use zksync_web3_decl::namespaces::ZksNamespaceClient;

use zksync_error::anvil::env::generic_error;
use zksync_error::anvil::state::StateLoaderError;
use zksync_error::documentation::Documented as _;
use zksync_error::error::IError as _;

mod bytecode_override;
mod cli;
mod utils;

#[tokio::main]
async fn main() -> Result<(), zksync_error::ZksyncError> {
    match main_inner().await {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error: {}", e.get_message());
            if let Ok(Some(documentation)) = e.get_documentation() {
                eprintln!("{documentation:#?}")
            };
            Err(e)
        }
    }
}

async fn main_inner() -> Result<(), zksync_error::ZksyncError> {
    // Check for deprecated options
    Cli::deprecated_config_option();

    let opt = Cli::parse();
    let command = opt.command.clone();

    let mut config = opt
        .into_test_node_config()
        .map_err(|e| generic_error!("{e}"))?;

    let log_level_filter = LevelFilter::from(config.log_level);
    let log_file = File::create(&config.log_file_path).map_err(|e| {
        zksync_error::anvil::env::LogFileAccessError {
            log_filename: config.log_file_path.clone(),
            wrapped_error: e.to_string(),
        }
    })?;

    // Initialize the tracing subscriber
    let observability = Observability::init(
        vec!["anvil_zksync".into()],
        log_level_filter,
        log_file,
        config.silent,
    )
    .map_err(|e| generic_error!("{e}"))?;

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
                    .map_err(|e| generic_error!("Failed to initialize client: {e:?}"))?;

                let fee = client.get_fee_params().await.map_err(|e| {
                    tracing::error!("Failed to fetch fee params: {:?}", e);
                    generic_error!("{e}")
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
                            .with_chain_id(config.chain_id.or(Some(TEST_NODE_NETWORK_ID)));
                    }
                    FeeParams::V1(_) => {
                        return Err(
                            generic_error!("Unsupported FeeParams::V1 in this context").into()
                        );
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

            update_with_fork_details(&mut config, fork_details_result)
                .await
                .map_err(|e| generic_error!("{e}"))?
        }
        Command::ReplayTx(replay_tx) => {
            let fork_details_result = ForkDetails::from_network_tx(
                &replay_tx.fork_url,
                replay_tx.tx,
                &config.cache_config,
            )
            .await;

            update_with_fork_details(&mut config, fork_details_result)
                .await
                .map_err(|e| generic_error!("{e}"))?
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
                return Err(generic_error!("{error}").into());
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

    let impersonation = ImpersonationManager::default();
    if config.enable_auto_impersonate {
        // Enable auto impersonation if configured
        impersonation.set_auto_impersonation(true);
    }
    let pool = TxPool::new(impersonation.clone(), config.transaction_order);

    let fee_input_provider = TestNodeFeeInputProvider::from_fork(fork_details.as_ref());
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

    let (node_inner, storage, blockchain, time) = InMemoryNodeInner::init(
        fork_details,
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
            tracing::error!("node executor ended with error: {:?}", err);
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
            .map_err(|e| generic_error!("{e}"))?;
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
                .context("allow origin is malformed")
                .map_err(|e| generic_error!("{e}"))?,
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
                        return Err(generic_error!(
                            "Failed to start server on host {} with port: {}",
                            host,
                            err
                        )
                        .into());
                    }
                }
            }
        }
    }
    let any_server_stopped =
        futures::future::select_all(server_handles.into_iter().map(|h| Box::pin(h.stopped())));

    // Load state from `--load-state` if provided
    for state_path in [config.load_state.as_deref(), config.state.as_deref()]
        .iter()
        .flatten()
    {
        let path_as_str = state_path.to_str().ok_or(StateLoaderError::GenericError {
            message: format!("Invalid path to the state {state_path:?}"),
        })?;

        let bytes = std::fs::read(state_path).map_err(|error| {
            zksync_error::anvil::state::LoadStateError {
                path: path_as_str.to_string(),
                reason: error.to_string(),
            }
        })?;
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
    };

    Ok(())
}
