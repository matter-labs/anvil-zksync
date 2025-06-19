use crate::filters::LogFilter;
use crate::node::inner::fork::ForkDetails;
use crate::node::time::{ReadTime, Time};
use crate::node::{create_genesis, create_genesis_from_json, TransactionResult};
use crate::utils::utc_datetime_from_epoch_ms;
use anvil_zksync_config::types::Genesis;
use anvil_zksync_types::api::DetailedTransaction;
use anyhow::Context;
use async_trait::async_trait;
use itertools::Itertools;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use zksync_contracts::BaseSystemContractsHashes;
use zksync_multivm::interface::storage::{ReadStorage, StoragePtr};
use zksync_multivm::interface::{FinishedL1Batch, L2Block, VmEvent};
use zksync_multivm::vm_latest::utils::l2_blocks::load_last_l2_block;
use zksync_types::block::{unpack_block_info, L1BatchHeader, L2BlockHasher};
use zksync_types::l2::L2Tx;
use zksync_types::writes::StateDiffRecord;
use zksync_types::{
    api, api::BlockId, h256_to_u256, web3::Bytes, AccountTreeId, Address, ExecuteTransactionCommon,
    L1BatchNumber, L2BlockNumber, ProtocolVersionId, StorageKey, H256, SYSTEM_CONTEXT_ADDRESS,
    SYSTEM_CONTEXT_BLOCK_INFO_POSITION, U256, U64,
};

/// Read-only view on blockchain state.
#[async_trait]
pub trait ReadBlockchain: Send + Sync + Debug {
    /// Alternative for [`Clone::clone`] that is object safe.
    fn dyn_cloned(&self) -> Box<dyn ReadBlockchain>;

    /// Current protocol version used by the chain.
    fn protocol_version(&self) -> ProtocolVersionId;

    /// Returns last sealed batch's number. At least one sealed batch is guaranteed to be present
    /// in the storage at any given time.
    async fn current_batch(&self) -> L1BatchNumber;

    /// Returns last sealed block's number. At least one sealed block is guaranteed to be present
    /// in the storage at any given time.
    async fn current_block_number(&self) -> L2BlockNumber;

    /// Returns last sealed block's hash. At least one sealed block is guaranteed to be present
    /// in the storage at any given time.
    async fn current_block_hash(&self) -> H256;

    /// Retrieve full block by its hash. Returns `None` if no block was found. Note that the block
    /// might still be a part of the chain but is available in the fork instead.
    async fn get_block_by_hash(&self, hash: &H256) -> Option<api::Block<api::TransactionVariant>>;

    /// Retrieve full block by its number. Returns `None` if no block was found. Note that the block
    /// might still be a part of the chain but is available in the fork instead.
    async fn get_block_by_number(
        &self,
        number: L2BlockNumber,
    ) -> Option<api::Block<api::TransactionVariant>>;

    /// Retrieve full block by id. Returns `None` if no block was found. Note that the block
    /// might still be a part of the chain but is available in the fork instead.
    async fn get_block_by_id(
        &self,
        block_id: api::BlockId,
    ) -> Option<api::Block<api::TransactionVariant>>;

    /// Retrieve block hash by its number. Returns `None` if no block was found. Note that the block
    /// might still be a part of the chain but is available in the fork instead.
    async fn get_block_hash_by_number(&self, number: L2BlockNumber) -> Option<H256>;

    /// Retrieve block hash by id. Returns `None` if no block was found. Note that the block
    /// might still be a part of the chain but is available in the fork instead.
    async fn get_block_hash_by_id(&self, block_id: api::BlockId) -> Option<H256>;

    /// Retrieve block number by its hash. Returns `None` if no block was found. Note that the block
    /// might still be a part of the chain but is available in the fork instead.
    async fn get_block_number_by_hash(&self, hash: &H256) -> Option<L2BlockNumber>;

    /// Retrieve block number by id. Returns `None` if no block was found. Note that the block
    /// might still be a part of the chain but is available in the fork instead.
    async fn get_block_number_by_id(&self, block_id: api::BlockId) -> Option<L2BlockNumber>;

