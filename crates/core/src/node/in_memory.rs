//! In-memory node, that supports forking other networks.
use super::inner::InMemoryNodeInner;
use super::inner::node_executor::NodeExecutorHandle;
use super::vm::AnvilVM;
use crate::delegate_vm;
use crate::deps::InMemoryStorage;
use crate::filters::EthFilters;
use crate::node::fee_model::TestNodeFeeInputProvider;
use crate::node::impersonate::{ImpersonationManager, ImpersonationState};
use crate::node::inner::blockchain::ReadBlockchain;
use crate::node::inner::storage::ReadStorageDyn;
use crate::node::inner::time::ReadTime;
use crate::node::sealer::BlockSealerState;
use crate::node::state::VersionedState;
use crate::node::state_override::apply_state_override;
use crate::node::traces::call_error::CallErrorTracer;
use crate::node::traces::decoder::CallTraceDecoderBuilder;
use crate::node::{BlockSealer, BlockSealerMode, NodeExecutor, TxBatch, TxPool};
use crate::observability::Observability;
use crate::system_contracts::SystemContracts;
use anvil_zksync_common::cache::CacheConfig;
use anvil_zksync_common::sh_println;
use anvil_zksync_common::shell::get_shell;
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_config::constants::{NON_FORK_FIRST_BLOCK_TIMESTAMP, TEST_NODE_NETWORK_ID};
use anvil_zksync_config::types::Genesis;
use anvil_zksync_traces::{
    build_call_trace_arena, decode_trace_arena, filter_call_trace_arena,
    identifier::SignaturesIdentifier, render_trace_arena_inner,
};
use anvil_zksync_types::{
    LogLevel, ShowGasDetails, ShowStorageLogs, ShowVMDetails, traces::CallTraceArena,
};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use zksync_contracts::{BaseSystemContracts, BaseSystemContractsHashes};
use zksync_error::anvil_zksync::node::{
    AnvilNodeError, AnvilNodeResult, generic_error, to_generic,
};
use zksync_error::anvil_zksync::state::{StateLoaderError, StateLoaderResult};
use zksync_multivm::interface::VmFactory;
use zksync_multivm::interface::storage::{
    ReadStorage, StoragePtr, StorageView, StorageWithOverrides,
};
use zksync_multivm::interface::{
    ExecutionResult, InspectExecutionMode, L1BatchEnv, L2BlockEnv, TxExecutionMode, VmInterface,
};
use zksync_multivm::tracers::CallTracer;
use zksync_multivm::utils::{get_batch_base_fee, get_max_batch_gas_limit};
use zksync_multivm::vm_latest::Vm;
use zksync_types::api::state_override::StateOverride;

use crate::node::fork::{ForkClient, ForkSource};
use crate::node::keys::StorageKeyLayout;
use zksync_multivm::VmVersion;
use zksync_multivm::vm_latest::{HistoryDisabled, ToTracerPointer};
use zksync_types::api::{Block, DebugCall, TransactionReceipt, TransactionVariant};
use zksync_types::block::{L1BatchHeader, L2BlockHasher, unpack_block_info};
use zksync_types::fee_model::BatchFeeInput;
use zksync_types::l2::L2Tx;
use zksync_types::storage::{
    EMPTY_UNCLES_HASH, SYSTEM_CONTEXT_ADDRESS, SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
};
use zksync_types::web3::Bytes;
use zksync_types::{
    AccountTreeId, Address, Bloom, H64, H160, H256, L1BatchNumber, L2BlockNumber, L2ChainId,
    PackedEthSignature, ProtocolVersionId, StorageKey, StorageValue, Transaction, U64, U256,
    h256_to_u256,
};

/// Max possible size of an ABI encoded tx (in bytes).
/// NOTE: this deviates slightly from the default value in the main node config,
/// that being `api.max_tx_size = 1_000_000`.
pub const MAX_TX_SIZE: usize = 1_200_000;
/// Acceptable gas overestimation limit.
pub const ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION: u64 = 1_000;
/// The maximum number of previous blocks to store the state for.
pub const MAX_PREVIOUS_STATES: u16 = 128;
/// The zks protocol version.
pub const PROTOCOL_VERSION: &str = "zks/1";

