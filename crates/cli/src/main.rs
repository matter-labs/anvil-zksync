use crate::bytecode_override::override_bytecodes;
use crate::cli::{Cli, Command, PeriodicStateDumper};
use crate::utils::{rpc_call, update_with_fork_details};
use alloy::primitives::Bytes;
use anvil_zksync_api_server::NodeServerBuilder;
use anvil_zksync_common::shell::{OutputMode, get_shell};
use anvil_zksync_common::utils::predeploys::PREDEPLOYS;
use anvil_zksync_common::{sh_eprintln, sh_err, sh_println};
use anvil_zksync_config::constants::{
    DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    DEFAULT_FAIR_PUBDATA_PRICE, DEFAULT_L1_GAS_PRICE, DEFAULT_L2_GAS_PRICE,
    EVM_EMULATOR_ENABLER_CALLDATA, LEGACY_RICH_WALLETS, PSEUDO_CALLER, RICH_WALLETS,
    TEST_NODE_NETWORK_ID,
};
use anvil_zksync_config::types::SystemContractsOptions;
use anvil_zksync_config::{ForkPrintInfo, L1Config};
use anvil_zksync_core::filters::EthFilters;
use anvil_zksync_core::node::error::format_revert_reason_hex;
use anvil_zksync_core::node::fork::ForkClient;
use anvil_zksync_core::node::{
    BlockSealer, BlockSealerMode, ImpersonationManager, InMemoryNode, InMemoryNodeInner,
    NodeExecutor, StorageKeyLayout, TestNodeFeeInputProvider, TxBatch, TxPool,
    traces::decoder::CallTraceDecoderBuilder,
};
use anvil_zksync_core::observability::Observability;
use anvil_zksync_core::system_contracts::SystemContractsBuilder;
use anvil_zksync_l1_sidecar::L1Sidecar;
use anvil_zksync_traces::format::debug_formatter::calls_from_debug_json;
use anvil_zksync_traces::identifier::SignaturesIdentifier;
use anvil_zksync_traces::{
    build_call_trace_arena, decode_trace_arena, filter_call_trace_arena, render_trace_arena_inner,
};
use anvil_zksync_types::L2TxBuilder;
use anyhow::Context;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{Value, json};
use std::fmt::Write;
use std::fs::File;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::{env, net::SocketAddr, str::FromStr};
use tokio::sync::RwLock;
use tower_http::cors::AllowOrigin;
use tracing_subscriber::filter::LevelFilter;
use zksync_error::anvil_zksync::AnvilZksyncError;
use zksync_error::anvil_zksync::generic::{generic_error, to_domain};
use zksync_error::{ICustomError, IError as _};
use zksync_multivm::interface::{ExecutionResult, Halt, VmExecutionResultAndLogs, VmRevertReason};
use zksync_telemetry::{TelemetryProps, get_telemetry, init_telemetry};
use zksync_types::fee_model::{FeeModelConfigV2, FeeParams};
use zksync_types::{
    CONTRACT_DEPLOYER_ADDRESS, EVM_PREDEPLOYS_MANAGER_ADDRESS, H160, L2BlockNumber, Nonce, U256,
};

mod bytecode_override;
mod cli;
mod utils;

const POSTHOG_API_KEY: &str = "phc_TsD52JxwkT2OXPHA2oKX2Lc3mf30hItCBrE9s9g1MKe";
const TELEMETRY_CONFIG_NAME: &str = "zksync-tooling";

