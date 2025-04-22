use anvil_zksync_api_decl::EthNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use function_name::named;
use jsonrpsee::core::{async_trait, RpcResult};
use zksync_types::api::state_override::StateOverride;
use zksync_types::api::{
    Block, BlockIdVariant, BlockNumber, FeeHistory, Log, Transaction, TransactionReceipt,
    TransactionVariant,
};
use zksync_types::transaction_request::CallRequest;
use zksync_types::web3::{Bytes, Index, SyncState, U64Number};
use zksync_types::{api, Address, H256, U256, U64};
use zksync_web3_decl::types::{Filter, FilterChanges};

use crate::error::{rpc_unsupported, JsonRPCAdapter};

pub struct EthNamespace {
    node: InMemoryNode,
}

impl EthNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

#[async_trait]
impl EthNamespaceServer for EthNamespace {
    async fn get_block_number(&self) -> RpcResult<U64> {
        Ok(self
            .node
            .get_block_number_impl()
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn chain_id(&self) -> RpcResult<U64> {
        Ok(self
            .node
            .get_chain_id()
            .await
            .map(U64::from)
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn call(
        &self,
        req: CallRequest,
        // TODO: Support
        _block: Option<BlockIdVariant>,
        // TODO: Support
        _state_override: Option<StateOverride>,
    ) -> RpcResult<Bytes> {
        Ok(self
            .node
            .call_impl(req)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn estimate_gas(
        &self,
        req: CallRequest,
        block: Option<BlockNumber>,
        // TODO: Support
        _state_override: Option<StateOverride>,
    ) -> RpcResult<U256> {
        Ok(self
            .node
            .estimate_gas_impl(req, block)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn gas_price(&self) -> RpcResult<U256> {
        Ok(self
            .node
            .gas_price_impl()
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn new_filter(&self, filter: Filter) -> RpcResult<U256> {
        Ok(self
            .node
            .new_filter_impl(filter)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn new_block_filter(&self) -> RpcResult<U256> {
        Ok(self
            .node
            .new_block_filter_impl()
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn uninstall_filter(&self, idx: U256) -> RpcResult<bool> {
        Ok(self
            .node
            .uninstall_filter_impl(idx)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn new_pending_transaction_filter(&self) -> RpcResult<U256> {
        Ok(self
            .node
            .new_pending_transaction_filter_impl()
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_logs(&self, filter: Filter) -> RpcResult<Vec<Log>> {
        Ok(self
            .node
            .get_logs_impl(filter)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_filter_logs(&self, filter_index: U256) -> RpcResult<FilterChanges> {
        Ok(self
            .node
            .get_filter_logs_impl(filter_index)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_filter_changes(&self, filter_index: U256) -> RpcResult<FilterChanges> {
        Ok(self
            .node
            .get_filter_changes_impl(filter_index)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<U256> {
        Ok(self
            .node
            .get_balance_impl(address, block)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_block_by_number(
        &self,
        block_number: BlockNumber,
        full_transactions: bool,
    ) -> RpcResult<Option<Block<TransactionVariant>>> {
        Ok(self
            .node
            .get_block_impl(api::BlockId::Number(block_number), full_transactions)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_block_by_hash(
        &self,
        hash: H256,
        full_transactions: bool,
    ) -> RpcResult<Option<Block<TransactionVariant>>> {
        Ok(self
            .node
            .get_block_impl(api::BlockId::Hash(hash), full_transactions)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_block_transaction_count_by_number(
        &self,
        block_number: BlockNumber,
    ) -> RpcResult<Option<U256>> {
        Ok(self
            .node
            .get_block_transaction_count_impl(api::BlockId::Number(block_number))
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    #[named]
    async fn get_block_receipts(
        &self,
        _block_id: api::BlockId,
    ) -> RpcResult<Option<Vec<TransactionReceipt>>> {
        rpc_unsupported(function_name!())
    }

    async fn get_block_transaction_count_by_hash(
        &self,
        block_hash: H256,
    ) -> RpcResult<Option<U256>> {
        Ok(self
            .node
            .get_block_transaction_count_impl(api::BlockId::Hash(block_hash))
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_code(&self, address: Address, block: Option<BlockIdVariant>) -> RpcResult<Bytes> {
        Ok(self
            .node
            .get_code_impl(address, block)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_storage_at(
        &self,
        address: Address,
        idx: U256,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<H256> {
        Ok(self
            .node
            .get_storage_impl(address, idx, block)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<U256> {
        Ok(self
            .node
            .get_transaction_count_impl(address, block)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_transaction_by_hash(&self, hash: H256) -> RpcResult<Option<Transaction>> {
        Ok(self
            .node
            .get_transaction_by_hash_impl(hash)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        Ok(self
            .node
            .get_transaction_by_block_and_index_impl(api::BlockId::Hash(block_hash), index)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_transaction_by_block_number_and_index(
        &self,
        block_number: BlockNumber,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        Ok(self
            .node
            .get_transaction_by_block_and_index_impl(api::BlockId::Number(block_number), index)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn get_transaction_receipt(&self, hash: H256) -> RpcResult<Option<TransactionReceipt>> {
        Ok(self
            .node
            .get_transaction_receipt_impl(hash)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn protocol_version(&self) -> RpcResult<String> {
        Ok(self.node.protocol_version_impl())
    }

    async fn send_raw_transaction(&self, tx_bytes: Bytes) -> RpcResult<H256> {
        Ok(self
            .node
            .send_raw_transaction_impl(tx_bytes)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn syncing(&self) -> RpcResult<SyncState> {
        Ok(self.node.syncing_impl())
    }

    async fn accounts(&self) -> RpcResult<Vec<Address>> {
        Ok(self
            .node
            .accounts_impl()
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    #[named]
    async fn coinbase(&self) -> RpcResult<Address> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn compilers(&self) -> RpcResult<Vec<String>> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn hashrate(&self) -> RpcResult<U256> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_uncle_count_by_block_hash(&self, _hash: H256) -> RpcResult<Option<U256>> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_uncle_count_by_block_number(
        &self,
        _number: BlockNumber,
    ) -> RpcResult<Option<U256>> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn mining(&self) -> RpcResult<bool> {
        rpc_unsupported(function_name!())
    }

    async fn fee_history(
        &self,
        block_count: U64Number,
        newest_block: BlockNumber,
        reward_percentiles: Option<Vec<f32>>,
    ) -> RpcResult<FeeHistory> {
        Ok(self
            .node
            .fee_history_impl(block_count.into(), newest_block, reward_percentiles)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    #[named]
    async fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        rpc_unsupported(function_name!())
    }
}
