//! In-memory node, that supports forking other networks.
use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    str::FromStr,
    sync::{Arc, RwLock},
};

use colored::Colorize;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use zksync_contracts::BaseSystemContracts;
use zksync_multivm::vm_latest::HistoryEnabled;
use zksync_multivm::{
    interface::{
        storage::{ReadStorage, StoragePtr, WriteStorage},
        Call, ExecutionResult, InspectExecutionMode, L1BatchEnv, L2Block, L2BlockEnv, SystemEnv,
        TxExecutionMode, VmExecutionResultAndLogs, VmFactory, VmInterface, VmInterfaceExt,
        VmInterfaceHistoryEnabled,
    },
    tracers::CallTracer,
    utils::{
        adjust_pubdata_price_for_tx, derive_base_fee_and_gas_per_pubdata, derive_overhead,
        get_batch_base_fee, get_max_batch_gas_limit, get_max_gas_per_pubdata_byte,
    },
    vm_latest::{
        constants::{BATCH_COMPUTATIONAL_GAS_LIMIT, BATCH_GAS_LIMIT, MAX_VM_PUBDATA_PER_BATCH},
        utils::l2_blocks::load_last_l2_block,
        HistoryDisabled, ToTracerPointer, Vm,
    },
    HistoryMode, VmVersion,
};
use zksync_types::{
    api::{Block, DebugCall, Log, TransactionReceipt, TransactionVariant},
    block::{build_bloom, unpack_block_info, L2BlockHasher},
    fee::Fee,
    fee_model::{BatchFeeInput, PubdataIndependentBatchFeeModelInput},
    get_code_key, get_nonce_key,
    l2::{L2Tx, TransactionType},
    utils::{decompose_full_nonce, nonces_to_full_nonce, storage_key_for_eth_balance},
    web3::{keccak256, Bytes, Index},
    AccountTreeId, Address, Bloom, BloomInput, L1BatchNumber, L2BlockNumber, PackedEthSignature,
    StorageKey, StorageValue, Transaction, ACCOUNT_CODE_STORAGE_ADDRESS, EMPTY_UNCLES_HASH, H160,
    H256, H64, MAX_L2_TX_GAS_LIMIT, SYSTEM_CONTEXT_ADDRESS, SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
    U256, U64,
};
use zksync_utils::{bytecode::hash_bytecode, h256_to_account_address, h256_to_u256, u256_to_h256};
use zksync_web3_decl::error::Web3Error;

use crate::node::impersonate::{ImpersonationManager, ImpersonationState};
use crate::node::time::{AdvanceTime, ReadTime, TimestampManager};
use crate::node::{BlockSealer, TxPool};
use crate::{
    bootloader_debug::{BootloaderDebug, BootloaderDebugTracer},
    config::{
        cache::CacheConfig,
        constants::{LEGACY_RICH_WALLETS, NON_FORK_FIRST_BLOCK_TIMESTAMP, RICH_WALLETS},
        show_details::{ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails},
        Genesis, TestNodeConfig,
    },
    console_log::ConsoleLogHandler,
    deps::{storage_view::StorageView, InMemoryStorage},
    filters::EthFilters,
    fork::{ForkDetails, ForkSource, ForkStorage},
    formatter,
    node::{
        call_error_tracer::CallErrorTracer, fee_model::TestNodeFeeInputProvider,
        storage_logs::print_storage_logs_details,
    },
    observability::Observability,
    system_contracts::{self, SystemContracts},
    utils::{bytecode_to_factory_dep, create_debug_output, into_jsrpc_error, to_human_size},
};

/// Max possible size of an ABI encoded tx (in bytes).
pub const MAX_TX_SIZE: usize = 1_000_000;
/// Acceptable gas overestimation limit.
pub const ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION: u64 = 1_000;
/// The maximum number of previous blocks to store the state for.
pub const MAX_PREVIOUS_STATES: u16 = 128;
/// The zks protocol version.
pub const PROTOCOL_VERSION: &str = "zks/1";

pub fn compute_hash<'a>(block_number: u64, tx_hashes: impl IntoIterator<Item = &'a H256>) -> H256 {
    let tx_bytes = tx_hashes
        .into_iter()
        .flat_map(|h| h.to_fixed_bytes())
        .collect::<Vec<_>>();
    let digest = [&block_number.to_be_bytes()[..], tx_bytes.as_slice()].concat();
    H256(keccak256(&digest))
}

pub fn create_genesis_from_json(
    genesis: &Genesis,
    timestamp: Option<u64>,
) -> Block<TransactionVariant> {
    let hash = genesis.hash.unwrap_or_else(|| compute_hash(0, []));
    let timestamp = timestamp
        .or(genesis.timestamp)
        .unwrap_or(NON_FORK_FIRST_BLOCK_TIMESTAMP);

    let l1_batch_env = genesis.l1_batch_env.clone().unwrap_or_else(|| L1BatchEnv {
        previous_batch_hash: None,
        number: L1BatchNumber(0),
        timestamp,
        fee_input: BatchFeeInput::pubdata_independent(0, 0, 0),
        fee_account: Address::zero(),
        enforced_base_fee: None,
        first_l2_block: L2BlockEnv {
            number: 0,
            timestamp,
            prev_block_hash: H256::zero(),
            max_virtual_blocks_to_create: 0,
        },
    });

    create_block(
        &l1_batch_env,
        hash,
        genesis.parent_hash.unwrap_or_else(H256::zero),
        genesis.block_number.unwrap_or(0),
        timestamp,
        genesis.transactions.clone().unwrap_or_default(),
        genesis.gas_used.unwrap_or_else(U256::zero),
        genesis.logs_bloom.unwrap_or_else(Bloom::zero),
    )
}

pub fn create_genesis<TX>(timestamp: Option<u64>) -> Block<TX> {
    let hash = compute_hash(0, []);
    let timestamp = timestamp.unwrap_or(NON_FORK_FIRST_BLOCK_TIMESTAMP);
    let batch_env = L1BatchEnv {
        previous_batch_hash: None,
        number: L1BatchNumber(0),
        timestamp,
        fee_input: BatchFeeInput::pubdata_independent(0, 0, 0),
        fee_account: Default::default(),
        enforced_base_fee: None,
        first_l2_block: L2BlockEnv {
            number: 0,
            timestamp,
            prev_block_hash: Default::default(),
            max_virtual_blocks_to_create: 0,
        },
    };
    create_block(
        &batch_env,
        hash,
        H256::zero(),
        0,
        timestamp,
        vec![],
        U256::zero(),
        Bloom::zero(),
    )
}

#[allow(clippy::too_many_arguments)]
fn create_block<TX>(
    batch_env: &L1BatchEnv,
    hash: H256,
    parent_hash: H256,
    number: u64,
    timestamp: u64,
    transactions: Vec<TX>,
    gas_used: U256,
    logs_bloom: Bloom,
) -> Block<TX> {
    Block {
        hash,
        parent_hash,
        uncles_hash: EMPTY_UNCLES_HASH, // Static for non-PoW chains, see EIP-3675
        number: U64::from(number),
        l1_batch_number: Some(U64::from(batch_env.number.0)),
        base_fee_per_gas: U256::from(get_batch_base_fee(batch_env, VmVersion::latest())),
        timestamp: U256::from(timestamp),
        l1_batch_timestamp: Some(U256::from(batch_env.timestamp)),
        transactions,
        gas_used,
        gas_limit: U256::from(get_max_batch_gas_limit(VmVersion::latest())),
        logs_bloom,
        author: Address::default(), // Matches core's behavior, irrelevant for ZKsync
        state_root: H256::default(), // Intentionally empty as blocks in ZKsync don't have state - batches do
        transactions_root: H256::default(), // Intentionally empty as blocks in ZKsync don't have state - batches do
        receipts_root: H256::default(), // Intentionally empty as blocks in ZKsync don't have state - batches do
        extra_data: Bytes::default(),   // Matches core's behavior, not used in ZKsync
        difficulty: U256::default(), // Empty for non-PoW chains, see EIP-3675, TODO: should be 2500000000000000 to match DIFFICULTY opcode
        total_difficulty: U256::default(), // Empty for non-PoW chains, see EIP-3675
        seal_fields: vec![],         // Matches core's behavior, TODO: remove
        uncles: vec![],              // Empty for non-PoW chains, see EIP-3675
        size: U256::default(),       // Matches core's behavior, TODO: perhaps it should be computed
        mix_hash: H256::default(),   // Empty for non-PoW chains, see EIP-3675
        nonce: H64::default(),       // Empty for non-PoW chains, see EIP-3675
    }
}

