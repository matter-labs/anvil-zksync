use super::fork::ForkDetails;
use super::time::{AdvanceTime, ReadTime};
use crate::node::time::TimestampWriter;
use crate::node::{compute_hash, create_genesis, create_genesis_from_json, TransactionResult};
use crate::utils::ArcRLock;
use anvil_zksync_config::types::Genesis;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use zksync_multivm::interface::storage::{ReadStorage, StoragePtr};
use zksync_multivm::interface::L2Block;
use zksync_multivm::vm_latest::utils::l2_blocks::load_last_l2_block;
use zksync_types::block::{unpack_block_info, L2BlockHasher};
use zksync_types::{
    api, AccountTreeId, L1BatchNumber, L2BlockNumber, StorageKey, H256, SYSTEM_CONTEXT_ADDRESS,
    SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
};
use zksync_utils::h256_to_u256;

/// A read-only blockchain representation. All clones agree on the internal state.
#[derive(Clone)]
pub struct BlockchainReader {
    /// Underlying read-only storage.
    inner: ArcRLock<Blockchain>,
}

impl BlockchainReader {
    pub(super) fn new(
        fork: Option<&ForkDetails>,
        genesis: Option<&Genesis>,
        genesis_timestamp: Option<u64>,
    ) -> (Self, BlockchainWriter) {
        let storage = if let Some(fork) = fork {
            Blockchain {
                current_batch: fork.l1_block,
                current_block: L2BlockNumber(fork.l2_miniblock as u32),
                current_block_hash: fork.l2_miniblock_hash,
                tx_results: Default::default(),
                blocks: HashMap::from_iter([(fork.l2_block.hash, fork.l2_block.clone())]),
                hashes: HashMap::from_iter([(
                    fork.l2_block.number.as_u32().into(),
                    fork.l2_block.hash,
                )]),
            }
        } else {
            let block_hash = compute_hash(0, []);
            let genesis_block: api::Block<api::TransactionVariant> = if let Some(genesis) = genesis
            {
                create_genesis_from_json(genesis, genesis_timestamp)
            } else {
                create_genesis(genesis_timestamp)
            };

            Blockchain {
                current_batch: L1BatchNumber(0),
                current_block: L2BlockNumber(0),
                current_block_hash: block_hash,
                tx_results: Default::default(),
                blocks: HashMap::from_iter([(block_hash, genesis_block)]),
                hashes: HashMap::from_iter([(L2BlockNumber(0), block_hash)]),
            }
        };
        let inner = Arc::new(RwLock::new(storage));
        (
            Self {
                inner: ArcRLock::wrap(inner.clone()),
            },
            BlockchainWriter { inner },
        )
    }

    pub async fn current_batch(&self) -> L1BatchNumber {
        self.inner.read().await.current_batch
    }

    pub async fn current_block_number(&self) -> L2BlockNumber {
        self.inner.read().await.current_block
    }

    pub async fn current_block_hash(&self) -> H256 {
        self.inner.read().await.current_block_hash
    }

    pub async fn get_block_by_hash(
        &self,
        hash: &H256,
    ) -> Option<api::Block<api::TransactionVariant>> {
        self.inspect_block_by_hash(hash, |block| block.clone())
            .await
    }

    pub async fn get_block_by_number(
        &self,
        number: L2BlockNumber,
    ) -> Option<api::Block<api::TransactionVariant>> {
        self.inspect_block_by_number(number, |block| block.clone())
            .await
    }

    pub async fn get_block_by_id(
        &self,
        block_id: api::BlockId,
    ) -> Option<api::Block<api::TransactionVariant>> {
        self.inspect_block_by_id(block_id, |block| block.clone())
            .await
    }

    pub async fn get_block_hash_by_number(&self, number: L2BlockNumber) -> Option<H256> {
        self.inspect_block_by_number(number, |block| block.hash)
            .await
    }

    pub async fn get_block_hash_by_id(&self, block_id: api::BlockId) -> Option<H256> {
        self.inspect_block_by_id(block_id, |block| block.hash).await
    }

    pub async fn get_block_number_by_hash(&self, hash: &H256) -> Option<L2BlockNumber> {
        self.inspect_block_by_hash(hash, |block| L2BlockNumber(block.number.as_u32()))
            .await
    }