pub fn compute_hash<'a>(
    protocol_version: ProtocolVersionId,
    number: L2BlockNumber,
    timestamp: u64,
    prev_l2_block_hash: H256,
    tx_hashes: impl IntoIterator<Item = &'a H256>,
) -> H256 {
    let mut block_hasher = L2BlockHasher::new(number, timestamp, prev_l2_block_hash);
    for tx_hash in tx_hashes.into_iter() {
        block_hasher.push_tx_hash(*tx_hash);
    }
    block_hasher.finalize(protocol_version)
}

pub fn create_genesis_from_json(
    protocol_version: ProtocolVersionId,
    genesis: &Genesis,
    timestamp: Option<u64>,
) -> (Block<TransactionVariant>, L1BatchHeader) {
    let hash = L2BlockHasher::legacy_hash(L2BlockNumber(0));
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
            interop_roots: vec![],
        },
    });

    let genesis_block = create_block(
        &l1_batch_env,
        hash,
        genesis.parent_hash.unwrap_or_else(H256::zero),
        genesis.block_number.unwrap_or(0),
        timestamp,
        genesis.transactions.clone().unwrap_or_default(),
        genesis.gas_used.unwrap_or_else(U256::zero),
        genesis.logs_bloom.unwrap_or_else(Bloom::zero),
    );
    let genesis_batch_header = L1BatchHeader::new(
        L1BatchNumber(0),
        timestamp,
        BaseSystemContractsHashes::default(),
        protocol_version,
    );

    (genesis_block, genesis_batch_header)
}

pub fn create_genesis<TX>(
    protocol_version: ProtocolVersionId,
    timestamp: Option<u64>,
) -> (Block<TX>, L1BatchHeader) {
    let hash = L2BlockHasher::legacy_hash(L2BlockNumber(0));
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
            interop_roots: vec![],
        },
    };
    let genesis_block = create_block(
        &batch_env,
        hash,
        H256::zero(),
        0,
        timestamp,
        vec![],
        U256::zero(),
        Bloom::zero(),
    );
    let genesis_batch_header = L1BatchHeader::new(
        L1BatchNumber(0),
        timestamp,
        BaseSystemContractsHashes::default(),
        protocol_version,
    );

    (genesis_block, genesis_batch_header)
}

