use super::InMemoryNodeInner;
use crate::bootloader_debug::BootloaderDebug;
use crate::console_log::ConsoleLogHandler;
use crate::deps::storage_view::StorageView;
use crate::formatter;
use crate::node::batch::{MainBatchExecutorFactory, TraceCalls};
use crate::node::inner::fork_storage::ForkStorage;
use crate::node::inner::in_memory_inner::BlockContext;
use crate::node::storage_logs::print_storage_logs_details;
use crate::node::time::Time;
use crate::node::{
    compute_hash, TestNodeFeeInputProvider, TransactionResult, TxBatch, TxExecutionInfo,
};
use crate::system_contracts::SystemContracts;
use crate::utils::create_debug_output;
use anvil_zksync_common::{sh_eprintln, sh_err, sh_println};
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_types::{ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use std::collections::HashMap;
use std::sync::Arc;
use zksync_contracts::{BaseSystemContracts, BaseSystemContractsHashes};
use zksync_multivm::interface::executor::{BatchExecutor, BatchExecutorFactory};
use zksync_multivm::interface::{
    BatchTransactionExecutionResult, ExecutionResult, FinishedL1Batch, L1BatchEnv, L2BlockEnv,
    TxExecutionMode, VmEvent, VmExecutionResultAndLogs,
};
use zksync_multivm::zk_evm_latest::ethereum_types::{H160, H256};
use zksync_types::block::L2BlockHasher;
use zksync_types::bytecode::BytecodeHash;
use zksync_types::commitment::{PubdataParams, PubdataType};
use zksync_types::web3::Bytes;
use zksync_types::{
    api, h256_to_address, Address, ExecuteTransactionCommon, L2BlockNumber, L2TxCommonData,
    ProtocolVersionId, Transaction, ACCOUNT_CODE_STORAGE_ADDRESS, U256, U64,
};

pub struct VmRunner {
    current_state: Option<VmRunnerState>,
    executor_factory: Box<dyn BatchExecutorFactory<StorageView<ForkStorage>>>,
    bootloader_debug_result: Arc<std::sync::RwLock<eyre::Result<BootloaderDebug, String>>>,

    time: Time,
    fork_storage: ForkStorage,
    system_contracts: SystemContracts,
    console_log_handler: ConsoleLogHandler,
}

#[derive(Debug)]
struct VmRunnerState {
    executor: Box<dyn BatchExecutor<StorageView<ForkStorage>>>,
    impersonating: bool,
    batch_env: L1BatchEnv,
    block_ctx: BlockContext,
    total_txs_executed: usize,
}

pub(super) struct TxBatchExecutionResult {
    pub(super) tx_results: Vec<TransactionResult>,
    pub(super) base_system_contracts_hashes: BaseSystemContractsHashes,
    pub(super) batch_env: L1BatchEnv,
    pub(super) block_ctxs: Vec<BlockContext>,
    pub(super) finished_l1_batch: Option<FinishedL1Batch>,
}

impl VmRunner {
    pub(super) fn new(
        time: Time,
        fork_storage: ForkStorage,
        system_contracts: SystemContracts,
    ) -> Self {
        let bootloader_debug_result = Arc::new(std::sync::RwLock::new(Err(
            "Tracer has not been run yet".to_string(),
        )));
        Self {
            current_state: None,
            executor_factory: Box::new(MainBatchExecutorFactory::<TraceCalls>::new(
                false,
                bootloader_debug_result.clone(),
            )),
            bootloader_debug_result,

            time,
            fork_storage,
            system_contracts,
            console_log_handler: ConsoleLogHandler::default(),
        }
    }
}

impl VmRunner {
    // Prints the gas details of the transaction for debugging purposes.
    fn display_detailed_gas_info(
        &self,
        bootloader_debug_result: Option<&Result<BootloaderDebug, String>>,
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
        tx_hash: H256,
        tx_data: &L2TxCommonData,
    ) -> anyhow::Result<()> {
        let max_gas = U256::from(u64::MAX);
        if tx_data.fee.gas_limit > max_gas || tx_data.fee.gas_per_pubdata_limit > max_gas {
            anyhow::bail!("exceeds block gas limit");
        }

        let l2_gas_price = batch_env.fee_input.fair_l2_gas_price();
        if tx_data.fee.max_fee_per_gas < l2_gas_price.into() {
            sh_eprintln!(
                "Submitted Tx is Unexecutable {:?} because of MaxFeePerGasTooLow {}",
                tx_hash,
                tx_data.fee.max_fee_per_gas
            );
            anyhow::bail!("block base fee higher than max fee per gas");
        }

        if tx_data.fee.max_fee_per_gas < tx_data.fee.max_priority_fee_per_gas {
            sh_eprintln!(
                "Submitted Tx is Unexecutable {:?} because of MaxPriorityFeeGreaterThanMaxFee {}",
                tx_hash,
                tx_data.fee.max_fee_per_gas
            );
            anyhow::bail!("max priority fee per gas higher than max fee per gas");
        }
        Ok(())
    }

    async fn run_tx_pretty(
        &mut self,
        tx: Transaction,
        executor: &mut dyn BatchExecutor<StorageView<ForkStorage>>,
        config: &TestNodeConfig,
        fee_input_provider: &TestNodeFeeInputProvider,
    ) -> anyhow::Result<BatchTransactionExecutionResult> {
        let BatchTransactionExecutionResult {
            tx_result,
            compression_result,
            call_traces,
        } = executor.execute_tx(tx.clone()).await?;
        compression_result?;

        let spent_on_pubdata =
            tx_result.statistics.gas_used - tx_result.statistics.computational_gas_used as u64;

        let status = match &tx_result.result {
            ExecutionResult::Success { .. } => "SUCCESS",
            ExecutionResult::Revert { .. } => "FAILED",
            ExecutionResult::Halt { .. } => "HALTED",
        };

        // Print transaction summary
        if config.show_tx_summary {
            formatter::print_transaction_summary(
                config.get_l2_gas_price(),
                &tx,
                &tx_result,
                status,
            );
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

        if !call_traces.is_empty() {
            if !config.disable_console_log {
                self.console_log_handler
                    .handle_calls_recursive(&call_traces);
            }

            if config.show_calls != ShowCalls::None {
                sh_println!(
                    "[Transaction Execution] ({} calls)",
                    call_traces[0].calls.len()
                );
                let num_calls = call_traces.len();
                for (i, call) in call_traces.iter().enumerate() {
                    let is_last_sibling = i == num_calls - 1;
                    let mut formatter = formatter::Formatter::new();
                    formatter.print_call(
                        tx.initiator_account(),
                        tx.execute.contract_address,
                        call,
                        is_last_sibling,
                        config.show_calls,
                        config.show_outputs,
                        config.resolve_hashes,
                    );
                }
            }
        }
        // Print event logs if enabled
        if config.show_event_logs {
            sh_println!("[Events] ({} events)", tx_result.logs.events.len());
            for (i, event) in tx_result.logs.events.iter().enumerate() {
                let is_last = i == tx_result.logs.events.len() - 1;
                let mut formatter = formatter::Formatter::new();
                formatter.print_event(event, config.resolve_hashes, is_last);
            }
        }

        Ok(BatchTransactionExecutionResult {
            tx_result,
            compression_result: Ok(()),
            call_traces,
        })
    }

    /// Runs transaction and commits it to a new block.
    async fn run_tx(
        &mut self,
        tx: Transaction,
        tx_index: u64,
        block_ctx: &BlockContext,
        batch_env: &L1BatchEnv,
        executor: &mut dyn BatchExecutor<StorageView<ForkStorage>>,
        config: &TestNodeConfig,
        fee_input_provider: &TestNodeFeeInputProvider,
    ) -> anyhow::Result<TransactionResult> {
        let tx_hash = tx.hash();
        let transaction_type = tx.tx_format();

        if let ExecuteTransactionCommon::L2(l2_tx_data) = &tx.common_data {
            self.validate_tx(batch_env, tx.hash(), l2_tx_data)?;
        }

        let BatchTransactionExecutionResult {
            tx_result: result,
            compression_result: _,
            call_traces,
        } = self
            .run_tx_pretty(tx.clone(), executor, config, fee_input_provider)
            .await?;

        if let ExecutionResult::Halt { reason } = result.result {
            // Halt means that something went really bad with the transaction execution (in most cases invalid signature,
            // but it could also be bootloader panic etc).
            // In such case, we should not persist the VM data, and we should pretend that transaction never existed.
            anyhow::bail!("Transaction HALT: {reason}");
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

        let known_bytecodes = saved_factory_deps.map(|bytecode_hash| {
            let bytecode = tx_factory_deps.get(&bytecode_hash).unwrap_or_else(|| {
                panic!(
                    "Failed to get factory deps on tx: bytecode hash: {:?}, tx hash: {}",
                    bytecode_hash,
                    tx.hash()
                )
            });
            (bytecode_hash, bytecode.clone())
        });

        // Write factory deps
        for (hash, code) in known_bytecodes {
            self.fork_storage.store_factory_dep(hash, code)
        }

        // Write storage logs
        for storage_log in result
            .logs
            .storage_logs
            .iter()
            .filter(|log| log.log.is_write())
        {
            self.fork_storage
                .set_value(storage_log.log.key, storage_log.log.value);
        }

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
            l2_to_l1_logs: vec![],
            status: if result.result.is_failed() {
                U64::from(0)
            } else {
                U64::from(1)
            },
            effective_gas_price: Some(batch_env.fee_input.fair_l2_gas_price().into()),
            transaction_type: Some((transaction_type as u32).into()),
            logs_bloom: Default::default(),
        };
        let debug = create_debug_output(&tx, &result, call_traces).expect("create debug output"); // OK to unwrap here as Halt is handled above

        Ok(TransactionResult {
            info: TxExecutionInfo {
                tx,
                batch_number: batch_env.number.0,
                miniblock_number: block_ctx.miniblock,
            },
            receipt: tx_receipt,
            debug,
        })
    }

    async fn init_batch(
        &mut self,
        impersonating: bool,
        system_contracts: BaseSystemContracts,
        node_inner: &InMemoryNodeInner,
    ) -> anyhow::Result<VmRunnerState> {
        // Prepare a new block context and a new batch env
        let system_env =
            node_inner.create_system_env(system_contracts, TxExecutionMode::VerifyExecute);
        let (batch_env, block_ctx) = node_inner.create_l1_batch_env().await;
        // Advance clock as we are consuming next timestamp for this block
        anyhow::ensure!(
            self.time.advance_timestamp() == block_ctx.timestamp,
            "advancing clock produced different timestamp than expected"
        );
        let storage = StorageView::new(self.fork_storage.clone());
        let pubdata_params = PubdataParams {
            l2_da_validator_address: Address::zero(),
            pubdata_type: PubdataType::Rollup,
        };
        let executor = if self.system_contracts.use_zkos {
            todo!("BatchExecutor support for zkos is yet to be implemented")
        } else {
            self.executor_factory
                .init_batch(storage, batch_env.clone(), system_env, pubdata_params)
        };

        Ok(VmRunnerState {
            executor,
            impersonating,
            batch_env,
            block_ctx,
            total_txs_executed: 0,
        })
    }

    pub(super) async fn run_tx_batch(
        &mut self,
        TxBatch { txs, impersonating }: TxBatch,
        node_inner: &mut InMemoryNodeInner,
    ) -> anyhow::Result<TxBatchExecutionResult> {
        let system_contracts = self
            .system_contracts
            .contracts(TxExecutionMode::VerifyExecute, impersonating)
            .clone();
        let base_system_contracts_hashes = system_contracts.hashes();
        let mut state = match self.current_state.take() {
            Some(state) if impersonating != state.impersonating => {
                // If impersonating status differs from the current one we have to return the state
                // and force seal the existing batch
                self.current_state = Some(state);
                Box::pin(self.force_seal_batch(node_inner)).await?;
                // Then we can safely initialize new batch with desired impersonation status
                self.init_batch(impersonating, system_contracts, node_inner)
                    .await?
            }
            Some(state) => {
                let mut executor = state.executor;
                let block_ctx = state.block_ctx.new_block(&mut self.time);

                let l2_block_env = L2BlockEnv {
                    number: block_ctx.miniblock as u32,
                    timestamp: block_ctx.timestamp,
                    prev_block_hash: state.block_ctx.hash,
                    max_virtual_blocks_to_create: 1,
                };
                executor.start_next_l2_block(l2_block_env).await?;

                VmRunnerState {
                    executor,
                    impersonating,
                    batch_env: state.batch_env,
                    block_ctx,
                    total_txs_executed: state.total_txs_executed,
                }
            }
            None => {
                self.init_batch(impersonating, system_contracts, node_inner)
                    .await?
            }
        };

        // Compute block hash. Note that the computed block hash here will be different than that in production.
        let tx_hashes = txs.iter().map(|t| t.hash()).collect::<Vec<_>>();
        state.block_ctx.hash = compute_hash(
            (state.block_ctx.miniblock as u32).into(),
            state.block_ctx.timestamp,
            state.block_ctx.prev_block_hash,
            &tx_hashes,
        );

        // Execute transactions and bootloader
        let mut tx_results = Vec::with_capacity(tx_hashes.len());
        let mut tx_index = 0;
        for tx in txs {
            let result = self
                .run_tx(
                    tx,
                    tx_index,
                    &state.block_ctx,
                    &state.batch_env,
                    state.executor.as_mut(),
                    &node_inner.config,
                    &node_inner.fee_input_provider,
                )
                .await;

            match result {
                Ok(tx_result) => {
                    tx_results.push(tx_result);
                    tx_index += 1;
                }
                Err(e) => {
                    sh_err!("Error while executing transaction: {e}");
                    state.executor.rollback_last_tx().await?;
                }
            }
        }
        // TODO: This is the correct hash as reported by VM, but we can't compute it correct above
        //       because we don't know which txs are going to be halted
        state.block_ctx.hash = compute_hash(
            (state.block_ctx.miniblock as u32).into(),
            state.block_ctx.timestamp,
            state.block_ctx.prev_block_hash,
            tx_results
                .iter()
                .map(|tx_result| &tx_result.receipt.transaction_hash),
        );
        state.total_txs_executed += tx_results.len();

        if self.should_seal(&state, &tx_results) {
            println!("SEALING {state:?}");
            let mut block_ctxs = vec![state.block_ctx.clone()];
            if !tx_results.is_empty() {
                println!("CREATING FICTIVE BLOCK");
                // Create an empty virtual block at the end of the batch (only if the last block was
                // not empty, i.e. virtual).
                let mut virtual_block_ctx = state.block_ctx.new_block(&mut self.time);
                virtual_block_ctx.hash = L2BlockHasher::new(
                    L2BlockNumber(virtual_block_ctx.miniblock as u32),
                    virtual_block_ctx.timestamp,
                    state.block_ctx.hash,
                )
                .finalize(ProtocolVersionId::latest());
                let l2_block_env = L2BlockEnv {
                    number: (state.block_ctx.miniblock + 1) as u32,
                    timestamp: state.block_ctx.timestamp + 1,
                    prev_block_hash: state.block_ctx.hash,
                    max_virtual_blocks_to_create: 1,
                };
                state.executor.start_next_l2_block(l2_block_env).await?;
                block_ctxs.push(virtual_block_ctx);
            }

            let (finished_l1_batch, _) = state.executor.finish_batch().await?;
            anyhow::ensure!(
                !finished_l1_batch
                    .block_tip_execution_result
                    .result
                    .is_failed(),
                "VM must not fail when finalizing block: {:#?}",
                finished_l1_batch.block_tip_execution_result.result
            );
            println!("HUH");

            // TODO: Save fictive block's storage logs, events, system/user L2->L1 logs
            // Write fictive block's storage logs
            for storage_log in finished_l1_batch
                .block_tip_execution_result
                .logs
                .storage_logs
                .iter()
                .filter(|log| log.log.is_write())
            {
                self.fork_storage
                    .set_value(storage_log.log.key, storage_log.log.value);
            }
            println!("WUT");

            Ok(TxBatchExecutionResult {
                tx_results,
                base_system_contracts_hashes,
                batch_env: state.batch_env,
                block_ctxs,
                finished_l1_batch: Some(finished_l1_batch),
            })
        } else {
            println!("NOT SEALING {state:?}");
            let batch_env = state.batch_env.clone();
            let block_ctxs = vec![state.block_ctx.clone()];
            self.current_state = Some(state);
            Ok(TxBatchExecutionResult {
                tx_results,
                base_system_contracts_hashes,
                batch_env,
                block_ctxs,
                finished_l1_batch: None,
            })
        }
    }

    fn should_seal(&self, state: &VmRunnerState, tx_results: &[TransactionResult]) -> bool {
        state.block_ctx.miniblock as u32 - state.batch_env.first_l2_block.number + 1 >= 100
            || tx_results.is_empty()
            || tx_results
                .iter()
                .any(|tx_result| !tx_result.receipt.l2_to_l1_logs.is_empty())
            || tx_results.iter().any(|tx_result| {
                matches!(
                    tx_result.info.tx.common_data,
                    ExecuteTransactionCommon::ProtocolUpgrade(_)
                )
            })
    }

    pub(super) async fn force_seal_batch(
        &mut self,
        node_inner: &mut InMemoryNodeInner,
    ) -> anyhow::Result<()> {
        if let Some(state) = &self.current_state {
            let tx_batch_execution_result = self
                .run_tx_batch(
                    TxBatch {
                        impersonating: state.impersonating,
                        txs: vec![],
                    },
                    node_inner,
                )
                .await?;
            node_inner.seal_block(tx_batch_execution_result).await?;
        }

        Ok(())
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
