use crate::bootloader_debug::{BootloaderDebug, BootloaderDebugTracer};
use crate::console_log::ConsoleLogHandler;
use crate::deps::storage_view::StorageView;
use crate::filters::EthFilters;
use crate::node::call_error_tracer::CallErrorTracer;
use crate::node::error::LoadStateError;
use crate::node::inner::blockchain::{Blockchain, ReadBlockchain};
use crate::node::inner::fork::{Fork, ForkClient, ForkSource};
use crate::node::inner::fork_storage::{ForkStorage, SerializableStorage};
use crate::node::inner::time::Time;
use crate::node::keys::StorageKeyLayout;
use crate::node::state::StateV1;
use crate::node::storage_logs::print_storage_logs_details;
use crate::node::vm::AnvilVM;
use crate::node::zkos::ZKOsVM;
use crate::node::{
    compute_hash, create_block, ImpersonationManager, Snapshot, TestNodeFeeInputProvider,
    TransactionResult, TxExecutionInfo, VersionedState, ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION,
    MAX_PREVIOUS_STATES, MAX_TX_SIZE,
};

use crate::system_contracts::SystemContracts;
use crate::utils::create_debug_output;
use crate::{delegate_vm, formatter, utils};
use anvil_zksync_config::constants::{
    LEGACY_RICH_WALLETS, NON_FORK_FIRST_BLOCK_TIMESTAMP, RICH_WALLETS,
};
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_types::{ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use anvil_zksync_common::{sh_println, sh_eprintln, sh_err};
use anyhow::Context;
use colored::Colorize;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use zksync_contracts::BaseSystemContracts;
use zksync_multivm::interface::storage::{ReadStorage, WriteStorage};
use zksync_multivm::interface::VmFactory;
use zksync_multivm::interface::{
    Call, ExecutionResult, InspectExecutionMode, L1BatchEnv, L2BlockEnv, SystemEnv,
    TxExecutionMode, VmExecutionResultAndLogs, VmInterface, VmInterfaceExt,
    VmInterfaceHistoryEnabled,
};
use zksync_multivm::vm_latest::Vm;

use zksync_multivm::tracers::CallTracer;
use zksync_multivm::utils::{
    adjust_pubdata_price_for_tx, derive_base_fee_and_gas_per_pubdata, derive_overhead,
    get_max_gas_per_pubdata_byte,
};
use zksync_multivm::vm_latest::constants::{
    BATCH_COMPUTATIONAL_GAS_LIMIT, BATCH_GAS_LIMIT, MAX_VM_PUBDATA_PER_BATCH,
};
use zksync_multivm::vm_latest::{
    HistoryDisabled, HistoryEnabled, HistoryMode, ToTracerPointer, TracerPointer,
};
use zksync_multivm::VmVersion;
use zksync_types::api::{BlockIdVariant, TransactionVariant};
use zksync_types::block::build_bloom;
use zksync_types::bytecode::BytecodeHash;
use zksync_types::fee::Fee;
use zksync_types::fee_model::{BatchFeeInput, PubdataIndependentBatchFeeModelInput};
use zksync_types::l2::{L2Tx, TransactionType};
use zksync_types::transaction_request::CallRequest;
use zksync_types::utils::{decompose_full_nonce, nonces_to_full_nonce};
use zksync_types::web3::{Bytes, Index};
use zksync_types::{
    api, h256_to_address, h256_to_u256, u256_to_h256, AccountTreeId, Address, Bloom, BloomInput,
    L1BatchNumber, L2BlockNumber, L2ChainId, StorageKey, StorageValue, Transaction,
    ACCOUNT_CODE_STORAGE_ADDRESS, H160, H256, MAX_L2_TX_GAS_LIMIT, U256, U64,
};
use zksync_web3_decl::error::Web3Error;

// TODO: Rename `InMemoryNodeInner` to something more sensible
/// Helper struct for InMemoryNode.
pub struct InMemoryNodeInner {
    /// Writeable blockchain state.
    blockchain: Blockchain,
    pub(super) time: Time,
    /// The fee input provider.
    pub fee_input_provider: TestNodeFeeInputProvider,
    // Map from filter_id to the eth filter
    pub filters: Arc<tokio::sync::RwLock<EthFilters>>,
    // TODO: Make private
    // Underlying storage
    pub fork_storage: ForkStorage,
    pub(super) fork: Fork,
    // Configuration.
    pub config: TestNodeConfig,
    pub console_log_handler: ConsoleLogHandler,
    system_contracts: SystemContracts,
    impersonation: ImpersonationManager,
    pub rich_accounts: HashSet<H160>,
    /// Keeps track of historical states indexed via block hash. Limited to [MAX_PREVIOUS_STATES].
    previous_states: IndexMap<H256, HashMap<StorageKey, StorageValue>>,
    storage_key_layout: StorageKeyLayout,
}

impl InMemoryNodeInner {
    /// Create the state to be used implementing [InMemoryNode].
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        blockchain: Blockchain,
        time: Time,
        fork_storage: ForkStorage,
        fork: Fork,
        fee_input_provider: TestNodeFeeInputProvider,
        filters: Arc<RwLock<EthFilters>>,
        config: TestNodeConfig,
        impersonation: ImpersonationManager,
        system_contracts: SystemContracts,
        storage_key_layout: StorageKeyLayout,
    ) -> Self {
        InMemoryNodeInner {
            blockchain,
            time,
            fee_input_provider,
            filters,
            fork_storage,
            fork,
            config,
            console_log_handler: ConsoleLogHandler::default(),
            system_contracts,
            impersonation,
            rich_accounts: HashSet::new(),
            previous_states: Default::default(),
            storage_key_layout,
        }
    }

    pub fn create_system_env(
        &self,
        base_system_contracts: BaseSystemContracts,
        execution_mode: TxExecutionMode,
    ) -> SystemEnv {
        SystemEnv {
            zk_porter_available: false,
            // TODO: when forking, we could consider taking the protocol version id from the fork itself.
            version: zksync_types::ProtocolVersionId::latest(),
            base_system_smart_contracts: base_system_contracts,
            bootloader_gas_limit: BATCH_COMPUTATIONAL_GAS_LIMIT,
            execution_mode,
            default_validation_computational_gas_limit: BATCH_COMPUTATIONAL_GAS_LIMIT,
            chain_id: self.fork_storage.chain_id,
        }
    }

    /// Create [L1BatchEnv] to be used in the VM.
    ///
    /// We compute l1/l2 block details from storage to support fork testing, where the storage
    /// can be updated mid execution and no longer matches with the initial node's state.
    /// The L1 & L2 timestamps are also compared with node's timestamp to ensure it always increases monotonically.
    pub async fn create_l1_batch_env(&self) -> (L1BatchEnv, BlockContext) {
        tracing::debug!("creating L1 batch env");

        let (last_l1_batch_number, last_l2_block) = self.blockchain.read().await.last_env(
            &StorageView::new(&self.fork_storage).into_rc_ptr(),
            &self.time,
        );

        let block_ctx = BlockContext {
            hash: H256::zero(),
            batch: (last_l1_batch_number + 1).0,
            miniblock: last_l2_block.number as u64 + 1,
            timestamp: self.time.peek_next_timestamp(),
        };

        let fee_input = if let Some(fork_details) = self.fork.details() {
            // TODO: This is a weird pattern. `TestNodeFeeInputProvider` should encapsulate fork's
            //       behavior by taking fork's fee input into account during initialization.
            BatchFeeInput::PubdataIndependent(PubdataIndependentBatchFeeModelInput {
                l1_gas_price: fork_details.l1_gas_price,
                fair_l2_gas_price: fork_details.l2_fair_gas_price,
                fair_pubdata_price: fork_details.fair_pubdata_price,
            })
        } else {
            self.fee_input_provider.get_batch_fee_input()
        };

        let batch_env = L1BatchEnv {
            // TODO: set the previous batch hash properly (take from fork, when forking, and from local storage, when this is not the first block).
            previous_batch_hash: None,
            number: L1BatchNumber::from(block_ctx.batch),
            timestamp: block_ctx.timestamp,
            fee_input,
            fee_account: H160::zero(),
            enforced_base_fee: None,
            first_l2_block: L2BlockEnv {
                // the 'current_miniblock' contains the block that was already produced.
                // So the next one should be one higher.
                number: block_ctx.miniblock as u32,
                timestamp: block_ctx.timestamp,
                prev_block_hash: last_l2_block.hash,
                // This is only used during zksyncEra block timestamp/number transition.
                // In case of starting a new network, it doesn't matter.
                // In theory , when forking mainnet, we should match this value
                // to the value that was set in the node at that time - but AFAIK
                // we don't have any API for this - so this might result in slightly
                // incorrect replays of transacions during the migration period, that
                // depend on block number or timestamp.
                max_virtual_blocks_to_create: 1,
            },
        };

        (batch_env, block_ctx)
    }

    async fn apply_batch(
        &mut self,
        blocks: impl IntoIterator<Item = api::Block<api::TransactionVariant>>,
        tx_results: impl IntoIterator<Item = TransactionResult>,
    ) {
        // TODO: Move this to a dedicated `PreviousStates` struct once we have one
        /// Archives the current state for later queries.
        fn archive_state(
            previous_states: &mut IndexMap<H256, HashMap<StorageKey, StorageValue>>,
            state: HashMap<StorageKey, StorageValue>,
            block_number: L2BlockNumber,
            block_hash: H256,
        ) {
            if previous_states.len() > MAX_PREVIOUS_STATES as usize {
                if let Some(entry) = previous_states.shift_remove_index(0) {
                    tracing::debug!("removing archived state for previous block {:#x}", entry.0);
                }
            }
            tracing::debug!("archiving state for {:#x} #{}", block_hash, block_number);
            previous_states.insert(block_hash, state);
        }

        let mut storage = self.blockchain.write().await;
        storage.current_batch += 1;
        storage.tx_results.extend(
            tx_results
                .into_iter()
                .map(|r| (r.receipt.transaction_hash, r)),
        );
        for (index, block) in blocks.into_iter().enumerate() {
            // archive current state before we produce new batch/blocks
            archive_state(
                &mut self.previous_states,
                self.fork_storage
                    .inner
                    .read()
                    .unwrap()
                    .raw_storage
                    .state
                    .clone(),
                storage.current_block,
                storage.current_block_hash,
            );
            storage.apply_block(block, index as u32);
        }
    }

    // Prints the gas details of the transaction for debugging purposes.
    fn display_detailed_gas_info(
        &self,
        bootloader_debug_result: Option<&eyre::Result<BootloaderDebug, String>>,
        spent_on_pubdata: u64,
    ) -> eyre::Result<(), String> {
        if let Some(bootloader_result) = bootloader_debug_result {
            let bootloader_debug = bootloader_result.clone()?;

            let gas_details = formatter::compute_gas_details(&bootloader_debug, spent_on_pubdata);
            let mut formatter = formatter::Formatter::new();

            let fee_model_config = self.fee_input_provider.get_fee_model_config();

            formatter.print_gas_details(&gas_details, &fee_model_config);

            Ok(())
        } else {
            Err("Bootloader tracer didn't finish.".to_owned())
        }
    }

    /// Validates L2 transaction
    fn validate_tx(&self, tx: &L2Tx) -> anyhow::Result<()> {
        let max_gas = U256::from(u64::MAX);
        if tx.common_data.fee.gas_limit > max_gas
            || tx.common_data.fee.gas_per_pubdata_limit > max_gas
        {
            anyhow::bail!("exceeds block gas limit");
        }

        let l2_gas_price = self.fee_input_provider.gas_price();
        if tx.common_data.fee.max_fee_per_gas < l2_gas_price.into() {
            sh_println!(
                "Submitted Tx is Unexecutable {:?} because of MaxFeePerGasTooLow {}",
                tx.hash(),
                tx.common_data.fee.max_fee_per_gas
            );
            anyhow::bail!("block base fee higher than max fee per gas");
        }

        if tx.common_data.fee.max_fee_per_gas < tx.common_data.fee.max_priority_fee_per_gas {
            sh_println!(
                "Submitted Tx is Unexecutable {:?} because of MaxPriorityFeeGreaterThanMaxFee {}",
                tx.hash(),
                tx.common_data.fee.max_fee_per_gas
            );
            anyhow::bail!("max priority fee per gas higher than max fee per gas");
        }
        Ok(())
    }

    /// Executes the given L2 transaction and returns all the VM logs.
    /// The bootloader can be omitted via specifying the `execute_bootloader` boolean.
    /// This causes the VM to produce 1 L2 block per L1 block, instead of the usual 2 blocks per L1 block.
    ///
    /// **NOTE**
    ///
    /// This function must only rely on data populated initially via [ForkDetails]:
    ///     * [InMemoryNodeInner::current_timestamp]
    ///     * [InMemoryNodeInner::current_batch]
    ///     * [InMemoryNodeInner::current_miniblock]
    ///     * [InMemoryNodeInner::current_miniblock_hash]
    ///     * [InMemoryNodeInner::fee_input_provider]
    ///
    /// And must _NEVER_ rely on data updated in [InMemoryNodeInner] during previous runs:
    /// (if used, they must never panic and/or have meaningful defaults)
    ///     * [InMemoryNodeInner::block_hashes]
    ///     * [InMemoryNodeInner::blocks]
    ///     * [InMemoryNodeInner::tx_results]
    ///
    /// This is because external users of the library may call this function to perform an isolated
    /// VM operation (optionally without bootloader execution) with an external storage and get the results back.
    /// So any data populated in [Self::run_l2_tx] will not be available for the next invocation.
    fn run_l2_tx_raw<VM: VmInterface, W: WriteStorage, H: HistoryMode>(
        &self,
        l2_tx: L2Tx,
        vm: &mut VM,
    ) -> anyhow::Result<TxExecutionOutput>
    where
        <VM as VmInterface>::TracerDispatcher: From<Vec<TracerPointer<W, H>>>,
    {
        let tx: Transaction = l2_tx.into();

        let call_tracer_result = Arc::new(OnceCell::default());
        let bootloader_debug_result = Arc::new(OnceCell::default());
        let error_flags_result = Arc::new(OnceCell::new());

        let tracers = vec![
            CallErrorTracer::new(error_flags_result.clone()).into_tracer_pointer(),
            CallTracer::new(call_tracer_result.clone()).into_tracer_pointer(),
            BootloaderDebugTracer {
                result: bootloader_debug_result.clone(),
            }
            .into_tracer_pointer(),
        ];
        let compressed_bytecodes = vm
            .push_transaction(tx.clone())
            .compressed_bytecodes
            .into_owned();
        let tx_result = vm.inspect(&mut tracers.into(), InspectExecutionMode::OneTx);

        let call_traces = call_tracer_result.get();

        let spent_on_pubdata =
            tx_result.statistics.gas_used - tx_result.statistics.computational_gas_used as u64;

        let status = match &tx_result.result {
            ExecutionResult::Success { .. } => "SUCCESS",
            ExecutionResult::Revert { .. } => "FAILED",
            ExecutionResult::Halt { .. } => "HALTED",
        };

        // Print transaction summary
        if self.config.show_tx_summary {
            sh_println!("\n");
            formatter::print_transaction_summary(
                self.config.get_l2_gas_price(),
                &tx,
                &tx_result,
                status,
            );
            sh_println!("\n");
        }
        // Print gas details if enabled
        if self.config.show_gas_details != ShowGasDetails::None {
            self.display_detailed_gas_info(bootloader_debug_result.get(), spent_on_pubdata)
                .unwrap_or_else(|err| {
                    sh_err!("{}", format!("Cannot display gas details: {err}").on_red());
                });
        }
        // Print storage logs if enabled
        if self.config.show_storage_logs != ShowStorageLogs::None {
            print_storage_logs_details(self.config.show_storage_logs, &tx_result);
        }
        // Print VM details if enabled
        if self.config.show_vm_details != ShowVMDetails::None {
            let mut formatter = formatter::Formatter::new();
            formatter.print_vm_details(&tx_result);
        }

        if let Some(call_traces) = call_traces {
            if !self.config.disable_console_log {
                self.console_log_handler.handle_calls_recursive(call_traces);
            }

            if self.config.show_calls != ShowCalls::None {
                sh_println!("\n");
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
                        self.config.show_calls,
                        self.config.show_outputs,
                        self.config.resolve_hashes,
                    );
                }
            }
        }
        // Print event logs if enabled
        if self.config.show_event_logs {
            sh_println!("\n");
            sh_println!("[Events] ({} events)", tx_result.logs.events.len());
            for (i, event) in tx_result.logs.events.iter().enumerate() {
                let is_last = i == tx_result.logs.events.len() - 1;
                let mut formatter = formatter::Formatter::new();
                formatter.print_event(event, self.config.resolve_hashes, is_last);
            }
            sh_println!("\n");
        }

        let mut bytecodes = HashMap::new();
        for b in &*compressed_bytecodes {
            zksync_types::bytecode::validate_bytecode(&b.original).context("Invalid bytecode")?;
            let hash = BytecodeHash::for_bytecode(&b.original).value();
            bytecodes.insert(hash, b.original.clone());
        }
        // Also add bytecodes that were created by EVM.
        for entry in &tx_result.dynamic_factory_deps {
            bytecodes.insert(*entry.0, entry.1.clone());
        }

        Ok(TxExecutionOutput {
            result: tx_result,
            call_traces: call_traces.cloned().unwrap_or_default(),
            bytecodes,
        })
    }

    /// Runs L2 transaction and commits it to a new block.
    fn run_l2_tx<VM: VmInterface, W: WriteStorage, H: HistoryMode>(
        &mut self,
        l2_tx: L2Tx,
        l2_tx_index: u64,
        block_ctx: &BlockContext,
        batch_env: &L1BatchEnv,
        vm: &mut VM,
    ) -> anyhow::Result<TransactionResult>
    where
        <VM as VmInterface>::TracerDispatcher: From<Vec<TracerPointer<W, H>>>,
    {
        let tx_hash = l2_tx.hash();
        let transaction_type = l2_tx.common_data.transaction_type;

        if self.config.show_tx_summary {
            sh_println!("\n");
            sh_println!("Validating {}", format!("{:?}", tx_hash).bold());
        }

        self.validate_tx(&l2_tx)?;

        if self.config.show_tx_summary {
            sh_println!("Executing {}", format!("{:?}", tx_hash).bold());
        }

        let TxExecutionOutput {
            result,
            bytecodes,
            call_traces,
        } = self.run_l2_tx_raw(l2_tx.clone(), vm)?;

        if let ExecutionResult::Halt { reason } = result.result {
            // Halt means that something went really bad with the transaction execution (in most cases invalid signature,
            // but it could also be bootloader panic etc).
            // In such case, we should not persist the VM data, and we should pretend that transaction never existed.
            anyhow::bail!("Transaction HALT: {reason}");
        }

        // Write all the factory deps.
        for (hash, code) in bytecodes {
            self.fork_storage.store_factory_dep(hash, code)
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
                transaction_index: Some(U64::from(l2_tx_index)),
                log_index: Some(U256::from(log_idx)),
                transaction_log_index: Some(U256::from(log_idx)),
                log_type: None,
                removed: Some(false),
                block_timestamp: Some(block_ctx.timestamp.into()),
            })
            .collect();
        let tx_receipt = api::TransactionReceipt {
            transaction_hash: tx_hash,
            transaction_index: U64::from(l2_tx_index),
            block_hash: block_ctx.hash,
            block_number: block_ctx.miniblock.into(),
            l1_batch_tx_index: None,
            l1_batch_number: Some(U64::from(batch_env.number.0)),
            from: l2_tx.initiator_account(),
            to: l2_tx.recipient_account(),
            cumulative_gas_used: Default::default(),
            gas_used: Some(l2_tx.common_data.fee.gas_limit - result.refunds.gas_refunded),
            contract_address: contract_address_from_tx_result(&result),
            logs,
            l2_to_l1_logs: vec![],
            status: if result.result.is_failed() {
                U64::from(0)
            } else {
                U64::from(1)
            },
            effective_gas_price: Some(self.fee_input_provider.gas_price().into()),
            transaction_type: Some((transaction_type as u32).into()),
            logs_bloom: Default::default(),
        };
        let debug = create_debug_output(&l2_tx, &result, call_traces).expect("create debug output"); // OK to unwrap here as Halt is handled above

        Ok(TransactionResult {
            info: TxExecutionInfo {
                tx: l2_tx,
                batch_number: batch_env.number.0,
                miniblock_number: block_ctx.miniblock,
            },
            receipt: tx_receipt,
            debug,
        })
    }

    fn run_l2_txs(
        &mut self,
        txs: Vec<L2Tx>,
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
        block_ctx: &mut BlockContext,
    ) -> Vec<TransactionResult> {
        let storage = StorageView::new(self.fork_storage.clone()).into_rc_ptr();

        let mut vm = if self.system_contracts.use_zkos {
            AnvilVM::ZKOs(ZKOsVM::<_, HistoryEnabled>::new(
                batch_env.clone(),
                system_env,
                storage.clone(),
                // TODO: this might be causing a deadlock.. check..
                &self.fork_storage.inner.read().unwrap().raw_storage,
            ))
        } else {
            AnvilVM::ZKSync(Vm::new(batch_env.clone(), system_env, storage.clone()))
        };

        // Compute block hash. Note that the computed block hash here will be different than that in production.
        let tx_hashes = txs.iter().map(|t| t.hash()).collect::<Vec<_>>();
        let hash = compute_hash(block_ctx.miniblock, &tx_hashes);
        block_ctx.hash = hash;

        // Execute transactions and bootloader
        let mut tx_results = Vec::with_capacity(tx_hashes.len());
        let mut tx_index = 0;
        for tx in txs {
            // Executing a next transaction means that a previous transaction was either rolled back (in which case its snapshot
            // was already removed), or that we build on top of it (in which case, it can be removed now).
            delegate_vm!(vm, pop_snapshot_no_rollback());
            // Save pre-execution VM snapshot.
            delegate_vm!(vm, make_snapshot());
            let result = match vm {
                AnvilVM::ZKSync(ref mut vm) => {
                    self.run_l2_tx(tx, tx_index, block_ctx, &batch_env, vm)
                }
                AnvilVM::ZKOs(ref mut vm) => {
                    self.run_l2_tx(tx, tx_index, block_ctx, &batch_env, vm)
                }
            };

            match result {
                //self.run_l2_tx(tx, tx_index, block_ctx, &batch_env, &mut vm) {
                Ok(tx_result) => {
                    tx_results.push(tx_result);
                    tx_index += 1;
                }
                Err(e) => {
                    sh_err!("Error while executing transaction: {e}");
                    delegate_vm!(vm, rollback_to_the_latest_snapshot());
                }
            }
        }
        delegate_vm!(vm, execute(InspectExecutionMode::Bootloader));

        // Write all the mutated keys (storage slots).
        for (key, value) in storage.borrow().modified_storage_keys() {
            self.fork_storage.set_value(*key, *value);
        }
        tx_results
    }

    pub(super) async fn seal_block(
        &mut self,
        txs: Vec<L2Tx>,
        system_contracts: BaseSystemContracts,
    ) -> anyhow::Result<L2BlockNumber> {
        // Prepare a new block context and a new batch env
        let system_env = self.create_system_env(system_contracts, TxExecutionMode::VerifyExecute);
        let (batch_env, mut block_ctx) = self.create_l1_batch_env().await;
        // Advance clock as we are consuming next timestamp for this block
        anyhow::ensure!(
            self.time.advance_timestamp() == block_ctx.timestamp,
            "advancing clock produced different timestamp than expected"
        );

        let tx_results = self.run_l2_txs(txs, batch_env.clone(), system_env, &mut block_ctx);

        let mut filters = self.filters.write().await;
        for tx_result in &tx_results {
            // TODO: Is this the right place to notify about new pending txs?
            filters.notify_new_pending_transaction(tx_result.receipt.transaction_hash);
            for log in &tx_result.receipt.logs {
                filters.notify_new_log(log, block_ctx.miniblock.into());
            }
        }
        drop(filters);

        let mut transactions = Vec::new();
        for (index, tx_result) in tx_results.iter().enumerate() {
            let mut transaction = zksync_types::api::Transaction::from(tx_result.info.tx.clone());
            transaction.block_hash = Some(block_ctx.hash);
            transaction.block_number = Some(U64::from(block_ctx.miniblock));
            transaction.transaction_index = Some(index.into());
            transaction.l1_batch_number = Some(U64::from(batch_env.number.0));
            transaction.l1_batch_tx_index = Some(Index::zero());
            if transaction.transaction_type == Some(U64::zero())
                || transaction.transaction_type.is_none()
            {
                transaction.v = transaction
                    .v
                    .map(|v| v + 35 + self.fork_storage.chain_id.as_u64() * 2);
            }
            transactions.push(TransactionVariant::Full(transaction));
        }

        // Build bloom hash
        let iter = tx_results
            .iter()
            .flat_map(|r| r.receipt.logs.iter())
            .flat_map(|event| {
                event
                    .topics
                    .iter()
                    .map(|topic| BloomInput::Raw(topic.as_bytes()))
                    .chain([BloomInput::Raw(event.address.as_bytes())])
            });
        let logs_bloom = build_bloom(iter);

        // Calculate how much gas was used across all txs
        let gas_used = tx_results
            .iter()
            .map(|r| r.debug.gas_used)
            .fold(U256::zero(), |acc, x| acc + x);

        // Construct the block
        let parent_block_hash = self
            .blockchain
            .get_block_hash_by_number(L2BlockNumber(block_ctx.miniblock as u32 - 1))
            .await
            .unwrap_or_default();
        let mut blocks = vec![create_block(
            &batch_env,
            block_ctx.hash,
            parent_block_hash,
            block_ctx.miniblock,
            block_ctx.timestamp,
            transactions,
            gas_used,
            logs_bloom,
        )];

        // Hack to ensure we don't mine two empty blocks in the same batch. Otherwise this creates
        // weird side effect on the VM side wrt virtual block logic.
        // TODO: Remove once we separate batch sealing from block sealing
        if !tx_results.is_empty() {
            // With the introduction of 'l2 blocks' (and virtual blocks),
            // we are adding one l2 block at the end of each batch (to handle things like remaining events etc).
            // You can look at insert_fictive_l2_block function in VM to see how this fake block is inserted.
            let parent_block_hash = block_ctx.hash;
            let block_ctx = block_ctx.new_block(&mut self.time);
            let hash = compute_hash(block_ctx.miniblock, []);

            let virtual_block = create_block(
                &batch_env,
                hash,
                parent_block_hash,
                block_ctx.miniblock,
                block_ctx.timestamp,
                vec![],
                U256::zero(),
                Bloom::zero(),
            );
            blocks.push(virtual_block);
        }
        let block_hashes = blocks.iter().map(|b| b.hash).collect::<Vec<_>>();
        self.apply_batch(blocks, tx_results).await;

        let mut filters = self.filters.write().await;
        for block_hash in block_hashes {
            filters.notify_new_block(block_hash);
        }
        drop(filters);

        Ok(L2BlockNumber(block_ctx.miniblock as u32))
    }

    /// Estimates the gas required for a given call request.
    ///
    /// # Arguments
    ///
    /// * `req` - A `CallRequest` struct representing the call request to estimate gas for.
    ///
    /// # Returns
    ///
    /// A `Result` with a `Fee` representing the estimated gas related data.
    pub async fn estimate_gas_impl(&self, req: CallRequest) -> Result<Fee, Web3Error> {
        let mut request_with_gas_per_pubdata_overridden = req;

        if let Some(ref mut eip712_meta) = request_with_gas_per_pubdata_overridden.eip712_meta {
            if eip712_meta.gas_per_pubdata == U256::zero() {
                eip712_meta.gas_per_pubdata =
                    get_max_gas_per_pubdata_byte(VmVersion::latest()).into();
            }
        }

        let is_eip712 = request_with_gas_per_pubdata_overridden
            .eip712_meta
            .is_some();
        let initiator_address = request_with_gas_per_pubdata_overridden
            .from
            .unwrap_or_default();
        let impersonating = self.impersonation.is_impersonating(&initiator_address);
        let system_contracts = self
            .system_contracts
            .contracts_for_fee_estimate(impersonating)
            .clone();

        let mut l2_tx = L2Tx::from_request(
            request_with_gas_per_pubdata_overridden.into(),
            MAX_TX_SIZE,
            self.system_contracts.allow_no_target(),
        )
        .map_err(Web3Error::SerializationError)?;

        let tx: Transaction = l2_tx.clone().into();

        let fee_input = {
            let fee_input = self.fee_input_provider.get_batch_fee_input_scaled();
            // In order for execution to pass smoothly, we need to ensure that block's required gasPerPubdata will be
            // <= to the one in the transaction itself.
            adjust_pubdata_price_for_tx(
                fee_input,
                tx.gas_per_pubdata_byte_limit(),
                None,
                VmVersion::latest(),
            )
        };

        let (base_fee, gas_per_pubdata_byte) =
            derive_base_fee_and_gas_per_pubdata(fee_input, VmVersion::latest());

        // Properly format signature
        if l2_tx.common_data.signature.is_empty() {
            l2_tx.common_data.signature = vec![0u8; 65];
            l2_tx.common_data.signature[64] = 27;
        }

        // The user may not include the proper transaction type during the estimation of
        // the gas fee. However, it is needed for the bootloader checks to pass properly.
        if is_eip712 {
            l2_tx.common_data.transaction_type = TransactionType::EIP712Transaction;
        }

        l2_tx.common_data.fee.gas_per_pubdata_limit =
            get_max_gas_per_pubdata_byte(VmVersion::latest()).into();
        l2_tx.common_data.fee.max_fee_per_gas = base_fee.into();
        l2_tx.common_data.fee.max_priority_fee_per_gas = base_fee.into();

        let execution_mode = TxExecutionMode::EstimateFee;
        let (mut batch_env, _) = self.create_l1_batch_env().await;
        batch_env.fee_input = fee_input;

        let system_env = self.create_system_env(system_contracts, execution_mode);

        // When the pubdata cost grows very high, the total gas limit required may become very high as well. If
        // we do binary search over any possible gas limit naively, we may end up with a very high number of iterations,
        // which affects performance.
        //
        // To optimize for this case, we first calculate the amount of gas needed to cover for the pubdata. After that, we
        // need to do a smaller binary search that is focused on computational gas limit only.
        let additional_gas_for_pubdata = if tx.is_l1() {
            // For L1 transactions the pubdata priced in such a way that the maximal computational
            // gas limit should be enough to cover for the pubdata as well, so no additional gas is provided there.
            0u64
        } else {
            // For L2 transactions, we estimate the amount of gas needed to cover for the pubdata by creating a transaction with infinite gas limit.
            // And getting how much pubdata it used.

            // In theory, if the transaction has failed with such large gas limit, we could have returned an API error here right away,
            // but doing it later on keeps the code more lean.
            let result = self.estimate_gas_step(
                l2_tx.clone(),
                gas_per_pubdata_byte,
                BATCH_GAS_LIMIT,
                batch_env.clone(),
                system_env.clone(),
                &self.fork_storage,
                self.system_contracts.use_zkos,
            );

            if result.statistics.pubdata_published > MAX_VM_PUBDATA_PER_BATCH.try_into().unwrap() {
                return Err(Web3Error::SubmitTransactionError(
                    "exceeds limit for published pubdata".into(),
                    Default::default(),
                ));
            }

            // It is assumed that there is no overflow here
            (result.statistics.pubdata_published as u64) * gas_per_pubdata_byte
        };

        // We are using binary search to find the minimal values of gas_limit under which the transaction succeeds
        let mut lower_bound = 0u64;
        let mut upper_bound = MAX_L2_TX_GAS_LIMIT;
        let mut attempt_count = 1;

        tracing::trace!("Starting gas estimation loop");
        while lower_bound + ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION < upper_bound {
            let mid = (lower_bound + upper_bound) / 2;
            tracing::trace!(
                "Attempt {} (lower_bound: {}, upper_bound: {}, mid: {})",
                attempt_count,
                lower_bound,
                upper_bound,
                mid
            );
            let try_gas_limit = additional_gas_for_pubdata + mid;

            let estimate_gas_result = self.estimate_gas_step(
                l2_tx.clone(),
                gas_per_pubdata_byte,
                try_gas_limit,
                batch_env.clone(),
                system_env.clone(),
                &self.fork_storage,
                self.system_contracts.use_zkos,
            );

            if estimate_gas_result.result.is_failed() {
                tracing::trace!("Attempt {} FAILED", attempt_count);
                lower_bound = mid + 1;
            } else {
                tracing::trace!("Attempt {} SUCCEEDED", attempt_count);
                upper_bound = mid;
            }
            attempt_count += 1;
        }

        tracing::trace!("Gas Estimation Values:");
        tracing::trace!("  Final upper_bound: {}", upper_bound);
        tracing::trace!(
            "  ESTIMATE_GAS_SCALE_FACTOR: {}",
            self.fee_input_provider.estimate_gas_scale_factor
        );
        tracing::trace!("  MAX_L2_TX_GAS_LIMIT: {}", MAX_L2_TX_GAS_LIMIT);
        let tx_body_gas_limit = upper_bound;
        let suggested_gas_limit = ((upper_bound + additional_gas_for_pubdata) as f32
            * self.fee_input_provider.estimate_gas_scale_factor)
            as u64;

        let estimate_gas_result = self.estimate_gas_step(
            l2_tx.clone(),
            gas_per_pubdata_byte,
            suggested_gas_limit,
            batch_env,
            system_env,
            &self.fork_storage,
            self.system_contracts.use_zkos,
        );

        let overhead = derive_overhead(
            suggested_gas_limit,
            gas_per_pubdata_byte as u32,
            tx.encoding_len(),
            l2_tx.common_data.transaction_type as u8,
            VmVersion::latest(),
        ) as u64;

        match estimate_gas_result.result {
            ExecutionResult::Revert { output } => {
                sh_eprintln!("{}", format!("Unable to estimate gas for the request with our suggested gas limit of {}. The transaction is most likely unexecutable. Breakdown of estimation:", suggested_gas_limit + overhead).red());
                sh_eprintln!(
                    "{}",
                    format!(
                        "\tEstimated transaction body gas cost: {}",
                        tx_body_gas_limit
                    )
                    .red()
                );
                sh_eprintln!(
                    "{}",
                    format!("\tGas for pubdata: {}", additional_gas_for_pubdata).red()
                );
                sh_eprintln!("{}", format!("\tOverhead: {}", overhead).red());
                let message = output.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );
                let data = output.encoded_data();
                sh_eprintln!("{}", pretty_message.on_red());
                Err(Web3Error::SubmitTransactionError(pretty_message, data))
            }
            ExecutionResult::Halt { reason } => {
                sh_eprintln!("{}", format!("Unable to estimate gas for the request with our suggested gas limit of {}. The transaction is most likely unexecutable. Breakdown of estimation:", suggested_gas_limit + overhead).red());
                sh_eprintln!(
                    "{}",
                    format!(
                        "\tEstimated transaction body gas cost: {}",
                        tx_body_gas_limit
                    )
                    .red()
                );
                sh_eprintln!(
                    "{}",
                    format!("\tGas for pubdata: {}", additional_gas_for_pubdata).red()
                );
                sh_eprintln!("{}", format!("\tOverhead: {}", overhead).red());
                let message = reason.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );

                sh_eprintln!("{}", pretty_message.on_red());
                Err(Web3Error::SubmitTransactionError(pretty_message, vec![]))
            }
            ExecutionResult::Success { .. } => {
                let full_gas_limit = match suggested_gas_limit.overflowing_add(overhead) {
                    (value, false) => value,
                    (_, true) => {
                        sh_println!("{}", "Overflow when calculating gas estimation. We've exceeded the block gas limit by summing the following values:".red());
                        sh_println!(
                            "{}",
                            format!(
                                "\tEstimated transaction body gas cost: {}",
                                tx_body_gas_limit
                            )
                            .red()
                        );
                        sh_println!(
                            "{}",
                            format!("\tGas for pubdata: {}", additional_gas_for_pubdata).red()
                        );
                        sh_println!("{}", format!("\tOverhead: {}", overhead).red());
                        return Err(Web3Error::SubmitTransactionError(
                            "exceeds block gas limit".into(),
                            Default::default(),
                        ));
                    }
                };

                tracing::trace!("Gas Estimation Results");
                tracing::trace!("  tx_body_gas_limit: {}", tx_body_gas_limit);
                tracing::trace!(
                    "  additional_gas_for_pubdata: {}",
                    additional_gas_for_pubdata
                );
                tracing::trace!("  overhead: {}", overhead);
                tracing::trace!("  full_gas_limit: {}", full_gas_limit);
                let fee = Fee {
                    max_fee_per_gas: base_fee.into(),
                    max_priority_fee_per_gas: 0u32.into(),
                    gas_limit: full_gas_limit.into(),
                    gas_per_pubdata_limit: gas_per_pubdata_byte.into(),
                };
                Ok(fee)
            }
        }
    }

    /// Runs fee estimation against a sandbox vm with the given gas_limit.
    #[allow(clippy::too_many_arguments)]
    fn estimate_gas_step(
        &self,
        mut l2_tx: L2Tx,
        gas_per_pubdata_byte: u64,
        tx_gas_limit: u64,
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
        fork_storage: &ForkStorage,
        is_zkos: bool,
    ) -> VmExecutionResultAndLogs {
        let tx: Transaction = l2_tx.clone().into();

        // Set gas_limit for transaction
        let gas_limit_with_overhead = tx_gas_limit
            + derive_overhead(
                tx_gas_limit,
                gas_per_pubdata_byte as u32,
                tx.encoding_len(),
                l2_tx.common_data.transaction_type as u8,
                VmVersion::latest(),
            ) as u64;
        l2_tx.common_data.fee.gas_limit = gas_limit_with_overhead.into();

        let storage = StorageView::new(fork_storage).into_rc_ptr();

        // The nonce needs to be updated
        let nonce = l2_tx.nonce();
        let nonce_key = self
            .storage_key_layout
            .get_nonce_key(&l2_tx.initiator_account());
        let full_nonce = storage.borrow_mut().read_value(&nonce_key);
        let (_, deployment_nonce) = decompose_full_nonce(h256_to_u256(full_nonce));
        let enforced_full_nonce = nonces_to_full_nonce(U256::from(nonce.0), deployment_nonce);
        storage
            .borrow_mut()
            .set_value(nonce_key, u256_to_h256(enforced_full_nonce));

        // We need to explicitly put enough balance into the account of the users
        let payer = l2_tx.payer();
        let balance_key = self
            .storage_key_layout
            .get_storage_key_for_base_token(&payer);
        let mut current_balance = h256_to_u256(storage.borrow_mut().read_value(&balance_key));
        let added_balance = l2_tx.common_data.fee.gas_limit * l2_tx.common_data.fee.max_fee_per_gas;
        current_balance += added_balance;
        storage
            .borrow_mut()
            .set_value(balance_key, u256_to_h256(current_balance));

        let mut vm = if is_zkos {
            let mut vm = ZKOsVM::<_, HistoryDisabled>::new(
                batch_env,
                system_env,
                storage,
                // TODO: this might be causing a deadlock.. check..
                &fork_storage.inner.read().unwrap().raw_storage,
            );
            // Temporary hack - as we update the 'storage' just above, but zkos loads its full
            // state from fork_storage (that is not updated).
            vm.update_inconsistent_keys(&[&nonce_key, &balance_key]);
            AnvilVM::ZKOs(vm)
        } else {
            AnvilVM::ZKSync(Vm::new(batch_env, system_env, storage))
        };

        let tx: Transaction = l2_tx.into();
        delegate_vm!(vm, push_transaction(tx));

        delegate_vm!(vm, execute(InspectExecutionMode::OneTx))
    }

    /// Creates a [Snapshot] of the current state of the node.
    pub async fn snapshot(&self) -> Result<Snapshot, String> {
        let blockchain = self.blockchain.read().await;
        let filters = self.filters.read().await.clone();
        let storage = self
            .fork_storage
            .inner
            .read()
            .map_err(|err| format!("failed acquiring read lock on storage: {:?}", err))?;

        Ok(Snapshot {
            current_batch: blockchain.current_batch,
            current_block: blockchain.current_block,
            current_block_hash: blockchain.current_block_hash,
            fee_input_provider: self.fee_input_provider.clone(),
            tx_results: blockchain.tx_results.clone(),
            blocks: blockchain.blocks.clone(),
            hashes: blockchain.hashes.clone(),
            filters,
            impersonation_state: self.impersonation.state(),
            rich_accounts: self.rich_accounts.clone(),
            previous_states: self.previous_states.clone(),
            raw_storage: storage.raw_storage.clone(),
            value_read_cache: storage.value_read_cache.clone(),
            factory_dep_cache: storage.factory_dep_cache.clone(),
        })
    }

    /// Restores a previously created [Snapshot] of the node.
    pub async fn restore_snapshot(&mut self, snapshot: Snapshot) -> Result<(), String> {
        let mut blockchain = self.blockchain.write().await;
        let mut storage = self
            .fork_storage
            .inner
            .write()
            .map_err(|err| format!("failed acquiring write lock on storage: {:?}", err))?;

        blockchain.current_batch = snapshot.current_batch;
        blockchain.current_block = snapshot.current_block;
        blockchain.current_block_hash = snapshot.current_block_hash;
        self.fee_input_provider = snapshot.fee_input_provider;
        blockchain.tx_results = snapshot.tx_results;
        blockchain.blocks = snapshot.blocks;
        blockchain.hashes = snapshot.hashes;
        // FIXME: This logic is incorrect but it doesn't matter as filters should not be a part of
        //        snapshots anyway
        self.filters = Arc::new(RwLock::new(snapshot.filters));
        self.impersonation.set_state(snapshot.impersonation_state);
        self.rich_accounts = snapshot.rich_accounts;
        self.previous_states = snapshot.previous_states;
        storage.raw_storage = snapshot.raw_storage;
        storage.value_read_cache = snapshot.value_read_cache;
        storage.factory_dep_cache = snapshot.factory_dep_cache;

        Ok(())
    }

    pub async fn dump_state(
        &self,
        preserve_historical_states: bool,
    ) -> anyhow::Result<VersionedState> {
        let blockchain = self.blockchain.read().await;
        let blocks = blockchain.blocks.values().cloned().collect();
        let transactions = blockchain.tx_results.values().cloned().collect();
        drop(blockchain);
        let fork_storage = self.fork_storage.dump_state();
        let historical_states = if preserve_historical_states {
            self.previous_states
                .iter()
                .map(|(k, v)| (*k, SerializableStorage(v.clone().into_iter().collect())))
                .collect()
        } else {
            Vec::new()
        };

        Ok(VersionedState::v1(StateV1 {
            blocks,
            transactions,
            fork_storage,
            historical_states,
        }))
    }

    pub async fn load_state(&mut self, state: VersionedState) -> Result<bool, LoadStateError> {
        let mut storage = self.blockchain.write().await;
        if storage.blocks.len() > 1 {
            tracing::debug!(
                blocks = storage.blocks.len(),
                "node has existing state; refusing to load new state"
            );
            return Err(LoadStateError::HasExistingState);
        }
        let state = match state {
            VersionedState::V1 { state, .. } => state,
            VersionedState::Unknown { version } => {
                return Err(LoadStateError::UnknownStateVersion(version))
            }
        };
        if state.blocks.is_empty() {
            tracing::debug!("new state has no blocks; refusing to load");
            return Err(LoadStateError::EmptyState);
        }

        storage.load_blocks(&mut self.time, state.blocks);
        storage.load_transactions(state.transactions);
        self.fork_storage.load_state(state.fork_storage);

        tracing::trace!(
            states = state.historical_states.len(),
            "loading historical states from supplied state"
        );
        self.previous_states.extend(
            state
                .historical_states
                .into_iter()
                .map(|(k, v)| (k, v.0.into_iter().collect())),
        );

        Ok(true)
    }

    pub async fn get_storage_at_block(
        &self,
        address: Address,
        idx: U256,
        block: Option<api::BlockIdVariant>,
    ) -> Result<H256, Web3Error> {
        let storage_key = StorageKey::new(AccountTreeId::new(address), u256_to_h256(idx));
        let storage = self.blockchain.read().await;

        let block_number = block
            .map(|block| match block {
                BlockIdVariant::BlockNumber(block_number) => Ok(utils::to_real_block_number(
                    block_number,
                    U64::from(storage.current_block.0),
                )),
                BlockIdVariant::BlockNumberObject(o) => Ok(utils::to_real_block_number(
                    o.block_number,
                    U64::from(storage.current_block.0),
                )),
                BlockIdVariant::BlockHashObject(o) => storage
                    .blocks
                    .get(&o.block_hash)
                    .map(|block| block.number)
                    .ok_or_else(|| {
                        sh_err!("unable to map block number to hash #{:#x}", o.block_hash);
                        Web3Error::InternalError(anyhow::Error::msg(
                            "Failed to map block number to hash.",
                        ))
                    }),
            })
            .unwrap_or_else(|| Ok(U64::from(storage.current_block.0)))?;
        // FIXME: Conversion mess above
        let block_number = L2BlockNumber(block_number.as_u32());

        if block_number == storage.current_block {
            match self.fork_storage.read_value_internal(&storage_key) {
                Ok(value) => Ok(H256(value.0)),
                Err(error) => Err(Web3Error::InternalError(anyhow::anyhow!(
                    "failed to read storage: {}",
                    error
                ))),
            }
        } else if storage.hashes.contains_key(&block_number) {
            let value = storage
                .hashes
                .get(&block_number)
                .and_then(|block_hash| self.previous_states.get(block_hash))
                .and_then(|state| state.get(&storage_key))
                .cloned()
                .unwrap_or_default();
            if !value.is_zero() {
                return Ok(value);
            }
            // TODO: Check if the rest of the logic below makes sense.
            //       AFAIU this branch can only be entered if the block was produced locally, but
            //       we query the fork regardless?
            match self.fork_storage.read_value_internal(&storage_key) {
                Ok(value) => Ok(H256(value.0)),
                Err(error) => Err(Web3Error::InternalError(anyhow::anyhow!(
                    "failed to read storage: {}",
                    error
                ))),
            }
        } else {
            Ok(self.fork.get_storage_at(address, idx, block).await?)
        }
    }

    pub async fn reset(&mut self, fork_client_opt: Option<ForkClient>) {
        let fork_details = fork_client_opt.as_ref().map(|client| &client.details);
        let blockchain = Blockchain::new(
            fork_details,
            self.config.genesis.as_ref(),
            self.config.genesis_timestamp,
        );
        let blockchain_storage = blockchain.read().await.clone();
        drop(std::mem::replace(
            &mut *self.blockchain.write().await,
            blockchain_storage,
        ));

        self.time.set_current_timestamp_unchecked(
            fork_details
                .map(|fd| fd.block_timestamp)
                .unwrap_or(NON_FORK_FIRST_BLOCK_TIMESTAMP),
        );

        drop(std::mem::take(&mut *self.filters.write().await));

        self.fork.reset_fork_client(fork_client_opt);
        let fork_storage = ForkStorage::new(
            self.fork.clone(),
            &self.config.system_contracts_options,
            self.config.use_evm_emulator,
            self.config.chain_id,
        );
        let mut old_storage = self.fork_storage.inner.write().unwrap();
        let mut new_storage = fork_storage.inner.write().unwrap();
        old_storage.raw_storage = std::mem::take(&mut new_storage.raw_storage);
        old_storage.value_read_cache = std::mem::take(&mut new_storage.value_read_cache);
        old_storage.factory_dep_cache = std::mem::take(&mut new_storage.factory_dep_cache);
        self.fork_storage.chain_id = fork_storage.chain_id;
        drop(old_storage);
        drop(new_storage);

        self.rich_accounts.clear();
        self.previous_states.clear();

        let rich_addresses = itertools::chain!(
            self.config
                .genesis_accounts
                .iter()
                .map(|acc| H160::from_slice(acc.address().as_ref())),
            self.config
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
            self.set_rich_account(address, self.config.genesis_balance);
        }
    }

    /// Adds a lot of tokens to a given account with a specified balance.
    pub fn set_rich_account(&mut self, address: H160, balance: U256) {
        let key = self
            .storage_key_layout
            .get_storage_key_for_base_token(&address);

        let keys = {
            let mut storage_view = StorageView::new(&self.fork_storage);
            // Set balance to the specified amount
            storage_view.set_value(key, u256_to_h256(balance));
            storage_view.modified_storage_keys().clone()
        };

        for (key, value) in keys.iter() {
            self.fork_storage.set_value(*key, *value);
        }
        self.rich_accounts.insert(address);
    }

    pub fn read_storage(&self) -> Box<dyn ReadStorage + '_> {
        Box::new(&self.fork_storage)
    }

    // TODO: Remove, this should also be made available from somewhere else
    pub fn chain_id(&self) -> L2ChainId {
        self.fork_storage.chain_id
    }
}