    pub async fn get_block_number_by_id(&self, block_id: api::BlockId) -> Option<L2BlockNumber> {
        self.inspect_block_by_id(block_id, |block| L2BlockNumber(block.number.as_u32()))
            .await
    }

    pub async fn inspect_block_by_hash<T>(
        &self,
        hash: &H256,
        f: impl FnOnce(&api::Block<api::TransactionVariant>) -> T,
    ) -> Option<T> {
        Some(f(self.inner.read().await.blocks.get(hash)?))
    }

    pub async fn inspect_block_by_number<T>(
        &self,
        number: L2BlockNumber,
        f: impl FnOnce(&api::Block<api::TransactionVariant>) -> T,
    ) -> Option<T> {
        let storage = self.inner.read().await;
        let hash = storage.get_block_hash_by_number(number)?;
        Some(f(storage.blocks.get(&hash)?))
    }

    pub async fn inspect_block_by_id<T>(
        &self,
        block_id: api::BlockId,
        f: impl FnOnce(&api::Block<api::TransactionVariant>) -> T,
    ) -> Option<T> {
        let storage = self.inner.read().await;
        let hash = storage.get_block_hash_by_id(block_id)?;
        Some(f(storage.blocks.get(&hash)?))
    }

    pub async fn get_tx_receipt(&self, tx_hash: &H256) -> Option<api::TransactionReceipt> {
        self.inspect_tx(tx_hash, |tx| tx.receipt.clone()).await
    }

    pub async fn get_tx_debug_info(
        &self,
        tx_hash: &H256,
        only_top: bool,
    ) -> Option<api::DebugCall> {
        self.inspect_tx(tx_hash, |tx| tx.debug_info(only_top)).await
    }

    pub async fn inspect_tx<T>(
        &self,
        tx_hash: &H256,
        f: impl FnOnce(&TransactionResult) -> T,
    ) -> Option<T> {
        Some(f(self.inner.read().await.tx_results.get(tx_hash)?))
    }

    // TODO: Seems like a strange pattern to allow this
    pub async fn inspect_all_txs<T>(
        &self,
        f: impl FnOnce(&HashMap<H256, TransactionResult>) -> T,
    ) -> T {
        f(&self.inner.read().await.tx_results)
    }
}

/// A single-instance writer to blockchain state that is only available to [`super::InMemoryNodeInner`].
pub(super) struct BlockchainWriter {
    pub(super) inner: Arc<RwLock<Blockchain>>,
}

impl BlockchainWriter {
    pub(super) async fn read(&self) -> RwLockReadGuard<Blockchain> {
        self.inner.read().await
    }

    pub(super) async fn write(&self) -> RwLockWriteGuard<Blockchain> {
        self.inner.write().await
    }
}

/// Stores the blockchain data (blocks, transactions)
#[derive(Clone)]
pub(super) struct Blockchain {
    /// The latest batch number that was already generated.
    /// Next block will go to the batch `current_batch + 1`.
    pub(super) current_batch: L1BatchNumber,
    /// The latest block number that was already generated.
    /// Next transaction will go to the block `current_block + 1`.
    pub(super) current_block: L2BlockNumber,
    /// The latest block hash.
    pub(super) current_block_hash: H256,
    /// Map from transaction to details about the execution.
    pub(super) tx_results: HashMap<H256, TransactionResult>,
    /// Map from block hash to information about the block.
    pub(super) blocks: HashMap<H256, api::Block<api::TransactionVariant>>,
    /// Map from block number to a block hash.
    pub(super) hashes: HashMap<L2BlockNumber, H256>,
}

impl Blockchain {
    pub(super) fn get_block_hash_by_number(&self, number: L2BlockNumber) -> Option<H256> {
        self.hashes.get(&number).map(|hash| *hash)
    }

    pub(super) fn get_block_hash_by_id(&self, block_id: api::BlockId) -> Option<H256> {
        match block_id {
            api::BlockId::Number(number) => {
                let number = match number {
                    api::BlockNumber::Finalized
                    | api::BlockNumber::Pending
                    | api::BlockNumber::Committed
                    | api::BlockNumber::L1Committed
                    | api::BlockNumber::Latest => self.current_block,
                    api::BlockNumber::Earliest => L2BlockNumber(0),
                    api::BlockNumber::Number(n) => L2BlockNumber(n.as_u32()),
                };
                self.hashes.get(&number).map(|hash| *hash)
            }
            api::BlockId::Hash(hash) => Some(hash),
        }
    }

