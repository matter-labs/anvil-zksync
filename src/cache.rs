use zksync_basic_types::H256;
use zksync_types::api::{Block, Transaction, TransactionVariant};
use zksync_types::Transaction as RawTransaction;
use rustc_hash::FxHashMap;


#[derive(Default, Debug, Clone)]
pub(crate) struct Cache {
    pub(crate) block_hashes: FxHashMap<u64, H256>, 
    pub(crate) blocks_full: FxHashMap<H256, Block<TransactionVariant>>,
    pub(crate) blocks_min: FxHashMap<H256, Block<TransactionVariant>>,
    pub(crate) raw_block_transactions: FxHashMap<H256, Vec<RawTransaction>>,
    pub(crate) transactions: FxHashMap<H256, Transaction>,
}

impl Cache {
    pub (crate) fn new() -> Self { 
        Self::default()
    }
}