/// Information about the executed transaction.
#[derive(Debug, Clone)]
pub struct TxExecutionInfo {
    pub tx: L2Tx,
    // Batch number where transaction was executed.
    pub batch_number: u32,
    pub miniblock_number: u64,
    #[allow(unused)]
    pub result: VmExecutionResultAndLogs,
}

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub info: TxExecutionInfo,
    pub receipt: TransactionReceipt,
    pub debug: DebugCall,
}

impl TransactionResult {
    /// Returns the debug information for the transaction.
    /// If `only_top` is true - will only return the top level call.
    pub fn debug_info(&self, only_top: bool) -> DebugCall {
        let calls = if only_top {
            vec![]
        } else {
            self.debug.calls.clone()
        };
        DebugCall {
            calls,
            ..self.debug.clone()
        }
    }
}

/// Helper struct for InMemoryNode.
/// S - is the Source of the Fork.
#[derive(Clone)]
pub struct InMemoryNodeInner<S> {
    /// The latest batch number that was already generated.
    /// Next block will be current_batch + 1
    pub current_batch: u32,
    /// The latest miniblock number that was already generated.
    /// Next transaction will go to the block current_miniblock + 1
    pub current_miniblock: u64,
    /// The latest miniblock hash.
    pub current_miniblock_hash: H256,
    /// The fee input provider.
    pub fee_input_provider: TestNodeFeeInputProvider,
    // Map from transaction to details about the exeuction
    pub tx_results: HashMap<H256, TransactionResult>,
    // Map from block hash to information about the block.
    pub blocks: HashMap<H256, Block<TransactionVariant>>,
    // Map from block number to a block hash.
    pub block_hashes: HashMap<u64, H256>,
    // Map from filter_id to the eth filter
    pub filters: EthFilters,
    // Underlying storage
    pub fork_storage: ForkStorage<S>,
    // Configuration.
    pub config: TestNodeConfig,
    pub console_log_handler: ConsoleLogHandler,
    pub system_contracts: SystemContracts,
    pub impersonation: ImpersonationManager,
    pub rich_accounts: HashSet<H160>,
    /// Keeps track of historical states indexed via block hash. Limited to [MAX_PREVIOUS_STATES].
    pub previous_states: IndexMap<H256, HashMap<StorageKey, StorageValue>>,
}

#[derive(Debug)]
pub struct TxExecutionOutput {
    result: VmExecutionResultAndLogs,
    call_traces: Vec<Call>,
    bytecodes: HashMap<U256, Vec<U256>>,
}

impl<S: std::fmt::Debug + ForkSource> InMemoryNodeInner<S> {
    /// Create the state to be used implementing [InMemoryNode].
    pub fn new(
        fork: Option<ForkDetails>,
        config: &TestNodeConfig,
        time: &TimestampManager,
        impersonation: ImpersonationManager,
    ) -> Self {
        let updated_config = config.clone();
        if config.enable_auto_impersonate {
            // Enable auto impersonation if configured
            impersonation.set_auto_impersonation(true);
        }

        if let Some(f) = &fork {
            let mut block_hashes = HashMap::<u64, H256>::new();
            block_hashes.insert(f.l2_block.number.as_u64(), f.l2_block.hash);
            let mut blocks = HashMap::<H256, Block<TransactionVariant>>::new();
            blocks.insert(f.l2_block.hash, f.l2_block.clone());

            let fee_input_provider = if let Some(params) = f.fee_params {
                TestNodeFeeInputProvider::from_fee_params_and_estimate_scale_factors(
                    params,
                    f.estimate_gas_price_scale_factor,
                    f.estimate_gas_scale_factor,
                )
            } else {
                TestNodeFeeInputProvider::from_estimate_scale_factors(
                    f.estimate_gas_price_scale_factor,
                    f.estimate_gas_scale_factor,
                )
            };
            time.set_current_timestamp_unchecked(f.block_timestamp);

            InMemoryNodeInner {
                current_batch: f.l1_block.0,
                current_miniblock: f.l2_miniblock,
                current_miniblock_hash: f.l2_miniblock_hash,
                fee_input_provider,
                tx_results: Default::default(),
                blocks,
                block_hashes,
                filters: Default::default(),
                fork_storage: ForkStorage::new(
                    fork,
                    &updated_config.system_contracts_options,
                    updated_config.use_evm_emulator,
                    updated_config.chain_id,
                ),
                config: updated_config.clone(),
                console_log_handler: ConsoleLogHandler::default(),
                system_contracts: SystemContracts::from_options(
                    &updated_config.system_contracts_options,
                    updated_config.use_evm_emulator,
                ),
                impersonation,
                rich_accounts: HashSet::new(),
                previous_states: Default::default(),
            }
        } else {
            let mut block_hashes = HashMap::<u64, H256>::new();
            let block_hash = compute_hash(0, []);
            block_hashes.insert(0, block_hash);
            let mut blocks = HashMap::<H256, Block<TransactionVariant>>::new();
            let genesis_block: Block<TransactionVariant> = if let Some(ref genesis) = config.genesis
            {
                create_genesis_from_json(genesis, config.genesis_timestamp)
            } else {
                create_genesis(config.genesis_timestamp)
            };

            blocks.insert(block_hash, genesis_block);
            let fee_input_provider = TestNodeFeeInputProvider::default();
            time.set_current_timestamp_unchecked(NON_FORK_FIRST_BLOCK_TIMESTAMP);

            InMemoryNodeInner {
                current_batch: 0,
                current_miniblock: 0,
                current_miniblock_hash: block_hash,
                fee_input_provider,
                tx_results: Default::default(),
                blocks,
                block_hashes,
                filters: Default::default(),
                fork_storage: ForkStorage::new(
                    fork,
                    &config.system_contracts_options,
                    config.use_evm_emulator,
                    updated_config.chain_id,
                ),
                config: config.clone(),
                console_log_handler: ConsoleLogHandler::default(),
                system_contracts: SystemContracts::from_options(
                    &config.system_contracts_options,
                    config.use_evm_emulator,
                ),
                impersonation,
                rich_accounts: HashSet::new(),
                previous_states: Default::default(),
            }
        }
    }

