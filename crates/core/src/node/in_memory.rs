//! In-memory node, that supports forking other networks.
use super::inner::fork::ForkDetails;
use super::inner::node_executor::NodeExecutorHandle;
use super::inner::InMemoryNodeInner;
use super::vm::AnvilVM;
use crate::deps::storage_view::StorageView;
use crate::deps::InMemoryStorage;
use crate::filters::EthFilters;
use crate::node::call_error_tracer::CallErrorTracer;
use crate::node::error::LoadStateError;
use crate::node::fee_model::TestNodeFeeInputProvider;
use crate::node::impersonate::{ImpersonationManager, ImpersonationState};
use crate::node::inner::blockchain::ReadBlockchain;
use crate::node::inner::storage::ReadStorageDyn;
use crate::node::inner::time::ReadTime;
use crate::node::sealer::BlockSealerState;
use crate::node::state::VersionedState;
use crate::node::{BlockSealer, BlockSealerMode, NodeExecutor, TxPool};
use crate::observability::Observability;
use crate::system_contracts::SystemContracts;
use crate::{delegate_vm, formatter};
use anvil_zksync_config::constants::{
    LEGACY_RICH_WALLETS, NON_FORK_FIRST_BLOCK_TIMESTAMP, RICH_WALLETS, TEST_NODE_NETWORK_ID,
};
use anvil_zksync_config::types::Genesis;
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_types::{LogLevel, ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use colored::Colorize;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use zksync_contracts::BaseSystemContracts;
use zksync_multivm::interface::storage::{ReadStorage, StoragePtr};
use zksync_multivm::interface::VmFactory;
use zksync_multivm::interface::{
    ExecutionResult, InspectExecutionMode, L1BatchEnv, L2BlockEnv, TxExecutionMode, VmInterface,
};
use zksync_multivm::tracers::CallTracer;
use zksync_multivm::utils::{get_batch_base_fee, get_max_batch_gas_limit};
use zksync_multivm::vm_latest::Vm;

use crate::node::keys::StorageKeyLayout;
use zksync_multivm::vm_latest::{HistoryDisabled, ToTracerPointer};
use zksync_multivm::VmVersion;
use zksync_types::api::{Block, DebugCall, TransactionReceipt, TransactionVariant};
use zksync_types::block::unpack_block_info;
use zksync_types::fee_model::BatchFeeInput;
use zksync_types::l2::L2Tx;
use zksync_types::storage::{
    EMPTY_UNCLES_HASH, SYSTEM_CONTEXT_ADDRESS, SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
};
use zksync_types::web3::{keccak256, Bytes};
use zksync_types::{
    h256_to_u256, AccountTreeId, Address, Bloom, L1BatchNumber, L2BlockNumber, L2ChainId,
    PackedEthSignature, StorageKey, StorageValue, Transaction, H160, H256, H64, U256, U64,
};

/// Max possible size of an ABI encoded tx (in bytes).
pub const MAX_TX_SIZE: usize = 1_000_000;
/// Acceptable gas overestimation limit.
pub const ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION: u64 = 1_000;
/// The maximum number of previous blocks to store the state for.
pub const MAX_PREVIOUS_STATES: u16 = 128;
/// The zks protocol version.
pub const PROTOCOL_VERSION: &str = "zks/1";

// TODO: Use L2BlockNumber
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
    pub tx: L2Tx,
    // Batch number where transaction was executed.
    pub batch_number: u32,
    pub miniblock_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub(crate) node_handle: NodeExecutorHandle,
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

    pub async fn reset(&self, fork: Option<ForkDetails>) -> Result<(), String> {
        self.inner.write().await.reset(fork).await;
        self.snapshots.write().await.clear();

        for wallet in LEGACY_RICH_WALLETS.iter() {
            let address = wallet.0;
            self.set_rich_account(
                H160::from_str(address).unwrap(),
                U256::from(100u128 * 10u128.pow(18)),
            )
            .await;
        }
        for wallet in RICH_WALLETS.iter() {
            let address = wallet.0;
            self.set_rich_account(
                H160::from_str(address).unwrap(),
                U256::from(100u128 * 10u128.pow(18)),
            )
            .await;
        }
        Ok(())
    }

    /// Applies multiple transactions across multiple blocks. All transactions are expected to be
    /// executable. Note that on error this method may leave node in partially applied state (i.e.
    /// some txs have been applied while others have not).
    pub async fn apply_txs(&self, txs: Vec<L2Tx>, max_transactions: usize) -> anyhow::Result<()> {
        tracing::debug!(count = txs.len(), "applying transactions");

        // Create a temporary tx pool (i.e. state is not shared with the node mempool).
        let pool = TxPool::new(
            self.impersonation.clone(),
            self.inner.read().await.config.transaction_order,
        );
        pool.add_txs(txs);

        while let Some(tx_batch) = pool.take_uniform(max_transactions) {
            // Getting contracts is reasonably cheap, so we don't cache them. We may need differing contracts
            // depending on whether impersonation should be enabled for a block.
            let expected_tx_hashes = tx_batch
                .txs
                .iter()
                .map(|tx| tx.hash())
                .collect::<HashSet<_>>();
            let block_numer = self.node_handle.seal_block_sync(tx_batch).await?;

            // Fetch the block that was just sealed
            let block = self
                .blockchain
                .get_block_by_number(block_numer)
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
                anyhow::bail!("Failed to apply some transactions: {:?}", diff_tx_hashes);
            }
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
    ) -> anyhow::Result<ExecutionResult> {
        let execution_mode = TxExecutionMode::EthCall;

        let inner = self.inner.read().await;

        // init vm

        let (batch_env, _) = inner.create_l1_batch_env().await;
        let system_env = inner.create_system_env(base_contracts, execution_mode);

        let storage = StorageView::new(inner.read_storage()).into_rc_ptr();

        let mut vm = if self.system_contracts.use_zkos() {
            AnvilVM::ZKOs(super::zkos::ZKOsVM::<_, HistoryDisabled>::new(
                batch_env,
                system_env,
                storage,
                // TODO: this might be causing a deadlock.. check..
                &inner.fork_storage.inner.read().unwrap().raw_storage,
                &self.system_contracts.zkos_config,
            ))
        } else {
            AnvilVM::ZKSync(Vm::new(batch_env, system_env, storage))
        };

        // We must inject *some* signature (otherwise bootloader code fails to generate hash).
        if l2_tx.common_data.signature.is_empty() {
            l2_tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        let tx: Transaction = l2_tx.into();
        delegate_vm!(vm, push_transaction(tx.clone()));

        let call_tracer_result = Arc::new(OnceCell::default());

        let tracers = vec![
            CallErrorTracer::new().into_tracer_pointer(),
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

        if inner.config.show_tx_summary {
            tracing::info!("");
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
            inner
                .console_log_handler
                .handle_calls_recursive(&call_traces);
        }

        if inner.config.show_calls != ShowCalls::None {
            tracing::info!("");
            tracing::info!(
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
                    inner.config.show_calls,
                    inner.config.show_outputs,
                    inner.config.resolve_hashes,
                );
            }
        }

        Ok(tx_result.result)
    }

    // Forcefully stores the given bytecode at a given account.
    pub async fn override_bytecode(
        &self,
        address: Address,
        bytecode: Vec<u8>,
    ) -> anyhow::Result<()> {
        self.node_handle.set_code_sync(address, bytecode).await
    }

    pub async fn dump_state(&self, preserve_historical_states: bool) -> anyhow::Result<Bytes> {
        let state = self
            .inner
            .read()
            .await
            .dump_state(preserve_historical_states)
            .await?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&serde_json::to_vec(&state)?)?;
        Ok(encoder.finish()?.into())
    }

    pub async fn load_state(&self, buf: Bytes) -> Result<bool, LoadStateError> {
        let orig_buf = &buf.0[..];
        let mut decoder = GzDecoder::new(orig_buf);
        let mut decoded_data = Vec::new();

        // Support both compressed and non-compressed state format
        let decoded = if decoder.header().is_some() {
            tracing::trace!(bytes = buf.0.len(), "decompressing state");
            decoder
                .read_to_end(decoded_data.as_mut())
                .map_err(LoadStateError::FailedDecompress)?;
            &decoded_data
        } else {
            &buf.0
        };
        tracing::trace!(bytes = decoded.len(), "deserializing state");
        let state: VersionedState =
            serde_json::from_slice(decoded).map_err(LoadStateError::FailedDeserialize)?;

        self.inner.write().await.load_state(state).await
    }

    pub async fn get_chain_id(&self) -> anyhow::Result<u32> {
        Ok(self
            .inner
            .read()
            .await
            .config
            .chain_id
            .unwrap_or(TEST_NODE_NETWORK_ID))
    }

    pub async fn get_show_calls(&self) -> anyhow::Result<String> {
        Ok(self.inner.read().await.config.show_calls.to_string())
    }

    pub async fn get_show_outputs(&self) -> anyhow::Result<bool> {
        Ok(self.inner.read().await.config.show_outputs)
    }

    pub fn get_current_timestamp(&self) -> anyhow::Result<u64> {
        Ok(self.time.current_timestamp())
    }

    pub async fn set_show_calls(&self, show_calls: ShowCalls) -> anyhow::Result<String> {
        self.inner.write().await.config.show_calls = show_calls;
        Ok(show_calls.to_string())
    }

    pub async fn set_show_outputs(&self, value: bool) -> anyhow::Result<bool> {
        self.inner.write().await.config.show_outputs = value;
        Ok(value)
    }

    pub async fn set_show_storage_logs(
        &self,
        show_storage_logs: ShowStorageLogs,
    ) -> anyhow::Result<String> {
        self.inner.write().await.config.show_storage_logs = show_storage_logs;
        Ok(show_storage_logs.to_string())
    }

    pub async fn set_show_vm_details(
        &self,
        show_vm_details: ShowVMDetails,
    ) -> anyhow::Result<String> {
        self.inner.write().await.config.show_vm_details = show_vm_details;
        Ok(show_vm_details.to_string())
    }

    pub async fn set_show_gas_details(
        &self,
        show_gas_details: ShowGasDetails,
    ) -> anyhow::Result<String> {
        self.inner.write().await.config.show_gas_details = show_gas_details;
        Ok(show_gas_details.to_string())
    }

    pub async fn set_resolve_hashes(&self, value: bool) -> anyhow::Result<bool> {
        self.inner.write().await.config.resolve_hashes = value;
        Ok(value)
    }

    pub async fn set_show_node_config(&self, value: bool) -> anyhow::Result<bool> {
        self.inner.write().await.config.show_node_config = value;
        Ok(value)
    }

    pub async fn set_show_tx_summary(&self, value: bool) -> anyhow::Result<bool> {
        self.inner.write().await.config.show_tx_summary = value;
        Ok(value)
    }

    pub async fn set_show_event_logs(&self, value: bool) -> anyhow::Result<bool> {
        self.inner.write().await.config.show_event_logs = value;
        Ok(value)
    }

    pub async fn set_disable_console_log(&self, value: bool) -> anyhow::Result<bool> {
        self.inner.write().await.config.disable_console_log = value;
        Ok(value)
    }

    pub fn set_log_level(&self, level: LogLevel) -> anyhow::Result<bool> {
        let Some(observability) = &self.observability else {
            anyhow::bail!("Node's logging is not set up.")
        };
        tracing::info!("setting log level to '{}'", level);
        observability.set_log_level(level)?;
        Ok(true)
    }

    pub fn set_logging(&self, directive: String) -> anyhow::Result<bool> {
        let Some(observability) = &self.observability else {
            anyhow::bail!("Node's logging is not set up.")
        };
        tracing::info!("setting logging to '{}'", directive);
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
    pub fn test_config(fork: Option<ForkDetails>, config: TestNodeConfig) -> Self {
        let fee_provider = TestNodeFeeInputProvider::from_fork(fork.as_ref());
        let impersonation = ImpersonationManager::default();
        let system_contracts = SystemContracts::from_options(
            &config.system_contracts_options,
            config.use_evm_emulator,
            config.zkos_config.clone(),
        );
        let storage_key_layout = if config.zkos_config.use_zkos {
            StorageKeyLayout::ZkOs
        } else {
            StorageKeyLayout::ZkEra
        };
        let (inner, storage, blockchain, time) = InMemoryNodeInner::init(
            fork,
            fee_provider,
            Arc::new(RwLock::new(Default::default())),
            config,
            impersonation.clone(),
            system_contracts.clone(),
            storage_key_layout,
        );
        let (node_executor, node_handle) =
            NodeExecutor::new(inner.clone(), system_contracts.clone(), storage_key_layout);
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

    pub fn test(fork: Option<ForkDetails>) -> Self {
        let config = TestNodeConfig::default();
        Self::test_config(fork, config)
    }
}