    /// Retrieve all transactions hashes from a block by its number. Returns `None` if no block was
    /// found. Note that the block might still be a part of the chain but is available in the fork
    /// instead.
    async fn get_block_tx_hashes_by_number(&self, number: L2BlockNumber) -> Option<Vec<H256>>;

    /// Retrieve all transactions hashes from a block by id. Returns `None` if no block was
    /// found. Note that the block might still be a part of the chain but is available in the fork
    /// instead.
    async fn get_block_tx_hashes_by_id(&self, block_id: api::BlockId) -> Option<Vec<H256>>;

    // TODO: Distinguish between block not found and tx not found
    /// Retrieve a transaction from a block by id and index of the transaction. Returns `None` if
    /// either no block was found or no transaction exists in the block under that index. Note that
    /// the block might still be a part of the chain but is available in the fork instead.
    async fn get_block_tx_by_id(
        &self,
        block_id: api::BlockId,
        index: usize,
    ) -> Option<api::Transaction>;

    /// Retrieve number of transactions in a block by id. Returns `None` if no block was
    /// found. Note that the block might still be a part of the chain but is available in the fork
    /// instead.
    async fn get_block_tx_count_by_id(&self, block_id: api::BlockId) -> Option<usize>;

    /// Retrieve block details (as defined in `zks_getBlockDetails`) by id. Returns `None` if no
    /// block was found. Note that the block might still be a part of the chain but is available in
    /// the fork instead.
    async fn get_block_details_by_number(
        &self,
        number: L2BlockNumber,
        // TODO: Values below should be fetchable from storage
        l2_fair_gas_price: u64,
        fair_pubdata_price: Option<u64>,
        base_system_contracts_hashes: BaseSystemContractsHashes,
    ) -> Option<api::BlockDetails>;

    /// Retrieve transaction receipt by transaction's hash. Returns `None` if no transaction was
    /// found. Note that the transaction might still be a part of the chain but is available in the
    /// fork instead.
    async fn get_tx_receipt(&self, tx_hash: &H256) -> Option<api::TransactionReceipt>;

    /// Retrieve transaction debug information by transaction's hash. Returns `None` if no transaction was
    /// found. Note that the transaction might still be a part of the chain but is available in the
    /// fork instead.
    async fn get_tx_debug_info(&self, tx_hash: &H256, only_top: bool) -> Option<api::DebugCall>;

    /// Retrieve transaction in API format by transaction's hash. Returns `None` if no transaction was
    /// found. Note that the transaction might still be a part of the chain but is available in the
    /// fork instead.
    async fn get_tx_api(&self, tx_hash: &H256) -> anyhow::Result<Option<api::Transaction>>;

    /// Retrieve detailed transaction (as defined in `anvil_mine_detailed`) by API transaction.
    /// Returns `None` if no transaction was found. Note that the transaction might still be a part
    /// of the chain but is available in the fork instead.
    async fn get_detailed_tx(&self, tx: api::Transaction) -> Option<DetailedTransaction>;

    /// Retrieve detailed transaction (as defined in `zks_getTransactionDetails`) by transaction's hash.
    /// Returns `None` if no transaction was found. Note that the transaction might still be a part
    /// of the chain but is available in the fork instead.
    async fn get_tx_details(&self, tx_hash: &H256) -> Option<api::TransactionDetails>;

    /// Retrieve ZKsync transaction (as defined in `zks_getRawBlockTransactions`) by transaction's hash.
    /// Returns `None` if no transaction was found. Note that the transaction might still be a part
    /// of the chain but is available in the fork instead.
    async fn get_zksync_tx(&self, tx_hash: &H256) -> Option<zksync_types::Transaction>;

    /// Retrieve all logs matching given filter. Does not return matching logs from pre-fork blocks.
    async fn get_filter_logs(&self, log_filter: &LogFilter) -> Vec<api::Log>;

    /// Retrieve batch header by its number.
    async fn get_batch_header(&self, batch_number: L1BatchNumber) -> Option<L1BatchHeader>;

    /// Retrieve batch state diffs by its number.
    async fn get_batch_state_diffs(
        &self,
        batch_number: L1BatchNumber,
    ) -> Option<Vec<StateDiffRecord>>;

    /// Retrieve batch aggregation root by its number.
    async fn get_batch_aggregation_root(&self, batch_number: L1BatchNumber) -> Option<H256>;

