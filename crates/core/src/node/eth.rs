use std::collections::HashSet;

use crate::formatter::ExecutionErrorReport;
use crate::node::error::{ToHaltError, ToRevertReason};
use anvil_zksync_common::{sh_err, sh_println};
use anyhow::Context as _;
use zksync_error::anvil_zksync::{halt::HaltError, revert::RevertError};
use zksync_multivm::interface::ExecutionResult;
use zksync_multivm::vm_latest::constants::ETH_CALL_GAS_LIMIT;
use zksync_types::h256_to_u256;
use zksync_types::{
    api,
    api::{Block, BlockIdVariant, BlockNumber, TransactionVariant},
    get_code_key,
    l2::L2Tx,
    transaction_request::TransactionRequest,
    PackedEthSignature, MAX_L1_TRANSACTION_GAS_LIMIT,
};
use zksync_types::{
    web3::{self, Bytes},
    Address, H160, H256, U256, U64,
};
use zksync_web3_decl::{
    error::Web3Error,
    types::{FeeHistory, Filter, FilterChanges, SyncState},
};

use crate::{
    filters::{FilterType, LogFilter},
    node::{InMemoryNode, MAX_TX_SIZE, PROTOCOL_VERSION},
    utils::{h256_to_u64, TransparentError},
};

impl InMemoryNode {
    pub async fn call_impl(
        &self,
        req: zksync_types::transaction_request::CallRequest,
    ) -> Result<Bytes, Web3Error> {
        let system_contracts = self.system_contracts.contracts_for_l2_call().clone();
        let mut tx = L2Tx::from_request(
            req.into(),
            MAX_TX_SIZE,
            self.system_contracts.allow_no_target(),
        )?;
        tx.common_data.fee.gas_limit = ETH_CALL_GAS_LIMIT.into();
        let call_result = self
            .run_l2_call(tx.clone(), system_contracts)
            .await
            .context("Invalid data due to invalid name")?;

        match call_result {
            ExecutionResult::Success { output } => Ok(output.into()),
            ExecutionResult::Revert { output } => {
                let message = output.to_user_friendly_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );

                let revert_reason: RevertError = output.clone().to_revert_reason().await;
                let error_report = ExecutionErrorReport::new(&revert_reason, Some(&tx));
                sh_println!("{}", error_report);

                Err(Web3Error::SubmitTransactionError(
                    pretty_message,
                    output.encoded_data(),
                ))
            }
            ExecutionResult::Halt { reason } => {
                let message = reason.to_string();
                let pretty_message = format!(
                    "execution halted {}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );

                let halt_error: HaltError = reason.clone().to_halt_error().await;
                let error_report = ExecutionErrorReport::new(&halt_error, Some(&tx));
                sh_println!("{}", error_report);

                Err(Web3Error::SubmitTransactionError(pretty_message, vec![]))
            }
        }
    }

    pub async fn send_raw_transaction_impl(&self, tx_bytes: Bytes) -> Result<H256, Web3Error> {
        let chain_id = self.chain_id().await;

        let (tx_req, hash) = TransactionRequest::from_bytes(&tx_bytes.0, chain_id)?;
        let mut l2_tx =
            L2Tx::from_request(tx_req, MAX_TX_SIZE, self.system_contracts.allow_no_target())?;

        l2_tx.set_input(tx_bytes.0, hash);
        if hash != l2_tx.hash() {
            let err = anyhow::anyhow!(
                "Invalid transaction data: computed hash does not match the provided hash."
            );
            return Err(err.into());
        };

        self.pool.add_tx(l2_tx);
        Ok(hash)
    }

    pub async fn send_transaction_impl(
        &self,
        mut tx: zksync_types::transaction_request::CallRequest,
    ) -> Result<H256, Web3Error> {
        let (chain_id, l2_gas_price) = {
            let reader = self.inner.read().await;
            (self.chain_id().await, reader.fee_input_provider.gas_price())
        };

        // Users might expect a "sensible default"
        if tx.gas.is_none() {
            tx.gas = Some(U256::from(MAX_L1_TRANSACTION_GAS_LIMIT));
        }

        // EIP-1559 gas fields should be processed separately
        if tx.gas_price.is_some() {
            if tx.max_fee_per_gas.is_some() || tx.max_priority_fee_per_gas.is_some() {
                let err = "Transaction contains unsupported fields: max_fee_per_gas or max_priority_fee_per_gas";
                sh_err!("{err}");
                return Err(TransparentError(err.into()).into());
            }
        } else {
            tx.gas_price = Some(tx.max_fee_per_gas.unwrap_or(U256::from(l2_gas_price)));
            tx.max_priority_fee_per_gas = Some(tx.max_priority_fee_per_gas.unwrap_or(U256::zero()));
            if tx.transaction_type.is_none() {
                tx.transaction_type = Some(zksync_types::EIP_1559_TX_TYPE.into());
            }
        }
        if tx.nonce.is_none() {
            let nonce_key = self.storage_key_layout.get_nonce_key(&tx.from.unwrap());
            tx.nonce = Some(h256_to_u256(self.storage.read_value_alt(&nonce_key).await?) + 1);
        }

        let mut tx_req = TransactionRequest::from(tx.clone());
        tx_req.chain_id = Some(chain_id.as_u64());
        // Needed to calculate hash. `v` is set to 0 as EIP1559 tx hash calculation uses boolean
        // parity instead of raw `v` value (i.e. 27 becomes 0, 28 becomes 1).
        tx_req.r = Some(U256::zero());
        tx_req.s = Some(U256::zero());
        tx_req.v = Some(U64::from(0));

        let bytes = tx_req.get_signed_bytes(&PackedEthSignature::from_rsv(
            &H256::zero(),
            &H256::zero(),
            0,
        ))?;

        let (mut tx_req, hash) = TransactionRequest::from_bytes(&bytes, chain_id)?;
        tx_req.from = tx.from;
        let mut l2_tx: L2Tx =
            L2Tx::from_request(tx_req, MAX_TX_SIZE, self.system_contracts.allow_no_target())?;
        l2_tx.set_input(bytes, hash);

        if !self
            .impersonation
            .is_impersonating(&l2_tx.common_data.initiator_address)
        {
            let err = format!(
                "Initiator address {:?} is not allowed to perform transactions",
                l2_tx.common_data.initiator_address
            );
            tracing::error!("\n{err}");
            return Err(TransparentError(err).into());
        }

        self.pool.add_tx(l2_tx);
        Ok(hash)
    }
}

impl InMemoryNode {
    pub async fn get_balance_impl(
        &self,
        address: Address,
        // TODO: Support
        _block: Option<BlockIdVariant>,
    ) -> anyhow::Result<U256> {
        let balance_key = self
            .storage_key_layout
            .get_storage_key_for_base_token(&address);
        match self.storage.read_value_alt(&balance_key).await {
            Ok(balance) => Ok(h256_to_u256(balance)),
            Err(error) => Err(anyhow::anyhow!("failed to read account balance: {error}")),
        }
    }

