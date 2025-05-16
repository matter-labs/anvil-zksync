use crate::filters::EthFilters;
use crate::formatter::ExecutionErrorReport;
use crate::node::boojumos::BoojumOsVM;
use crate::node::diagnostics::transaction::known_addresses_after_transaction;
use crate::node::diagnostics::vm::traces::extract_addresses;
use crate::node::error::{LoadStateError, ToHaltError, ToRevertReason};
use crate::node::inner::blockchain::Blockchain;
use crate::node::inner::fork::{Fork, ForkClient, ForkSource};
use crate::node::inner::fork_storage::{ForkStorage, SerializableStorage};
use crate::node::inner::time::Time;
use crate::node::inner::vm_runner::TxBatchExecutionResult;
use crate::node::keys::StorageKeyLayout;
use crate::node::state::StateV1;
use crate::node::traces::decoder::CallTraceDecoderBuilder;
use crate::node::vm::AnvilVM;
use crate::node::{
    create_block, ImpersonationManager, Snapshot, TestNodeFeeInputProvider, TransactionResult,
    VersionedState, ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION, MAX_PREVIOUS_STATES, MAX_TX_SIZE,
};
use crate::system_contracts::SystemContracts;
use crate::{delegate_vm, utils};
use anvil_zksync_common::sh_println;
use anvil_zksync_common::shell::get_shell;
use anvil_zksync_config::constants::{
    LEGACY_RICH_WALLETS, NON_FORK_FIRST_BLOCK_TIMESTAMP, RICH_WALLETS,
};
use anvil_zksync_config::types::BoojumConfig;
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_traces::identifier::SignaturesIdentifier;
use anvil_zksync_traces::{
    build_call_trace_arena, decode_trace_arena, filter_call_trace_arena, render_trace_arena_inner,
};
use colored::Colorize;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use zksync_contracts::{BaseSystemContracts, BaseSystemContractsHashes};
use zksync_error::anvil_zksync::node::AnvilNodeResult;
use zksync_error::anvil_zksync::{halt::HaltError, revert::RevertError};
use zksync_multivm::interface::storage::{ReadStorage, StorageView, WriteStorage};
use zksync_multivm::interface::{
    BatchTransactionExecutionResult, ExecutionResult, FinishedL1Batch, InspectExecutionMode,
    L1BatchEnv, L2BlockEnv, SystemEnv, TxExecutionMode, VmExecutionResultAndLogs, VmFactory,
    VmInterface,
};
use zksync_multivm::tracers::{CallTracer, TracerDispatcher};
use zksync_multivm::utils::{
    adjust_pubdata_price_for_tx, derive_base_fee_and_gas_per_pubdata, derive_overhead,
    get_max_gas_per_pubdata_byte,
};
use zksync_multivm::vm_latest::constants::{
    BATCH_COMPUTATIONAL_GAS_LIMIT, MAX_VM_PUBDATA_PER_BATCH,
};
use zksync_multivm::vm_latest::{HistoryDisabled, Vm};
use zksync_multivm::{MultiVmTracer, VmVersion};
use zksync_types::api::{BlockIdVariant, TransactionVariant};
use zksync_types::block::build_bloom;
use zksync_types::fee::Fee;
use zksync_types::fee_model::{BatchFeeInput, PubdataIndependentBatchFeeModelInput};
use zksync_types::l1::L1Tx;
use zksync_types::l2::{L2Tx, TransactionType};
use zksync_types::message_root::{AGG_TREE_HEIGHT_KEY, AGG_TREE_NODES_KEY};
use zksync_types::transaction_request::CallRequest;
use zksync_types::utils::{decompose_full_nonce, nonces_to_full_nonce};
use zksync_types::web3::{keccak256, Index};
use zksync_types::Nonce;
use zksync_types::{
    api, h256_to_u256, u256_to_h256, AccountTreeId, Address, Bloom, BloomInput,
    ExecuteTransactionCommon, L1BatchNumber, L2BlockNumber, L2ChainId, StorageKey, StorageValue,
    Transaction, H160, H256, L2_MESSAGE_ROOT_ADDRESS, MAX_L2_TX_GAS_LIMIT, U256, U64,
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
            version: self.blockchain.protocol_version,
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
            &StorageView::new(&self.fork_storage).to_rc_ptr(),
            &self.time,
        );

        let block_ctx = BlockContext {
            hash: H256::zero(),
            batch: (last_l1_batch_number + 1).0,
            miniblock: last_l2_block.number as u64 + 1,
            timestamp: self.time.peek_next_timestamp(),
            prev_block_hash: last_l2_block.hash,
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

    #[allow(clippy::too_many_arguments)]
    async fn apply_batch(
        &mut self,
        batch_timestamp: u64,
        base_system_contracts_hashes: BaseSystemContractsHashes,
        block: api::Block<api::TransactionVariant>,
        virtual_block: Option<api::Block<api::TransactionVariant>>,
        tx_results: Vec<TransactionResult>,
        finished_l1_batch: FinishedL1Batch,
        modified_storage_keys: HashMap<StorageKey, StorageValue>,
    ) {
        // TODO: `apply_batch` is leaking a lot of abstractions and should be wholly contained inside `Blockchain`.
        //       Additionally, a dedicated `PreviousStates` struct would help with separation of concern.
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
        let new_bytecodes = tx_results
            .iter()
            .flat_map(|tr| tr.new_bytecodes.clone())
            .collect::<Vec<_>>();
        let aggregation_root = self.read_aggregation_root(&modified_storage_keys);
        storage.apply_batch(
            batch_timestamp,
            base_system_contracts_hashes,
            tx_results,
            finished_l1_batch,
            aggregation_root,
        );

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
        storage.apply_block(block, 0);

        // Apply new factory deps
        for (hash, code) in new_bytecodes {
            self.fork_storage.store_factory_dep(hash, code)
        }

        // Apply storage writes
        for (key, value) in modified_storage_keys {
            self.fork_storage.set_value(key, value);
        }

        if let Some(virtual_block) = virtual_block {
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
            storage.apply_block(virtual_block, 1);
        }
    }

    fn n_dim_array_key_in_layout(array_key: usize, indices: &[U256]) -> H256 {
        let mut key: H256 = u256_to_h256(array_key.into());

        for index in indices {
            key = H256(keccak256(key.as_bytes()));
            key = u256_to_h256(h256_to_u256(key).overflowing_add(*index).0);
        }

        key
    }

    fn read_aggregation_root(
        &self,
        modified_storage_keys: &HashMap<StorageKey, StorageValue>,
    ) -> H256 {
        let agg_tree_height_slot = StorageKey::new(
            AccountTreeId::new(L2_MESSAGE_ROOT_ADDRESS),
            H256::from_low_u64_be(AGG_TREE_HEIGHT_KEY as u64),
        );

        let agg_tree_height = modified_storage_keys
            .get(&agg_tree_height_slot)
            .copied()
            .unwrap_or_else(|| {
                self.fork_storage
                    .read_value_internal(&agg_tree_height_slot)
                    .unwrap()
            });
        let agg_tree_height = h256_to_u256(agg_tree_height);

        // `nodes[height][0]`
        let agg_tree_root_hash_key =
            Self::n_dim_array_key_in_layout(AGG_TREE_NODES_KEY, &[agg_tree_height, U256::zero()]);
        let agg_tree_root_hash_slot = StorageKey::new(
            AccountTreeId::new(L2_MESSAGE_ROOT_ADDRESS),
            agg_tree_root_hash_key,
        );

        modified_storage_keys
            .get(&agg_tree_root_hash_slot)
            .copied()
            .unwrap_or_else(|| {
                self.fork_storage
                    .read_value_internal(&agg_tree_root_hash_slot)
                    .unwrap()
            })
    }

    pub(super) async fn seal_block(
        &mut self,
        tx_batch_execution_result: TxBatchExecutionResult,
    ) -> AnvilNodeResult<L2BlockNumber> {
        let TxBatchExecutionResult {
            tx_results,
            base_system_contracts_hashes,
            batch_env,
            block_ctxs,
            finished_l1_batch,
            modified_storage_keys,
        } = tx_batch_execution_result;

        let mut filters = self.filters.write().await;
        for tx_result in &tx_results {
            // TODO: Is this the right place to notify about new pending txs?
            filters.notify_new_pending_transaction(tx_result.receipt.transaction_hash);
            for log in &tx_result.receipt.logs {
                filters.notify_new_log(log, block_ctxs[0].miniblock.into());
            }
        }
        drop(filters);

        let mut transactions = Vec::new();
        for (index, tx_result) in tx_results.iter().enumerate() {
            let mut transaction = if let Ok(l2_tx) =
                <Transaction as TryInto<L2Tx>>::try_into(tx_result.info.tx.clone())
            {
                api::Transaction::from(l2_tx)
            } else {
                // TODO: Build proper API transaction for upgrade transactions
                api::Transaction {
                    hash: tx_result.info.tx.hash(),
                    ..Default::default()
                }
            };
            transaction.block_hash = Some(block_ctxs[0].hash);
            transaction.block_number = Some(U64::from(block_ctxs[0].miniblock));
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
        let block = create_block(
            &batch_env,
            block_ctxs[0].hash,
            block_ctxs[0].prev_block_hash,
            block_ctxs[0].miniblock,
            block_ctxs[0].timestamp,
            transactions,
            gas_used,
            logs_bloom,
        );

        // Make sure optional virtual block gets saved too
        let virtual_block = if block_ctxs.len() == 2 {
            Some(create_block(
                &batch_env,
                block_ctxs[1].hash,
                block_ctxs[1].prev_block_hash,
                block_ctxs[1].miniblock,
                block_ctxs[1].timestamp,
                vec![],
                U256::zero(),
                Bloom::zero(),
            ))
        } else {
            None
        };

        // Use first block's timestamp as batch timestamp
        self.apply_batch(
            batch_env.timestamp,
            base_system_contracts_hashes,
            block,
            virtual_block,
            tx_results,
            finished_l1_batch,
            modified_storage_keys,
        )
        .await;

        let mut filters = self.filters.write().await;
        for block_ctx in &block_ctxs {
            filters.notify_new_block(block_ctx.hash);
        }
        drop(filters);

        Ok(L2BlockNumber(block_ctxs[0].miniblock as u32))
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

        let mut l2_tx = L2Tx::from_request(
            request_with_gas_per_pubdata_overridden.into(),
            MAX_TX_SIZE,
            self.system_contracts.allow_no_target(),
        )
        .map_err(Web3Error::SerializationError)?;

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

        self.estimate_gas_inner(l2_tx.into()).await
    }

    pub async fn estimate_l1_to_l2_gas_impl(&self, req: CallRequest) -> Result<U256, Web3Error> {
        let mut request_with_gas_per_pubdata_overridden = req;

        if let Some(ref mut eip712_meta) = request_with_gas_per_pubdata_overridden.eip712_meta {
            if eip712_meta.gas_per_pubdata == U256::zero() {
                eip712_meta.gas_per_pubdata =
                    get_max_gas_per_pubdata_byte(VmVersion::latest()).into();
            }
        }

        let l1_tx = L1Tx::from_request(
            request_with_gas_per_pubdata_overridden,
            self.system_contracts.allow_no_target(),
        )
        .map_err(Web3Error::SerializationError)?;

        Ok(self.estimate_gas_inner(l1_tx.into()).await?.gas_limit)
    }

    async fn estimate_gas_inner(&self, mut tx: Transaction) -> Result<Fee, Web3Error> {
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
        match &mut tx.common_data {
            ExecuteTransactionCommon::L1(l1_common_data) => {
                l1_common_data.max_fee_per_gas = base_fee.into();
            }
            ExecuteTransactionCommon::L2(l2_common_data) => {
                l2_common_data.fee.max_fee_per_gas = base_fee.into();
                l2_common_data.fee.max_priority_fee_per_gas = base_fee.into();
            }
            ExecuteTransactionCommon::ProtocolUpgrade(_) => unimplemented!(),
        }

        let execution_mode = TxExecutionMode::EstimateFee;
        let (mut batch_env, _) = self.create_l1_batch_env().await;
        batch_env.fee_input = fee_input;

        let initiator_address = tx.initiator_account();
        let impersonating = self.impersonation.is_impersonating(&initiator_address);
        let system_contracts = self
            .system_contracts
            .contracts_for_fee_estimate(impersonating)
            .clone();
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

            // If the transaction has failed with such a large gas limit, we return an API error here right away,
            // since the inferred gas bounds would be unreliable in this case.
            let result = self
                .check_if_executable(
                    tx.clone(),
                    gas_per_pubdata_byte,
                    batch_env.clone(),
                    system_env.clone(),
                )
                .await?;

            if result.statistics.pubdata_published > (MAX_VM_PUBDATA_PER_BATCH as u32) {
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

            let estimate_gas_result = self
                .estimate_gas_step(
                    tx.clone(),
                    gas_per_pubdata_byte,
                    try_gas_limit,
                    batch_env.clone(),
                    system_env.clone(),
                    &self.fork_storage,
                    &self.system_contracts.boojum,
                    false,
                )
                .tx_result;

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

        let estimate_gas_result = self
            .estimate_gas_step(
                tx.clone(),
                gas_per_pubdata_byte,
                suggested_gas_limit,
                batch_env,
                system_env,
                &self.fork_storage,
                &self.system_contracts.boojum,
                false,
            )
            .tx_result;

        let overhead = derive_overhead(
            suggested_gas_limit,
            gas_per_pubdata_byte as u32,
            tx.encoding_len(),
            tx.tx_format() as u8,
            VmVersion::latest(),
        ) as u64;

        match &estimate_gas_result.result {
            ExecutionResult::Revert { output } => {
                let message = output.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );
                let data = output.encoded_data();

                let revert_reason: RevertError = output.clone().to_revert_reason().await;
                let error_report = ExecutionErrorReport::new(&revert_reason, Some(&tx));
                sh_println!("{}", error_report);

                Err(Web3Error::SubmitTransactionError(pretty_message, data))
            }
            ExecutionResult::Halt { reason } => {
                let message = reason.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );

                let halt_error: HaltError = reason.clone().to_halt_error().await;
                let error_report = ExecutionErrorReport::new(&halt_error, Some(&tx));
                sh_println!("{}", error_report);

                Err(Web3Error::SubmitTransactionError(pretty_message, vec![]))
            }
            ExecutionResult::Success { .. } => {
                let full_gas_limit = match suggested_gas_limit.overflowing_add(overhead) {
                    (value, false) => value,
                    (_, true) => {
                        tracing::info!("Overflow when calculating gas estimation. We've exceeded the block gas limit by summing the following values:");
                        tracing::info!(
                            "\tEstimated transaction body gas cost: {}",
                            tx_body_gas_limit
                        );
                        tracing::info!("\tGas for pubdata: {}", additional_gas_for_pubdata);
                        tracing::info!("\tOverhead: {}", overhead);

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
        mut tx: Transaction,
        gas_per_pubdata_byte: u64,
        tx_gas_limit: u64,
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
        fork_storage: &ForkStorage,
        boojum: &BoojumConfig,
        trace_calls: bool,
    ) -> BatchTransactionExecutionResult {
        // Set gas_limit for transaction
        let gas_limit_with_overhead = tx_gas_limit
            + derive_overhead(
                tx_gas_limit,
                gas_per_pubdata_byte as u32,
                tx.encoding_len(),
                tx.tx_format() as u8,
                VmVersion::latest(),
            ) as u64;
        match &mut tx.common_data {
            ExecuteTransactionCommon::L1(l1_common_data) => {
                l1_common_data.gas_limit = gas_limit_with_overhead.into();
                // Since `tx.execute.value` is supplied by the client and is not checked against the current balance (unlike for L2 transactions),
                // we may hit an integer overflow. Ditto for protocol upgrade transactions below.
                let required_funds = (l1_common_data.gas_limit * l1_common_data.max_fee_per_gas)
                    .checked_add(tx.execute.value)
                    .unwrap();
                l1_common_data.to_mint = required_funds;
            }
            ExecuteTransactionCommon::L2(l2_common_data) => {
                l2_common_data.fee.gas_limit = gas_limit_with_overhead.into();
            }
            ExecuteTransactionCommon::ProtocolUpgrade(_) => unimplemented!(),
        }

        let storage = StorageView::new(fork_storage).to_rc_ptr();

        let nonce_key = self
            .storage_key_layout
            .get_nonce_key(&tx.initiator_account());
        let stored_full = h256_to_u256(storage.borrow_mut().read_value(&nonce_key));
        let (account_nonce, deployment_nonce) = decompose_full_nonce(stored_full);

        let supplied = tx.nonce().map(|n| U256::from(n.0));

        let final_nonce = match supplied {
            // caller explicitly set a nonce ≥ on-chain ⇒ honour it
            Some(n) if n >= account_nonce => n,
            // otherwise use the current chain nonce
            _ => account_nonce,
        };

        let enforced_full = nonces_to_full_nonce(final_nonce, deployment_nonce);
        if enforced_full != stored_full {
            storage
                .borrow_mut()
                .set_value(nonce_key, u256_to_h256(enforced_full));
        }

        // We need to explicitly put enough balance into the account of the users
        let payer = tx.payer();
        let balance_key = self
            .storage_key_layout
            .get_storage_key_for_base_token(&payer);
        let mut current_balance = h256_to_u256(storage.borrow_mut().read_value(&balance_key));
        match &mut tx.common_data {
            ExecuteTransactionCommon::L1(l1_common_data) => {
                let added_balance = l1_common_data.gas_limit * l1_common_data.max_fee_per_gas;
                current_balance += added_balance;
                storage
                    .borrow_mut()
                    .set_value(balance_key, u256_to_h256(current_balance));
            }
            ExecuteTransactionCommon::L2(l2_common_data) => {
                let added_balance =
                    l2_common_data.fee.gas_limit * l2_common_data.fee.max_fee_per_gas;
                current_balance += added_balance;
                storage
                    .borrow_mut()
                    .set_value(balance_key, u256_to_h256(current_balance));

                let nonce_u32: u32 = final_nonce.try_into().expect("nonce always fits into u32");
                l2_common_data.nonce = Nonce(nonce_u32);
            }
            ExecuteTransactionCommon::ProtocolUpgrade(_) => unimplemented!(),
        }

        let mut vm = if boojum.use_boojum {
            let mut vm = BoojumOsVM::<_, HistoryDisabled>::new(
                batch_env,
                system_env,
                storage,
                // TODO: this might be causing a deadlock.. check..
                &fork_storage.inner.read().unwrap().raw_storage,
                boojum,
            );
            // Temporary hack - as we update the 'storage' just above, but boojumos loads its full
            // state from fork_storage (that is not updated).
            vm.update_inconsistent_keys(&[&nonce_key, &balance_key]);
            AnvilVM::BoojumOs(vm)
        } else {
            AnvilVM::ZKSync(Vm::new(batch_env, system_env, storage))
        };

        delegate_vm!(vm, push_transaction(tx));

        let call_tracer_result = Arc::new(OnceCell::default());
        let tracer_dispatcher = if trace_calls {
            let tracers: Vec<Box<dyn MultiVmTracer<StorageView<&ForkStorage>, HistoryDisabled>>> =
                vec![CallTracer::new(call_tracer_result.clone()).into_tracer_pointer()];
            TracerDispatcher::from(tracers)
        } else {
            Default::default()
        };

        let tx_result = match &mut vm {
            AnvilVM::BoojumOs(vm) => {
                vm.inspect(&mut tracer_dispatcher.into(), InspectExecutionMode::OneTx)
            }
            AnvilVM::ZKSync(vm) => {
                vm.inspect(&mut tracer_dispatcher.into(), InspectExecutionMode::OneTx)
            }
        };
        let call_traces = Arc::try_unwrap(call_tracer_result)
            .expect("failed extracting call traces")
            .take()
            .unwrap_or_default();
        BatchTransactionExecutionResult {
            tx_result: Box::new(tx_result),
            compression_result: Ok(()),
            call_traces,
        }
    }

    async fn check_if_executable(
        &self,
        tx: Transaction,
        gas_per_pubdata_byte: u64,
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
    ) -> Result<VmExecutionResultAndLogs, Web3Error> {
        let verbosity = get_shell().verbosity;
        let mut known_addresses = known_addresses_after_transaction(&tx);
        let BatchTransactionExecutionResult {
            tx_result,
            call_traces,
            ..
        } = self.estimate_gas_step(
            tx.clone(),
            gas_per_pubdata_byte,
            // TODO: check what the max value should be here.
            MAX_L2_TX_GAS_LIMIT,
            batch_env,
            system_env,
            &self.fork_storage,
            &self.system_contracts.boojum,
            true,
        );

        let error = match tx_result.result {
            ExecutionResult::Success { .. } => {
                // Transaction is executable with max gas, proceed with gas estimation
                return Ok(*tx_result);
            }
            ExecutionResult::Revert { ref output } => {
                let message = output.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );
                let data = output.encoded_data();

                if verbosity >= 1 {
                    let revert_reason: RevertError = output.clone().to_revert_reason().await;
                    let error_report = ExecutionErrorReport::new(&revert_reason, Some(&tx));
                    sh_println!(
                        "{}: {}\n{}",
                        "error".red().bold(),
                        "Gas estimation encountered unexecutable transaction".red(),
                        error_report
                    );
                }

                Web3Error::SubmitTransactionError(pretty_message, data)
            }
            ExecutionResult::Halt { ref reason } => {
                let message = reason.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );

                if verbosity >= 1 {
                    let halt_error: HaltError = reason.clone().to_halt_error().await;
                    let error_report = ExecutionErrorReport::new(&halt_error, Some(&tx));
                    sh_println!(
                        "{}: {}\n{}",
                        "error".red().bold(),
                        "Gas estimation encountered unexecutable transaction".red(),
                        error_report
                    );
                }

                Web3Error::SubmitTransactionError(pretty_message, vec![])
            }
        };

        if !call_traces.is_empty() && verbosity >= 2 {
            let mut builder = CallTraceDecoderBuilder::default();

            builder = builder.with_signature_identifier(
                SignaturesIdentifier::new(
                    Some(self.config.get_cache_dir().into()),
                    self.config.offline,
                )
                .map_err(|err| anyhow::anyhow!("Failed to create SignaturesIdentifier: {err:#}"))?,
            );

            let decoder = builder.build();
            let mut arena = build_call_trace_arena(&call_traces, &tx_result);
            decode_trace_arena(&mut arena, &decoder).await?;

            extract_addresses(&arena, &mut known_addresses);

            let filtered_arena = filter_call_trace_arena(&arena, verbosity);
            let trace_output = render_trace_arena_inner(&filtered_arena, false);
            sh_println!("\nTraces:\n{}", trace_output);
        }
        Err(error)
    }

    /// Creates a [Snapshot] of the current state of the node.
    pub async fn snapshot(&self) -> AnvilNodeResult<Snapshot> {
        let blockchain = self.blockchain.read().await;
        let filters = self.filters.read().await.clone();
        let storage = self
            .fork_storage
            .inner
            .read()
            .expect("failed acquiring read lock on storage");

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
    pub async fn restore_snapshot(&mut self, snapshot: Snapshot) -> AnvilNodeResult<()> {
        let mut blockchain = self.blockchain.write().await;
        let mut storage = self
            .fork_storage
            .inner
            .write()
            .expect("failed acquiring write lock on storage");

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
                        tracing::error!("unable to map block number to hash #{:#x}", o.block_hash);
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
        } else if let Some(block_hash) = storage.hashes.get(&block_number) {
            let state = self
                .previous_states
                .get(block_hash)
                .ok_or_else(|| Web3Error::PrunedBlock(block_number))?;
            if let Some(value) = state.get(&storage_key) {
                return Ok(*value);
            }
            // Block was produced locally but slot hasn't been touched since we forked
            Ok(self.fork.get_storage_at_forked(address, idx).await?)
        } else {
            // Block was not produced locally so we assume it comes from fork
            Ok(self.fork.get_storage_at(address, idx, block).await?)
        }
    }

    pub async fn reset(&mut self, fork_client_opt: Option<ForkClient>) {
        let fork_details = fork_client_opt.as_ref().map(|client| &client.details);
        let blockchain = Blockchain::new(
            self.blockchain.protocol_version,
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
            self.config.system_contracts_options,
            self.blockchain.protocol_version,
            self.config.chain_id,
            self.config.system_contracts_path.as_deref(),
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

/// Keeps track of a block's batch number, miniblock number and timestamp.
/// Useful for keeping track of the current context when creating multiple blocks.
#[derive(Debug, Clone, Default)]
pub struct BlockContext {
    pub hash: H256,
    pub batch: u32,
    pub miniblock: u64,
    pub timestamp: u64,
    pub prev_block_hash: H256,
}

impl BlockContext {
    /// Create the next batch instance that uses the same batch number, and has all other parameters incremented by `1`.
    pub(super) fn new_block(&self, time: &mut Time) -> BlockContext {
        Self {
            hash: H256::zero(),
            batch: self.batch,
            miniblock: self.miniblock.saturating_add(1),
            timestamp: time.advance_timestamp(),
            prev_block_hash: self.hash,
        }
    }
}

// Test utils
#[cfg(test)]
pub mod testing {
    use super::*;
    use zksync_types::ProtocolVersionId;

    pub struct InnerNodeTester {
        pub node: Arc<RwLock<InMemoryNodeInner>>,
    }

    impl InnerNodeTester {
        pub fn test() -> Self {
            let config = TestNodeConfig::default();
            let fee_provider = TestNodeFeeInputProvider::default();
            let impersonation = ImpersonationManager::default();
            let system_contracts = SystemContracts::from_options(
                config.system_contracts_options,
                config.system_contracts_path.clone(),
                ProtocolVersionId::latest(),
                config.use_evm_interpreter,
                config.boojum.clone(),
            );
            let storage_key_layout = if config.boojum.use_boojum {
                StorageKeyLayout::BoojumOs
            } else {
                StorageKeyLayout::ZkEra
            };
            let (node, _, _, _, _, _) = InMemoryNodeInner::init(
                None,
                fee_provider,
                Arc::new(RwLock::new(Default::default())),
                config,
                impersonation.clone(),
                system_contracts.clone(),
                storage_key_layout,
                false,
            );
            InnerNodeTester { node }
        }
    }

    impl InMemoryNodeInner {
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

        pub fn insert_previous_state(
            &mut self,
            hash: H256,
            state: HashMap<StorageKey, StorageValue>,
        ) {
            self.previous_states.insert(hash, state);
        }

        pub fn get_previous_state(&self, hash: H256) -> Option<HashMap<StorageKey, StorageValue>> {
            self.previous_states.get(&hash).cloned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::node::create_genesis;
    use crate::testing;
    use itertools::Itertools;
    use zksync_types::block::L2BlockHasher;
    use zksync_types::ProtocolVersionId;

    #[tokio::test]
    async fn test_create_genesis_creates_block_with_hash_and_zero_parent_hash() {
        let (first_block, first_batch) =
            create_genesis::<TransactionVariant>(ProtocolVersionId::latest(), Some(1000));

        assert_eq!(
            first_block.hash,
            L2BlockHasher::legacy_hash(L2BlockNumber(0))
        );
        assert_eq!(first_block.parent_hash, H256::zero());

        assert_eq!(first_batch.number, L1BatchNumber(0));
    }

    #[tokio::test]
    async fn test_snapshot() {
        let tester = InnerNodeTester::test();
        let mut writer = tester.node.write().await;

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
                    new_bytecodes: vec![],
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
        let tester = InnerNodeTester::test();
        let mut writer = tester.node.write().await;

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
                    new_bytecodes: vec![],
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
                    new_bytecodes: vec![],
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