    async fn get_raw_transaction(&self, tx_hash: H256) -> Option<Bytes>;

    async fn get_raw_transactions(&self, block_number: BlockId) -> Vec<Bytes>;
}

impl Clone for Box<dyn ReadBlockchain> {
    fn clone(&self) -> Self {
        self.dyn_cloned()
    }
}

#[derive(Debug, Clone)]
pub(super) struct Blockchain {
    inner: Arc<RwLock<BlockchainState>>,
    pub(super) protocol_version: ProtocolVersionId,
}

impl Blockchain {
    async fn inspect_block_by_hash<T>(
        &self,
        hash: &H256,
        f: impl FnOnce(&api::Block<api::TransactionVariant>) -> T,
    ) -> Option<T> {
        Some(f(self.inner.read().await.blocks.get(hash)?))
    }

    async fn inspect_block_by_number<T>(
        &self,
        number: L2BlockNumber,
        f: impl FnOnce(&api::Block<api::TransactionVariant>) -> T,
    ) -> Option<T> {
        let storage = self.inner.read().await;
        let hash = storage.get_block_hash_by_number(number)?;
        Some(f(storage.blocks.get(&hash)?))
    }

    async fn inspect_block_by_id<T>(
        &self,
        block_id: api::BlockId,
        f: impl FnOnce(&api::Block<api::TransactionVariant>) -> T,
    ) -> Option<T> {
        let storage = self.inner.read().await;
        let hash = storage.get_block_hash_by_id(block_id)?;
        Some(f(storage.blocks.get(&hash)?))
    }

    async fn inspect_tx<T>(
        &self,
        tx_hash: &H256,
        f: impl FnOnce(&TransactionResult) -> T,
    ) -> Option<T> {
        Some(f(self.inner.read().await.tx_results.get(tx_hash)?))
    }

    async fn inspect_batch<T>(
        &self,
        batch_number: &L1BatchNumber,
        f: impl FnOnce(&StoredL1BatchInfo) -> T,
    ) -> Option<T> {
        Some(f(self.inner.read().await.batches.get(batch_number)?))
    }

    // FIXME: Do not use for new functionality and delete once its only usage is migrated away.
    async fn inspect_all_txs<T>(
        &self,
        f: impl FnOnce(&HashMap<H256, TransactionResult>) -> T,
    ) -> T {
        f(&self.inner.read().await.tx_results)
    }
}

#[async_trait]
impl ReadBlockchain for Blockchain {
    fn dyn_cloned(&self) -> Box<dyn ReadBlockchain> {
        Box::new(self.clone())
    }

    fn protocol_version(&self) -> ProtocolVersionId {
        self.protocol_version
    }

    async fn current_batch(&self) -> L1BatchNumber {
        self.inner.read().await.current_batch
    }

    async fn current_block_number(&self) -> L2BlockNumber {
        self.inner.read().await.current_block
    }

    async fn current_block_hash(&self) -> H256 {
        self.inner.read().await.current_block_hash
    }

    async fn get_block_by_hash(&self, hash: &H256) -> Option<api::Block<api::TransactionVariant>> {
        self.inspect_block_by_hash(hash, |block| block.clone())
            .await
    }

    async fn get_block_by_number(
        &self,
        number: L2BlockNumber,
    ) -> Option<api::Block<api::TransactionVariant>> {
        self.inspect_block_by_number(number, |block| block.clone())
            .await
    }

    async fn get_block_by_id(
        &self,
        block_id: api::BlockId,
    ) -> Option<api::Block<api::TransactionVariant>> {
        self.inspect_block_by_id(block_id, |block| block.clone())
            .await
    }

    async fn get_block_hash_by_number(&self, number: L2BlockNumber) -> Option<H256> {
        self.inspect_block_by_number(number, |block| block.hash)
            .await
    }

    async fn get_block_hash_by_id(&self, block_id: api::BlockId) -> Option<H256> {
        self.inspect_block_by_id(block_id, |block| block.hash).await
    }

    async fn get_block_number_by_hash(&self, hash: &H256) -> Option<L2BlockNumber> {
        self.inspect_block_by_hash(hash, |block| L2BlockNumber(block.number.as_u32()))
            .await
    }