    pub async fn get_block_impl(
        &self,
        block_id: api::BlockId,
        full_transactions: bool,
    ) -> anyhow::Result<Option<Block<TransactionVariant>>> {
        let maybe_block = {
            if let Some(block) = self.blockchain.get_block_by_id(block_id).await {
                Some(block)
            } else {
                self.fork.get_block_by_id(block_id).await?
            }
        };

        match maybe_block {
            Some(mut block) => {
                let block_hash = block.hash;
                block.transactions = block
                    .transactions
                    .into_iter()
                    .map(|transaction| match &transaction {
                        TransactionVariant::Full(inner) => {
                            if full_transactions {
                                transaction
                            } else {
                                TransactionVariant::Hash(inner.hash)
                            }
                        }
                        TransactionVariant::Hash(_) => {
                            if full_transactions {
                                panic!("unexpected non full transaction for block {}", block_hash)
                            } else {
                                transaction
                            }
                        }
                    })
                    .collect();

                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    pub async fn get_code_impl(
        &self,
        address: Address,
        // TODO: Support
        _block: Option<BlockIdVariant>,
    ) -> anyhow::Result<Bytes> {
        let code_key = get_code_key(&address);
        match self.storage.read_value_alt(&code_key).await {
            Ok(code_hash) => match self.storage.load_factory_dep_alt(code_hash).await {
                Ok(raw_code) => {
                    let code = raw_code.unwrap_or_default();
                    Ok(Bytes::from(code))
                }
                Err(error) => Err(anyhow::anyhow!("failed to load factory dep: {error}")),
            },
            Err(error) => Err(anyhow::anyhow!("failed to read code storage: {error}")),
        }
    }

    pub async fn get_transaction_count_impl(
        &self,
        address: Address,
        // TODO: Support
        _block: Option<BlockIdVariant>,
    ) -> anyhow::Result<U256> {
        let nonce_key = self.storage_key_layout.get_nonce_key(&address);
        match self.storage.read_value_alt(&nonce_key).await {
            Ok(result) => Ok(h256_to_u64(result).into()),
            Err(error) => Err(anyhow::anyhow!("failed to read nonce storage: {error}")),
        }
    }

    pub async fn get_transaction_receipt_impl(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<api::TransactionReceipt>> {
        // TODO: Call fork if not found
        Ok(self.blockchain.get_tx_receipt(&hash).await)
    }

    pub async fn get_transaction_by_hash_impl(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<api::Transaction>> {
        // try retrieving transaction from memory, and if unavailable subsequently from the fork
        match self.blockchain.get_tx_api(&hash).await? {
            Some(tx) => Ok(Some(tx)),
            None => self.fork.get_transaction_by_hash(hash).await,
        }
    }

    pub async fn get_block_number_impl(&self) -> anyhow::Result<U64> {
        Ok(U64::from(self.blockchain.current_block_number().await.0))
    }

    pub async fn estimate_gas_impl(
        &self,
        req: zksync_types::transaction_request::CallRequest,
        // TODO: Support
        _block: Option<BlockNumber>,
    ) -> Result<U256, Web3Error> {
        let fee = self.inner.read().await.estimate_gas_impl(req).await?;
        Ok(fee.gas_limit)
    }

    pub async fn gas_price_impl(&self) -> anyhow::Result<U256> {
        let fair_l2_gas_price: u64 = self.inner.read().await.fee_input_provider.gas_price();
        Ok(U256::from(fair_l2_gas_price))
    }

    pub async fn new_filter_impl(&self, filter: Filter) -> anyhow::Result<U256> {
        let from_block = filter.from_block.unwrap_or(BlockNumber::Latest);
        let to_block = filter.to_block.unwrap_or(BlockNumber::Latest);
        let addresses = filter.address.unwrap_or_default().0;
        let mut topics: [Option<HashSet<H256>>; 4] = Default::default();

        if let Some(filter_topics) = filter.topics {
            filter_topics
                .into_iter()
                .take(4)
                .enumerate()
                .for_each(|(i, maybe_topic_set)| {
                    if let Some(topic_set) = maybe_topic_set {
                        topics[i] = Some(topic_set.0.into_iter().collect());
                    }
                })
        }
        self.inner
            .write()
            .await
            .filters
            .write()
            .await
            .add_log_filter(from_block, to_block, addresses, topics)
            .map_err(anyhow::Error::msg)
    }

    pub async fn new_block_filter_impl(&self) -> anyhow::Result<U256> {
        self.inner
            .write()
            .await
            .filters
            .write()
            .await
            .add_block_filter()
            .map_err(anyhow::Error::msg)
    }

    pub async fn new_pending_transaction_filter_impl(&self) -> anyhow::Result<U256> {
        self.inner
            .write()
            .await
            .filters
            .write()
            .await
            .add_pending_transaction_filter()
            .map_err(anyhow::Error::msg)
    }

    pub async fn uninstall_filter_impl(&self, id: U256) -> anyhow::Result<bool> {
        Ok(self
            .inner
            .write()
            .await
            .filters
            .write()
            .await
            .remove_filter(id))
    }

    pub async fn get_logs_impl(
        &self,
        filter: Filter,
    ) -> anyhow::Result<Vec<zksync_types::api::Log>> {
        let from_block = filter.from_block.unwrap_or(BlockNumber::Earliest);
        let to_block = filter.to_block.unwrap_or(BlockNumber::Latest);
        let addresses = filter.address.unwrap_or_default().0;
        let mut topics: [Option<HashSet<H256>>; 4] = Default::default();

        if let Some(filter_topics) = filter.topics {
            filter_topics
                .into_iter()
                .take(4)
                .enumerate()
                .for_each(|(i, maybe_topic_set)| {
                    if let Some(topic_set) = maybe_topic_set {
                        topics[i] = Some(topic_set.0.into_iter().collect());
                    }
                })
        }

        // TODO: LogFilter should really resolve `from_block` and `to_block` during init and not
        //       on every `matches` call.
        let log_filter = LogFilter::new(from_block, to_block, addresses, topics);
        Ok(self.blockchain.get_filter_logs(&log_filter).await)
    }

    pub async fn get_filter_logs_impl(&self, id: U256) -> anyhow::Result<FilterChanges> {
        let logs = match self.inner.read().await.filters.read().await.get_filter(id) {
            Some(FilterType::Log(log_filter)) => self.blockchain.get_filter_logs(log_filter).await,
            _ => {
                anyhow::bail!("Failed to acquire read lock for filter logs.")
            }
        };

        Ok(FilterChanges::Logs(logs))
    }

    pub async fn get_filter_changes_impl(&self, id: U256) -> anyhow::Result<FilterChanges> {
        self.inner
            .write()
            .await
            .filters
            .write()
            .await
            .get_new_changes(id)
            .map_err(anyhow::Error::msg)
    }

    pub async fn get_block_transaction_count_impl(
        &self,
        block_id: api::BlockId,
    ) -> Result<Option<U256>, Web3Error> {
        let count = match self.blockchain.get_block_tx_count_by_id(block_id).await {
            Some(count) => Some(U256::from(count)),
            None => {
                self.fork
                    .get_block_transaction_count_by_id(block_id)
                    .await?
            }
        };

        // TODO: Is this right? What is the purpose of having `Option` here then?
        match count {
            Some(count) => Ok(Some(count)),
            None => Err(Web3Error::NoBlock),
        }
    }

    pub async fn get_storage_impl(
        &self,
        address: Address,
        idx: U256,
        block: Option<api::BlockIdVariant>,
    ) -> Result<H256, Web3Error> {
        self.inner
            .read()
            .await
            .get_storage_at_block(address, idx, block)
            .await
    }

    pub async fn get_transaction_by_block_and_index_impl(
        &self,
        block_id: api::BlockId,
        index: web3::Index,
    ) -> anyhow::Result<Option<api::Transaction>> {
        match self
            .blockchain
            .get_block_tx_by_id(block_id, index.as_usize())
            .await
        {
            Some(tx) => Ok(Some(tx)),
            None => {
                self.fork
                    .get_transaction_by_block_id_and_index(block_id, index)
                    .await
            }
        }
    }

    pub fn protocol_version_impl(&self) -> String {
        PROTOCOL_VERSION.to_string()
    }

    pub fn syncing_impl(&self) -> SyncState {
        SyncState::NotSyncing
    }

    pub async fn accounts_impl(&self) -> anyhow::Result<Vec<H160>> {
        Ok(self
            .inner
            .read()
            .await
            .rich_accounts
            .clone()
            .into_iter()
            .collect())
    }

    pub async fn fee_history_impl(
        &self,
        block_count: u64,
        // TODO: Support
        _newest_block: BlockNumber,
        reward_percentiles: Option<Vec<f32>>,
    ) -> anyhow::Result<zksync_types::api::FeeHistory> {
        let current_block = self.blockchain.current_block_number().await;

        let block_count = (block_count as usize)
            .min(1024)
            // Can't be more than the total number of blocks
            .clamp(1, current_block.0 as usize + 1);

        let mut base_fee_per_gas =
            vec![U256::from(self.inner.read().await.fee_input_provider.gas_price()); block_count];

        let oldest_block = current_block + 1 - base_fee_per_gas.len() as u32;
        // We do not store gas used ratio for blocks, returns array of zeroes as a placeholder.
        let gas_used_ratio = vec![0.0; base_fee_per_gas.len()];
        // Effective priority gas price is currently 0.
        let reward = Some(vec![
            vec![
                U256::zero();
                reward_percentiles.map_or(0, |v| v.len())
            ];
            base_fee_per_gas.len()
        ]);

        // `base_fee_per_gas` for next miniblock cannot be calculated, appending last fee as a placeholder.
        base_fee_per_gas.push(*base_fee_per_gas.last().unwrap());

        Ok(zksync_types::api::FeeHistory {
            inner: FeeHistory {
                oldest_block: web3::BlockNumber::Number(oldest_block.0.into()),
                base_fee_per_gas,
                gas_used_ratio,
                reward,
                base_fee_per_blob_gas: Default::default(),
                blob_gas_used_ratio: Default::default(),
            },
            l2_pubdata_price: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::fork::{ForkClient, ForkConfig};
    use crate::node::TransactionResult;
    use crate::{
        node::{compute_hash, InMemoryNode},
        testing::{
            self, default_tx_debug_info, ForkBlockConfig, LogBuilder, MockServer,
            TransactionResponseBuilder,
        },
    };
    use anvil_zksync_config::constants::{
        DEFAULT_ACCOUNT_BALANCE, DEFAULT_L2_GAS_PRICE, NON_FORK_FIRST_BLOCK_TIMESTAMP,
    };
    use maplit::hashmap;
    use url::Url;
    use zksync_multivm::utils::get_max_batch_gas_limit;
    use zksync_types::l2::TransactionType;
    use zksync_types::vm::VmVersion;
    use zksync_types::{
        api,
        api::{BlockHashObject, BlockNumber, BlockNumberObject, TransactionReceipt},
        utils::deployed_address_create,
        Bloom, K256PrivateKey, L2BlockNumber, StorageKey, EMPTY_UNCLES_HASH,
    };
    use zksync_types::{u256_to_h256, web3, AccountTreeId, Nonce};
    use zksync_web3_decl::types::{SyncState, ValueOrArray};

    async fn test_node(url: Url) -> InMemoryNode {
        InMemoryNode::test(Some(
            ForkClient::at_block_number(ForkConfig::unknown(url), None)
                .await
                .unwrap(),
        ))
    }

    #[tokio::test]
    async fn test_eth_syncing() {
        let node = InMemoryNode::test(None);
        let syncing = node.syncing_impl();
        assert!(matches!(syncing, SyncState::NotSyncing));
    }

    #[tokio::test]
    async fn test_get_fee_history_with_1_block() {
        let node = InMemoryNode::test(None);

        let fee_history = node
            .fee_history_impl(1, BlockNumber::Latest, Some(vec![25.0, 50.0, 75.0]))
            .await
            .expect("fee_history failed")
            .inner;

        assert_eq!(
            fee_history.oldest_block,
            web3::BlockNumber::Number(U64::from(0))
        );
        assert_eq!(
            fee_history.base_fee_per_gas,
            vec![U256::from(DEFAULT_L2_GAS_PRICE); 2]
        );
        assert_eq!(fee_history.gas_used_ratio, vec![0.0]);
        assert_eq!(fee_history.reward, Some(vec![vec![U256::from(0); 3]]));
    }

    #[tokio::test]
    async fn test_get_fee_history_with_no_reward_percentiles() {
        let node = InMemoryNode::test(None);

        let fee_history = node
            .fee_history_impl(1, BlockNumber::Latest, Some(vec![]))
            .await
            .expect("fee_history failed")
            .inner;

        assert_eq!(
            fee_history.oldest_block,
            web3::BlockNumber::Number(U64::from(0))
        );
        assert_eq!(
            fee_history.base_fee_per_gas,
            vec![U256::from(DEFAULT_L2_GAS_PRICE); 2]
        );
        assert_eq!(fee_history.gas_used_ratio, vec![0.0]);
        assert_eq!(fee_history.reward, Some(vec![vec![]]));
    }

    #[tokio::test]
    async fn test_get_fee_history_with_multiple_blocks() {
        // Arrange
        let node = InMemoryNode::test(None);
        testing::apply_tx(&node, H256::repeat_byte(0x01)).await;

        // Act
        let latest_block = node
            .get_block_number_impl()
            .await
            .expect("Block number fetch failed");
        let fee_history = node
            .fee_history_impl(2, BlockNumber::Latest, Some(vec![25.0, 50.0, 75.0]))
            .await
            .expect("fee_history failed")
            .inner;

        // Assert
        // We should receive 2 fees: from block 1 and 2.
        assert_eq!(latest_block, U64::from(2));
        assert_eq!(
            fee_history.oldest_block,
            web3::BlockNumber::Number(U64::from(1))
        );
        assert_eq!(
            fee_history.base_fee_per_gas,
            vec![U256::from(DEFAULT_L2_GAS_PRICE); 3]
        );
        assert_eq!(fee_history.gas_used_ratio, vec![0.0, 0.0]);
        assert_eq!(fee_history.reward, Some(vec![vec![U256::from(0); 3]; 2]));
    }

    #[tokio::test]
    async fn test_get_block_by_hash_returns_none_for_non_existing_block() {
        let node = InMemoryNode::test(None);

        let result = node
            .get_block_impl(api::BlockId::Hash(H256::repeat_byte(0x01)), false)
            .await
            .expect("failed fetching block by hash");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_node_has_genesis_block() {
        let node = InMemoryNode::test(None);

        let block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::Latest), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");

        assert_eq!(0, block.number.as_u64());
        assert_eq!(compute_hash(0, []), block.hash);
    }

    #[tokio::test]
    async fn test_node_creates_genesis_block_with_hash_and_zero_parent_hash() {
        let node = InMemoryNode::test(None);

        let block = node
            .get_block_impl(api::BlockId::Hash(compute_hash(0, [])), false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(block.parent_hash, H256::zero());
    }

    #[tokio::test]
    async fn test_node_produces_blocks_with_parent_hash_links() {
        let node = InMemoryNode::test(None);
        testing::apply_tx(&node, H256::repeat_byte(0x01)).await;

        let genesis_block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::from(0)), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");
        let first_block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::from(1)), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");
        let second_block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::from(2)), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");

        assert_eq!(genesis_block.hash, first_block.parent_hash);
        assert_eq!(first_block.hash, second_block.parent_hash);
    }

    #[tokio::test]
    async fn test_get_block_by_hash_for_produced_block() {
        let node = InMemoryNode::test(None);
        let tx_hash = H256::repeat_byte(0x01);
        let (expected_block_hash, _, _) = testing::apply_tx(&node, tx_hash).await;
        let genesis_block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::from(0)), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");

        let actual_block = node
            .get_block_impl(api::BlockId::Hash(expected_block_hash), false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        let expected_block: Block<TransactionVariant> = Block {
            hash: expected_block_hash,
            parent_hash: genesis_block.hash,
            uncles_hash: EMPTY_UNCLES_HASH,
            author: Default::default(),
            state_root: Default::default(),
            transactions_root: Default::default(),
            receipts_root: Default::default(),
            number: U64::from(1),
            l1_batch_number: Some(U64::from(1)),
            gas_used: actual_block.gas_used, // Checked separately, see below
            gas_limit: U256::from(get_max_batch_gas_limit(VmVersion::latest())),
            base_fee_per_gas: actual_block.base_fee_per_gas, // Checked separately, see below
            extra_data: Default::default(),
            logs_bloom: actual_block.logs_bloom, // Checked separately, see below
            timestamp: U256::from(NON_FORK_FIRST_BLOCK_TIMESTAMP + 1),
            l1_batch_timestamp: Some(U256::from(NON_FORK_FIRST_BLOCK_TIMESTAMP + 1)),
            difficulty: Default::default(),
            total_difficulty: Default::default(),
            seal_fields: vec![],
            uncles: vec![],
            transactions: vec![TransactionVariant::Hash(tx_hash)],
            size: Default::default(),
            mix_hash: Default::default(),
            nonce: Default::default(),
        };

        assert_eq!(expected_block, actual_block);

        // It is hard to predict the values below without repeating the exact logic used to calculate
        // them. We are resorting to some basic sanity checks instead.
        assert!(actual_block.gas_used > U256::zero());
        assert!(actual_block.base_fee_per_gas > U256::zero());
        assert_ne!(actual_block.logs_bloom, Bloom::zero());
    }

    #[tokio::test]
    async fn test_node_block_mapping_is_correctly_populated_when_using_fork_source() {
        let input_block_number = 8;
        let input_block_hash = H256::repeat_byte(0x01);
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: input_block_number,
            hash: input_block_hash,
            transaction_count: 0,
        });

        let node = test_node(mock_server.url()).await;
        assert!(
            node.blockchain
                .get_block_by_hash(&input_block_hash)
                .await
                .is_some(),
            "block wasn't cached"
        );
    }

    #[tokio::test]
    async fn test_get_block_by_hash_uses_fork_source() {
        let input_block_hash = H256::repeat_byte(0x01);

        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let mock_block_number = 8;
        let block_response = testing::BlockResponseBuilder::new()
            .set_hash(input_block_hash)
            .set_number(mock_block_number)
            .build_result();
        mock_server.expect(
            "eth_getBlockByHash",
            Some(serde_json::json!([format!("{input_block_hash:#x}"), true])),
            block_response,
        );
        let node = test_node(mock_server.url()).await;

        let actual_block = node
            .get_block_impl(api::BlockId::Hash(input_block_hash), false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(mock_block_number), actual_block.number);
        assert_eq!(Some(U64::from(6)), actual_block.l1_batch_number);
    }

    #[tokio::test]
    async fn test_get_block_by_number_returns_none_for_non_existing_block() {
        let node = InMemoryNode::test(None);

        let result = node
            .get_block_impl(
                api::BlockId::Number(BlockNumber::Number(U64::from(42))),
                false,
            )
            .await
            .expect("failed fetching block by number");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_block_by_number_for_produced_block() {
        let node = InMemoryNode::test(None);
        let tx_hash = H256::repeat_byte(0x01);
        let (expected_block_hash, _, _) = testing::apply_tx(&node, tx_hash).await;
        let expected_block_number = 1;
        let genesis_block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::from(0)), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");

        let actual_block = node
            .get_block_impl(
                api::BlockId::Number(BlockNumber::Number(U64::from(expected_block_number))),
                false,
            )
            .await
            .expect("failed fetching block by number")
            .expect("no block");

        let expected_block: Block<TransactionVariant> = Block {
            hash: expected_block_hash,
            parent_hash: genesis_block.hash,
            uncles_hash: EMPTY_UNCLES_HASH,
            author: Default::default(),
            state_root: Default::default(),
            transactions_root: Default::default(),
            receipts_root: Default::default(),
            number: U64::from(expected_block_number),
            l1_batch_number: Some(U64::from(1)),
            gas_used: actual_block.gas_used, // Checked separately, see below
            gas_limit: U256::from(get_max_batch_gas_limit(VmVersion::latest())),
            base_fee_per_gas: actual_block.base_fee_per_gas, // Checked separately, see below
            extra_data: Default::default(),
            logs_bloom: actual_block.logs_bloom, // Checked separately, see below
            timestamp: U256::from(NON_FORK_FIRST_BLOCK_TIMESTAMP + 1),
            l1_batch_timestamp: Some(U256::from(NON_FORK_FIRST_BLOCK_TIMESTAMP + 1)),
            difficulty: Default::default(),
            total_difficulty: Default::default(),
            seal_fields: vec![],
            uncles: vec![],
            transactions: vec![TransactionVariant::Hash(tx_hash)],
            size: Default::default(),
            mix_hash: Default::default(),
            nonce: Default::default(),
        };

        assert_eq!(expected_block, actual_block);

        // It is hard to predict the values below without repeating the exact logic used to calculate
        // them. We are resorting to some basic sanity checks instead.
        assert!(actual_block.gas_used > U256::zero());
        assert!(actual_block.base_fee_per_gas > U256::zero());
        assert_ne!(actual_block.logs_bloom, Bloom::zero());
    }

    #[tokio::test]
    async fn test_get_block_by_number_for_produced_block_full_txs() {
        let node = InMemoryNode::test(None);
        let tx_hash = H256::repeat_byte(0x01);
        let (block_hash, _, tx) = testing::apply_tx(&node, tx_hash).await;
        let expected_block_number = 1;

        let mut actual_block = node
            .get_block_impl(
                api::BlockId::Number(BlockNumber::Number(U64::from(expected_block_number))),
                true,
            )
            .await
            .expect("failed fetching block by number")
            .expect("no block");

        assert_eq!(actual_block.transactions.len(), 1);
        let actual_tx = match actual_block.transactions.remove(0) {
            TransactionVariant::Full(tx) => tx,
            TransactionVariant::Hash(_) => unreachable!(),
        };
        let expected_tx = api::Transaction {
            hash: tx_hash,
            nonce: U256::from(0),
            block_hash: Some(block_hash),
            block_number: Some(U64::from(expected_block_number)),
            transaction_index: Some(U64::from(0)),
            from: Some(tx.initiator_account()),
            to: tx.recipient_account(),
            value: U256::from(1),
            gas_price: Some(tx.common_data.fee.max_fee_per_gas),
            gas: tx.common_data.fee.gas_limit,
            input: Default::default(),
            v: actual_tx.v,               // Checked separately, see below
            r: actual_tx.r,               // Checked separately, see below
            s: actual_tx.s,               // Checked separately, see below
            y_parity: actual_tx.y_parity, // Checked separately, see below
            raw: None,
            transaction_type: Some(U64::from(TransactionType::EIP712Transaction as u32)),
            access_list: None,
            max_fee_per_gas: Some(tx.common_data.fee.max_fee_per_gas),
            max_priority_fee_per_gas: Some(tx.common_data.fee.max_priority_fee_per_gas),
            chain_id: U256::from(260),
            l1_batch_number: Some(U64::from(1)),
            l1_batch_tx_index: Some(U64::from(0)),
        };
        assert_eq!(expected_tx, actual_tx);

        // TODO: Verify that the TX is signed properly (use alloy to abstract from zksync-core code?)
    }

    #[tokio::test]
    async fn test_get_block_by_number_uses_fork_source_if_missing_number() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let mock_block_number = 8;
        let block_response = testing::BlockResponseBuilder::new()
            .set_number(mock_block_number)
            .build_result();
        mock_server.expect(
            "eth_getBlockByNumber",
            Some(serde_json::json!(["0x8", true])),
            block_response,
        );
        let node = test_node(mock_server.url()).await;

        let actual_block = node
            .get_block_impl(
                api::BlockId::Number(BlockNumber::Number(U64::from(8))),
                false,
            )
            .await
            .expect("failed fetching block by hash")
            .expect("no block");
        assert_eq!(U64::from(mock_block_number), actual_block.number);
    }

    #[tokio::test]
    async fn test_get_block_by_number_for_latest_block_produced_locally() {
        let node = InMemoryNode::test(None);
        testing::apply_tx(&node, H256::repeat_byte(0x01)).await;

        // The latest block, will be the 'virtual' one with 0 transactions (block 2).
        let virtual_block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::Latest), true)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(U64::from(2), virtual_block.number);
        assert_eq!(0, virtual_block.transactions.len());

        let actual_block = node
            .get_block_impl(
                api::BlockId::Number(BlockNumber::Number(U64::from(1))),
                true,
            )
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(U64::from(1), actual_block.number);
        assert_eq!(1, actual_block.transactions.len());
    }

    #[tokio::test]
    async fn test_get_block_by_number_uses_locally_available_block_for_latest_block() {
        let input_block_number = 10;
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: input_block_number,
            hash: H256::repeat_byte(0x01),
            transaction_count: 0,
        });