#[allow(clippy::too_many_arguments)]
pub fn create_block<TX>(
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxExecutionInfo {
    pub tx: Transaction,
    // Batch number where transaction was executed.
    pub batch_number: u32,
    pub miniblock_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub info: TxExecutionInfo,
    pub new_bytecodes: Vec<(H256, Vec<u8>)>,
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

/// Creates a restorable snapshot for the [InMemoryNodeInner]. The snapshot contains all the necessary
/// data required to restore the [InMemoryNodeInner] state to a previous point in time.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub(crate) current_batch: L1BatchNumber,
    pub(crate) current_block: L2BlockNumber,
    pub(crate) current_block_hash: H256,
    // Currently, the fee is static and the fee input provider is immutable during the test node life cycle,
    // but in the future, it may contain some mutable state.
    pub(crate) fee_input_provider: TestNodeFeeInputProvider,
    pub(crate) tx_results: HashMap<H256, TransactionResult>,
    pub(crate) blocks: HashMap<H256, Block<TransactionVariant>>,
    pub(crate) hashes: HashMap<L2BlockNumber, H256>,
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
pub struct InMemoryNode {
    /// A thread safe reference to the [InMemoryNodeInner].
    pub(crate) inner: Arc<RwLock<InMemoryNodeInner>>,
    pub(crate) blockchain: Box<dyn ReadBlockchain>,
    pub(crate) storage: Box<dyn ReadStorageDyn>,
    pub(crate) fork: Box<dyn ForkSource>,
    pub node_handle: NodeExecutorHandle,
    /// List of snapshots of the [InMemoryNodeInner]. This is bounded at runtime by [MAX_SNAPSHOTS].
    pub(crate) snapshots: Arc<RwLock<Vec<Snapshot>>>,
    pub(crate) time: Box<dyn ReadTime>,
    pub(crate) impersonation: ImpersonationManager,
    /// An optional handle to the observability stack
    pub(crate) observability: Option<Observability>,
    pub(crate) pool: TxPool,
    pub(crate) sealer_state: BlockSealerState,
    pub(crate) system_contracts: SystemContracts,
    pub(crate) storage_key_layout: StorageKeyLayout,
}

impl InMemoryNode {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        inner: Arc<RwLock<InMemoryNodeInner>>,
        blockchain: Box<dyn ReadBlockchain>,
        storage: Box<dyn ReadStorageDyn>,
        fork: Box<dyn ForkSource>,
        node_handle: NodeExecutorHandle,
        observability: Option<Observability>,
        time: Box<dyn ReadTime>,
        impersonation: ImpersonationManager,
        pool: TxPool,
        sealer_state: BlockSealerState,
        system_contracts: SystemContracts,
        storage_key_layout: StorageKeyLayout,
    ) -> Self {
        InMemoryNode {
            inner,
            blockchain,
            storage,
            fork,
            node_handle,
            snapshots: Default::default(),
            time,
            impersonation,
            observability,
            pool,
            sealer_state,
            system_contracts,
            storage_key_layout,
        }
    }

    /// Replays transactions consequently in a new block. All transactions are expected to be
    /// executable and will become a part of the resulting block.
    pub async fn replay_txs(&self, txs: Vec<Transaction>) -> AnvilNodeResult<()> {
        let tx_batch = TxBatch {
            impersonating: false,
            txs,
        };
        let expected_tx_hashes = tx_batch
            .txs
            .iter()
            .map(|tx| tx.hash())
            .collect::<HashSet<_>>();
        let block_number = self.node_handle.seal_block_sync(tx_batch).await?;
        // Fetch the block that was just sealed
        let block = self
            .blockchain
            .get_block_by_number(block_number)
            .await
            .expect("freshly sealed block could not be found in storage");

        // Calculate tx hash set from that block
        let actual_tx_hashes = block
            .transactions
            .iter()
            .map(|tx| match tx {
                TransactionVariant::Full(tx) => tx.hash,
                TransactionVariant::Hash(tx_hash) => *tx_hash,
            })
            .collect::<HashSet<_>>();

        // Calculate the difference between expected transaction hash set and the actual one.
        // If the difference is not empty it means some transactions were not executed (i.e.
        // were halted).
        let diff_tx_hashes = expected_tx_hashes
            .difference(&actual_tx_hashes)
            .collect::<Vec<_>>();
        if !diff_tx_hashes.is_empty() {
            return Err(generic_error!(
                "Failed to replay transactions: {diff_tx_hashes:?}. Please report this."
            ));
        }

        Ok(())
    }

    /// Adds a lot of tokens to a given account with a specified balance.
    pub async fn set_rich_account(&self, address: H160, balance: U256) {
        self.inner.write().await.set_rich_account(address, balance)
    }

    /// Runs L2 'eth call' method - that doesn't commit to a block.
    pub async fn run_l2_call(
        &self,
        mut l2_tx: L2Tx,
        base_contracts: BaseSystemContracts,
        state_override: Option<StateOverride>,
    ) -> AnvilNodeResult<ExecutionResult> {
        let execution_mode = TxExecutionMode::EthCall;

        let inner = self.inner.read().await;

        // init vm

        let (batch_env, _) = inner.create_l1_batch_env().await;
        let system_env = inner.create_system_env(base_contracts, execution_mode);

        let storage_override = if let Some(state_override) = state_override {
            apply_state_override(inner.read_storage(), state_override)
        } else {
            // Do not spawn a new thread in the most frequent case.
            StorageWithOverrides::new(inner.read_storage())
        };

        let storage = StorageView::new(storage_override).to_rc_ptr();

        let mut vm = if self.system_contracts.zksync_os.zksync_os {
            AnvilVM::ZKsyncOs(super::zksync_os::ZKsyncOsVM::<_, HistoryDisabled>::new(
                batch_env,
                system_env,
                storage,
                // TODO: this might be causing a deadlock.. check..
                &inner.fork_storage.inner.read().unwrap().raw_storage,
                &self.system_contracts.zksync_os,
            ))
        } else {
            AnvilVM::Era(Vm::new(batch_env, system_env, storage))
        };

        // We must inject *some* signature (otherwise bootloader code fails to generate hash).
        if l2_tx.common_data.signature.is_empty() {
            l2_tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        let tx: Transaction = l2_tx.into();
        delegate_vm!(vm, push_transaction(tx.clone()));

        let call_tracer_result = Arc::new(OnceCell::default());
        let error_flags_result = Arc::new(OnceCell::new());

        let tracers = vec![
            CallErrorTracer::new(error_flags_result.clone()).into_tracer_pointer(),
            CallTracer::new(call_tracer_result.clone()).into_tracer_pointer(),
        ];
        let tx_result = delegate_vm!(
            vm,
            inspect(&mut tracers.into(), InspectExecutionMode::OneTx)
        );

        let call_traces = Arc::try_unwrap(call_tracer_result)
            .unwrap()
            .take()
            .unwrap_or_default();

        let verbosity = get_shell().verbosity;
        if !call_traces.is_empty() && verbosity >= 2 {
            let tx_result_for_arena = tx_result.clone();
            let mut builder = CallTraceDecoderBuilder::base();
            builder = builder.with_signature_identifier(SignaturesIdentifier::global());

            let decoder = builder.build();
            let arena: CallTraceArena = futures::executor::block_on(async {
                let blocking_result = tokio::task::spawn_blocking(move || {
                    let mut arena = build_call_trace_arena(&call_traces, &tx_result_for_arena);
                    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                    rt.block_on(async {
                        decode_trace_arena(&mut arena, &decoder).await;
                        Ok(arena)
                    })
                })
                .await;

                let inner_result: Result<CallTraceArena, AnvilNodeError> =
                    blocking_result.expect("spawn_blocking failed");
                inner_result
            })?;

            let filtered_arena = filter_call_trace_arena(&arena, verbosity);
            let trace_output = render_trace_arena_inner(&filtered_arena, false);
            sh_println!("\nTraces:\n{}", trace_output);
        }

        Ok(tx_result.result)
    }

    // Forcefully stores the given bytecode at a given account.
    pub async fn override_bytecode(
        &self,
        address: Address,
        bytecode: Vec<u8>,
    ) -> AnvilNodeResult<()> {
        self.node_handle.set_code_sync(address, bytecode).await
    }

    pub async fn dump_state(&self, preserve_historical_states: bool) -> AnvilNodeResult<Bytes> {
        let state = self
            .inner
            .read()
            .await
            .dump_state(preserve_historical_states)
            .await?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&serde_json::to_vec(&state).map_err(to_generic)?)
            .map_err(to_generic)?;
        Ok(encoder.finish().map_err(to_generic)?.into())
    }

    pub async fn load_state(&self, buf: Bytes) -> StateLoaderResult<bool> {
        let orig_buf = &buf.0[..];
        let mut decoder = GzDecoder::new(orig_buf);
        let mut decoded_data = Vec::new();

        // Support both compressed and non-compressed state format
        let decoded = if decoder.header().is_some() {
            tracing::trace!(bytes = buf.0.len(), "decompressing state");
            decoder.read_to_end(decoded_data.as_mut()).map_err(|e| {
                StateLoaderError::StateDecompression {
                    details: e.to_string(),
                }
            })?;
            &decoded_data
        } else {
            &buf.0
        };
        tracing::trace!(bytes = decoded.len(), "deserializing state");
        let state: VersionedState = serde_json::from_slice(decoded).map_err(|e| {
            StateLoaderError::StateDeserialization {
                details: e.to_string(),
            }
        })?;

        self.inner.write().await.load_state(state).await
    }

    pub async fn get_chain_id(&self) -> AnvilNodeResult<u32> {
        Ok(self
            .inner
            .read()
            .await
            .config
            .chain_id
            .unwrap_or(TEST_NODE_NETWORK_ID))
    }

    pub fn get_current_timestamp(&self) -> AnvilNodeResult<u64> {
        Ok(self.time.current_timestamp())
    }

    pub async fn set_show_storage_logs(
        &self,
        show_storage_logs: ShowStorageLogs,
    ) -> AnvilNodeResult<String> {
        self.inner.write().await.config.show_storage_logs = show_storage_logs;
        Ok(show_storage_logs.to_string())
    }

    pub async fn set_show_vm_details(
        &self,
        show_vm_details: ShowVMDetails,
    ) -> AnvilNodeResult<String> {
        self.inner.write().await.config.show_vm_details = show_vm_details;
        Ok(show_vm_details.to_string())
    }

    pub async fn set_show_gas_details(
        &self,
        show_gas_details: ShowGasDetails,
    ) -> AnvilNodeResult<String> {
        self.inner.write().await.config.show_gas_details = show_gas_details;
        Ok(show_gas_details.to_string())
    }

    pub async fn set_show_node_config(&self, value: bool) -> AnvilNodeResult<bool> {
        self.inner.write().await.config.show_node_config = value;
        Ok(value)
    }

    pub fn set_log_level(&self, level: LogLevel) -> AnvilNodeResult<bool> {
        let Some(observability) = &self.observability else {
            return Err(generic_error!("Node's logging is not set up."));
        };
        tracing::debug!("setting log level to '{}'", level);
        observability.set_log_level(level)?;
        Ok(true)
    }

    pub fn set_logging(&self, directive: String) -> AnvilNodeResult<bool> {
        let Some(observability) = &self.observability else {
            return Err(generic_error!("Node's logging is not set up."));
        };
        tracing::debug!("setting logging to '{}'", directive);
        observability.set_logging(directive)?;
        Ok(true)
    }

    pub async fn chain_id(&self) -> L2ChainId {
        self.inner.read().await.chain_id()
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

// Test utils
// TODO: Consider builder pattern with sensible defaults
// #[cfg(test)]
// TODO: Mark with #[cfg(test)] once it is not used in other modules
impl InMemoryNode {
    pub fn test_config(fork_client_opt: Option<ForkClient>, config: TestNodeConfig) -> Self {
        let fee_provider = TestNodeFeeInputProvider::from_fork(
            fork_client_opt.as_ref().map(|client| &client.details),
            &config.base_token_config,
        );
        let impersonation = ImpersonationManager::default();
        let system_contracts = SystemContracts::from_options(
            config.system_contracts_options,
            config.system_contracts_path.clone(),
            ProtocolVersionId::latest(),
            config.use_evm_interpreter,
            config.zksync_os.clone(),
        );
        let storage_key_layout = if config.zksync_os.zksync_os {
            StorageKeyLayout::ZKsyncOs
        } else {
            StorageKeyLayout::Era
        };
        let (inner, storage, blockchain, time, fork, vm_runner) = InMemoryNodeInner::init(
            fork_client_opt,
            fee_provider,
            Arc::new(RwLock::new(Default::default())),
            config,
            impersonation.clone(),
            system_contracts.clone(),
            storage_key_layout,
            false,
        );
        let (node_executor, node_handle) =
            NodeExecutor::new(inner.clone(), vm_runner, storage_key_layout);
        let pool = TxPool::new(
            impersonation.clone(),
            anvil_zksync_types::TransactionOrder::Fifo,
        );
        let tx_listener = pool.add_tx_listener();
        let (block_sealer, block_sealer_state) = BlockSealer::new(
            BlockSealerMode::immediate(1000, tx_listener),
            pool.clone(),
            node_handle.clone(),
        );
        tokio::spawn(node_executor.run());
        tokio::spawn(block_sealer.run());
        Self::new(
            inner,
            blockchain,
            storage,
            fork,
            node_handle,
            None,
            time,
            impersonation,
            pool,
            block_sealer_state,
            system_contracts,
            storage_key_layout,
        )
    }

    pub fn test(fork_client_opt: Option<ForkClient>) -> Self {
        let config = TestNodeConfig {
            cache_config: CacheConfig::None,
            ..Default::default()
        };
        Self::test_config(fork_client_opt, config)
    }
}

#[cfg(test)]
impl InMemoryNode {
    pub async fn apply_txs(
        &self,
        txs: impl IntoIterator<Item = Transaction>,
    ) -> AnvilNodeResult<Vec<TransactionReceipt>> {
        use backon::{ConstantBuilder, Retryable};
        use std::time::Duration;

        let txs = Vec::from_iter(txs);
        let expected_tx_hashes = txs.iter().map(|tx| tx.hash()).collect::<Vec<_>>();
        self.pool.add_txs(txs);

        let mut receipts = Vec::with_capacity(expected_tx_hashes.len());
        for tx_hash in expected_tx_hashes {
            let receipt = (|| async {
                self.blockchain
                    .get_tx_receipt(&tx_hash)
                    .await
                    .ok_or(generic_error!("missing tx receipt"))
            })
            .retry(
                ConstantBuilder::default()
                    .with_delay(Duration::from_millis(200))
                    .with_max_times(5),
            )
            .await?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }
}