    async fn get_block_number_by_id(&self, block_id: api::BlockId) -> Option<L2BlockNumber> {
        self.inspect_block_by_id(block_id, |block| L2BlockNumber(block.number.as_u32()))
            .await
    }

    async fn get_block_tx_hashes_by_number(&self, number: L2BlockNumber) -> Option<Vec<H256>> {
        self.get_block_tx_hashes_by_id(api::BlockId::Number(api::BlockNumber::Number(
            number.0.into(),
        )))
        .await
    }

    async fn get_block_tx_hashes_by_id(&self, block_id: api::BlockId) -> Option<Vec<H256>> {
        self.inspect_block_by_id(block_id, |block| {
            block
                .transactions
                .iter()
                .map(|tx| match tx {
                    api::TransactionVariant::Full(tx) => tx.hash,
                    api::TransactionVariant::Hash(hash) => *hash,
                })
                .collect_vec()
        })
        .await
    }

    async fn get_block_tx_by_id(
        &self,
        block_id: api::BlockId,
        index: usize,
    ) -> Option<api::Transaction> {
        self.inspect_block_by_id(block_id, |block| {
            block.transactions.get(index).map(|tv| match tv {
                api::TransactionVariant::Full(tx) => tx.clone(),
                api::TransactionVariant::Hash(_) => {
                    unreachable!("we only store full txs in blocks")
                }
            })
        })
        .await
        .flatten()
    }

    async fn get_block_tx_count_by_id(&self, block_id: api::BlockId) -> Option<usize> {
        self.inspect_block_by_id(block_id, |block| block.transactions.len())
            .await
    }

    async fn get_block_details_by_number(
        &self,
        number: L2BlockNumber,
        l2_fair_gas_price: u64,
        fair_pubdata_price: Option<u64>,
        base_system_contracts_hashes: BaseSystemContractsHashes,
    ) -> Option<api::BlockDetails> {
        self.inspect_block_by_number(number, |block| api::BlockDetails {
            number: L2BlockNumber(block.number.as_u32()),
            l1_batch_number: L1BatchNumber(block.l1_batch_number.unwrap_or_default().as_u32()),
            base: api::BlockDetailsBase {
                timestamp: block.timestamp.as_u64(),
                l1_tx_count: 1,
                l2_tx_count: block.transactions.len(),
                root_hash: Some(block.hash),
                status: api::BlockStatus::Verified,
                commit_tx_hash: None,
                commit_tx_finality: None,
                commit_chain_id: None,
                committed_at: None,
                prove_tx_hash: None,
                prove_tx_finality: None,
                prove_chain_id: None,
                proven_at: None,
                execute_tx_hash: None,
                execute_tx_finality: None,
                execute_chain_id: None,
                executed_at: None,
                precommit_tx_hash: None,
                precommit_tx_finality: None,
                precommitted_at: None,
                precommit_chain_id: None,
                l1_gas_price: 0,
                l2_fair_gas_price,
                fair_pubdata_price,
                base_system_contracts_hashes,
                commit_tx_finality: None,
                prove_tx_finality: None,
                execute_tx_finality: None,
            },
            operator_address: Address::zero(),
            protocol_version: Some(self.protocol_version),
        })
        .await
    }

    async fn get_tx_receipt(&self, tx_hash: &H256) -> Option<api::TransactionReceipt> {
        self.inspect_tx(tx_hash, |tx| tx.receipt.clone()).await
    }

    async fn get_tx_debug_info(&self, tx_hash: &H256, only_top: bool) -> Option<api::DebugCall> {
        self.inspect_tx(tx_hash, |tx| tx.debug_info(only_top)).await
    }