#[derive(Debug)]
pub struct TxExecutionOutput {
    result: VmExecutionResultAndLogs,
    call_traces: Vec<Call>,
    bytecodes: HashMap<H256, Vec<u8>>,
}

/// Keeps track of a block's batch number, miniblock number and timestamp.
/// Useful for keeping track of the current context when creating multiple blocks.
#[derive(Debug, Clone, Default)]
pub struct BlockContext {
    pub hash: H256,
    pub batch: u32,
    pub miniblock: u64,
    pub timestamp: u64,
}

impl BlockContext {
    /// Create the next batch instance that uses the same batch number, and has all other parameters incremented by `1`.
    fn new_block(&self, time: &mut Time) -> BlockContext {
        Self {
            hash: H256::zero(),
            batch: self.batch,
            miniblock: self.miniblock.saturating_add(1),
            timestamp: time.advance_timestamp(),
        }
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

// Test utils
#[cfg(test)]
impl InMemoryNodeInner {
    pub fn test_config(config: TestNodeConfig) -> Arc<RwLock<Self>> {
        let fee_provider = TestNodeFeeInputProvider::default();
        let impersonation = ImpersonationManager::default();
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
        let (inner, _, _, _, _) = InMemoryNodeInner::init(
            None,
            fee_provider,
            Arc::new(RwLock::new(Default::default())),
            config,
            impersonation.clone(),
            system_contracts.clone(),
            storage_key_layout,
        );
        inner
    }

    pub fn test() -> Arc<RwLock<Self>> {
        Self::test_config(TestNodeConfig::default())
    }

    /// Deploys a contract with the given bytecode.
    pub async fn deploy_contract(
        &mut self,
        tx_hash: H256,
        private_key: &zksync_types::K256PrivateKey,
        bytecode: Vec<u8>,
        calldata: Option<Vec<u8>>,
        nonce: zksync_types::Nonce,
    ) -> H256 {
        use alloy::dyn_abi::{DynSolValue, JsonAbiExt};
        use alloy::json_abi::{Function, Param, StateMutability};

        let salt = [0u8; 32];
        let bytecode_hash = BytecodeHash::for_bytecode(&bytecode).value().0;
        let call_data = calldata.unwrap_or_default();

        let create = Function {
            name: "create".to_string(),
            inputs: vec![
                Param {
                    name: "_salt".to_string(),
                    ty: "bytes32".to_string(),
                    components: vec![],
                    internal_type: None,
                },
                Param {
                    name: "_bytecodeHash".to_string(),
                    ty: "bytes32".to_string(),
                    components: vec![],
                    internal_type: None,
                },
                Param {
                    name: "_input".to_string(),
                    ty: "bytes".to_string(),
                    components: vec![],
                    internal_type: None,
                },
            ],
            outputs: vec![Param {
                name: "".to_string(),
                ty: "address".to_string(),
                components: vec![],
                internal_type: None,
            }],
            state_mutability: StateMutability::Payable,
        };

        let data = create
            .abi_encode_input(&[
                DynSolValue::FixedBytes(salt.into(), salt.len()),
                DynSolValue::FixedBytes(
                    bytecode_hash[..].try_into().expect("invalid hash length"),
                    bytecode_hash.len(),
                ),
                DynSolValue::Bytes(call_data),
            ])
            .expect("failed to encode function data");

        let mut tx = L2Tx::new_signed(
            Some(zksync_types::CONTRACT_DEPLOYER_ADDRESS),
            data.to_vec(),
            nonce,
            Fee {
                gas_limit: U256::from(400_000_000),
                max_fee_per_gas: U256::from(50_000_000),
                max_priority_fee_per_gas: U256::from(50_000_000),
                gas_per_pubdata_limit: U256::from(50000),
            },
            U256::from(0),
            zksync_types::L2ChainId::from(260),
            private_key,
            vec![bytecode],
            Default::default(),
        )
        .expect("failed signing tx");
        tx.set_input(vec![], tx_hash);

        let system_contracts = self
            .system_contracts
            .system_contracts_for_initiator(&self.impersonation, &tx.initiator_account());
        let block_number = self
            .seal_block(vec![tx], system_contracts)
            .await
            .expect("failed deploying contract");

        self.blockchain
            .read()
            .await
            .get_block_hash_by_number(block_number)
            .unwrap()
    }

    // TODO: Return L2BlockNumber
    /// Applies a transaction with a given hash to the node and returns the block hash.
    pub async fn apply_tx(&mut self, tx_hash: H256) -> (H256, U64, L2Tx) {
        let tx = crate::testing::TransactionBuilder::new()
            .set_hash(tx_hash)
            .build();

        self.set_rich_account(
            tx.common_data.initiator_address,
            U256::from(100u128 * 10u128.pow(18)),
        );
        let system_contracts = self
            .system_contracts
            .system_contracts_for_initiator(&self.impersonation, &tx.initiator_account());
        let block_number = self
            .seal_block(vec![tx.clone()], system_contracts)
            .await
            .expect("failed applying tx");

        let block_hash = self
            .blockchain
            .read()
            .await
            .get_block_hash_by_number(block_number)
            .unwrap();

        (block_hash, U64::from(block_number.0), tx)
    }

    pub async fn insert_block(&mut self, hash: H256, block: api::Block<TransactionVariant>) {
        self.blockchain.write().await.blocks.insert(hash, block);
    }

    pub async fn insert_block_hash(&mut self, number: L2BlockNumber, hash: H256) {
        self.blockchain.write().await.hashes.insert(number, hash);
    }

    pub async fn insert_tx_result(&mut self, hash: H256, tx_result: TransactionResult) {
        self.blockchain
            .write()
            .await
            .tx_results
            .insert(hash, tx_result);
    }

    pub fn insert_previous_state(&mut self, hash: H256, state: HashMap<StorageKey, StorageValue>) {
        self.previous_states.insert(hash, state);
    }

    pub fn get_previous_state(&self, hash: H256) -> Option<HashMap<StorageKey, StorageValue>> {
        self.previous_states.get(&hash).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::create_genesis;
    use crate::node::fork::ForkDetails;
    use crate::node::inner::fork_storage::ForkStorage;
    use crate::testing;
    use crate::testing::{TransactionBuilder, STORAGE_CONTRACT_BYTECODE};
    use alloy::dyn_abi::{DynSolType, DynSolValue};
    use alloy::primitives::U256 as AlloyU256;
    use anvil_zksync_config::constants::{
        DEFAULT_ACCOUNT_BALANCE, DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
        DEFAULT_ESTIMATE_GAS_SCALE_FACTOR, DEFAULT_FAIR_PUBDATA_PRICE, DEFAULT_L2_GAS_PRICE,
        TEST_NODE_NETWORK_ID,
    };
    use anvil_zksync_config::types::SystemContractsOptions;
    use anvil_zksync_config::TestNodeConfig;
    use itertools::Itertools;
    use zksync_types::{utils::deployed_address_create, K256PrivateKey, Nonce};

    async fn test_vm(
        node: &mut InMemoryNodeInner,
        system_contracts: BaseSystemContracts,
    ) -> (
        BlockContext,
        L1BatchEnv,
        Vm<StorageView<ForkStorage>, HistoryDisabled>,
    ) {
        let storage = StorageView::new(node.fork_storage.clone()).into_rc_ptr();
        let system_env = node.create_system_env(system_contracts, TxExecutionMode::VerifyExecute);
        let (batch_env, block_ctx) = node.create_l1_batch_env().await;
        let vm: Vm<_, HistoryDisabled> = Vm::new(batch_env.clone(), system_env, storage);

        (block_ctx, batch_env, vm)
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
        let inner = InMemoryNodeInner::test();
        let mut node = inner.write().await;
        let tx = TransactionBuilder::new()
            .set_gas_limit(U256::from(u64::MAX) + 1)
            .build();
        node.set_rich_account(tx.initiator_account(), U256::from(100u128 * 10u128.pow(18)));

        let system_contracts = node
            .system_contracts
            .system_contracts_for_initiator(&node.impersonation, &tx.initiator_account());
        let (block_ctx, batch_env, mut vm) = test_vm(&mut node, system_contracts).await;
        let err = node
            .run_l2_tx(tx, 0, &block_ctx, &batch_env, &mut vm)
            .unwrap_err();
        assert_eq!(err.to_string(), "exceeds block gas limit");
    }

    #[tokio::test]
    async fn test_run_l2_tx_validates_tx_max_fee_per_gas_too_low() {
        let inner = InMemoryNodeInner::test();
        let mut node = inner.write().await;
        let tx = TransactionBuilder::new()
            .set_max_fee_per_gas(U256::from(DEFAULT_L2_GAS_PRICE - 1))
            .build();
        node.set_rich_account(tx.initiator_account(), U256::from(100u128 * 10u128.pow(18)));

        let system_contracts = node
            .system_contracts
            .system_contracts_for_initiator(&node.impersonation, &tx.initiator_account());
        let (block_ctx, batch_env, mut vm) = test_vm(&mut node, system_contracts).await;
        let err = node
            .run_l2_tx(tx, 0, &block_ctx, &batch_env, &mut vm)
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "block base fee higher than max fee per gas"
        );
    }

    #[tokio::test]
    async fn test_run_l2_tx_validates_tx_max_priority_fee_per_gas_higher_than_max_fee_per_gas() {
        let inner = InMemoryNodeInner::test();
        let mut node = inner.write().await;
        let tx = TransactionBuilder::new()
            .set_max_priority_fee_per_gas(U256::from(250_000_000 + 1))
            .build();
        node.set_rich_account(tx.initiator_account(), U256::from(100u128 * 10u128.pow(18)));

        let system_contracts = node
            .system_contracts
            .system_contracts_for_initiator(&node.impersonation, &tx.initiator_account());
        let (block_ctx, batch_env, mut vm) = test_vm(&mut node, system_contracts).await;
        let err = node
            .run_l2_tx(tx, 0, &block_ctx, &batch_env, &mut vm)
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "max priority fee per gas higher than max fee per gas"
        );
    }

    #[tokio::test]
    async fn test_create_genesis_creates_block_with_hash_and_zero_parent_hash() {
        let first_block = create_genesis::<TransactionVariant>(Some(1000));

        assert_eq!(first_block.hash, compute_hash(0, []));
        assert_eq!(first_block.parent_hash, H256::zero());
    }

    #[tokio::test]
    async fn test_run_l2_tx_raw_does_not_panic_on_mock_fork_client_call() {
        // Perform a transaction to get storage to an intermediate state
        let inner = InMemoryNodeInner::test();
        let mut node = inner.write().await;
        let tx = TransactionBuilder::new().build();
        node.set_rich_account(tx.initiator_account(), U256::from(100u128 * 10u128.pow(18)));

        let system_contracts = node
            .system_contracts
            .system_contracts_for_initiator(&node.impersonation, &tx.initiator_account());
        node.seal_block(vec![tx], system_contracts).await.unwrap();

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
            node.fork_storage.inner.read().unwrap().raw_storage.clone(),
        );
        let impersonation = ImpersonationManager::default();
        let (node, _, _, _, _) = InMemoryNodeInner::init(
            Some(mock_fork_client),
            TestNodeFeeInputProvider::default(),
            Arc::new(RwLock::new(Default::default())),
            TestNodeConfig::default(),
            impersonation,
            node.system_contracts.clone(),
            node.storage_key_layout,
        );
        let mut node = node.write().await;

        let tx = TransactionBuilder::new().build();

        let system_contracts = node
            .system_contracts
            .system_contracts_for_initiator(&node.impersonation, &tx.initiator_account());
        let (_, _, mut vm) = test_vm(&mut node, system_contracts).await;
        node.run_l2_tx_raw(tx, &mut vm)
            .expect("transaction must pass with mock fork client");
    }