        let node = test_node(mock_server.url()).await;

        let actual_block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::Latest), false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");
        assert_eq!(U64::from(input_block_number), actual_block.number);
    }

    #[tokio::test]
    async fn test_get_block_by_number_uses_fork_source_for_earliest_block() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let input_block_number = 1;
        mock_server.expect(
            "eth_getBlockByNumber",
            Some(serde_json::json!(["earliest", true])),
            testing::BlockResponseBuilder::new()
                .set_number(input_block_number)
                .build_result(),
        );
        let node = test_node(mock_server.url()).await;

        let actual_block = node
            .get_block_impl(api::BlockId::Number(BlockNumber::Earliest), false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");
        assert_eq!(U64::from(input_block_number), actual_block.number);
    }

    #[tokio::test]
    async fn test_get_block_by_number_uses_locally_available_for_latest_alike_blocks() {
        for block_number in [
            BlockNumber::Pending,
            BlockNumber::Committed,
            BlockNumber::Finalized,
        ] {
            let input_block_number = 10;
            let mock_server = MockServer::run_with_config(ForkBlockConfig {
                number: input_block_number,
                hash: H256::repeat_byte(0xab),
                transaction_count: 0,
            });
            let node = test_node(mock_server.url()).await;

            let actual_block = node
                .get_block_impl(api::BlockId::Number(block_number), false)
                .await
                .expect("failed fetching block by hash")
                .expect("no block");
            assert_eq!(
                U64::from(input_block_number),
                actual_block.number,
                "case {}",
                block_number,
            );
        }
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_hash_for_produced_block() {
        let node = InMemoryNode::test(None);

        let (expected_block_hash, _, _) = testing::apply_tx(&node, H256::repeat_byte(0x01)).await;
        let actual_transaction_count = node
            .get_block_transaction_count_impl(api::BlockId::Hash(expected_block_hash))
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(U256::from(1), actual_transaction_count);
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_hash_uses_fork_source() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let input_block_hash = H256::repeat_byte(0x01);
        let input_transaction_count = 1;
        mock_server.expect(
            "eth_getBlockTransactionCountByHash",
            Some(serde_json::json!([format!("{:#x}", input_block_hash)])),
            serde_json::json!(format!("{:#x}", input_transaction_count)),
        );
        let node = test_node(mock_server.url()).await;

        let actual_transaction_count = node
            .get_block_transaction_count_impl(api::BlockId::Hash(input_block_hash))
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(
            U256::from(input_transaction_count),
            actual_transaction_count
        );
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_number_for_produced_block() {
        let node = InMemoryNode::test(None);

        testing::apply_tx(&node, H256::repeat_byte(0x01)).await;
        let actual_transaction_count = node
            .get_block_transaction_count_impl(api::BlockId::Number(api::BlockNumber::Number(
                U64::from(1),
            )))
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(U256::from(1), actual_transaction_count);
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_number_uses_fork_source() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let input_block_number = 1;
        let input_transaction_count = 1;
        mock_server.expect(
            "eth_getBlockTransactionCountByNumber",
            Some(serde_json::json!([format!("{:#x}", input_block_number)])),
            serde_json::json!(format!("{:#x}", input_transaction_count)),
        );

        let node = test_node(mock_server.url()).await;

        let actual_transaction_count = node
            .get_block_transaction_count_impl(api::BlockId::Number(BlockNumber::Number(U64::from(
                1,
            ))))
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(
            U256::from(input_transaction_count),
            actual_transaction_count
        );
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_number_earliest_uses_fork_source() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let input_transaction_count = 1;
        mock_server.expect(
            "eth_getBlockTransactionCountByNumber",
            Some(serde_json::json!(["earliest"])),
            serde_json::json!(format!("{:#x}", input_transaction_count)),
        );

        let node = test_node(mock_server.url()).await;

        let actual_transaction_count = node
            .get_block_transaction_count_impl(api::BlockId::Number(BlockNumber::Earliest))
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(
            U256::from(input_transaction_count),
            actual_transaction_count
        );
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_number_latest_alike_uses_fork_source() {
        for block_number in [
            BlockNumber::Latest,
            BlockNumber::Pending,
            BlockNumber::Committed,
            BlockNumber::Finalized,
        ] {
            let input_transaction_count = 1;
            let mock_server = MockServer::run_with_config(ForkBlockConfig {
                number: 10,
                transaction_count: input_transaction_count,
                hash: H256::repeat_byte(0xab),
            });

            let node = test_node(mock_server.url()).await;

            let actual_transaction_count = node
                .get_block_transaction_count_impl(api::BlockId::Number(block_number))
                .await
                .expect("failed fetching block by hash")
                .expect("no result");

            assert_eq!(
                U256::from(input_transaction_count),
                actual_transaction_count,
                "case {}",
                block_number,
            );
        }
    }

    #[tokio::test]
    async fn test_get_transaction_receipt_uses_produced_block_hash() {
        let node = InMemoryNode::test(None);
        let tx_hash = H256::repeat_byte(0x01);
        let (expected_block_hash, _, _) = testing::apply_tx(&node, tx_hash).await;

        let actual_tx_receipt = node
            .get_transaction_receipt_impl(tx_hash)
            .await
            .expect("failed fetching transaction receipt by hash")
            .expect("no transaction receipt");

        assert_eq!(expected_block_hash, actual_tx_receipt.block_hash);
    }

    #[tokio::test]
    async fn test_new_block_filter_returns_filter_id() {
        let node = InMemoryNode::test(None);

        let actual_filter_id = node
            .new_block_filter_impl()
            .await
            .expect("failed creating filter");

        assert_eq!(U256::from(1), actual_filter_id);
    }

    #[tokio::test]
    async fn test_new_filter_returns_filter_id() {
        let node = InMemoryNode::test(None);

        let actual_filter_id = node
            .new_filter_impl(Filter::default())
            .await
            .expect("failed creating filter");

        assert_eq!(U256::from(1), actual_filter_id);
    }

    #[tokio::test]
    async fn test_new_pending_transaction_filter_returns_filter_id() {
        let node = InMemoryNode::test(None);

        let actual_filter_id = node
            .new_pending_transaction_filter_impl()
            .await
            .expect("failed creating filter");

        assert_eq!(U256::from(1), actual_filter_id);
    }

    #[tokio::test]
    async fn test_uninstall_filter_returns_true_if_filter_exists() {
        let node = InMemoryNode::test(None);
        let filter_id = node
            .new_block_filter_impl()
            .await
            .expect("failed creating filter");

        let actual_result = node
            .uninstall_filter_impl(filter_id)
            .await
            .expect("failed creating filter");

        assert!(actual_result);
    }

    #[tokio::test]
    async fn test_uninstall_filter_returns_false_if_filter_does_not_exist() {
        let node = InMemoryNode::test(None);

        let actual_result = node
            .uninstall_filter_impl(U256::from(100))
            .await
            .expect("failed creating filter");

        assert!(!actual_result);
    }

    #[tokio::test]
    async fn test_get_filter_changes_returns_block_hash_updates_only_once() {
        let node = InMemoryNode::test(None);
        let filter_id = node
            .new_block_filter_impl()
            .await
            .expect("failed creating filter");
        let (block_hash, _, _) = testing::apply_tx(&node, H256::repeat_byte(0x1)).await;

        match node
            .get_filter_changes_impl(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Hashes(result) => {
                // Get the block hash and the virtual block hash.
                assert_eq!(2, result.len());
                assert_eq!(block_hash, result[0]);
            }
            changes => panic!("unexpected filter changes: {:?}", changes),
        }

        match node
            .get_filter_changes_impl(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Empty(_) => (),
            changes => panic!("expected no changes in the second call, got {:?}", changes),
        }
    }

    #[tokio::test]
    async fn test_get_filter_changes_returns_log_updates_only_once() {
        let node = InMemoryNode::test(None);
        let filter_id = node
            .new_filter_impl(Filter {
                from_block: None,
                to_block: None,
                address: None,
                topics: None,
                block_hash: None,
            })
            .await
            .expect("failed creating filter");
        testing::apply_tx(&node, H256::repeat_byte(0x1)).await;

        match node
            .get_filter_changes_impl(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Logs(result) => assert_eq!(4, result.len()),
            changes => panic!("unexpected filter changes: {:?}", changes),
        }

        match node
            .get_filter_changes_impl(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Empty(_) => (),
            changes => panic!("expected no changes in the second call, got {:?}", changes),
        }
    }

    #[tokio::test]
    async fn test_get_filter_changes_returns_pending_transaction_updates_only_once() {
        let node = InMemoryNode::test(None);
        let filter_id = node
            .new_pending_transaction_filter_impl()
            .await
            .expect("failed creating filter");
        testing::apply_tx(&node, H256::repeat_byte(0x1)).await;

        match node
            .get_filter_changes_impl(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Hashes(result) => assert_eq!(vec![H256::repeat_byte(0x1)], result),
            changes => panic!("unexpected filter changes: {:?}", changes),
        }

        match node
            .get_filter_changes_impl(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Empty(_) => (),
            changes => panic!("expected no changes in the second call, got {:?}", changes),
        }
    }

    #[tokio::test]
    async fn test_produced_block_archives_previous_blocks() {
        let node = InMemoryNode::test(None);

        let input_storage_key = StorageKey::new(
            AccountTreeId::new(H160::repeat_byte(0x1)),
            u256_to_h256(U256::zero()),
        );
        let input_storage_value = H256::repeat_byte(0xcd);
        node.inner
            .write()
            .await
            .fork_storage
            .set_value(input_storage_key, input_storage_value);
        let initial_miniblock = node.blockchain.current_block_number().await;

        testing::apply_tx(&node, H256::repeat_byte(0x1)).await;
        let current_miniblock = node.blockchain.current_block_number().await;

        for miniblock in initial_miniblock.0..current_miniblock.0 {
            let hash = node
                .blockchain
                .get_block_hash_by_number(L2BlockNumber(miniblock))
                .await
                .unwrap();
            let previous_state = node.inner.read().await.get_previous_state(hash).unwrap();
            let actual_cached_value = previous_state.get(&input_storage_key).copied();

            assert_eq!(
                Some(input_storage_value),
                actual_cached_value,
                "unexpected cached state value for block {}",
                miniblock
            );
        }
    }

    #[tokio::test]
    async fn test_get_storage_fetches_zero_value_for_non_existent_key() {
        let node = InMemoryNode::test(None);

        let value = node
            .get_storage_impl(H160::repeat_byte(0xf1), U256::from(1024), None)
            .await
            .expect("failed retrieving storage");
        assert_eq!(H256::zero(), value);
    }

    #[tokio::test]
    async fn test_get_storage_uses_fork_to_get_value_for_historical_block() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_address = H160::repeat_byte(0x1);
        let input_storage_value = H256::repeat_byte(0xcd);
        mock_server.expect(
            "eth_getStorageAt",
            Some(serde_json::json!([
                format!("{:#x}", input_address),
                "0x0",
                { "blockNumber": "0x2" },
            ])),
            serde_json::json!(format!("{:#x}", input_storage_value)),
        );

        let node = test_node(mock_server.url()).await;

        let actual_value = node
            .get_storage_impl(
                input_address,
                U256::zero(),
                Some(zksync_types::api::BlockIdVariant::BlockNumberObject(
                    BlockNumberObject {
                        block_number: BlockNumber::Number(U64::from(2)),
                    },
                )),
            )
            .await
            .expect("failed retrieving storage");
        assert_eq!(input_storage_value, actual_value);
    }

    #[tokio::test]
    async fn test_get_storage_uses_archived_storage_to_get_value_for_missing_key() {
        let input_address = H160::repeat_byte(0x1);
        let input_storage_key = StorageKey::new(
            AccountTreeId::new(input_address),
            u256_to_h256(U256::zero()),
        );
        let input_storage_value = H256::repeat_byte(0xcd);

        let node = InMemoryNode::test(None);
        {
            let mut writer = node.inner.write().await;
            let historical_block = Block::<TransactionVariant> {
                hash: H256::repeat_byte(0x2),
                number: U64::from(2),
                ..Default::default()
            };
            writer
                .insert_block_hash(L2BlockNumber(2), historical_block.hash)
                .await;

            writer.insert_previous_state(
                historical_block.hash,
                hashmap! {
                    input_storage_key => input_storage_value,
                },
            );
            writer
                .insert_block(historical_block.hash, historical_block)
                .await;
        }

        let actual_value = node
            .get_storage_impl(
                input_address,
                U256::zero(),
                Some(zksync_types::api::BlockIdVariant::BlockNumberObject(
                    BlockNumberObject {
                        block_number: BlockNumber::Number(U64::from(2)),
                    },
                )),
            )
            .await
            .expect("failed retrieving storage");
        assert_eq!(input_storage_value, actual_value);
    }

    // FIXME: Multi-threaded flavor is needed because of the `block_on` mess inside `ForkStorage`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_get_storage_uses_fork_to_get_value_for_latest_block_for_missing_key() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_address = H160::repeat_byte(0x1);
        let input_storage_value = H256::repeat_byte(0xcd);
        mock_server.expect(
            "eth_getStorageAt",
            Some(serde_json::json!([
                format!("{:#x}", input_address),
                "0x0",
                "0xa",
            ])),
            serde_json::json!(format!("{:#x}", input_storage_value)),
        );

        let node = test_node(mock_server.url()).await;
        {
            let mut writer = node.inner.write().await;
            let historical_block = Block::<TransactionVariant> {
                hash: H256::repeat_byte(0x2),
                number: U64::from(2),
                ..Default::default()
            };
            writer
                .insert_block_hash(L2BlockNumber(2), historical_block.hash)
                .await;
            writer.insert_previous_state(historical_block.hash, Default::default());
            writer
                .insert_block(historical_block.hash, historical_block)
                .await;
        };

        let actual_value = node
            .get_storage_impl(
                input_address,
                U256::zero(),
                Some(zksync_types::api::BlockIdVariant::BlockNumberObject(
                    BlockNumberObject {
                        block_number: BlockNumber::Number(U64::from(2)),
                    },
                )),
            )
            .await
            .expect("failed retrieving storage");
        assert_eq!(input_storage_value, actual_value);
    }

    #[tokio::test]
    async fn test_get_storage_fetches_state_for_deployed_smart_contract_in_current_block() {
        let node = InMemoryNode::test(None);

        let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xef)).unwrap();
        let from_account = private_key.address();
        node.set_rich_account(from_account, U256::from(DEFAULT_ACCOUNT_BALANCE))
            .await;

        let deployed_address = deployed_address_create(from_account, U256::zero());

        testing::deploy_contract(
            &node,
            H256::repeat_byte(0x1),
            &private_key,
            hex::decode(testing::STORAGE_CONTRACT_BYTECODE).unwrap(),
            None,
            Nonce(0),
        )
        .await;

        let number1 = node
            .get_storage_impl(deployed_address, U256::from(0), None)
            .await
            .expect("failed retrieving storage at slot 0");
        assert_eq!(U256::from(1024), h256_to_u256(number1));

        let number2 = node
            .get_storage_impl(deployed_address, U256::from(1), None)
            .await
            .expect("failed retrieving storage at slot 1");
        assert_eq!(U256::MAX, h256_to_u256(number2));
    }

    #[tokio::test]
    async fn test_get_storage_fetches_state_for_deployed_smart_contract_in_old_block() {
        let node = InMemoryNode::test(None);

        let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xef)).unwrap();
        let from_account = private_key.address();
        node.set_rich_account(from_account, U256::from(DEFAULT_ACCOUNT_BALANCE))
            .await;

        let deployed_address = deployed_address_create(from_account, U256::zero());

        let initial_block_hash = testing::deploy_contract(
            &node,
            H256::repeat_byte(0x1),
            &private_key,
            hex::decode(testing::STORAGE_CONTRACT_BYTECODE).unwrap(),
            None,
            Nonce(0),
        )
        .await;

        // simulate a tx modifying the storage
        testing::apply_tx(&node, H256::repeat_byte(0x2)).await;
        let key = StorageKey::new(
            AccountTreeId::new(deployed_address),
            u256_to_h256(U256::from(0)),
        );
        node.inner
            .write()
            .await
            .fork_storage
            .inner
            .write()
            .unwrap()
            .raw_storage
            .state
            .insert(key, u256_to_h256(U256::from(512)));

        let number1_current = node
            .get_storage_impl(deployed_address, U256::from(0), None)
            .await
            .expect("failed retrieving storage at slot 0");
        assert_eq!(U256::from(512), h256_to_u256(number1_current));

        let number1_old = node
            .get_storage_impl(
                deployed_address,
                U256::from(0),
                Some(api::BlockIdVariant::BlockHashObject(BlockHashObject {
                    block_hash: initial_block_hash,
                })),
            )
            .await
            .expect("failed retrieving storage at slot 0");
        assert_eq!(U256::from(1024), h256_to_u256(number1_old));
    }

    #[tokio::test]
    async fn test_get_filter_logs_returns_matching_logs_for_valid_id() {
        let node = InMemoryNode::test(None);

        // populate tx receipts with 2 tx each having logs
        {
            let mut writer = node.inner.write().await;
            writer
                .insert_tx_result(
                    H256::repeat_byte(0x1),
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        receipt: TransactionReceipt {
                            logs: vec![LogBuilder::new()
                                .set_address(H160::repeat_byte(0xa1))
                                .build()],
                            ..Default::default()
                        },
                        debug: default_tx_debug_info(),
                    },
                )
                .await;
            writer
                .insert_tx_result(
                    H256::repeat_byte(0x2),
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        receipt: TransactionReceipt {
                            logs: vec![
                                LogBuilder::new()
                                    .set_address(H160::repeat_byte(0xa1))
                                    .build(),
                                LogBuilder::new()
                                    .set_address(H160::repeat_byte(0xa2))
                                    .build(),
                            ],
                            ..Default::default()
                        },
                        debug: default_tx_debug_info(),
                    },
                )
                .await;
        }

        let filter_id = node
            .new_filter_impl(Filter {
                address: Some(ValueOrArray(vec![H160::repeat_byte(0xa1)])),
                ..Default::default()
            })
            .await
            .expect("failed creating filter");

        match node
            .get_filter_logs_impl(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Logs(result) => assert_eq!(2, result.len()),
            changes => panic!("unexpected filter changes: {:?}", changes),
        }
    }

    #[tokio::test]
    async fn test_get_filter_logs_returns_error_for_invalid_id() {
        let node = InMemoryNode::test(None);

        // populate tx receipts with 2 tx each having logs
        {
            let mut writer = node.inner.write().await;
            writer
                .insert_tx_result(
                    H256::repeat_byte(0x1),
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        receipt: TransactionReceipt {
                            logs: vec![LogBuilder::new()
                                .set_address(H160::repeat_byte(0xa1))
                                .build()],
                            ..Default::default()
                        },
                        debug: default_tx_debug_info(),
                    },
                )
                .await;
        }

        let invalid_filter_id = U256::from(100);
        let result = node.get_filter_logs_impl(invalid_filter_id).await;

        assert!(result.is_err(), "expected an error for invalid filter id");
    }

    #[tokio::test]
    async fn test_get_logs_returns_matching_logs() {
        let node = InMemoryNode::test(None);

        // populate tx receipts with 2 tx each having logs
        {
            let mut writer = node.inner.write().await;
            writer
                .insert_tx_result(
                    H256::repeat_byte(0x1),
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        receipt: TransactionReceipt {
                            logs: vec![LogBuilder::new()
                                .set_address(H160::repeat_byte(0xa1))
                                .build()],
                            ..Default::default()
                        },
                        debug: testing::default_tx_debug_info(),
                    },
                )
                .await;
            writer
                .insert_tx_result(
                    H256::repeat_byte(0x2),
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        receipt: TransactionReceipt {
                            logs: vec![
                                LogBuilder::new()
                                    .set_address(H160::repeat_byte(0xa1))
                                    .build(),
                                LogBuilder::new()
                                    .set_address(H160::repeat_byte(0xa2))
                                    .build(),
                            ],
                            ..Default::default()
                        },
                        debug: testing::default_tx_debug_info(),
                    },
                )
                .await;
        }

        let result = node
            .get_logs_impl(Filter {
                address: Some(ValueOrArray(vec![H160::repeat_byte(0xa2)])),
                ..Default::default()
            })
            .await
            .expect("failed getting filter changes");
        assert_eq!(1, result.len());

        let result = node
            .get_logs_impl(Filter {
                address: Some(ValueOrArray(vec![H160::repeat_byte(0xa1)])),
                ..Default::default()
            })
            .await
            .expect("failed getting filter changes");
        assert_eq!(2, result.len());

        let result = node
            .get_logs_impl(Filter {
                address: Some(ValueOrArray(vec![H160::repeat_byte(0x11)])),
                ..Default::default()
            })
            .await
            .expect("failed getting filter changes");
        assert_eq!(0, result.len());
    }

    #[tokio::test]
    async fn test_accounts_impl() {
        let node = InMemoryNode::test(None);

        let private_key = H256::repeat_byte(0x01);
        let from_account = K256PrivateKey::from_bytes(private_key).unwrap().address();
        node.set_rich_account(from_account, U256::from(DEFAULT_ACCOUNT_BALANCE))
            .await;

        let account_result = node.accounts_impl().await;
        let expected_accounts: Vec<H160> = vec![from_account];

        match account_result {
            Ok(accounts) => {
                assert_eq!(expected_accounts, accounts);
            }
            Err(e) => {
                panic!("Failed to fetch accounts: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_returns_none_for_invalid_block_hash() {
        let node = InMemoryNode::test(None);
        let input_tx_hash = H256::repeat_byte(0x01);
        let (input_block_hash, _, _) = testing::apply_tx(&node, input_tx_hash).await;
        let invalid_block_hash = H256::repeat_byte(0xab);
        assert_ne!(input_block_hash, invalid_block_hash);

        let result = node
            .get_transaction_by_block_and_index_impl(
                api::BlockId::Hash(invalid_block_hash),
                U64::from(0),
            )
            .await
            .expect("failed fetching transaction");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_returns_none_for_invalid_index() {
        let node = InMemoryNode::test(None);
        let input_tx_hash = H256::repeat_byte(0x01);
        let (input_block_hash, _, _) = testing::apply_tx(&node, input_tx_hash).await;

        let result = node
            .get_transaction_by_block_and_index_impl(
                api::BlockId::Hash(input_block_hash),
                U64::from(10),
            )
            .await
            .expect("failed fetching transaction");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_returns_transaction_for_valid_input() {
        let node = InMemoryNode::test(None);
        let input_tx_hash = H256::repeat_byte(0x01);
        let (input_block_hash, _, _) = testing::apply_tx(&node, input_tx_hash).await;

        let actual_tx = node
            .get_transaction_by_block_and_index_impl(
                api::BlockId::Hash(input_block_hash),
                U64::from(0),
            )
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_fetches_from_fork_if_block_missing() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_block_hash = H256::repeat_byte(0x01);
        let input_tx_hash = H256::repeat_byte(0x02);
        mock_server.expect(
            "eth_getTransactionByBlockHashAndIndex",
            Some(serde_json::json!([
                format!("{:#x}", input_block_hash),
                "0x1"
            ])),
            TransactionResponseBuilder::new()
                .set_hash(input_tx_hash)
                .set_block_hash(input_block_hash)
                .set_block_number(U64::from(100))
                .build_result(),
        );

        let node = test_node(mock_server.url()).await;

        let actual_tx = node
            .get_transaction_by_block_and_index_impl(
                api::BlockId::Hash(input_block_hash),
                U64::from(1),
            )
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
        assert_eq!(Some(U64::from(100)), actual_tx.block_number);
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_returns_none_for_invalid_block_number()
    {
        let node = InMemoryNode::test(None);
        let input_tx_hash = H256::repeat_byte(0x01);
        let (input_block_hash, _, _) = testing::apply_tx(&node, input_tx_hash).await;
        let invalid_block_hash = H256::repeat_byte(0xab);
        assert_ne!(input_block_hash, invalid_block_hash);

        let result = node
            .get_transaction_by_block_and_index_impl(
                api::BlockId::Number(BlockNumber::Number(U64::from(100))),
                U64::from(0),
            )
            .await
            .expect("failed fetching transaction");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_returns_none_for_invalid_index() {
        let node = InMemoryNode::test(None);
        let input_tx_hash = H256::repeat_byte(0x01);
        testing::apply_tx(&node, input_tx_hash).await;

        let result = node
            .get_transaction_by_block_and_index_impl(
                api::BlockId::Number(BlockNumber::Latest),
                U64::from(10),
            )
            .await
            .expect("failed fetching transaction");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_returns_transaction_for_valid_input() {
        let node = InMemoryNode::test(None);
        let input_tx_hash = H256::repeat_byte(0x01);
        let (_, input_block_number, _) = testing::apply_tx(&node, input_tx_hash).await;

        let actual_tx = node
            .get_transaction_by_block_and_index_impl(
                api::BlockId::Number(BlockNumber::Number(input_block_number)),
                U64::from(0),
            )
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_fetches_from_fork_if_block_missing() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_block_hash = H256::repeat_byte(0x01);
        let input_block_number = U64::from(100);
        let input_tx_hash = H256::repeat_byte(0x02);
        mock_server.expect(
            "eth_getTransactionByBlockNumberAndIndex",
            Some(serde_json::json!([
                format!("{:#x}", input_block_number),
                "0x1"
            ])),
            TransactionResponseBuilder::new()
                .set_hash(input_tx_hash)
                .set_block_hash(input_block_hash)
                .set_block_number(input_block_number)
                .build_result(),
        );

        let node = test_node(mock_server.url()).await;

        let actual_tx = node
            .get_transaction_by_block_and_index_impl(
                api::BlockId::Number(BlockNumber::Number(input_block_number)),
                U64::from(1),
            )
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
        assert_eq!(Some(input_block_number), actual_tx.block_number);
    }

    #[tokio::test]
    async fn test_protocol_version_returns_currently_supported_version() {
        let node = InMemoryNode::test(None);

        let expected_version = String::from(PROTOCOL_VERSION);
        let actual_version = node.protocol_version_impl();

        assert_eq!(expected_version, actual_version);
    }
}