    async fn get_tx_api(&self, tx_hash: &H256) -> anyhow::Result<Option<api::Transaction>> {
        self.inspect_tx(tx_hash, |TransactionResult { info, receipt, .. }| {
            let l2_tx: L2Tx =
                info.tx.clone().try_into().map_err(|_| {
                    anyhow::anyhow!("inspection of non-L2 transactions is unsupported")
                })?;
            let chain_id = l2_tx
                .common_data
                .extract_chain_id()
                .context("tx has malformed chain id")?;
            let input_data = l2_tx
                .common_data
                .input
                .context("tx is missing input data")?;
            anyhow::Ok(api::Transaction {
                hash: *tx_hash,
                nonce: U256::from(l2_tx.common_data.nonce.0),
                // FIXME: This is mega-incorrect but this whole method should be reworked in general
                block_hash: Some(*tx_hash),
                block_number: Some(U64::from(info.miniblock_number)),
                transaction_index: Some(receipt.transaction_index),
                from: Some(info.tx.initiator_account()),
                to: info.tx.recipient_account(),
                value: info.tx.execute.value,
                gas_price: Some(U256::from(0)),
                gas: Default::default(),
                input: input_data.data.into(),
                v: Some(chain_id.into()),
                r: Some(U256::zero()), // TODO: Shouldn't we set the signature?
                s: Some(U256::zero()), // TODO: Shouldn't we set the signature?
                y_parity: Some(U64::zero()), // TODO: Shouldn't we set the signature?
                raw: None,
                transaction_type: {
                    let tx_type = match l2_tx.common_data.transaction_type {
                        zksync_types::l2::TransactionType::LegacyTransaction => 0,
                        zksync_types::l2::TransactionType::EIP2930Transaction => 1,
                        zksync_types::l2::TransactionType::EIP1559Transaction => 2,
                        zksync_types::l2::TransactionType::EIP712Transaction => 113,
                        zksync_types::l2::TransactionType::PriorityOpTransaction => 255,
                        zksync_types::l2::TransactionType::ProtocolUpgradeTransaction => 254,
                    };
                    Some(tx_type.into())
                },
                access_list: None,
                max_fee_per_gas: Some(l2_tx.common_data.fee.max_fee_per_gas),
                max_priority_fee_per_gas: Some(l2_tx.common_data.fee.max_priority_fee_per_gas),
                chain_id: U256::from(chain_id),
                l1_batch_number: Some(U64::from(info.batch_number as u64)),
                l1_batch_tx_index: None,
            })
        })
        .await
        .transpose()
    }

    async fn get_detailed_tx(&self, tx: api::Transaction) -> Option<DetailedTransaction> {
        self.inspect_tx(
            &tx.hash.clone(),
            |TransactionResult { ref debug, .. }| {
                let output = Some(debug.output.clone());
                let revert_reason = debug.revert_reason.clone();
                DetailedTransaction {
                    inner: tx,
                    output,
                    revert_reason,
                }
            },
        )
        .await
    }

    async fn get_tx_details(&self, tx_hash: &H256) -> Option<api::TransactionDetails> {
        self.inspect_tx(tx_hash, |TransactionResult { info, receipt, .. }| {
            api::TransactionDetails {
                is_l1_originated: false,
                status: api::TransactionStatus::Included,
                // if these are not set, fee is effectively 0
                fee: receipt.effective_gas_price.unwrap_or_default()
                    * receipt.gas_used.unwrap_or_default(),
                gas_per_pubdata: info.tx.gas_per_pubdata_byte_limit(),
                initiator_address: info.tx.initiator_account(),
                received_at: utc_datetime_from_epoch_ms(info.tx.received_timestamp_ms),
                eth_commit_tx_hash: None,
                eth_prove_tx_hash: None,
                eth_execute_tx_hash: None,
                eth_precommit_tx_hash: None,
            }
        })
        .await
    }

    async fn get_zksync_tx(&self, tx_hash: &H256) -> Option<zksync_types::Transaction> {
        self.inspect_tx(tx_hash, |TransactionResult { info, .. }| info.tx.clone())
            .await
    }

    async fn get_filter_logs(&self, log_filter: &LogFilter) -> Vec<api::Log> {
        let latest_block_number = self.current_block_number().await;
        // FIXME: This should traverse blocks from `log_filter.from_block` to `log_filter.to_block`
        //        instead. This way we can drastically reduce search scope and avoid holding the
        //        lock for prolonged amounts of time.
        self.inspect_all_txs(|tx_results| {
            tx_results
                .values()
                .flat_map(|tx_result| {
                    tx_result
                        .receipt
                        .logs
                        .iter()
                        .filter(|log| log_filter.matches(log, U64::from(latest_block_number.0)))
                        .cloned()
                })
                .collect_vec()
        })
        .await
    }