    #[tokio::test]
    async fn test_transact_returns_data_in_built_in_without_security_mode() {
        let inner = InMemoryNodeInner::test_config(TestNodeConfig {
            system_contracts_options: SystemContractsOptions::BuiltInWithoutSecurity,
            ..Default::default()
        });
        let mut node = inner.write().await;

        let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xef)).unwrap();
        let from_account = private_key.address();
        node.set_rich_account(from_account, U256::from(DEFAULT_ACCOUNT_BALANCE));

        let deployed_address = deployed_address_create(from_account, U256::zero());
        node.deploy_contract(
            H256::repeat_byte(0x1),
            &private_key,
            hex::decode(STORAGE_CONTRACT_BYTECODE).unwrap(),
            None,
            Nonce(0),
        )
        .await;

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

        let system_contracts = node
            .system_contracts
            .system_contracts_for_initiator(&node.impersonation, &tx.initiator_account());
        let (_, _, mut vm) = test_vm(&mut node, system_contracts).await;
        let TxExecutionOutput { result, .. } = node.run_l2_tx_raw(tx, &mut vm).expect("failed tx");

        match result.result {
            ExecutionResult::Success { output } => {
                let actual = decode_tx_result(&output, DynSolType::Uint(256));
                let expected = DynSolValue::Uint(AlloyU256::from(1024), 256);
                assert_eq!(expected, actual, "invalid result");
            }
            _ => panic!("invalid result {:?}", result.result),
        }
    }

    #[tokio::test]
    async fn test_snapshot() {
        let node = InMemoryNodeInner::test();
        let mut writer = node.write().await;

        {
            let mut blockchain = writer.blockchain.write().await;
            blockchain
                .blocks
                .insert(H256::repeat_byte(0x1), Default::default());
            blockchain
                .hashes
                .insert(L2BlockNumber(1), H256::repeat_byte(0x1));
            blockchain.tx_results.insert(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: Default::default(),
                    debug: testing::default_tx_debug_info(),
                },
            );
            blockchain.current_batch = L1BatchNumber(1);
            blockchain.current_block = L2BlockNumber(1);
            blockchain.current_block_hash = H256::repeat_byte(0x1);
        }
        writer.time.set_current_timestamp_unchecked(1);
        writer
            .filters
            .write()
            .await
            .add_block_filter()
            .expect("failed adding block filter");
        writer.impersonation.impersonate(H160::repeat_byte(0x1));
        writer.rich_accounts.insert(H160::repeat_byte(0x1));
        writer
            .previous_states
            .insert(H256::repeat_byte(0x1), Default::default());
        writer.fork_storage.set_value(
            StorageKey::new(AccountTreeId::new(H160::repeat_byte(0x1)), H256::zero()),
            H256::repeat_byte(0x1),
        );

        let storage = writer.fork_storage.inner.read().unwrap();
        let blockchain = writer.blockchain.read().await;
        let expected_snapshot = Snapshot {
            current_batch: blockchain.current_batch,
            current_block: blockchain.current_block,
            current_block_hash: blockchain.current_block_hash,
            fee_input_provider: writer.fee_input_provider.clone(),
            tx_results: blockchain.tx_results.clone(),
            blocks: blockchain.blocks.clone(),
            hashes: blockchain.hashes.clone(),
            filters: writer.filters.read().await.clone(),
            impersonation_state: writer.impersonation.state(),
            rich_accounts: writer.rich_accounts.clone(),
            previous_states: writer.previous_states.clone(),
            raw_storage: storage.raw_storage.clone(),
            value_read_cache: storage.value_read_cache.clone(),
            factory_dep_cache: storage.factory_dep_cache.clone(),
        };
        drop(blockchain);
        let actual_snapshot = writer.snapshot().await.expect("failed taking snapshot");

        assert_eq!(
            expected_snapshot.current_batch,
            actual_snapshot.current_batch
        );
        assert_eq!(
            expected_snapshot.current_block,
            actual_snapshot.current_block
        );
        assert_eq!(
            expected_snapshot.current_block_hash,
            actual_snapshot.current_block_hash
        );
        assert_eq!(
            expected_snapshot.fee_input_provider,
            actual_snapshot.fee_input_provider
        );
        assert_eq!(
            expected_snapshot.tx_results.keys().collect_vec(),
            actual_snapshot.tx_results.keys().collect_vec()
        );
        assert_eq!(expected_snapshot.blocks, actual_snapshot.blocks);
        assert_eq!(expected_snapshot.hashes, actual_snapshot.hashes);
        assert_eq!(expected_snapshot.filters, actual_snapshot.filters);
        assert_eq!(
            expected_snapshot.impersonation_state,
            actual_snapshot.impersonation_state
        );
        assert_eq!(
            expected_snapshot.rich_accounts,
            actual_snapshot.rich_accounts
        );
        assert_eq!(
            expected_snapshot.previous_states,
            actual_snapshot.previous_states
        );
        assert_eq!(expected_snapshot.raw_storage, actual_snapshot.raw_storage);
        assert_eq!(
            expected_snapshot.value_read_cache,
            actual_snapshot.value_read_cache
        );
        assert_eq!(
            expected_snapshot.factory_dep_cache,
            actual_snapshot.factory_dep_cache
        );
    }

    #[tokio::test]
    async fn test_snapshot_restore() {
        let node = InMemoryNodeInner::test();
        let mut writer = node.write().await;

        {
            let mut blockchain = writer.blockchain.write().await;
            blockchain
                .blocks
                .insert(H256::repeat_byte(0x1), Default::default());
            blockchain
                .hashes
                .insert(L2BlockNumber(1), H256::repeat_byte(0x1));
            blockchain.tx_results.insert(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: Default::default(),
                    debug: testing::default_tx_debug_info(),
                },
            );
            blockchain.current_batch = L1BatchNumber(1);
            blockchain.current_block = L2BlockNumber(1);
            blockchain.current_block_hash = H256::repeat_byte(0x1);
        }
        writer.time.set_current_timestamp_unchecked(1);
        writer
            .filters
            .write()
            .await
            .add_block_filter()
            .expect("failed adding block filter");
        writer.impersonation.impersonate(H160::repeat_byte(0x1));
        writer.rich_accounts.insert(H160::repeat_byte(0x1));
        writer
            .previous_states
            .insert(H256::repeat_byte(0x1), Default::default());
        writer.fork_storage.set_value(
            StorageKey::new(AccountTreeId::new(H160::repeat_byte(0x1)), H256::zero()),
            H256::repeat_byte(0x1),
        );

        let blockchain = writer.blockchain.read().await;
        let expected_snapshot = {
            let storage = writer.fork_storage.inner.read().unwrap();
            Snapshot {
                current_batch: blockchain.current_batch,
                current_block: blockchain.current_block,
                current_block_hash: blockchain.current_block_hash,
                fee_input_provider: writer.fee_input_provider.clone(),
                tx_results: blockchain.tx_results.clone(),
                blocks: blockchain.blocks.clone(),
                hashes: blockchain.hashes.clone(),
                filters: writer.filters.read().await.clone(),
                impersonation_state: writer.impersonation.state(),
                rich_accounts: writer.rich_accounts.clone(),
                previous_states: writer.previous_states.clone(),
                raw_storage: storage.raw_storage.clone(),
                value_read_cache: storage.value_read_cache.clone(),
                factory_dep_cache: storage.factory_dep_cache.clone(),
            }
        };
        drop(blockchain);

        // snapshot and modify node state
        let snapshot = writer.snapshot().await.expect("failed taking snapshot");

        {
            let mut blockchain = writer.blockchain.write().await;
            blockchain
                .blocks
                .insert(H256::repeat_byte(0x2), Default::default());
            blockchain
                .hashes
                .insert(L2BlockNumber(2), H256::repeat_byte(0x2));
            blockchain.tx_results.insert(
                H256::repeat_byte(0x2),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: Default::default(),
                    debug: testing::default_tx_debug_info(),
                },
            );
            blockchain.current_batch = L1BatchNumber(2);
            blockchain.current_block = L2BlockNumber(2);
            blockchain.current_block_hash = H256::repeat_byte(0x2);
        }
        writer.time.set_current_timestamp_unchecked(2);
        writer
            .filters
            .write()
            .await
            .add_pending_transaction_filter()
            .expect("failed adding pending transaction filter");
        writer.impersonation.impersonate(H160::repeat_byte(0x2));
        writer.rich_accounts.insert(H160::repeat_byte(0x2));
        writer
            .previous_states
            .insert(H256::repeat_byte(0x2), Default::default());
        writer.fork_storage.set_value(
            StorageKey::new(AccountTreeId::new(H160::repeat_byte(0x2)), H256::zero()),
            H256::repeat_byte(0x2),
        );

        // restore
        writer
            .restore_snapshot(snapshot)
            .await
            .expect("failed restoring snapshot");

        let storage = writer.fork_storage.inner.read().unwrap();
        let blockchain = writer.blockchain.read().await;
        assert_eq!(expected_snapshot.current_batch, blockchain.current_batch);
        assert_eq!(expected_snapshot.current_block, blockchain.current_block);
        assert_eq!(
            expected_snapshot.current_block_hash,
            blockchain.current_block_hash
        );

        assert_eq!(
            expected_snapshot.fee_input_provider,
            writer.fee_input_provider
        );
        assert_eq!(
            expected_snapshot.tx_results.keys().collect_vec(),
            blockchain.tx_results.keys().collect_vec()
        );
        assert_eq!(expected_snapshot.blocks, blockchain.blocks);
        assert_eq!(expected_snapshot.hashes, blockchain.hashes);
        assert_eq!(expected_snapshot.filters, *writer.filters.read().await);
        assert_eq!(
            expected_snapshot.impersonation_state,
            writer.impersonation.state()
        );
        assert_eq!(expected_snapshot.rich_accounts, writer.rich_accounts);
        assert_eq!(expected_snapshot.previous_states, writer.previous_states);
        assert_eq!(expected_snapshot.raw_storage, storage.raw_storage);
        assert_eq!(expected_snapshot.value_read_cache, storage.value_read_cache);
        assert_eq!(
            expected_snapshot.factory_dep_cache,
            storage.factory_dep_cache
        );
    }
}
