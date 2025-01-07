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
    BlockProducer, BlockSealer, BlockSealerMode, ImpersonationManager, InMemoryNode,
    InMemoryNodeInner, TestNodeFeeInputProvider, TxPool,
};
use anvil_zksync_core::observability::Observability;
use anvil_zksync_core::system_contracts::SystemContracts;
use anyhow::{anyhow, Context};
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

mod bytecode_override;
mod cli;
mod utils;

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

    let impersonation = ImpersonationManager::default();
    if config.enable_auto_impersonate {
        // Enable auto impersonation if configured
        impersonation.set_auto_impersonation(true);
    }
    let pool = TxPool::new(impersonation.clone(), config.transaction_order);

    let fee_input_provider = TestNodeFeeInputProvider::from_fork(fork_details.as_ref());
    let filters = Arc::new(RwLock::new(EthFilters::default()));
    let system_contracts =
        SystemContracts::from_options(&config.system_contracts_options, config.use_evm_emulator);

    let (node_inner, _fork_storage, blockchain, time) = InMemoryNodeInner::init(
        fork_details,
        fee_input_provider.clone(),
        filters,
        config.clone(),
        impersonation.clone(),
        system_contracts.clone(),
    );

    let (block_producer, block_producer_handle) =
        BlockProducer::new(node_inner.clone(), system_contracts.clone());
    let sealing_mode = if config.no_mining {
        BlockSealerMode::noop()
    } else if let Some(block_time) = config.block_time {
        BlockSealerMode::fixed_time(config.max_transactions, block_time)
    } else {
        BlockSealerMode::immediate(config.max_transactions, pool.add_tx_listener())
    };
    let (block_sealer, block_sealer_state) =
        BlockSealer::new(sealing_mode, pool.clone(), block_producer_handle.clone());

    let node: InMemoryNode = InMemoryNode::new(
        node_inner,
        blockchain,
        block_producer_handle,
        Some(observability),
        time,
        impersonation,
        pool,
        block_sealer_state,
        system_contracts,
    );

    if let Some(ref bytecodes_dir) = config.override_bytecodes_dir {
        override_bytecodes(&node, bytecodes_dir.to_string())
            .await
            .unwrap();
    }

    if !transactions_to_replay.is_empty() {
        let _ = node.apply_txs(transactions_to_replay, config.max_transactions);
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
    );
    if config.health_check_endpoint {
        server_builder.enable_health_api()
    }
    if !config.no_cors {
        server_builder.enable_cors();
    }
    let mut server_handles = Vec::with_capacity(config.host.len());
    for host in &config.host {
        let addr = SocketAddr::new(*host, config.port);
        server_handles.push(server_builder.clone().build(addr).await.run());
    }
    let any_server_stopped =
        futures::future::select_all(server_handles.into_iter().map(|h| Box::pin(h.stopped())));

    let dump_state = config.dump_state.clone();
    let dump_interval = config
        .state_interval
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(60)); // Default to 60 seconds
    let preserve_historical_states = config.preserve_historical_states;
    let node_for_dumper = node.clone();
    let state_dumper = tokio::task::spawn(PeriodicStateDumper::new(
        node_for_dumper,
        dump_state,
        dump_interval,
        preserve_historical_states,
    ));

    config.print(fork_print_info.as_ref());

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::trace!("received shutdown signal, shutting down");
        },
        _ = any_server_stopped => {
            tracing::trace!("node server was stopped")
        },
        _ = block_producer => {
            tracing::trace!("block producer was stopped")
        },
        _ = block_sealer => {
            tracing::trace!("block sealer was stopped")
        },
        _ = state_dumper => {
            tracing::trace!("state dumper was stopped")
        },
    }

    Ok(())
}
