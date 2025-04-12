use crate::bootloader_debug::BootloaderDebug;
use crate::formatter::{self, ExecutionErrorReport};
use crate::node::batch::{MainBatchExecutorFactory, TraceCalls};
use crate::node::error::ToHaltError;
use crate::node::inner::fork_storage::ForkStorage;
use crate::node::inner::in_memory_inner::BlockContext;
use crate::node::storage_logs::print_storage_logs_details;
use crate::node::time::Time;
use crate::node::{
    compute_hash, InMemoryNodeInner, TestNodeFeeInputProvider, TransactionResult, TxBatch,
    TxExecutionInfo,
};
use crate::system_contracts::SystemContracts;
use crate::utils::create_debug_output;
use anvil_zksync_common::shell::get_shell;
use anvil_zksync_common::{sh_eprintln, sh_err, sh_println, sh_warn};
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_traces::decode::CallTraceDecoderBuilder;
use anvil_zksync_traces::{
    build_call_trace_arena, decode_trace_arena, filter_call_trace_arena,
    identifier::SignaturesIdentifier, render_trace_arena_inner,
};
use anvil_zksync_types::{ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use indicatif::ProgressBar;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use zksync_contracts::BaseSystemContractsHashes;
use zksync_error::anvil_zksync;
use zksync_error::anvil_zksync::node::{AnvilNodeError, AnvilNodeResult};
use zksync_multivm::interface::executor::BatchExecutor;
use zksync_multivm::interface::storage::WriteStorage;
use zksync_multivm::interface::{
    BatchTransactionExecutionResult, ExecutionResult, FinishedL1Batch, L1BatchEnv, L2BlockEnv,
    TxExecutionMode, VmEvent, VmExecutionResultAndLogs,
};
use zksync_multivm::zk_evm_latest::ethereum_types::{Address, H160, U256, U64};
use zksync_types::block::L2BlockHasher;
use zksync_types::bytecode::BytecodeHash;
use zksync_types::commitment::{PubdataParams, PubdataType};
use zksync_types::web3::Bytes;
use zksync_types::{
    api, h256_to_address, ExecuteTransactionCommon, L2BlockNumber, L2TxCommonData, StorageKey,
    StorageValue, Transaction, ACCOUNT_CODE_STORAGE_ADDRESS,
};

pub struct VmRunner {
    executor_factory: MainBatchExecutorFactory<TraceCalls>,
    bootloader_debug_result: Arc<RwLock<eyre::Result<BootloaderDebug, String>>>,

    time: Time,
    fork_storage: ForkStorage,
    system_contracts: SystemContracts,
    /// Whether VM should generate system logs.
    generate_system_logs: bool,
    /// Optional field for reporting progress while replaying transactions.
    progress_report: Option<ProgressBar>,
}

pub(super) struct TxBatchExecutionResult {
    pub(super) tx_results: Vec<TransactionResult>,
    pub(super) base_system_contracts_hashes: BaseSystemContractsHashes,
    pub(super) batch_env: L1BatchEnv,
    pub(super) block_ctxs: Vec<BlockContext>,
    pub(super) finished_l1_batch: FinishedL1Batch,
    pub(super) modified_storage_keys: HashMap<StorageKey, StorageValue>,
}

impl VmRunner {
    pub(super) fn new(
        time: Time,
        fork_storage: ForkStorage,
        system_contracts: SystemContracts,
        generate_system_logs: bool,
        enforced_bytecode_compression: bool,
    ) -> Self {
        let bootloader_debug_result = Arc::new(std::sync::RwLock::new(Err(
            "Tracer has not been run yet".to_string(),
        )));
        Self {
            executor_factory: MainBatchExecutorFactory::<TraceCalls>::new(
                enforced_bytecode_compression,
                bootloader_debug_result.clone(),
            ),
            bootloader_debug_result,

            time,
            fork_storage,
            system_contracts,
            generate_system_logs,
            progress_report: None,
        }
    }
}

impl VmRunner {
    // Prints the gas details of the transaction for debugging purposes.
    fn display_detailed_gas_info(
        &self,
        bootloader_debug_result: Option<&eyre::Result<BootloaderDebug, String>>,
        spent_on_pubdata: u64,
        fee_input_provider: &TestNodeFeeInputProvider,
    ) -> eyre::Result<(), String> {
        if let Some(bootloader_result) = bootloader_debug_result {
            let bootloader_debug = bootloader_result.clone()?;

            let gas_details = formatter::compute_gas_details(&bootloader_debug, spent_on_pubdata);
            let mut formatter = formatter::Formatter::new();

            let fee_model_config = fee_input_provider.get_fee_model_config();

            formatter.print_gas_details(&gas_details, &fee_model_config);

            Ok(())
        } else {
            Err("Bootloader tracer didn't finish.".to_owned())
        }
    }

    /// Validates L2 transaction
    fn validate_tx(
        &self,
        batch_env: &L1BatchEnv,
        tx_data: &L2TxCommonData,
    ) -> Result<(), anvil_zksync::tx_invalid::TransactionValidationError> {
        let max_gas = U256::from(u64::MAX);
        if tx_data.fee.gas_limit > max_gas {
            return Err(anvil_zksync::tx_invalid::InvalidGasLimit {
                tx_gas_limit: Box::new(tx_data.fee.gas_limit),
                max_gas: Box::new(max_gas),
            });
        }

        if tx_data.fee.gas_per_pubdata_limit > max_gas {
            return Err(anvil_zksync::tx_invalid::GasPerPubdataLimit {
                tx_gas_per_pubdata_limit: Box::new(tx_data.fee.gas_per_pubdata_limit),
                max_gas: Box::new(max_gas),
            });
        }

        let l2_gas_price = batch_env.fee_input.fair_l2_gas_price();
        if tx_data.fee.max_fee_per_gas < l2_gas_price.into() {
            return Err(anvil_zksync::tx_invalid::MaxFeePerGasTooLow {
                max_fee_per_gas: Box::new(tx_data.fee.max_fee_per_gas),
                l2_gas_price: Box::new(l2_gas_price.into()),
            });
        }

        if tx_data.fee.max_fee_per_gas < tx_data.fee.max_priority_fee_per_gas {
            return Err(anvil_zksync::tx_invalid::MaxPriorityFeeGreaterThanMaxFee {
                max_fee_per_gas: Box::new(tx_data.fee.max_fee_per_gas),
                max_priority_fee_per_gas: Box::new(tx_data.fee.max_priority_fee_per_gas),
            });
        }
        Ok(())
    }

    async fn run_tx_pretty(
        &mut self,
        tx: &Transaction,
        executor: &mut dyn BatchExecutor<ForkStorage>,
        config: &TestNodeConfig,
        fee_input_provider: &TestNodeFeeInputProvider,
    ) -> AnvilNodeResult<BatchTransactionExecutionResult> {
        // Check if target address has code before executing the transaction
        if let Some(to_address) = tx.recipient_account() {
            let code_key = zksync_types::get_code_key(&to_address);
            let bytecode_hash = zksync_multivm::interface::storage::ReadStorage::read_value(
                &mut self.fork_storage,
                &code_key,
            );

            // If bytecode hash is zero, there's no code at this address
            if bytecode_hash.is_zero() {
                sh_warn!(
                    "Transaction {} was sent to address {to_address}, which is not associated with any contract.",
                    tx.hash()
                );
            }
        }

        let BatchTransactionExecutionResult {
            tx_result,
            compression_result,
            call_traces,
        } = executor.execute_tx(tx.clone()).await?;
        compression_result.map_err(|_inner| {
            // We ignore `inner` because bytecode
            // compression error currently does not hold
            // any precise information
            anvil_zksync::node::TransactionHalt {
                inner: Box::new(anvil_zksync::halt::FailedToPublishCompressedBytecodes),
                transaction_hash: Box::new(tx.hash()),
            }
        })?;

        let spent_on_pubdata =
            tx_result.statistics.gas_used - tx_result.statistics.computational_gas_used as u64;

        let status = match &tx_result.result {
            ExecutionResult::Success { .. } => "SUCCESS",
            ExecutionResult::Revert { .. } => "FAILED",
            ExecutionResult::Halt { .. } => "HALTED",
        };

        // Print transaction summary
        if config.show_tx_summary {
            formatter::print_transaction_summary(config.get_l2_gas_price(), tx, &tx_result, status);
        }
        // Print gas details if enabled
        if config.show_gas_details != ShowGasDetails::None {
            self.display_detailed_gas_info(
                Some(&self.bootloader_debug_result.read().unwrap()),
                spent_on_pubdata,
                fee_input_provider,
            )
            .unwrap_or_else(|err| {
                sh_err!("{}", format!("Cannot display gas details: {err}"));
            });
        }
        // Print storage logs if enabled
        if config.show_storage_logs != ShowStorageLogs::None {
            print_storage_logs_details(config.show_storage_logs, &tx_result);
        }
        // Print VM details if enabled
        if config.show_vm_details != ShowVMDetails::None {
            let mut formatter = formatter::Formatter::new();
            formatter.print_vm_details(&tx_result);
        }

        let verbosity = get_shell().verbosity;
        if !call_traces.is_empty() && verbosity >= 2 {
            let mut builder = CallTraceDecoderBuilder::new();

            builder = builder.with_signature_identifier(
                SignaturesIdentifier::new(Some(config.get_cache_dir().into()), config.offline)
                    .map_err(|err| {
                        anyhow::anyhow!("Failed to create SignaturesIdentifier: {:#}", err)
                    })?,
            );

            let decoder = builder.build();
            let mut arena = build_call_trace_arena(&call_traces, &tx_result);
            decode_trace_arena(&mut arena, &decoder).await?;

            let filtered_arena = filter_call_trace_arena(&arena, verbosity);
            let trace_output = render_trace_arena_inner(&filtered_arena, false);
            sh_println!("\nTraces:\n{}", trace_output);
        }

        Ok(BatchTransactionExecutionResult {
            tx_result,
            compression_result: Ok(()),
            call_traces,
        })
    }

    /// Runs transaction and commits it to a new block.
    #[allow(clippy::too_many_arguments)]
    async fn run_tx(
        &mut self,
        tx: &Transaction,
        tx_index: u64,
        next_log_index: &mut usize,
        block_ctx: &BlockContext,
        batch_env: &L1BatchEnv,
        executor: &mut dyn BatchExecutor<ForkStorage>,
        config: &TestNodeConfig,
        fee_input_provider: &TestNodeFeeInputProvider,
    ) -> AnvilNodeResult<TransactionResult> {
        let tx_hash = tx.hash();
        let transaction_type = tx.tx_format();

        if let ExecuteTransactionCommon::L2(l2_tx_data) = &tx.common_data {
            // If the transaction can not be validated, we return immediately
            self.validate_tx(batch_env, l2_tx_data).map_err(|e| {
                anvil_zksync::node::TransactionValidationFailed {
                    inner: Box::new(e),
                    transaction_hash: Box::new(tx_hash),
                }
            })?;
        }

        let BatchTransactionExecutionResult {
            tx_result: result,
            compression_result: _,
            call_traces,
        } = self
            .run_tx_pretty(tx, executor, config, fee_input_provider)
            .await?;

        if let ExecutionResult::Halt { reason } = result.result {
            // Halt means that something went really bad with the transaction execution
            // (in most cases invalid signature, but it could also be bootloader panic etc).
            // In such cases, we should not persist the VM data and should pretend that
            // the transaction never existed.
            return Err(anvil_zksync::node::TransactionHalt {
                inner: Box::new(reason.to_halt_error().await),
                transaction_hash: Box::new(tx_hash),
            });
        }
        let saved_factory_deps = VmEvent::extract_bytecodes_marked_as_known(&result.logs.events);

        // Get transaction factory deps
        let factory_deps = &tx.execute.factory_deps;
        let mut tx_factory_deps: HashMap<_, _> = factory_deps
            .iter()
            .map(|bytecode| {
                (
                    BytecodeHash::for_bytecode(bytecode).value(),
                    bytecode.clone(),
                )
            })
            .collect();
        // Ensure that *dynamic* factory deps (ones that may be created when executing EVM contracts)
        // are added into the lookup map as well.
        tx_factory_deps.extend(result.dynamic_factory_deps.clone());

        let new_bytecodes = saved_factory_deps
            .map(|bytecode_hash| {
                let bytecode = tx_factory_deps.get(&bytecode_hash).unwrap_or_else(|| {
                    panic!(
                        "Failed to get factory deps on tx: bytecode hash: {:?}, tx hash: {}",
                        bytecode_hash,
                        tx.hash()
                    )
                });
                (bytecode_hash, bytecode.clone())
            })
            .collect::<Vec<_>>();

        let logs = result
            .logs
            .events
            .iter()
            .enumerate()
            .map(|(log_idx, log)| api::Log {
                address: log.address,
                topics: log.indexed_topics.clone(),
                data: Bytes(log.value.clone()),
                block_hash: Some(block_ctx.hash),
                block_number: Some(block_ctx.miniblock.into()),
                l1_batch_number: Some(U64::from(batch_env.number.0)),
                transaction_hash: Some(tx_hash),
                transaction_index: Some(U64::from(tx_index)),
                log_index: Some(U256::from(log_idx)),
                transaction_log_index: Some(U256::from(log_idx)),
                log_type: None,
                removed: Some(false),
                block_timestamp: Some(block_ctx.timestamp.into()),
            })
            .collect();
        let tx_receipt = api::TransactionReceipt {
            transaction_hash: tx_hash,
            transaction_index: U64::from(tx_index),
            block_hash: block_ctx.hash,
            block_number: block_ctx.miniblock.into(),
            l1_batch_tx_index: Some(U64::from(tx_index)),
            l1_batch_number: Some(U64::from(batch_env.number.0)),
            from: tx.initiator_account(),
            to: tx.recipient_account(),
            cumulative_gas_used: Default::default(),
            gas_used: Some(tx.gas_limit() - result.refunds.gas_refunded),
            contract_address: contract_address_from_tx_result(&result),
            logs,
            l2_to_l1_logs: result
                .logs
                .user_l2_to_l1_logs
                .iter()
                .enumerate()
                .map(|(log_index, log)| api::L2ToL1Log {
                    block_hash: Some(block_ctx.hash),
                    block_number: block_ctx.miniblock.into(),
                    l1_batch_number: Some(U64::from(batch_env.number.0)),
                    log_index: U256::from(*next_log_index + log_index),
                    transaction_index: U64::from(tx_index),
                    transaction_hash: tx_hash,
                    transaction_log_index: U256::from(log_index),
                    tx_index_in_l1_batch: Some(U64::from(tx_index)),
                    shard_id: log.0.shard_id.into(),
                    is_service: log.0.is_service,
                    sender: log.0.sender,
                    key: log.0.key,
                    value: log.0.value,
                })
                .collect(),
            status: if result.result.is_failed() {
                U64::from(0)
            } else {
                U64::from(1)
            },
            effective_gas_price: Some(fee_input_provider.gas_price().into()),
            transaction_type: Some((transaction_type as u32).into()),
            logs_bloom: Default::default(),
        };
        *next_log_index += result.logs.user_l2_to_l1_logs.len();
        let debug = create_debug_output(tx, &result, call_traces).expect("create debug output"); // OK to unwrap here as Halt is handled above

        Ok(TransactionResult {
            info: TxExecutionInfo {
                tx: tx.clone(),
                batch_number: batch_env.number.0,
                miniblock_number: block_ctx.miniblock,
            },
            new_bytecodes,
            receipt: tx_receipt,
            debug,
        })
    }

    pub(super) async fn run_tx_batch(
        &mut self,
        TxBatch { txs, impersonating }: TxBatch,
        node_inner: &mut InMemoryNodeInner,
    ) -> AnvilNodeResult<TxBatchExecutionResult> {
        let system_contracts = self
            .system_contracts
            .contracts(TxExecutionMode::VerifyExecute, impersonating)
            .clone();
        let base_system_contracts_hashes = system_contracts.hashes();
        // Prepare a new block context and a new batch env
        let system_env =
            node_inner.create_system_env(system_contracts, TxExecutionMode::VerifyExecute);
        let (batch_env, mut block_ctx) = node_inner.create_l1_batch_env().await;
        // Advance clock as we are consuming next timestamp for this block

        if self.time.advance_timestamp() != block_ctx.timestamp {
            return Err(anvil_zksync::node::generic_error!(
                "Advancing clock produced different timestamp than expected. This should never happen -- please report this as a bug."
            ));
        };

        let pubdata_params = PubdataParams {
            l2_da_validator_address: Address::zero(),
            pubdata_type: PubdataType::Rollup,
        };
        let mut executor = if self.system_contracts.use_zkos {
            todo!("BatchExecutor support for zkos is yet to be implemented")
        } else {
            self.executor_factory.init_main_batch(
                self.fork_storage.clone(),
                batch_env.clone(),
                system_env.clone(),
                pubdata_params,
            )
        };

        // Compute block hash. Note that the computed block hash here will be different than that in production.
        let tx_hashes = txs.iter().map(|t| t.hash()).collect::<Vec<_>>();
        block_ctx.hash = compute_hash(
            system_env.version,
            (block_ctx.miniblock as u32).into(),
            block_ctx.timestamp,
            block_ctx.prev_block_hash,
            &tx_hashes,
        );

        // Execute transactions and bootloader
        let mut tx_results = Vec::with_capacity(tx_hashes.len());
        let mut tx_index = 0;
        let mut next_log_index = 0;
        let total = txs.len();

        for tx in txs {
            if let Some(ref pb) = self.progress_report {
                pb.set_message(format!(
                    "Replaying transaction {}/{} from 0x{:x}...",
                    tx_index + 1,
                    total,
                    tx.hash()
                ));
            }

            let result = self
                .run_tx(
                    &tx,
                    tx_index,
                    &mut next_log_index,
                    &block_ctx,
                    &batch_env,
                    &mut executor,
                    &node_inner.config,
                    &node_inner.fee_input_provider,
                )
                .await;

            // Update progress bar
            if let Some(ref pb) = self.progress_report {
                pb.inc(1);
            }
            match result {
                Ok(tx_result) => {
                    tx_results.push(tx_result);
                    tx_index += 1;
                }
                Err(e) => {
                    match &e {
                        // Validation errors are reported and the execution proceeds
                        AnvilNodeError::TransactionValidationFailed { .. } => {
                            let error_report = ExecutionErrorReport::new(&e, Some(&tx));
                            sh_eprintln!("{error_report}");
                            executor.rollback_last_tx().await?;
                        }
                        // Halts are reported and the execution proceeds
                        AnvilNodeError::TransactionHalt { inner, .. } => {
                            let error_report = ExecutionErrorReport::new(inner.as_ref(), Some(&tx));
                            sh_eprintln!("{error_report}");
                            executor.rollback_last_tx().await?;
                        }
                        // Other errors are not recoverable so we pass them up
                        // the execution stack immediately
                        _ => return Err(e),
                    }
                }
            }
        }
        // TODO: This is the correct hash as reported by VM, but we can't compute it correct above
        //       because we don't know which txs are going to be halted
        block_ctx.hash = compute_hash(
            system_env.version,
            (block_ctx.miniblock as u32).into(),
            block_ctx.timestamp,
            block_ctx.prev_block_hash,
            tx_results
                .iter()
                .map(|tx_result| &tx_result.receipt.transaction_hash),
        );

        let mut block_ctxs = vec![block_ctx.clone()];
        if !tx_results.is_empty() {
            // Create an empty virtual block at the end of the batch (only if the last block was
            // not empty, i.e. virtual).
            let mut virtual_block_ctx = block_ctx.new_block(&mut self.time);
            virtual_block_ctx.hash = L2BlockHasher::new(
                L2BlockNumber(virtual_block_ctx.miniblock as u32),
                virtual_block_ctx.timestamp,
                block_ctx.hash,
            )
            .finalize(system_env.version);
            let l2_block_env = L2BlockEnv {
                number: (block_ctx.miniblock + 1) as u32,
                timestamp: block_ctx.timestamp + 1,
                prev_block_hash: block_ctx.hash,
                max_virtual_blocks_to_create: 1,
            };
            executor.start_next_l2_block(l2_block_env).await?;
            block_ctxs.push(virtual_block_ctx);
        }

        let (finished_l1_batch, modified_storage_keys) = if self.generate_system_logs {
            // If system log generation is enabled we run realistic (and time-consuming) bootloader flow
            let (finished_l1_batch, storage_view) = Box::new(executor).finish_batch().await?;
            (
                finished_l1_batch,
                storage_view.modified_storage_keys().clone(),
            )
        } else {
            // Otherwise we mock the execution with a single bootloader iteration
            let mut finished_l1_batch = FinishedL1Batch::mock();
            let (bootloader_execution_result, storage_view) = executor.bootloader().await?;
            finished_l1_batch.block_tip_execution_result = bootloader_execution_result;
            (
                finished_l1_batch,
                storage_view.modified_storage_keys().clone(),
            )
        };
        assert!(
            !finished_l1_batch
                .block_tip_execution_result
                .result
                .is_failed(),
            "VM must not fail when finalizing block: {:#?}",
            finished_l1_batch.block_tip_execution_result.result
        );

        Ok(TxBatchExecutionResult {
            tx_results,
            base_system_contracts_hashes,
            batch_env,
            block_ctxs,
            finished_l1_batch,
            modified_storage_keys,
        })
    }

    /// Set or unset the progress report.
    pub fn set_progress_report(&mut self, bar: Option<ProgressBar>) {
        self.progress_report = bar;
    }
}

fn contract_address_from_tx_result(execution_result: &VmExecutionResultAndLogs) -> Option<H160> {
    for query in execution_result.logs.storage_logs.iter().rev() {
        if query.log.is_write() && query.log.key.address() == &ACCOUNT_CODE_STORAGE_ADDRESS {
            return Some(h256_to_address(query.log.key.key()));
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::node::fork::{Fork, ForkClient, ForkDetails};
    use crate::testing::{TransactionBuilder, STORAGE_CONTRACT_BYTECODE};
    use alloy::dyn_abi::{DynSolType, DynSolValue};
    use alloy::primitives::U256 as AlloyU256;
    use anvil_zksync::node::AnvilNodeResult;
    use anvil_zksync_common::cache::CacheConfig;
    use anvil_zksync_config::constants::{
        DEFAULT_ACCOUNT_BALANCE, DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
        DEFAULT_ESTIMATE_GAS_SCALE_FACTOR, DEFAULT_FAIR_PUBDATA_PRICE, DEFAULT_L1_GAS_PRICE,
        DEFAULT_L2_GAS_PRICE, TEST_NODE_NETWORK_ID,
    };
    use anvil_zksync_config::types::SystemContractsOptions;
    use std::str::FromStr;
    use zksync_multivm::interface::executor::BatchExecutorFactory;
    use zksync_multivm::interface::storage::StorageView;
    use zksync_multivm::interface::{L2Block, SystemEnv};
    use zksync_multivm::vm_latest::constants::BATCH_COMPUTATIONAL_GAS_LIMIT;
    use zksync_multivm::vm_latest::utils::l2_blocks::load_last_l2_block;
    use zksync_types::fee::Fee;
    use zksync_types::fee_model::BatchFeeInput;
    use zksync_types::l2::{L2Tx, TransactionType};
    use zksync_types::utils::deployed_address_create;
    use zksync_types::{
        u256_to_h256, K256PrivateKey, L1BatchNumber, L2ChainId, Nonce, ProtocolVersionId, H256,
    };

    struct VmRunnerTester {
        vm_runner: VmRunner,
        config: TestNodeConfig,
        system_contracts: SystemContracts,
    }

    impl VmRunnerTester {
        fn new_custom(fork_client: Option<ForkClient>, config: TestNodeConfig) -> Self {
            let time = Time::new(0);
            let fork_storage = ForkStorage::new(
                Fork::new(fork_client, CacheConfig::None),
                SystemContractsOptions::BuiltIn,
                ProtocolVersionId::latest(),
                None,
                None,
            );
            let system_contracts = SystemContracts::from_options(
                config.system_contracts_options,
                config.system_contracts_path.clone(),
                ProtocolVersionId::latest(),
                config.use_evm_emulator,
                config.use_zkos,
            );
            let vm_runner = VmRunner::new(
                time,
                fork_storage,
                system_contracts.clone(),
                false,
                config.is_bytecode_compression_enforced(),
            );
            VmRunnerTester {
                vm_runner,
                config,
                system_contracts,
            }
        }

        fn new() -> Self {
            Self::new_custom(None, TestNodeConfig::default())
        }

        fn make_rich(&self, account: &Address) {
            let key = zksync_types::utils::storage_key_for_eth_balance(account);
            self.vm_runner
                .fork_storage
                .set_value(key, u256_to_h256(U256::from(DEFAULT_ACCOUNT_BALANCE)));
        }

        async fn test_tx(&mut self, tx: Transaction) -> AnvilNodeResult<TransactionResult> {
            Ok(self.test_txs(vec![tx]).await?.into_iter().next().unwrap())
        }

        async fn test_txs(
            &mut self,
            txs: Vec<Transaction>,
        ) -> AnvilNodeResult<Vec<TransactionResult>> {
            let system_env = SystemEnv {
                zk_porter_available: false,
                version: ProtocolVersionId::latest(),
                base_system_smart_contracts: self
                    .system_contracts
                    .contracts(TxExecutionMode::VerifyExecute, false)
                    .clone(),
                bootloader_gas_limit: BATCH_COMPUTATIONAL_GAS_LIMIT,
                execution_mode: TxExecutionMode::VerifyExecute,
                default_validation_computational_gas_limit: BATCH_COMPUTATIONAL_GAS_LIMIT,
                chain_id: L2ChainId::from(TEST_NODE_NETWORK_ID),
            };
            let last_l2_block = load_last_l2_block(
                &StorageView::new(self.vm_runner.fork_storage.clone()).to_rc_ptr(),
            )
            .unwrap_or_else(|| L2Block {
                number: 0,
                hash: H256::from_str(
                    "0xe8e77626586f73b955364c7b4bbf0bb7f7685ebd40e852b164633a4acbd3244c",
                )
                .unwrap(),
                timestamp: 0,
            });
            let block_ctx = BlockContext {
                hash: Default::default(),
                batch: last_l2_block.number + 1,
                miniblock: (last_l2_block.number + 1) as u64,
                timestamp: last_l2_block.timestamp + 1,
                prev_block_hash: last_l2_block.hash,
            };
            let batch_env = L1BatchEnv {
                previous_batch_hash: None,
                number: L1BatchNumber::from(block_ctx.batch),
                timestamp: block_ctx.timestamp,
                fee_input: BatchFeeInput::l1_pegged(DEFAULT_L1_GAS_PRICE, DEFAULT_L2_GAS_PRICE),
                fee_account: H160::zero(),
                enforced_base_fee: None,
                first_l2_block: L2BlockEnv {
                    number: block_ctx.miniblock as u32,
                    timestamp: block_ctx.timestamp,
                    prev_block_hash: block_ctx.prev_block_hash,
                    max_virtual_blocks_to_create: 1,
                },
            };
            let mut executor = self.vm_runner.executor_factory.init_batch(
                self.vm_runner.fork_storage.clone(),
                batch_env.clone(),
                system_env,
                PubdataParams::default(),
            );

            let mut log_index = 0;
            let mut results = vec![];
            for (i, tx) in txs.into_iter().enumerate() {
                results.push(
                    self.vm_runner
                        .run_tx(
                            &tx,
                            i as u64,
                            &mut log_index,
                            &block_ctx,
                            &batch_env,
                            executor.as_mut(),
                            &self.config,
                            &TestNodeFeeInputProvider::default(),
                        )
                        .await?,
                );
            }
            Ok(results)
        }
    }

    /// Decodes a `bytes` tx result to its concrete parameter type.
    fn decode_tx_result(output: &[u8], param_type: DynSolType) -> DynSolValue {
        let result = DynSolType::Bytes
            .abi_decode(output)
            .expect("failed decoding output");
        let result_bytes = match result {
            DynSolValue::Bytes(bytes) => bytes,
            _ => panic!("expected bytes but got a different type"),
        };

        param_type
            .abi_decode(&result_bytes)
            .expect("failed decoding output")
    }

    #[tokio::test]
    async fn test_run_l2_tx_validates_tx_gas_limit_too_high() {
        let mut tester = VmRunnerTester::new();
        let tx = TransactionBuilder::new()
            .set_gas_limit(U256::from(u64::MAX) + 1)
            .build();
        let max_gas = U256::from(u64::MAX);
        let expected = AnvilNodeError::TransactionValidationFailed {
            transaction_hash: Box::new(tx.hash()),
            inner: Box::new(anvil_zksync::tx_invalid::InvalidGasLimit {
                tx_gas_limit: Box::new(tx.common_data.fee.gas_limit),
                max_gas: Box::new(max_gas),
            }),
        };
        let err = tester.test_tx(tx.into()).await.unwrap_err();
        assert_eq!(err, expected);
    }

    #[tokio::test]
    async fn test_run_l2_tx_validates_tx_max_fee_per_gas_too_low() {
        let mut tester = VmRunnerTester::new();
        let tx = TransactionBuilder::new()
            .set_max_fee_per_gas(U256::from(DEFAULT_L2_GAS_PRICE - 1))
            .build();
        let expected = AnvilNodeError::TransactionValidationFailed {
            transaction_hash: Box::new(tx.hash()),
            inner: Box::new(anvil_zksync::tx_invalid::MaxFeePerGasTooLow {
                max_fee_per_gas: Box::new(tx.common_data.fee.max_fee_per_gas),
                l2_gas_price: Box::new(DEFAULT_L2_GAS_PRICE.into()),
            }),
        };
        let err = tester.test_tx(tx.into()).await.unwrap_err();
        assert_eq!(err, expected);
    }

    #[tokio::test]
    async fn test_run_l2_tx_validates_tx_max_priority_fee_per_gas_higher_than_max_fee_per_gas() {
        let mut tester = VmRunnerTester::new();
        let max_priority_fee_per_gas = U256::from(250_000_000 + 1);
        let tx = TransactionBuilder::new()
            .set_max_priority_fee_per_gas(max_priority_fee_per_gas)
            .build();

        let expected = AnvilNodeError::TransactionValidationFailed {
            transaction_hash: Box::new(tx.hash()),
            inner: Box::new(anvil_zksync::tx_invalid::MaxPriorityFeeGreaterThanMaxFee {
                max_fee_per_gas: Box::new(tx.common_data.fee.max_fee_per_gas),
                max_priority_fee_per_gas: Box::new(tx.common_data.fee.max_priority_fee_per_gas),
            }),
        };
        let err = tester.test_tx(tx.into()).await.unwrap_err();
        assert_eq!(err, expected);
    }

    #[tokio::test]
    async fn test_run_tx_raw_does_not_panic_on_mock_fork_client_call() {
        let mut tester = VmRunnerTester::new();

        // Perform a transaction to get storage to an intermediate state
        let tx = TransactionBuilder::new().build();
        tester.make_rich(&tx.initiator_account());
        let res = tester.test_tx(tx.into()).await.unwrap();
        assert_eq!(res.receipt.status, U64::from(1));

        // Execute next transaction using a fresh in-memory node and mocked fork client
        let fork_details = ForkDetails {
            chain_id: TEST_NODE_NETWORK_ID.into(),
            batch_number: L1BatchNumber(1),
            block_number: L2BlockNumber(2),
            block_hash: Default::default(),
            block_timestamp: 1002,
            api_block: api::Block::default(),
            l1_gas_price: 1000,
            l2_fair_gas_price: DEFAULT_L2_GAS_PRICE,
            fair_pubdata_price: DEFAULT_FAIR_PUBDATA_PRICE,
            estimate_gas_price_scale_factor: DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
            estimate_gas_scale_factor: DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
            ..Default::default()
        };
        let mock_fork_client = ForkClient::mock(
            fork_details,
            tester
                .vm_runner
                .fork_storage
                .inner
                .read()
                .unwrap()
                .raw_storage
                .clone(),
        );
        let mut tester =
            VmRunnerTester::new_custom(Some(mock_fork_client), TestNodeConfig::default());
        let tx = TransactionBuilder::new().build();
        tester.make_rich(&tx.initiator_account());
        tester
            .test_tx(tx.into())
            .await
            .expect("transaction must pass with mock fork client");
    }

    #[tokio::test]
    async fn test_transact_returns_data_in_built_in_without_security_mode() {
        let mut tester = VmRunnerTester::new_custom(
            None,
            TestNodeConfig {
                system_contracts_options: SystemContractsOptions::BuiltInWithoutSecurity,
                ..Default::default()
            },
        );

        let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xef)).unwrap();
        let from_account = private_key.address();
        tester.make_rich(&from_account);

        let deployed_address = deployed_address_create(from_account, U256::zero());
        let deploy_tx = TransactionBuilder::deploy_contract(
            &private_key,
            hex::decode(STORAGE_CONTRACT_BYTECODE).unwrap(),
            None,
            Nonce(0),
        );

        let mut tx = L2Tx::new_signed(
            Some(deployed_address),
            hex::decode("bbf55335").unwrap(), // keccak selector for "transact_retrieve1()"
            Nonce(1),
            Fee {
                gas_limit: U256::from(4_000_000),
                max_fee_per_gas: U256::from(250_000_000),
                max_priority_fee_per_gas: U256::from(250_000_000),
                gas_per_pubdata_limit: U256::from(50000),
            },
            U256::from(0),
            zksync_types::L2ChainId::from(260),
            &private_key,
            vec![],
            Default::default(),
        )
        .expect("failed signing tx");
        tx.common_data.transaction_type = TransactionType::LegacyTransaction;
        tx.set_input(vec![], H256::repeat_byte(0x2));

        let result = tester
            .test_txs(vec![deploy_tx.into(), tx.into()])
            .await
            .expect("failed tx");
        assert_eq!(
            result[1].receipt.status,
            U64::from(1),
            "invalid status {:?}",
            result[1].receipt.status
        );

        let actual = decode_tx_result(&result[1].debug.output.0, DynSolType::Uint(256));
        let expected = DynSolValue::Uint(AlloyU256::from(1024), 256);
        assert_eq!(expected, actual, "invalid result");
    }
}
