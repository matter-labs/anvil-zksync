use serde::Deserialize;
use zksync_types::api::TransactionVariant;
use zksync_types::{Bloom, H256, U256};
use zksync_vm_interface::L1BatchEnv;

/// Genesis
#[derive(Deserialize, Clone, Debug)]
pub struct Genesis {
    /// The hash of the genesis block. If not provided, it can be computed.
    pub hash: Option<H256>,
    /// The parent hash of the genesis block. Usually zero.
    pub parent_hash: Option<H256>,
    /// The block number of the genesis block. Usually zero.
    pub block_number: Option<u64>,
    /// The timestamp of the genesis block.
    pub timestamp: Option<u64>,
    // /// The L1 batch environment.
    pub l1_batch_env: Option<L1BatchEnv>,
    /// The transactions included in the genesis block.
    pub transactions: Option<Vec<TransactionVariant>>,
    /// The amount of gas used.
    pub gas_used: Option<U256>,
    /// The logs bloom filter.
    pub logs_bloom: Option<Bloom>,
}