    /// Create [L1BatchEnv] to be used in the VM.
    ///
    /// We compute l1/l2 block details from storage to support fork testing, where the storage
    /// can be updated mid execution and no longer matches with the initial node's state.
    /// The L1 & L2 timestamps are also compared with node's timestamp to ensure it always increases monotonically.
    pub fn create_l1_batch_env<T: ReadTime, ST: ReadStorage>(
        &self,
        time: &T,
        storage: StoragePtr<ST>,
    ) -> (L1BatchEnv, BlockContext) {
        tracing::debug!("Creating l1 batch env...");

        let last_l1_block_num = load_last_l1_batch(storage.clone())
            .map(|(num, _)| num as u32)
            .unwrap_or(self.current_batch);
        let last_l2_block = load_last_l2_block(&storage).unwrap_or_else(|| L2Block {
            number: self.current_miniblock as u32,
            hash: L2BlockHasher::legacy_hash(L2BlockNumber(self.current_miniblock as u32)),
            timestamp: time.current_timestamp(),
        });

        let block_ctx = BlockContext {
            hash: H256::zero(),
            batch: last_l1_block_num.saturating_add(1),
            miniblock: (last_l2_block.number as u64).saturating_add(1),
            timestamp: time.peek_next_timestamp(),
        };

        let fee_input = if let Some(fork) = &self
            .fork_storage
            .inner
            .read()
            .expect("fork_storage lock is already held by the current thread")
            .fork
        {
            BatchFeeInput::PubdataIndependent(PubdataIndependentBatchFeeModelInput {
                l1_gas_price: fork.l1_gas_price,
                fair_l2_gas_price: fork.l2_fair_gas_price,
                fair_pubdata_price: fork.fair_pubdata_price,
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

    /// Estimates the gas required for a given call request.
    ///
    /// # Arguments
    ///
    /// * `req` - A `CallRequest` struct representing the call request to estimate gas for.
    ///
    /// # Returns
    ///
    /// A `Result` with a `Fee` representing the estimated gas related data.
    pub fn estimate_gas_impl<T: ReadTime>(
        &self,
        time: &T,
        req: zksync_types::transaction_request::CallRequest,
    ) -> jsonrpc_core::Result<Fee> {
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
        let allow_no_target = system_contracts.evm_emulator.is_some();

        let mut l2_tx = L2Tx::from_request(
            request_with_gas_per_pubdata_overridden.into(),
            MAX_TX_SIZE,
            allow_no_target,
        )
        .map_err(|err| into_jsrpc_error(Web3Error::SerializationError(err)))?;

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

        let storage_view = StorageView::new(&self.fork_storage);
        let storage = storage_view.into_rc_ptr();

        let execution_mode = TxExecutionMode::EstimateFee;
        let (mut batch_env, _) = self.create_l1_batch_env(time, storage.clone());
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
            let result = InMemoryNodeInner::estimate_gas_step(
                l2_tx.clone(),
                gas_per_pubdata_byte,
                BATCH_GAS_LIMIT,
                batch_env.clone(),
                system_env.clone(),
                &self.fork_storage,
            );

            if result.statistics.pubdata_published > MAX_VM_PUBDATA_PER_BATCH.try_into().unwrap() {
                return Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                    "exceeds limit for published pubdata".into(),
                    Default::default(),
                )));
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

            let estimate_gas_result = InMemoryNodeInner::estimate_gas_step(
                l2_tx.clone(),
                gas_per_pubdata_byte,
                try_gas_limit,
                batch_env.clone(),
                system_env.clone(),
                &self.fork_storage,
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

        let estimate_gas_result = InMemoryNodeInner::estimate_gas_step(
            l2_tx.clone(),
            gas_per_pubdata_byte,
            suggested_gas_limit,
            batch_env,
            system_env,
            &self.fork_storage,
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
                tracing::info!("{}", format!("Unable to estimate gas for the request with our suggested gas limit of {}. The transaction is most likely unexecutable. Breakdown of estimation:", suggested_gas_limit + overhead).red());
                tracing::info!(
                    "{}",
                    format!(
                        "\tEstimated transaction body gas cost: {}",
                        tx_body_gas_limit
                    )
                    .red()
                );
                tracing::info!(
                    "{}",
                    format!("\tGas for pubdata: {}", additional_gas_for_pubdata).red()
                );
                tracing::info!("{}", format!("\tOverhead: {}", overhead).red());
                let message = output.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );
                let data = output.encoded_data();
                tracing::info!("{}", pretty_message.on_red());
                Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                    pretty_message,
                    data,
                )))
            }
            ExecutionResult::Halt { reason } => {
                tracing::info!("{}", format!("Unable to estimate gas for the request with our suggested gas limit of {}. The transaction is most likely unexecutable. Breakdown of estimation:", suggested_gas_limit + overhead).red());
                tracing::info!(
                    "{}",
                    format!(
                        "\tEstimated transaction body gas cost: {}",
                        tx_body_gas_limit
                    )
                    .red()
                );
                tracing::info!(
                    "{}",
                    format!("\tGas for pubdata: {}", additional_gas_for_pubdata).red()
                );
                tracing::info!("{}", format!("\tOverhead: {}", overhead).red());
                let message = reason.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );

                tracing::info!("{}", pretty_message.on_red());
                Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                    pretty_message,
                    vec![],
                )))
            }
            ExecutionResult::Success { .. } => {
                let full_gas_limit = match suggested_gas_limit.overflowing_add(overhead) {
                    (value, false) => value,
                    (_, true) => {
                        tracing::info!("{}", "Overflow when calculating gas estimation. We've exceeded the block gas limit by summing the following values:".red());
                        tracing::info!(
                            "{}",
                            format!(
                                "\tEstimated transaction body gas cost: {}",
                                tx_body_gas_limit
                            )
                            .red()
                        );
                        tracing::info!(
                            "{}",
                            format!("\tGas for pubdata: {}", additional_gas_for_pubdata).red()
                        );
                        tracing::info!("{}", format!("\tOverhead: {}", overhead).red());
                        return Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                            "exceeds block gas limit".into(),
                            Default::default(),
                        )));
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
        mut l2_tx: L2Tx,
        gas_per_pubdata_byte: u64,
        tx_gas_limit: u64,
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
        fork_storage: &ForkStorage<S>,
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
        let nonce_key = get_nonce_key(&l2_tx.initiator_account());
        let full_nonce = storage.borrow_mut().read_value(&nonce_key);
        let (_, deployment_nonce) = decompose_full_nonce(h256_to_u256(full_nonce));
        let enforced_full_nonce = nonces_to_full_nonce(U256::from(nonce.0), deployment_nonce);
        storage
            .borrow_mut()
            .set_value(nonce_key, u256_to_h256(enforced_full_nonce));

        // We need to explicitly put enough balance into the account of the users
        let payer = l2_tx.payer();
        let balance_key = storage_key_for_eth_balance(&payer);
        let mut current_balance = h256_to_u256(storage.borrow_mut().read_value(&balance_key));
        let added_balance = l2_tx.common_data.fee.gas_limit * l2_tx.common_data.fee.max_fee_per_gas;
        current_balance += added_balance;
        storage
            .borrow_mut()
            .set_value(balance_key, u256_to_h256(current_balance));

        let mut vm: Vm<_, HistoryDisabled> = Vm::new(batch_env, system_env, storage.clone());

        let tx: Transaction = l2_tx.into();
        vm.push_transaction(tx);

        vm.execute(InspectExecutionMode::OneTx)
    }

    /// Archives the current state for later queries.
    pub fn archive_state(&mut self) -> Result<(), String> {
        if self.previous_states.len() > MAX_PREVIOUS_STATES as usize {
            if let Some(entry) = self.previous_states.shift_remove_index(0) {
                tracing::debug!("removing archived state for previous block {:#x}", entry.0);
            }
        }
        tracing::debug!(
            "archiving state for {:#x} #{}",
            self.current_miniblock_hash,
            self.current_miniblock
        );
        self.previous_states.insert(
            self.current_miniblock_hash,
            self.fork_storage
                .inner
                .read()
                .map_err(|err| err.to_string())?
                .raw_storage
                .state
                .clone(),
        );

        Ok(())
    }

    /// Creates a [Snapshot] of the current state of the node.
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let storage = self
            .fork_storage
            .inner
            .read()
            .map_err(|err| format!("failed acquiring read lock on storage: {:?}", err))?;

        Ok(Snapshot {
            current_batch: self.current_batch,
            current_miniblock: self.current_miniblock,
            current_miniblock_hash: self.current_miniblock_hash,
            fee_input_provider: self.fee_input_provider.clone(),
            tx_results: self.tx_results.clone(),
            blocks: self.blocks.clone(),
            block_hashes: self.block_hashes.clone(),
            filters: self.filters.clone(),
            impersonation_state: self.impersonation.state(),
            rich_accounts: self.rich_accounts.clone(),
            previous_states: self.previous_states.clone(),
            raw_storage: storage.raw_storage.clone(),
            value_read_cache: storage.value_read_cache.clone(),
            factory_dep_cache: storage.factory_dep_cache.clone(),
        })
    }

    /// Restores a previously created [Snapshot] of the node.
    pub fn restore_snapshot(&mut self, snapshot: Snapshot) -> Result<(), String> {
        let mut storage = self
            .fork_storage
            .inner
            .write()
            .map_err(|err| format!("failed acquiring write lock on storage: {:?}", err))?;

        self.current_batch = snapshot.current_batch;
        self.current_miniblock = snapshot.current_miniblock;
        self.current_miniblock_hash = snapshot.current_miniblock_hash;
        self.fee_input_provider = snapshot.fee_input_provider;
        self.tx_results = snapshot.tx_results;
        self.blocks = snapshot.blocks;
        self.block_hashes = snapshot.block_hashes;
        self.filters = snapshot.filters;
        self.impersonation.set_state(snapshot.impersonation_state);
        self.rich_accounts = snapshot.rich_accounts;
        self.previous_states = snapshot.previous_states;
        storage.raw_storage = snapshot.raw_storage;
        storage.value_read_cache = snapshot.value_read_cache;
        storage.factory_dep_cache = snapshot.factory_dep_cache;

        Ok(())
    }

    fn apply_block<T: AdvanceTime>(
        &mut self,
        time: &mut T,
        block: Block<TransactionVariant>,
        index: u32,
    ) {
        // archive current state before we produce new batch/blocks
        if let Err(err) = self.archive_state() {
            tracing::error!(
                "failed archiving state for block {}: {}",
                self.current_miniblock,
                err
            );
        }

        self.current_miniblock = self.current_miniblock.saturating_add(1);
        let expected_timestamp = time.advance_timestamp();

        let actual_l1_batch_number = block
            .l1_batch_number
            .expect("block must have a l1_batch_number");
        if actual_l1_batch_number.as_u32() != self.current_batch {
            panic!(
                "expected next block to have batch_number {}, got {}",
                self.current_batch,
                actual_l1_batch_number.as_u32()
            );
        }

        if block.number.as_u64() != self.current_miniblock {
            panic!(
                "expected next block to have miniblock {}, got {} | {index}",
                self.current_miniblock,
                block.number.as_u64()
            );
        }

        if block.timestamp.as_u64() != expected_timestamp {
            panic!(
                "expected next block to have timestamp {}, got {} | {index}",
                expected_timestamp,
                block.timestamp.as_u64()
            );
        }

        let block_hash = block.hash;
        self.current_miniblock_hash = block_hash;
        self.block_hashes.insert(block.number.as_u64(), block.hash);
        self.blocks.insert(block.hash, block);
        self.filters.notify_new_block(block_hash);
    }
}