    pub(super) fn last_env<S: ReadStorage>(
        &self,
        storage: &StoragePtr<S>,
        time: &TimestampWriter,
    ) -> (L1BatchNumber, L2Block) {
        // TODO: This whole logic seems off to me, reconsider if we need it at all.
        //       Specifically it is weird that we might not have our latest block in the storage.
        //       Likely has to do with genesis but let's make it clear if that is actually the case.
        let last_l1_batch_number = load_last_l1_batch(storage)
            .map(|(num, _)| L1BatchNumber(num as u32))
            .unwrap_or(self.current_batch);
        let last_l2_block = load_last_l2_block(storage).unwrap_or_else(|| L2Block {
            number: self.current_block.0,
            hash: L2BlockHasher::legacy_hash(self.current_block),
            timestamp: time.current_timestamp(),
        });
        (last_l1_batch_number, last_l2_block)
    }

    pub(super) fn apply_block(&mut self, block: api::Block<api::TransactionVariant>, index: u32) {
        let latest_block = self.blocks.get(&self.current_block_hash).unwrap();
        self.current_block = self.current_block + 1;

        let actual_l1_batch_number = block
            .l1_batch_number
            .expect("block must have a l1_batch_number");
        if L1BatchNumber(actual_l1_batch_number.as_u32()) != self.current_batch {
            panic!(
                "expected next block to have batch_number {}, got {}",
                self.current_batch,
                actual_l1_batch_number.as_u32()
            );
        }

        if L2BlockNumber(block.number.as_u32()) != self.current_block {
            panic!(
                "expected next block to have miniblock {}, got {} | {index}",
                self.current_block,
                block.number.as_u64()
            );
        }

        if block.timestamp.as_u64() <= latest_block.timestamp.as_u64() {
            panic!(
                "expected next block to have timestamp bigger than {}, got {} | {index}",
                latest_block.timestamp.as_u64(),
                block.timestamp.as_u64()
            );
        }

        let block_hash = block.hash;
        self.current_block_hash = block_hash;
        self.hashes
            .insert(L2BlockNumber(block.number.as_u32()), block.hash);
        self.blocks.insert(block.hash, block);
    }

    pub(super) fn load_blocks<T: AdvanceTime>(
        &mut self,
        time: &mut T,
        blocks: Vec<api::Block<api::TransactionVariant>>,
    ) {
        tracing::trace!(
            blocks = blocks.len(),
            "loading new blocks from supplied state"
        );
        for block in blocks {
            let number = block.number.as_u64();
            tracing::trace!(
                number,
                hash = %block.hash,
                "loading new block from supplied state"
            );

            self.hashes.insert(L2BlockNumber(number as u32), block.hash);
            self.blocks.insert(block.hash, block);
        }

        // Safe unwrap as there was at least one block in the loaded state
        let latest_block = self.blocks.values().max_by_key(|b| b.number).unwrap();
        let latest_number = latest_block.number.as_u64();
        let latest_hash = latest_block.hash;
        let Some(latest_batch_number) = latest_block.l1_batch_number.map(|n| n.as_u32()) else {
            panic!("encountered a block with no batch; this is not supposed to happen")
        };
        let latest_timestamp = latest_block.timestamp.as_u64();
        tracing::info!(
            number = latest_number,
            hash = %latest_hash,
            batch_number = latest_batch_number,
            timestamp = latest_timestamp,
            "latest block after loading state"
        );
        self.current_block = L2BlockNumber(latest_number as u32);
        self.current_block_hash = latest_hash;
        self.current_batch = L1BatchNumber(latest_batch_number);
        time.reset_to(latest_timestamp);
    }

    pub(super) fn load_transactions(&mut self, transactions: Vec<TransactionResult>) {
        tracing::trace!(
            transactions = transactions.len(),
            "loading new transactions from supplied state"
        );
        for transaction in transactions {
            tracing::trace!(
                hash = %transaction.receipt.transaction_hash,
                "loading new transaction from supplied state"
            );
            self.tx_results
                .insert(transaction.receipt.transaction_hash, transaction);
        }
    }
}

fn load_last_l1_batch<S: ReadStorage>(storage: &StoragePtr<S>) -> Option<(u64, u64)> {
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
