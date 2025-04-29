//! anvil-zksync, that supports forking other networks.

mod batch;
pub mod diagnostics;
pub mod error;
mod fee_model;
mod impersonate;
pub mod inner;
mod keys;
mod pool;
mod sealer;
mod state;
mod storage_logs;
pub mod traces;
mod vm;
mod zkos;

use crate::deps::InMemoryStorage;
use crate::filters::EthFilters;
use crate::node::impersonate::ImpersonationState;
use anvil_zksync_config::constants::NON_FORK_FIRST_BLOCK_TIMESTAMP;
use anvil_zksync_config::types::Genesis;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use zksync_contracts::BaseSystemContractsHashes;
use zksync_multivm::interface::{L1BatchEnv, L2BlockEnv};
use zksync_multivm::utils::{get_batch_base_fee, get_max_batch_gas_limit};
use zksync_multivm::zk_evm_latest::ethereum_types::{Address, Bloom, H160, H256, H64, U256, U64};
use zksync_multivm::VmVersion;
use zksync_types::api::{Block, DebugCall, TransactionReceipt, TransactionVariant};
use zksync_types::block::{L1BatchHeader, L2BlockHasher};
use zksync_types::fee_model::BatchFeeInput;
use zksync_types::web3::Bytes;
use zksync_types::{
    L1BatchNumber, L2BlockNumber, ProtocolVersionId, StorageKey, StorageValue, Transaction,
    EMPTY_UNCLES_HASH,
};

pub use self::{
    fee_model::TestNodeFeeInputProvider, impersonate::ImpersonationManager, keys::StorageKeyLayout,
    node_executor::NodeExecutor, pool::TxBatch, pool::TxPool, sealer::BlockSealer,
    sealer::BlockSealerMode, sealer::BlockSealerState, state::VersionedState, vm::AnvilVM,
    zkos::ZKOsVM,
};
pub use inner::InMemoryNodeInner;
pub use inner::{blockchain, fork, node_executor, time};

/// Max possible size of an ABI encoded tx (in bytes).
pub const MAX_TX_SIZE: usize = 1_000_000;
/// Acceptable gas overestimation limit.
pub const ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION: u64 = 1_000;
/// The maximum number of previous blocks to store the state for.
pub const MAX_PREVIOUS_STATES: u16 = 128;
/// The zks protocol version.
pub const PROTOCOL_VERSION: &str = "zks/1";

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
