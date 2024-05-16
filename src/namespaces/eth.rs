use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_types::{
    api::{BlockIdVariant, BlockNumber, Transaction, TransactionReceipt, TransactionVariant},
    transaction_request::CallRequest,
    web3::{Bytes, FeeHistory, Index, SyncState},
    Address, H256, U256, U64,
};
use zksync_web3_decl::types::{Block, Filter, FilterChanges, Log};

#[rpc]
pub trait EthNamespaceT {
    #[rpc(name = "eth_blockNumber")]
    fn get_block_number(&self) -> BoxFuture<Result<U64>>;

    #[rpc(name = "eth_chainId")]
    fn chain_id(&self) -> BoxFuture<Result<U64>>;

    #[rpc(name = "eth_call")]
    fn call(&self, req: CallRequest, block: Option<BlockIdVariant>) -> BoxFuture<Result<Bytes>>;

    #[rpc(name = "eth_estimateGas")]
    fn estimate_gas(
        &self,
        req: CallRequest,
        _block: Option<BlockNumber>,
    ) -> BoxFuture<Result<U256>>;

    #[rpc(name = "eth_gasPrice")]
    fn gas_price(&self) -> BoxFuture<Result<U256>>;

    #[rpc(name = "eth_newFilter")]
    fn new_filter(&self, filter: Filter) -> BoxFuture<Result<U256>>;

    #[rpc(name = "eth_newBlockFilter")]
    fn new_block_filter(&self) -> BoxFuture<Result<U256>>;

    #[rpc(name = "eth_uninstallFilter")]
    fn uninstall_filter(&self, idx: U256) -> BoxFuture<Result<bool>>;

    #[rpc(name = "eth_newPendingTransactionFilter")]
    fn new_pending_transaction_filter(&self) -> BoxFuture<Result<U256>>;

    #[rpc(name = "eth_getLogs")]
    fn get_logs(&self, filter: Filter) -> BoxFuture<Result<Vec<Log>>>;

    #[rpc(name = "eth_getFilterLogs")]
    fn get_filter_logs(&self, filter_index: U256) -> BoxFuture<Result<FilterChanges>>;

    #[rpc(name = "eth_getFilterChanges")]
    fn get_filter_changes(&self, filter_index: U256) -> BoxFuture<Result<FilterChanges>>;

    #[rpc(name = "eth_getBalance")]
    fn get_balance(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> BoxFuture<Result<U256>>;

    #[rpc(name = "eth_getBlockByNumber")]
    fn get_block_by_number(
        &self,
        block_number: BlockNumber,
        full_transactions: bool,
    ) -> BoxFuture<Result<Option<Block<TransactionVariant>>>>;

    #[rpc(name = "eth_getBlockByHash")]
    fn get_block_by_hash(
        &self,
        hash: H256,
        full_transactions: bool,
    ) -> BoxFuture<Result<Option<Block<TransactionVariant>>>>;

    #[rpc(name = "eth_getBlockTransactionCountByNumber")]
    fn get_block_transaction_count_by_number(
        &self,
        block_number: BlockNumber,
    ) -> BoxFuture<Result<Option<U256>>>;

    #[rpc(name = "eth_getBlockTransactionCountByHash")]
    fn get_block_transaction_count_by_hash(
        &self,
        block_hash: H256,
    ) -> BoxFuture<Result<Option<U256>>>;

    #[rpc(name = "eth_getCode")]
    fn get_code(&self, address: Address, block: Option<BlockIdVariant>)
        -> BoxFuture<Result<Bytes>>;

    #[rpc(name = "eth_getStorageAt")]
    fn get_storage(
        &self,
        address: Address,
        idx: U256,
        block: Option<BlockIdVariant>,
    ) -> BoxFuture<Result<H256>>;

    #[rpc(name = "eth_getTransactionCount")]
    fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> BoxFuture<Result<U256>>;

    #[rpc(name = "eth_getTransactionByHash")]
    fn get_transaction_by_hash(&self, hash: H256) -> BoxFuture<Result<Option<Transaction>>>;

    #[rpc(name = "eth_getTransactionByBlockHashAndIndex")]
    fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> BoxFuture<Result<Option<Transaction>>>;

    #[rpc(name = "eth_getTransactionByBlockNumberAndIndex")]
    fn get_transaction_by_block_number_and_index(
        &self,
        block_number: BlockNumber,
        index: Index,
    ) -> BoxFuture<Result<Option<Transaction>>>;

    #[rpc(name = "eth_getTransactionReceipt")]
    fn get_transaction_receipt(&self, hash: H256) -> BoxFuture<Result<Option<TransactionReceipt>>>;

    #[rpc(name = "eth_protocolVersion")]
    fn protocol_version(&self) -> BoxFuture<Result<String>>;

    #[rpc(name = "eth_sendRawTransaction")]
    fn send_raw_transaction(&self, tx_bytes: Bytes) -> BoxFuture<Result<H256>>;

    #[rpc(name = "eth_syncing")]
    fn syncing(&self) -> BoxFuture<Result<SyncState>>;

    #[rpc(name = "eth_accounts")]
    fn accounts(&self) -> BoxFuture<Result<Vec<Address>>>;

    #[rpc(name = "eth_coinbase")]
    fn coinbase(&self) -> BoxFuture<Result<Address>>;

    #[rpc(name = "eth_getCompilers")]
    fn compilers(&self) -> BoxFuture<Result<Vec<String>>>;

    #[rpc(name = "eth_hashrate")]
    fn hashrate(&self) -> BoxFuture<Result<U256>>;

    #[rpc(name = "eth_getUncleCountByBlockHash")]
    fn get_uncle_count_by_block_hash(&self, hash: H256) -> BoxFuture<Result<Option<U256>>>;

    #[rpc(name = "eth_getUncleCountByBlockNumber")]
    fn get_uncle_count_by_block_number(
        &self,
        number: BlockNumber,
    ) -> BoxFuture<Result<Option<U256>>>;

    #[rpc(name = "eth_mining")]
    fn mining(&self) -> BoxFuture<Result<bool>>;

    #[rpc(name = "eth_feeHistory")]
    fn fee_history(
        &self,
        block_count: U64,
        newest_block: BlockNumber,
        reward_percentiles: Vec<f32>,
    ) -> BoxFuture<Result<FeeHistory>>;
}