    async fn get_batch_header(&self, batch_number: L1BatchNumber) -> Option<L1BatchHeader> {
        self.inspect_batch(&batch_number, |StoredL1BatchInfo { header, .. }| {
            header.clone()
        })
        .await
    }

    async fn get_batch_state_diffs(
        &self,
        batch_number: L1BatchNumber,
    ) -> Option<Vec<StateDiffRecord>> {
        self.inspect_batch(
            &batch_number,
            |StoredL1BatchInfo { state_diffs, .. }| state_diffs.clone(),
        )
        .await
    }

    async fn get_batch_aggregation_root(&self, batch_number: L1BatchNumber) -> Option<H256> {
        self.inspect_batch(
            &batch_number,
            |StoredL1BatchInfo {
                 aggregation_root, ..
             }| *aggregation_root,
        )
        .await
    }

    async fn get_raw_transaction(&self, tx_hash: H256) -> Option<Bytes> {
        // self.inspect_tx(tx_hash, |TransactionResult { info, .. }| info.tx.clone())
        //     .await
        None
        // kl todo
    }

    async fn get_raw_transactions(&self, block_number: BlockId) -> Vec<Bytes> {
        // self.inspect_block_by_number(block_number, |block| block.transactions.clone())
        //     .await
        vec![]
        // kl todo
    }
}

impl Blockchain {
    pub(super) fn new(
        protocol_version: ProtocolVersionId,
        fork_details: Option<&ForkDetails>,
        genesis: Option<&Genesis>,
        genesis_timestamp: Option<u64>,
    ) -> Blockchain {
        let state = if let Some(fork_details) = fork_details {
            BlockchainState {
                protocol_version: fork_details.protocol_version,
                current_batch: fork_details.batch_number,
                current_block: fork_details.block_number,
                current_block_hash: fork_details.block_hash,
                tx_results: Default::default(),
                blocks: HashMap::from_iter([(
                    fork_details.block_hash,
                    fork_details.api_block.clone(),
                )]),
                hashes: HashMap::from_iter([(fork_details.block_number, fork_details.block_hash)]),
                // As we do not support L1-L2 communication when running in forking mode, batches are
                // irrelevant.
                batches: HashMap::from_iter([]),
            }
        } else {
            let (genesis_block, genesis_batch_header) = if let Some(genesis) = genesis {
                create_genesis_from_json(protocol_version, genesis, genesis_timestamp)
            } else {
                create_genesis(protocol_version, genesis_timestamp)
            };
            let block_hash = genesis_block.hash;
            let genesis_batch_info = StoredL1BatchInfo {
                header: genesis_batch_header,
                state_diffs: Vec::new(),
                aggregation_root: H256::zero(),
            };

            BlockchainState {
                protocol_version,
                current_batch: L1BatchNumber(0),
                current_block: L2BlockNumber(0),
                current_block_hash: block_hash,
                tx_results: Default::default(),
                blocks: HashMap::from_iter([(block_hash, genesis_block)]),
                hashes: HashMap::from_iter([(L2BlockNumber(0), block_hash)]),
                batches: HashMap::from_iter([(L1BatchNumber(0), genesis_batch_info)]),
            }
        };
        let protocol_version = state.protocol_version;
        let inner = Arc::new(RwLock::new(state));
        Self {
            inner,
            protocol_version,
        }
    }
}

impl Blockchain {
    pub(super) async fn read(&self) -> RwLockReadGuard<BlockchainState> {
        self.inner.read().await
    }

    pub(super) async fn write(&self) -> RwLockWriteGuard<BlockchainState> {
        self.inner.write().await
    }
}

/// Stores the blockchain data (blocks, transactions)
#[derive(Debug, Clone)]
pub(super) struct BlockchainState {
    /// Protocol version for all produced blocks.
    pub(super) protocol_version: ProtocolVersionId,
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
    /// Map from batch number to batch info. Hash is not used as the key because it is not
    /// necessarily computed by the time this entry is inserted (i.e. it is not an inherent property
    /// of a batch).
    batches: HashMap<L1BatchNumber, StoredL1BatchInfo>,
}

