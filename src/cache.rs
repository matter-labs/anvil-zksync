use eyre::Result;
use rustc_hash::FxHashMap;
use serde::Serialize;
use zksync_basic_types::H256;
use zksync_types::api::{Block, Transaction, TransactionVariant};
use zksync_types::Transaction as RawTransaction;

const CACHE_DIR: &'static str = ".cache";

#[derive(Default, Debug, Clone)]
pub(crate) struct Cache {
    pub(crate) block_hashes: FxHashMap<u64, H256>,
    pub(crate) blocks_full: FxHashMap<H256, Block<TransactionVariant>>,
    pub(crate) blocks_min: FxHashMap<H256, Block<TransactionVariant>>,
    pub(crate) block_raw_transactions: FxHashMap<H256, Vec<RawTransaction>>,
    pub(crate) transactions: FxHashMap<H256, Transaction>,
}

impl Cache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get_block(
        &self,
        hash: &H256,
        full_transactions: bool,
    ) -> Option<&Block<TransactionVariant>> {
        if full_transactions {
            self.blocks_full.get(hash)
        } else {
            self.blocks_min.get(hash)
        }
    }

    pub(crate) fn insert_block(
        &self,
        hash: H256,
        full_transactions: bool,
        block: Block<TransactionVariant>,
    ) {
        if full_transactions {
            self.blocks_full.insert(hash, block);
        } else {
            self.blocks_min.insert(hash, block);
        }
    }

    pub(crate) fn get_block_hash(&self, number: u64) -> Option<&H256> {
        self.block_hashes.get(&number)
    }

    pub(crate) fn insert_block_hash(&self, number: u64, hash: H256) {
        self.block_hashes.insert(number, hash);
        Self::write(format!("block-hashes/{number}"), serde_json::to_string(&hash).expect("failed encoding value").as_bytes());
    }

    pub(crate) fn get_block_raw_transactions(&self, hash: &H256) -> Option<&Vec<RawTransaction>> {
        self.block_raw_transactions.get(&hash)
    }

    pub(crate) fn insert_block_raw_transactions(&self, hash: H256, transactions: Vec<RawTransaction>) {
        self.block_raw_transactions.insert(hash, transactions);
        Self::write(format!("block-raw-transactions/{hash}"), transactions.as_bytes());
    }

    fn write(key: String, data: &[u8]) {

    }
}