async fn start_program(opt: Cli) -> Result<(), AnvilZksyncError> {
    // Check for deprecated options
    Cli::deprecated_config_option();

    if opt.silent.unwrap_or(false) {
        let mut shell = get_shell();
        shell.output_mode = OutputMode::Quiet;
    }
    // We keep a serialized version of the provided arguments to communicate them later if the arguments were incorrect.
    let debug_opt_string_repr = format!("{opt:#?}");

    let command = opt.command.clone();

    let mut config = opt.clone().into_test_node_config().map_err(to_domain)?;

    // Set verbosity level for the shell
    {
        let mut shell = get_shell();
        shell.verbosity = config.verbosity;
        shell.output_mode = if config.silent {
            OutputMode::Quiet
        } else {
            OutputMode::Normal
        };
    }
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

    // Install the global signatures identifier.
    if let Err(err) =
        SignaturesIdentifier::install(Some(config.get_cache_dir().into()), config.offline).await
    {
        tracing::error!("Failed to install signatures identifier: {err}");
    }

    // Use `Command::Run` as default.
    let command = command.as_ref().unwrap_or(&Command::Run);
    let (fork_client, transactions_to_replay) = match command {
        Command::Run => {
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
                .with_l1_pubdata_price(config.l1_pubdata_price.or(Some(DEFAULT_FAIR_PUBDATA_PRICE)))
                .with_chain_id(config.chain_id.or(Some(TEST_NODE_NETWORK_ID)));
            (None, Vec::new())
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
        Command::DebugTrace(_) => (None, Vec::new()),
    };

    // Ensure that system_contracts_path is only used with Local.
    if config.system_contracts_options != SystemContractsOptions::Local
        && config.system_contracts_path.is_some()
    {
        return Err(to_domain(generic_error!(
            "The --system-contracts-path option can only be specified when --dev-system-contracts is set to 'local'."
        )));
    }
    if let SystemContractsOptions::Local = config.system_contracts_options {
        // if local system contracts specified, check if the path exists else use env var
        // ZKSYNC_HOME
        let path: Option<PathBuf> = config
            .system_contracts_path
            .clone()
            .or_else(|| env::var_os("ZKSYNC_HOME").map(PathBuf::from));

        if let Some(path) = path {
            if !path.exists() || !path.is_dir() {
                return Err(to_domain(generic_error!(
                    "The specified system contracts path '{}' does not exist or is not a directory.",
                    path.to_string_lossy()
                )));
            }
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
                )));
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

    let fee_input_provider = TestNodeFeeInputProvider::from_fork(
        fork_client.as_ref().map(|f| &f.details),
        &config.base_token_config,
    );
    let filters = Arc::new(RwLock::new(EthFilters::default()));

    // Build system contracts
    let system_contracts = SystemContractsBuilder::new()
        .system_contracts_options(config.system_contracts_options)
        .system_contracts_path(config.system_contracts_path.clone())
        .protocol_version(config.protocol_version())
        .with_evm_interpreter(config.use_evm_interpreter)
        .with_zksync_os(config.zksync_os.clone())
        .build();

    let storage_key_layout = if config.zksync_os.zksync_os {
        StorageKeyLayout::ZKsyncOs
    } else {
        StorageKeyLayout::Era
    };

    let is_fork_mode = fork_client.is_some();
    let (node_inner, storage, blockchain, time, fork, vm_runner) = InMemoryNodeInner::init(
        fork_client,
        fee_input_provider.clone(),
        filters,
        config.clone(),
        impersonation.clone(),
        system_contracts.clone(),
        storage_key_layout,
        // Only produce system logs if L1 is enabled
        config.l1_config.is_some(),
    );

    let mut node_service_tasks: Vec<Pin<Box<dyn Future<Output = anyhow::Result<()>>>>> = Vec::new();
    let (node_executor, node_handle) =
        NodeExecutor::new(node_inner.clone(), vm_runner, storage_key_layout);
    let l1_sidecar = match config.l1_config.as_ref() {
        Some(_) if fork_print_info.is_some() => {
            return Err(zksync_error::anvil_zksync::env::InvalidArguments {
                details: "Running L1 in forking mode is unsupported".into(),
                arguments: debug_opt_string_repr,
            }
            .into());
        }
        Some(L1Config::Spawn { port }) => {
            let (l1_sidecar, l1_sidecar_runner) = L1Sidecar::process(
                config.protocol_version(),
                *port,
                blockchain.clone(),
                node_handle.clone(),
                pool.clone(),
                config.auto_execute_l1,
            )
            .await
            .map_err(to_domain)?;
            node_service_tasks.push(Box::pin(l1_sidecar_runner.run()));
            l1_sidecar
        }
        Some(L1Config::External { address }) => {
            let (l1_sidecar, l1_sidecar_runner) = L1Sidecar::external(
                config.protocol_version(),
                address,
                blockchain.clone(),
                node_handle.clone(),
                pool.clone(),
                config.auto_execute_l1,
            )
            .await
            .map_err(to_domain)?;
            node_service_tasks.push(Box::pin(l1_sidecar_runner.run()));
            l1_sidecar
        }
        None => L1Sidecar::none(),
    };
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
        node_handle.clone(),
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
            sh_err!("{err}");

            if let Some(tel) = get_telemetry() {
                let _ = tel.track_error(Box::new(&err)).await;
            }
        }
    });

    // track start of node if offline is false
    if let Some(tel) = get_telemetry() {
        let cli_telemetry_props = opt.clone().into_telemetry_props();
        let _ = tel
            .track_event(
                "node_started",
                TelemetryProps::new()
                    .insert("params", Some(cli_telemetry_props))
                    .take(),
            )
            .await;
    }

    if config.use_evm_interpreter {
        // We need to enable EVM interpreter by setting `allowedBytecodeTypesToDeploy` in `ContractDeployer`
        // to `1` (i.e. `AllowedBytecodeTypes::EraVmAndEVM`).
        node.impersonate_account(PSEUDO_CALLER).unwrap();
        node.set_rich_account(PSEUDO_CALLER, U256::from(1_000_000_000_000u64))
            .await;
        let chain_id = node.chain_id().await;
        let mut txs = Vec::with_capacity(PREDEPLOYS.len() + 1);
        txs.push(
            L2TxBuilder::new(
                PSEUDO_CALLER,
                Nonce(0),
                U256::from(300_000),
                U256::from(u32::MAX),
                chain_id,
            )
            .with_to(CONTRACT_DEPLOYER_ADDRESS)
            .with_calldata(Bytes::from_static(EVM_EMULATOR_ENABLER_CALLDATA).to_vec())
            .build_impersonated()
            .into(),
        );

        // If evm emulator is enabled, and not in fork mode, deploy pre-deploys for dev convenience
        if !is_fork_mode {
            let mut nonce = Nonce(1);
            for pd in PREDEPLOYS.iter() {
                let data = pd.encode_manager_call().unwrap();
                txs.push(
                    L2TxBuilder::new(
                        PSEUDO_CALLER,
                        nonce,
                        U256::from(10_000_000), // high limit for pre-deploys
                        U256::from(u32::MAX),
                        chain_id,
                    )
                    .with_to(EVM_PREDEPLOYS_MANAGER_ADDRESS)
                    .with_calldata(data)
                    .build_impersonated()
                    .into(),
                );
                nonce += 1;
            }
        }

        node_handle
            .seal_block_sync(TxBatch {
                impersonating: true,
                txs,
            })
            .await
            .map_err(to_domain)?;
        node.set_rich_account(PSEUDO_CALLER, U256::from(0)).await;
        node.stop_impersonating_account(PSEUDO_CALLER).unwrap();
    }

    if let Some(bytecodes_dir) = &config.override_bytecodes_dir {
        override_bytecodes(&node, bytecodes_dir.to_string())
            .await
            .unwrap();
    }

    if !transactions_to_replay.is_empty() {
        sh_println!("Executing transactions from the block.");
        let total_txs = transactions_to_replay.len() as u64;
        let pb = ProgressBar::new(total_txs);
        pb.enable_steady_tick(std::time::Duration::from_secs(1));
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} tx ({eta})")
                .unwrap()
                .with_key("eta", |state: &indicatif::ProgressState, w: &mut dyn Write| {
                    write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
                })
                .progress_chars("#>-")
            );

        node.node_handle
            .set_progress_report(Some(pb.clone()))
            .await
            .map_err(to_domain)?;

        node.replay_txs(transactions_to_replay)
            .await
            .map_err(to_domain)?;

        pb.finish_and_clear();
        sh_println!("Done replaying transactions.");

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

    // Use `debug-trace` command to get debug info for a transaction
    // TODO: clean this up and move to a separate function?
    if let Command::DebugTrace(args) = command {
        let params = if args.only_top {
            json!([args.tx, { "tracer": "callTracer", "tracerConfig": { "onlyTopCall": true } }])
        } else {
            json!([args.tx])
        };

        let rpc_url = args.rpc_url.to_config().url.to_string();
        let result_value: serde_json::Value =
            match rpc_call::<serde_json::Value>(&rpc_url, "debug_traceTransaction", params).await {
                Ok(v) => v,
                Err(e) => {
                    sh_println!("debug_traceTransaction failed: {e}");
                    return Ok(());
                }
            };

        if result_value.is_null() {
            sh_println!("No debug info found for tx {:#x}", args.tx);
            return Ok(());
        }

        // TODO: add improved halt/revert handling
        let exec_result = {
            let out_bytes: Vec<u8> = result_value
                .get("output")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<Bytes>().ok())
                .map(|b| b.to_vec())
                .unwrap_or_default();

            if let Some(err) = result_value.get("error").and_then(Value::as_str) {
                ExecutionResult::Halt {
                    reason: Halt::TracerCustom(err.to_string()),
                }
            } else if result_value
                .get("reverted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                ExecutionResult::Revert {
                    output: VmRevertReason::General {
                        msg: format_revert_reason_hex(&out_bytes),
                        data: out_bytes.clone(),
                    },
                }
            } else {
                ExecutionResult::Success { output: out_bytes }
            }
        };
        let tx_result_for_arena = VmExecutionResultAndLogs::mock(exec_result);

        if args.raw {
            sh_println!(
                "Raw debug payload:\n{}",
                serde_json::to_string_pretty(&result_value).unwrap_or_default()
            );
        }

        let envelope = serde_json::json!({ "result": result_value });
        let raw_str = serde_json::to_string(&envelope).unwrap();
        let call_traces = match calls_from_debug_json(&raw_str) {
            Ok(calls) => calls,
            Err(err) => {
                sh_println!("Failed to map debug JSON to internal Calls: {err}");
                return Ok(());
            }
        };

        let verbosity = get_shell().verbosity;
        if !call_traces.is_empty() && verbosity >= 2 {
            let mut builder = CallTraceDecoderBuilder::default();
            builder = builder.with_signature_identifier(SignaturesIdentifier::global());
            let decoder = builder.build();

            let mut arena = build_call_trace_arena(&call_traces, &tx_result_for_arena);
            decode_trace_arena(&mut arena, &decoder).await;

            let filtered = filter_call_trace_arena(&arena, verbosity);
            let out = render_trace_arena_inner(&filtered, false);
            sh_println!("\nTraces:\n{}", out);
        } else {
            sh_println!("(No calls or verbosity < 2)");
        }

        return Ok(());
    }

    let any_server_stopped =
        futures::future::select_all(server_handles.into_iter().map(|h| Box::pin(h.stopped())));

    let state_path = config.load_state.as_ref().or(config.state.as_ref());
    if let Some(state_path) = state_path {
        let bytes = std::fs::read(state_path).map_err(|error| {
            zksync_error::anvil_zksync::state::StateFileAccess {
                path: state_path.to_string_lossy().to_string(),
                reason: error.to_string(),
            }
        })?;
        node.load_state(zksync_types::web3::Bytes(bytes))
            .await
            .map_err(to_domain)?;
    }

    let dump_state_path = config.dump_state.clone().or_else(|| config.state.clone());
    let dump_interval = config
        .state_interval
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(60)); // Default to 60 seconds
    let preserve_historical_states = config.preserve_historical_states;
    let node_for_dumper = node.clone();
    let state_dumper = PeriodicStateDumper::new(
        node_for_dumper,
        dump_state_path,
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
        (result, _, _) = node_service_stopped => {
            // Propagate error that might have happened inside one of the services
            result.map_err(to_domain)?;
            tracing::trace!("node service was stopped")
        }
    }

    SignaturesIdentifier::global().save().await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), AnvilZksyncError> {
    let cli = Cli::parse();
    let offline = cli.offline;

    if !offline {
        init_telemetry(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            TELEMETRY_CONFIG_NAME,
            Some(POSTHOG_API_KEY.into()),
            None,
            None,
        )
        .await
        .map_err(|inner| zksync_error::anvil_zksync::env::GenericError {
            message: format!("Failed to initialize telemetry collection subsystem: {inner}."),
        })?;
    }

    if let Err(err) = start_program(cli).await {
        // Track only if telemetry is active
        if let Some(tel) = get_telemetry() {
            let _ = tel.track_error(Box::new(&err.to_unified())).await;
        }
        sh_eprintln!("{}", err.to_unified().get_message());
        return Err(err);
    }
    Ok(())
}