/// Creates a restorable snapshot for the [InMemoryNodeInner]. The snapshot contains all the necessary
/// data required to restore the [InMemoryNodeInner] state to a previous point in time.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub(crate) current_batch: u32,
    pub(crate) current_miniblock: u64,
    pub(crate) current_miniblock_hash: H256,
    // Currently, the fee is static and the fee input provider is immutable during the test node life cycle,
    // but in the future, it may contain some mutable state.
    pub(crate) fee_input_provider: TestNodeFeeInputProvider,
    pub(crate) tx_results: HashMap<H256, TransactionResult>,
    pub(crate) blocks: HashMap<H256, Block<TransactionVariant>>,
    pub(crate) block_hashes: HashMap<u64, H256>,
    pub(crate) filters: EthFilters,
    pub(crate) impersonation_state: ImpersonationState,
    pub(crate) rich_accounts: HashSet<H160>,
    pub(crate) previous_states: IndexMap<H256, HashMap<StorageKey, StorageValue>>,
    pub(crate) raw_storage: InMemoryStorage,
    pub(crate) value_read_cache: HashMap<StorageKey, H256>,
    pub(crate) factory_dep_cache: HashMap<H256, Option<Vec<u8>>>,
}

/// In-memory node, that can be used for local & unit testing.
/// It also supports the option of forking testnet/mainnet.
/// All contents are removed when object is destroyed.
#[derive(Clone)]
pub struct InMemoryNode<S: Clone> {
    /// A thread safe reference to the [InMemoryNodeInner].
    pub(crate) inner: Arc<RwLock<InMemoryNodeInner<S>>>,
    /// List of snapshots of the [InMemoryNodeInner]. This is bounded at runtime by [MAX_SNAPSHOTS].
    pub(crate) snapshots: Arc<RwLock<Vec<Snapshot>>>,
    /// Configuration option that survives reset.
    #[allow(dead_code)]
    pub(crate) system_contracts_options: system_contracts::Options,
    pub(crate) time: TimestampManager,
    pub(crate) impersonation: ImpersonationManager,
    /// An optional handle to the observability stack
    pub(crate) observability: Option<Observability>,
    pub(crate) pool: TxPool,
    pub(crate) sealer: BlockSealer,
}

fn contract_address_from_tx_result(execution_result: &VmExecutionResultAndLogs) -> Option<H160> {
    for query in execution_result.logs.storage_logs.iter().rev() {
        if query.log.is_write() && query.log.key.address() == &ACCOUNT_CODE_STORAGE_ADDRESS {
            return Some(h256_to_account_address(query.log.key.key()));
        }
    }
    None
}

impl<S: ForkSource + std::fmt::Debug + Clone> Default for InMemoryNode<S> {
    fn default() -> Self {
        let impersonation = ImpersonationManager::default();
        InMemoryNode::new(
            None,
            None,
            &TestNodeConfig::default(),
            TimestampManager::default(),
            impersonation.clone(),
            TxPool::new(impersonation),
            BlockSealer::default(),
        )
    }
}

impl<S: ForkSource + std::fmt::Debug + Clone> InMemoryNode<S> {
    pub fn new(
        fork: Option<ForkDetails>,
        observability: Option<Observability>,
        config: &TestNodeConfig,
        time: TimestampManager,
        impersonation: ImpersonationManager,
        pool: TxPool,
        sealer: BlockSealer,
    ) -> Self {
        let system_contracts_options = config.system_contracts_options;
        let inner = InMemoryNodeInner::new(fork, config, &time, impersonation.clone());
        InMemoryNode {
            inner: Arc::new(RwLock::new(inner)),
            snapshots: Default::default(),
            system_contracts_options,
            time,
            impersonation,
            observability,
            pool,
            sealer,
        }
    }

    // Common pattern in tests
    // TODO: Refactor InMemoryNode with a builder pattern
    pub fn default_fork(fork: Option<ForkDetails>) -> Self {
        let impersonation = ImpersonationManager::default();
        Self::new(
            fork,
            None,
            &Default::default(),
            TimestampManager::default(),
            impersonation.clone(),
            TxPool::new(impersonation),
            BlockSealer::default(),
        )
    }

    pub fn get_inner(&self) -> Arc<RwLock<InMemoryNodeInner<S>>> {
        self.inner.clone()
    }

    pub fn get_cache_config(&self) -> Result<CacheConfig, String> {
        let inner = self
            .inner
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        inner.fork_storage.get_cache_config()
    }

    pub fn get_fork_url(&self) -> Result<String, String> {
        let inner = self
            .inner
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        inner.fork_storage.get_fork_url()
    }

    fn get_config(&self) -> Result<TestNodeConfig, String> {
        let inner = self
            .inner
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

        Ok(inner.config.clone())
    }

    pub fn reset(&self, fork: Option<ForkDetails>) -> Result<(), String> {
        let config = self.get_config()?;
        let inner = InMemoryNodeInner::new(fork, &config, &self.time, self.impersonation.clone());

        let mut writer = self
            .snapshots
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        writer.clear();

        {
            let mut guard = self
                .inner
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            *guard = inner;
        }

        for wallet in LEGACY_RICH_WALLETS.iter() {
            let address = wallet.0;
            self.set_rich_account(
                H160::from_str(address).unwrap(),
                U256::from(100u128 * 10u128.pow(18)),
            );
        }
        for wallet in RICH_WALLETS.iter() {
            let address = wallet.0;
            self.set_rich_account(
                H160::from_str(address).unwrap(),
                U256::from(100u128 * 10u128.pow(18)),
            );
        }
        Ok(())
    }