/// Represents stored information about a particular batch.
#[derive(Debug, Clone)]
struct StoredL1BatchInfo {
    header: L1BatchHeader,
    state_diffs: Vec<StateDiffRecord>,
    aggregation_root: H256,
}

impl BlockchainState {
    pub(super) fn get_block_hash_by_number(&self, number: L2BlockNumber) -> Option<H256> {
        self.hashes.get(&number).copied()
    }

    pub(super) fn get_block_hash_by_id(&self, block_id: api::BlockId) -> Option<H256> {
        match block_id {
            api::BlockId::Number(number) => {
                let number = match number {
                    api::BlockNumber::FastFinalized | // TODO: Review how we handle these
                    api::BlockNumber::Finalized
                    | api::BlockNumber::Pending
                    | api::BlockNumber::Committed
                    | api::BlockNumber::L1Committed
                    | api::BlockNumber::Latest
                    | api::BlockNumber::FastFinalized
                    | api::BlockNumber::Precommitted => self.current_block,
                    api::BlockNumber::Earliest => L2BlockNumber(0),
                    api::BlockNumber::Number(n) => L2BlockNumber(n.as_u32())
                };
                self.hashes.get(&number).copied()
            }
            api::BlockId::Hash(hash) => Some(hash),
        }
    }

    pub(super) fn last_env<S: ReadStorage>(
        &self,
        storage: &StoragePtr<S>,
        time_writer: &Time,
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
            timestamp: time_writer.current_timestamp(),
        });
        (last_l1_batch_number, last_l2_block)
    }

    pub(super) fn apply_block(&mut self, block: api::Block<api::TransactionVariant>, index: u32) {
        let latest_block = self.blocks.get(&self.current_block_hash).unwrap();
        self.current_block += 1;

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

    pub(super) fn apply_batch(
        &mut self,
        batch_timestamp: u64,
        base_system_contracts_hashes: BaseSystemContractsHashes,
        tx_results: Vec<TransactionResult>,
        finished_l1_batch: FinishedL1Batch,
        aggregation_root: H256,
    ) {
        self.current_batch += 1;

        let l2_to_l1_messages = VmEvent::extract_long_l2_to_l1_messages(
            &finished_l1_batch.final_execution_state.events,
        );
        let l1_tx_count = tx_results
            .iter()
            .filter(|tx| matches!(tx.info.tx.common_data, ExecuteTransactionCommon::L1(_)))
            .count() as u16;
        let priority_ops_onchain_data = tx_results
            .iter()
            .filter_map(|tx| match &tx.info.tx.common_data {
                ExecuteTransactionCommon::L1(l1_tx) => {
                    Some(l1_tx.onchain_metadata().onchain_data.clone())
                }
                ExecuteTransactionCommon::L2(_) => None,
                ExecuteTransactionCommon::ProtocolUpgrade(_) => None,
            })
            .collect();
        let header = L1BatchHeader {
            number: self.current_batch,
            timestamp: batch_timestamp,
            l1_tx_count,
            l2_tx_count: tx_results.len() as u16 - l1_tx_count,
            priority_ops_onchain_data,
            l2_to_l1_logs: finished_l1_batch.final_execution_state.user_l2_to_l1_logs,
            l2_to_l1_messages,
            bloom: Default::default(), // This is unused in core
            used_contract_hashes: finished_l1_batch.final_execution_state.used_contract_hashes,
            base_system_contracts_hashes,
            system_logs: finished_l1_batch.final_execution_state.system_logs,
            protocol_version: Some(self.protocol_version),
            pubdata_input: finished_l1_batch.pubdata_input,
            fee_address: Default::default(), // TODO: Use real fee address
            batch_fee_input: Default::default(), // TODO: Use real batch fee input
        };
        let batch_info = StoredL1BatchInfo {
            header,
            state_diffs: finished_l1_batch.state_diffs.unwrap_or_default(),
            aggregation_root,
        };
        self.batches.insert(self.current_batch, batch_info);
        self.tx_results.extend(
            tx_results
                .into_iter()
                .map(|r| (r.receipt.transaction_hash, r)),
        );
    }

    pub(super) fn load_blocks(
        &mut self,
        time: &mut Time,
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