    /// Applies multiple transactions - but still one per L1 batch.
    pub fn apply_txs(&self, txs: Vec<L2Tx>) -> anyhow::Result<()> {
        tracing::info!("Running {:?} transactions (one per batch)", txs.len());

        // Lock time so that the produced blocks are guaranteed to be sequential in time.
        let mut time = self.time.lock();
        for tx in txs {
            // Getting contracts is reasonably cheap, so we don't cache them. We may need differing contracts
            // depending on whether impersonation should be enabled for a transaction.
            let system_contracts = self.system_contracts_for_tx(tx.initiator_account())?;
            self.seal_block(&mut time, vec![tx], system_contracts)?;
        }

        Ok(())
    }

    /// Adds a lot of tokens to a given account with a specified balance.
    pub fn set_rich_account(&self, address: H160, balance: U256) {
        let key = storage_key_for_eth_balance(&address);

        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::info!("Failed to acquire write lock: {}", e);
                return;
            }
        };

        let keys = {
            let mut storage_view = StorageView::new(&inner.fork_storage);
            // Set balance to the specified amount
            storage_view.set_value(key, u256_to_h256(balance));
            storage_view.modified_storage_keys().clone()
        };

        for (key, value) in keys.iter() {
            inner.fork_storage.set_value(*key, *value);
        }
        inner.rich_accounts.insert(address);
    }

    pub fn system_contracts_for_l2_call(&self) -> anyhow::Result<BaseSystemContracts> {
        let inner = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock"))?;
        Ok(inner.system_contracts.contracts_for_l2_call().clone())
    }

    pub fn system_contracts_for_tx(
        &self,
        tx_initiator: Address,
    ) -> anyhow::Result<BaseSystemContracts> {
        let inner = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock"))?;
        Ok(if inner.impersonation.is_impersonating(&tx_initiator) {
            tracing::info!("🕵️ Executing tx from impersonated account {tx_initiator:?}");
            inner
                .system_contracts
                .contracts(TxExecutionMode::VerifyExecute, true)
                .clone()
        } else {
            inner
                .system_contracts
                .contracts(TxExecutionMode::VerifyExecute, false)
                .clone()
        })
    }

    /// Runs L2 'eth call' method - that doesn't commit to a block.
    pub fn run_l2_call(
        &self,
        mut l2_tx: L2Tx,
        base_contracts: BaseSystemContracts,
    ) -> anyhow::Result<ExecutionResult> {
        let execution_mode = TxExecutionMode::EthCall;

        let inner = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;

        let storage = StorageView::new(&inner.fork_storage).into_rc_ptr();

        // init vm

        let (batch_env, _) = inner.create_l1_batch_env(&self.time, storage.clone());
        let system_env = inner.create_system_env(base_contracts, execution_mode);

        let mut vm: Vm<_, HistoryDisabled> = Vm::new(batch_env, system_env, storage.clone());

        // We must inject *some* signature (otherwise bootloader code fails to generate hash).
        if l2_tx.common_data.signature.is_empty() {
            l2_tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        let tx: Transaction = l2_tx.into();
        vm.push_transaction(tx);

        let call_tracer_result = Arc::new(OnceCell::default());

        let tracers = vec![
            CallErrorTracer::new().into_tracer_pointer(),
            CallTracer::new(call_tracer_result.clone()).into_tracer_pointer(),
        ];
        let tx_result = vm.inspect(&mut tracers.into(), InspectExecutionMode::OneTx);

        let call_traces = Arc::try_unwrap(call_tracer_result)
            .unwrap()
            .take()
            .unwrap_or_default();

        if inner.config.show_tx_summary {
            match &tx_result.result {
                ExecutionResult::Success { output } => {
                    tracing::info!("Call: {}", "SUCCESS".green());
                    let output_bytes = zksync_types::web3::Bytes::from(output.clone());
                    tracing::info!("Output: {}", serde_json::to_string(&output_bytes).unwrap());
                }
                ExecutionResult::Revert { output } => {
                    tracing::info!("Call: {}: {}", "FAILED".red(), output);
                }
                ExecutionResult::Halt { reason } => {
                    tracing::info!("Call: {} {}", "HALTED".red(), reason)
                }
            };
        }

        if !inner.config.disable_console_log {
            tracing::info!("");
            tracing::info!("=== Console Logs: ");
            for call in &call_traces {
                inner.console_log_handler.handle_call_recursive(call);
            }
        }

        if inner.config.show_calls != ShowCalls::None {
            tracing::info!("");
            tracing::info!("=== Call traces:");
            for call in &call_traces {
                formatter::print_call(
                    call,
                    0,
                    &inner.config.show_calls,
                    inner.config.show_outputs,
                    inner.config.resolve_hashes,
                );
            }
        }

        Ok(tx_result.result)
    }

    fn display_detailed_gas_info(
        &self,
        bootloader_debug_result: Option<&eyre::Result<BootloaderDebug, String>>,
        spent_on_pubdata: u64,
    ) -> eyre::Result<(), String> {
        if let Some(bootloader_result) = bootloader_debug_result {
            let bootloader_debug = bootloader_result.clone()?;

            tracing::info!("┌─────────────────────────┐");
            tracing::info!("│       GAS DETAILS       │");
            tracing::info!("└─────────────────────────┘");

            // Total amount of gas (should match tx.gas_limit).
            let total_gas_limit = bootloader_debug
                .total_gas_limit_from_user
                .saturating_sub(bootloader_debug.reserved_gas);

            let intrinsic_gas = total_gas_limit - bootloader_debug.gas_limit_after_intrinsic;
            let gas_for_validation =
                bootloader_debug.gas_limit_after_intrinsic - bootloader_debug.gas_after_validation;

            let gas_spent_on_compute = bootloader_debug.gas_spent_on_execution
                - bootloader_debug.gas_spent_on_bytecode_preparation;

            let gas_used = intrinsic_gas
                + gas_for_validation
                + bootloader_debug.gas_spent_on_bytecode_preparation
                + gas_spent_on_compute;

            tracing::info!(
                "Gas - Limit: {} | Used: {} | Refunded: {}",
                to_human_size(total_gas_limit),
                to_human_size(gas_used),
                to_human_size(bootloader_debug.refund_by_operator)
            );

            if bootloader_debug.total_gas_limit_from_user != total_gas_limit {
                tracing::info!(
                    "{}",
                    format!(
                "  WARNING: user actually provided more gas {}, but system had a lower max limit.",
                to_human_size(bootloader_debug.total_gas_limit_from_user)
            )
                    .yellow()
                );
            }
            if bootloader_debug.refund_computed != bootloader_debug.refund_by_operator {
                tracing::info!(
                    "{}",
                    format!(
                        "  WARNING: Refund by VM: {}, but operator refunded more: {}",
                        to_human_size(bootloader_debug.refund_computed),
                        to_human_size(bootloader_debug.refund_by_operator)
                    )
                    .yellow()
                );
            }

            if bootloader_debug.refund_computed + gas_used != total_gas_limit {
                tracing::info!(
                    "{}",
                    format!(
                        "  WARNING: Gas totals don't match. {} != {} , delta: {}",
                        to_human_size(bootloader_debug.refund_computed + gas_used),
                        to_human_size(total_gas_limit),
                        to_human_size(
                            total_gas_limit.abs_diff(bootloader_debug.refund_computed + gas_used)
                        )
                    )
                    .yellow()
                );
            }

            let bytes_published = spent_on_pubdata / bootloader_debug.gas_per_pubdata.as_u64();

            tracing::info!(
                "During execution published {} bytes to L1, @{} each - in total {} gas",
                to_human_size(bytes_published.into()),
                to_human_size(bootloader_debug.gas_per_pubdata),
                to_human_size(spent_on_pubdata.into())
            );

            tracing::info!("Out of {} gas used, we spent:", to_human_size(gas_used));
            tracing::info!(
                "  {:>15} gas ({:>2}%) for transaction setup",
                to_human_size(intrinsic_gas),
                to_human_size(intrinsic_gas * 100 / gas_used)
            );
            tracing::info!(
                "  {:>15} gas ({:>2}%) for bytecode preparation (decompression etc)",
                to_human_size(bootloader_debug.gas_spent_on_bytecode_preparation),
                to_human_size(bootloader_debug.gas_spent_on_bytecode_preparation * 100 / gas_used)
            );
            tracing::info!(
                "  {:>15} gas ({:>2}%) for account validation",
                to_human_size(gas_for_validation),
                to_human_size(gas_for_validation * 100 / gas_used)
            );
            tracing::info!(
                "  {:>15} gas ({:>2}%) for computations (opcodes)",
                to_human_size(gas_spent_on_compute),
                to_human_size(gas_spent_on_compute * 100 / gas_used)
            );

            tracing::info!("");
            tracing::info!("");
            tracing::info!(
                "{}",
                "=== Transaction setup cost breakdown ===".to_owned().bold(),
            );

            tracing::info!("Total cost: {}", to_human_size(intrinsic_gas).bold());
            tracing::info!(
                "  {:>15} gas ({:>2}%) fixed cost",
                to_human_size(bootloader_debug.intrinsic_overhead),
                to_human_size(bootloader_debug.intrinsic_overhead * 100 / intrinsic_gas)
            );
            tracing::info!(
                "  {:>15} gas ({:>2}%) operator cost",
                to_human_size(bootloader_debug.operator_overhead),
                to_human_size(bootloader_debug.operator_overhead * 100 / intrinsic_gas)
            );

            tracing::info!("");
            tracing::info!(
                "  FYI: operator could have charged up to: {}, so you got {}% discount",
                to_human_size(bootloader_debug.required_overhead),
                to_human_size(
                    (bootloader_debug.required_overhead - bootloader_debug.operator_overhead) * 100
                        / bootloader_debug.required_overhead
                )
            );

            {
                let fee_model_config = self
                    .inner
                    .read()
                    .expect("Failed to acquire reading lock")
                    .fee_input_provider
                    .get_fee_model_config();
                tracing::info!(
                    "Publishing full block costs the operator around {} l2 gas",
                    to_human_size(
                        bootloader_debug.gas_per_pubdata * fee_model_config.batch_overhead_l1_gas
                    ),
                );
            }
            tracing::info!("Your transaction has contributed to filling up the block in the following way (we take the max contribution as the cost):");
            tracing::info!(
                "  Length overhead:  {:>15}",
                to_human_size(bootloader_debug.overhead_for_length)
            );
            tracing::info!(
                "  Slot overhead:    {:>15}",
                to_human_size(bootloader_debug.overhead_for_slot)
            );
            tracing::info!("Also, with every spent gas unit you potentially can pay some additional amount of gas for filling up the block by execution limits");
            tracing::info!(
                "This overhead is included in the gas price, although now it's set to zero"
            );
            tracing::info!("And with every pubdata byte, you potentially can pay an additional amount of gas for filling up the block by pubdata limit");
            tracing::info!("This overhead is included in the `gas_per_pubdata` price");
            Ok(())
        } else {
            Err("Booloader tracer didn't finish.".to_owned())
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

        let l2_gas_price = self
            .inner
            .read()
            .expect("failed acquiring reader")
            .fee_input_provider
            .gas_price();
        if tx.common_data.fee.max_fee_per_gas < l2_gas_price.into() {
            tracing::info!(
                "Submitted Tx is Unexecutable {:?} because of MaxFeePerGasTooLow {}",
                tx.hash(),
                tx.common_data.fee.max_fee_per_gas
            );
            anyhow::bail!("block base fee higher than max fee per gas");
        }

        if tx.common_data.fee.max_fee_per_gas < tx.common_data.fee.max_priority_fee_per_gas {
            tracing::info!(
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
    pub fn run_l2_tx_raw<W: WriteStorage, H: HistoryMode>(
        &self,
        l2_tx: L2Tx,
        vm: &mut Vm<W, H>,
    ) -> anyhow::Result<TxExecutionOutput> {
        let inner = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock"))?;

        let tx: Transaction = l2_tx.into();

        let call_tracer_result = Arc::new(OnceCell::default());
        let bootloader_debug_result = Arc::new(OnceCell::default());

        let tracers = vec![
            CallErrorTracer::new().into_tracer_pointer(),
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

        let call_traces = call_tracer_result.get().unwrap();

        let spent_on_pubdata =
            tx_result.statistics.gas_used - tx_result.statistics.computational_gas_used as u64;

        if inner.config.show_tx_summary {
            tracing::info!("┌─────────────────────────┐");
            tracing::info!("│   TRANSACTION SUMMARY   │");
            tracing::info!("└─────────────────────────┘");

            match &tx_result.result {
                ExecutionResult::Success { .. } => {
                    tracing::info!("Transaction: {}", "SUCCESS".green())
                }
                ExecutionResult::Revert { .. } => tracing::info!("Transaction: {}", "FAILED".red()),
                ExecutionResult::Halt { .. } => tracing::info!("Transaction: {}", "HALTED".red()),
            }

            tracing::info!("Initiator: {:?}", tx.initiator_account());
            tracing::info!("Payer: {:?}", tx.payer());
            tracing::info!(
                "Gas - Limit: {} | Used: {} | Refunded: {}",
                to_human_size(tx.gas_limit()),
                to_human_size(tx.gas_limit() - tx_result.refunds.gas_refunded),
                to_human_size(tx_result.refunds.gas_refunded.into())
            );
        }

        if inner.config.show_gas_details != ShowGasDetails::None {
            let info =
                self.display_detailed_gas_info(bootloader_debug_result.get(), spent_on_pubdata);
            if info.is_err() {
                tracing::info!(
                    "{}\nError: {}",
                    "!!! FAILED TO GET DETAILED GAS INFO !!!".to_owned().red(),
                    info.unwrap_err()
                );
            }
        }

        if inner.config.show_storage_logs != ShowStorageLogs::None {
            print_storage_logs_details(&inner.config.show_storage_logs, &tx_result);
        }

        if inner.config.show_vm_details != ShowVMDetails::None {
            formatter::print_vm_details(&tx_result);
        }

        if !inner.config.disable_console_log {
            tracing::info!("");
            tracing::info!("==== Console logs: ");

            for call in call_traces {
                inner.console_log_handler.handle_call_recursive(call);
            }
        }

        if inner.config.show_calls != ShowCalls::None {
            tracing::info!("");
            let call_traces_count = if !call_traces.is_empty() {
                // All calls/sub-calls are stored within the first call trace
                call_traces[0].calls.len()
            } else {
                0
            };
            tracing::info!(
                "==== {}",
                format!("{:?} call traces. ", call_traces_count).bold()
            );

            for call in call_traces {
                formatter::print_call(
                    call,
                    0,
                    &inner.config.show_calls,
                    inner.config.show_outputs,
                    inner.config.resolve_hashes,
                );
            }
        }

        if inner.config.show_event_logs {
            tracing::info!("");
            tracing::info!(
                "==== {}",
                format!("{} events", tx_result.logs.events.len()).bold()
            );
            for event in &tx_result.logs.events {
                formatter::print_event(event, inner.config.resolve_hashes);
            }
        }

        let mut bytecodes = HashMap::new();
        for b in &*compressed_bytecodes {
            let (hash, bytecode) = bytecode_to_factory_dep(b.original.clone()).map_err(|err| {
                tracing::error!("{}", format!("cannot convert bytecode: {err}").on_red());
                err
            })?;
            bytecodes.insert(hash, bytecode);
        }

        Ok(TxExecutionOutput {
            result: tx_result,
            call_traces: call_traces.clone(),
            bytecodes,
        })
    }

    /// Runs L2 transaction and commits it to a new block.
    pub fn run_l2_tx<W: WriteStorage, H: HistoryMode>(
        &self,
        l2_tx: L2Tx,
        block_ctx: &BlockContext,
        batch_env: &L1BatchEnv,
        vm: &mut Vm<W, H>,
    ) -> anyhow::Result<()> {
        let tx_hash = l2_tx.hash();
        let transaction_type = l2_tx.common_data.transaction_type;

        let show_tx_summary = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock"))?
            .config
            .show_tx_summary;

        if show_tx_summary {
            tracing::info!("");
            tracing::info!("Validating {}", format!("{:?}", tx_hash).bold());
        }

        self.validate_tx(&l2_tx)?;

        if show_tx_summary {
            tracing::info!("Executing {}", format!("{:?}", tx_hash).bold());
        }

        self.inner
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?
            .filters
            .notify_new_pending_transaction(tx_hash);

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
        let mut inner = self
            .inner
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        for (hash, code) in bytecodes.iter() {
            inner.fork_storage.store_factory_dep(
                u256_to_h256(*hash),
                code.iter()
                    .flat_map(|entry| {
                        let mut bytes = vec![0u8; 32];
                        entry.to_big_endian(&mut bytes);
                        bytes.to_vec()
                    })
                    .collect(),
            )
        }

        let logs = result
            .logs
            .events
            .iter()
            .enumerate()
            .map(|(log_idx, log)| Log {
                address: log.address,
                topics: log.indexed_topics.clone(),
                data: Bytes(log.value.clone()),
                block_hash: Some(block_ctx.hash),
                block_number: Some(block_ctx.miniblock.into()),
                l1_batch_number: Some(U64::from(batch_env.number.0)),
                transaction_hash: Some(tx_hash),
                transaction_index: Some(U64::zero()),
                log_index: Some(U256::from(log_idx)),
                transaction_log_index: Some(U256::from(log_idx)),
                log_type: None,
                removed: Some(false),
                block_timestamp: Some(block_ctx.timestamp.into()),
            })
            .collect();
        for log in &logs {
            inner
                .filters
                .notify_new_log(log, block_ctx.miniblock.into());
        }
        let tx_receipt = TransactionReceipt {
            transaction_hash: tx_hash,
            transaction_index: U64::from(0),
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
            effective_gas_price: Some(inner.fee_input_provider.gas_price().into()),
            transaction_type: Some((transaction_type as u32).into()),
            logs_bloom: Default::default(),
        };
        let debug = create_debug_output(&l2_tx, &result, call_traces).expect("create debug output"); // OK to unwrap here as Halt is handled above
        inner.tx_results.insert(
            tx_hash,
            TransactionResult {
                info: TxExecutionInfo {
                    tx: l2_tx,
                    batch_number: batch_env.number.0,
                    miniblock_number: block_ctx.miniblock,
                    result,
                },
                receipt: tx_receipt,
                debug,
            },
        );

        Ok(())
    }

    // Requirement for `TimeExclusive` ensures that we have exclusive writeable access to time
    // manager. Meaning we can construct blocks and apply them without worrying about TOCTOU with
    // timestamps.
    pub fn seal_block<T: AdvanceTime>(
        &self,
        time: &mut T,
        txs: Vec<L2Tx>,
        system_contracts: BaseSystemContracts,
    ) -> anyhow::Result<L2BlockNumber> {
        // Prepare a new block context and a new batch env
        let inner = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock"))?;
        let storage = StorageView::new(inner.fork_storage.clone()).into_rc_ptr();
        let system_env = inner.create_system_env(system_contracts, TxExecutionMode::VerifyExecute);
        let (batch_env, mut block_ctx) = inner.create_l1_batch_env(time, storage.clone());
        drop(inner);

        let mut vm: Vm<_, HistoryEnabled> = Vm::new(batch_env.clone(), system_env, storage.clone());

        // Compute block hash. Note that the computed block hash here will be different than that in production.
        let tx_hashes = txs.iter().map(|t| t.hash()).collect::<Vec<_>>();
        let hash = compute_hash(block_ctx.miniblock, &tx_hashes);
        block_ctx.hash = hash;

        // Execute transactions and bootloader
        for tx in txs {
            // Executing a next transaction means that a previous transaction was either rolled back (in which case its snapshot
            // was already removed), or that we build on top of it (in which case, it can be removed now).
            vm.pop_snapshot_no_rollback();
            // Save pre-execution VM snapshot.
            vm.make_snapshot();
            if let Err(e) = self.run_l2_tx(tx, &block_ctx, &batch_env, &mut vm) {
                tracing::error!("Error while executing transaction: {e}");
                vm.rollback_to_the_latest_snapshot();
            }
        }
        vm.execute(InspectExecutionMode::Bootloader);

        // Write all the mutated keys (storage slots).
        let mut inner = self
            .inner
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        for (key, value) in storage.borrow().modified_storage_keys() {
            inner.fork_storage.set_value(*key, *value);
        }

        let mut transactions = Vec::new();
        let mut tx_results = Vec::new();
        for tx_hash in &tx_hashes {
            let Some(tx_result) = inner.tx_results.get(tx_hash) else {
                // Skipping halted transaction
                continue;
            };
            tx_results.push(&tx_result.info.result);

            let mut transaction = zksync_types::api::Transaction::from(tx_result.info.tx.clone());
            transaction.block_hash = Some(block_ctx.hash);
            transaction.block_number = Some(U64::from(block_ctx.miniblock));
            transaction.transaction_index = Some(Index::zero());
            transaction.l1_batch_number = Some(U64::from(batch_env.number.0));
            transaction.l1_batch_tx_index = Some(Index::zero());
            transactions.push(TransactionVariant::Full(transaction));
        }

        // Build bloom hash
        let iter = tx_results
            .iter()
            .flat_map(|r| r.logs.events.iter())
            .flat_map(|event| {
                event
                    .indexed_topics
                    .iter()
                    .map(|topic| BloomInput::Raw(topic.as_bytes()))
                    .chain([BloomInput::Raw(event.address.as_bytes())])
            });
        let logs_bloom = build_bloom(iter);

        // Calculate how much gas was used across all txs
        let gas_used = tx_results
            .iter()
            .map(|r| U256::from(r.statistics.gas_used))
            .fold(U256::zero(), |acc, x| acc + x);

        // Construct the block
        let parent_block_hash = inner
            .block_hashes
            .get(&(block_ctx.miniblock - 1))
            .cloned()
            .unwrap_or_default();
        let block = create_block(
            &batch_env,
            hash,
            parent_block_hash,
            block_ctx.miniblock,
            block_ctx.timestamp,
            transactions,
            gas_used,
            logs_bloom,
        );
        inner.current_batch = inner.current_batch.saturating_add(1);
        inner.apply_block(time, block, 0);

        // Hack to ensure we don't mine twice the amount of requested empty blocks (i.e. one per
        // batch).
        // TODO: Remove once we separate batch sealing from block sealing
        if !tx_hashes.is_empty() {
            // With the introduction of 'l2 blocks' (and virtual blocks),
            // we are adding one l2 block at the end of each batch (to handle things like remaining events etc).
            // You can look at insert_fictive_l2_block function in VM to see how this fake block is inserted.
            let parent_block_hash = block_ctx.hash;
            let block_ctx = block_ctx.new_block(time);
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
            inner.apply_block(time, virtual_block, 1);
        }

        Ok(L2BlockNumber(block_ctx.miniblock as u32))
    }

    // Forcefully stores the given bytecode at a given account.
    pub fn override_bytecode(&self, address: &Address, bytecode: &[u8]) -> Result<(), String> {
        let mut inner = self
            .inner
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        let code_key = get_code_key(address);

        let bytecode_hash = hash_bytecode(bytecode);

        inner
            .fork_storage
            .store_factory_dep(bytecode_hash, bytecode.to_owned());

        inner.fork_storage.set_value(code_key, bytecode_hash);

        Ok(())
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
}

impl BlockContext {
    /// Create the next batch instance that uses the same batch number, and has all other parameters incremented by `1`.
    pub fn new_block<T: ReadTime>(&self, time: &T) -> BlockContext {
        Self {
            hash: H256::zero(),
            batch: self.batch,
            miniblock: self.miniblock.saturating_add(1),
            timestamp: time.peek_next_timestamp(),
        }
    }
}

pub fn load_last_l1_batch<S: ReadStorage>(storage: StoragePtr<S>) -> Option<(u64, u64)> {
    // Get block number and timestamp
    let current_l1_batch_info_key = StorageKey::new(
        AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
        SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
    );
    let mut storage_ptr = storage.borrow_mut();
    let current_l1_batch_info = storage_ptr.read_value(&current_l1_batch_info_key);
    let (batch_number, batch_timestamp) = unpack_block_info(h256_to_u256(current_l1_batch_info));
    let block_number = batch_number as u32;
    if block_number == 0 {
        // The block does not exist yet
        return None;
    }
    Some((batch_number, batch_timestamp))
}

#[cfg(test)]
mod tests {
    use ethabi::{Token, Uint};
    use std::fmt::Debug;
    use zksync_types::{utils::deployed_address_create, K256PrivateKey, Nonce};

    use super::*;
    use crate::{
        config::{
            constants::{
                DEFAULT_ACCOUNT_BALANCE, DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
                DEFAULT_ESTIMATE_GAS_SCALE_FACTOR, DEFAULT_FAIR_PUBDATA_PRICE,
                DEFAULT_L2_GAS_PRICE, TEST_NODE_NETWORK_ID,
            },
            TestNodeConfig,
        },
        http_fork_source::HttpForkSource,
        node::InMemoryNode,
        system_contracts::Options,
        testing,
    };

    fn test_vm<S: Clone + Debug + ForkSource>(
        node: &InMemoryNode<S>,
        system_contracts: BaseSystemContracts,
    ) -> (
        BlockContext,
        L1BatchEnv,
        Vm<StorageView<ForkStorage<S>>, HistoryDisabled>,
    ) {
        let inner = node.inner.read().unwrap();
        let storage = StorageView::new(inner.fork_storage.clone()).into_rc_ptr();
        let system_env = inner.create_system_env(system_contracts, TxExecutionMode::VerifyExecute);
        let (batch_env, block_ctx) = inner.create_l1_batch_env(&node.time, storage.clone());
        let vm: Vm<_, HistoryDisabled> = Vm::new(batch_env.clone(), system_env, storage);

        (block_ctx, batch_env, vm)
    }

    #[tokio::test]
    async fn test_run_l2_tx_validates_tx_gas_limit_too_high() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let tx = testing::TransactionBuilder::new()
            .set_gas_limit(U256::from(u64::MAX) + 1)
            .build();
        node.set_rich_account(
            tx.common_data.initiator_address,
            U256::from(100u128 * 10u128.pow(18)),
        );

        let system_contracts = node
            .system_contracts_for_tx(tx.initiator_account())
            .unwrap();
        let (block_ctx, batch_env, mut vm) = test_vm(&node, system_contracts.clone());
        let err = node
            .run_l2_tx(tx, &block_ctx, &batch_env, &mut vm)
            .unwrap_err();
        assert_eq!(err.to_string(), "exceeds block gas limit");
    }

    #[tokio::test]
    async fn test_run_l2_tx_validates_tx_max_fee_per_gas_too_low() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let tx = testing::TransactionBuilder::new()
            .set_max_fee_per_gas(U256::from(DEFAULT_L2_GAS_PRICE - 1))
            .build();
        node.set_rich_account(
            tx.common_data.initiator_address,
            U256::from(100u128 * 10u128.pow(18)),
        );

        let system_contracts = node
            .system_contracts_for_tx(tx.initiator_account())
            .unwrap();
        let (block_ctx, batch_env, mut vm) = test_vm(&node, system_contracts.clone());
        let err = node
            .run_l2_tx(tx, &block_ctx, &batch_env, &mut vm)
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "block base fee higher than max fee per gas"
        );
    }

    #[tokio::test]
    async fn test_run_l2_tx_validates_tx_max_priority_fee_per_gas_higher_than_max_fee_per_gas() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let tx = testing::TransactionBuilder::new()
            .set_max_priority_fee_per_gas(U256::from(250_000_000 + 1))
            .build();
        node.set_rich_account(
            tx.common_data.initiator_address,
            U256::from(100u128 * 10u128.pow(18)),
        );

        let system_contracts = node
            .system_contracts_for_tx(tx.initiator_account())
            .unwrap();
        let (block_ctx, batch_env, mut vm) = test_vm(&node, system_contracts.clone());
        let err = node
            .run_l2_tx(tx, &block_ctx, &batch_env, &mut vm)
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
    async fn test_run_l2_tx_raw_does_not_panic_on_external_storage_call() {
        // Perform a transaction to get storage to an intermediate state
        let node = InMemoryNode::<HttpForkSource>::default();
        let tx = testing::TransactionBuilder::new().build();
        node.set_rich_account(
            tx.common_data.initiator_address,
            U256::from(100u128 * 10u128.pow(18)),
        );
        let system_contracts = node
            .system_contracts_for_tx(tx.initiator_account())
            .unwrap();
        node.seal_block(&mut node.time.lock(), vec![tx], system_contracts)
            .unwrap();
        let external_storage = node.inner.read().unwrap().fork_storage.clone();

        // Execute next transaction using a fresh in-memory node and the external fork storage
        let mock_db = testing::ExternalStorage {
            raw_storage: external_storage.inner.read().unwrap().raw_storage.clone(),
        };
        let impersonation = ImpersonationManager::default();
        let node: InMemoryNode<testing::ExternalStorage> = InMemoryNode::new(
            Some(ForkDetails {
                fork_source: Box::new(mock_db),
                chain_id: TEST_NODE_NETWORK_ID.into(),
                l1_block: L1BatchNumber(1),
                l2_block: Block::default(),
                l2_miniblock: 2,
                l2_miniblock_hash: Default::default(),
                block_timestamp: 1002,
                overwrite_chain_id: None,
                l1_gas_price: 1000,
                l2_fair_gas_price: DEFAULT_L2_GAS_PRICE,
                fair_pubdata_price: DEFAULT_FAIR_PUBDATA_PRICE,
                fee_params: None,
                estimate_gas_price_scale_factor: DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
                estimate_gas_scale_factor: DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
                cache_config: CacheConfig::default(),
            }),
            None,
            &Default::default(),
            TimestampManager::default(),
            impersonation.clone(),
            TxPool::new(impersonation),
            BlockSealer::default(),
        );

        let tx = testing::TransactionBuilder::new().build();
        let system_contracts = node
            .system_contracts_for_tx(tx.initiator_account())
            .unwrap();
        let (_, _, mut vm) = test_vm(&node, system_contracts);
        node.run_l2_tx_raw(tx, &mut vm)
            .expect("transaction must pass with external storage");
    }

    #[tokio::test]
    async fn test_transact_returns_data_in_built_in_without_security_mode() {
        let impersonation = ImpersonationManager::default();
        let node = InMemoryNode::<HttpForkSource>::new(
            None,
            None,
            &TestNodeConfig {
                system_contracts_options: Options::BuiltInWithoutSecurity,
                ..Default::default()
            },
            TimestampManager::default(),
            impersonation.clone(),
            TxPool::new(impersonation),
            BlockSealer::default(),
        );

        let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xef)).unwrap();
        let from_account = private_key.address();
        node.set_rich_account(from_account, U256::from(DEFAULT_ACCOUNT_BALANCE));

        let deployed_address = deployed_address_create(from_account, U256::zero());
        testing::deploy_contract(
            &node,
            H256::repeat_byte(0x1),
            &private_key,
            hex::decode(testing::STORAGE_CONTRACT_BYTECODE).unwrap(),
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

        let system_contracts = node
            .system_contracts_for_tx(tx.initiator_account())
            .unwrap();
        let (_, _, mut vm) = test_vm(&node, system_contracts);
        let TxExecutionOutput { result, .. } = node.run_l2_tx_raw(tx, &mut vm).expect("failed tx");

        match result.result {
            ExecutionResult::Success { output } => {
                let actual = testing::decode_tx_result(&output, ethabi::ParamType::Uint(256));
                let expected = Token::Uint(Uint::from(1024u64));
                assert_eq!(expected, actual, "invalid result");
            }
            _ => panic!("invalid result {:?}", result.result),
        }
    }
}
