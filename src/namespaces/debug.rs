use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_types::{
    api::{BlockId, BlockNumber, DebugCall, ResultDebugCall, TracerConfig},
    transaction_request::CallRequest,
    H256,
};

#[rpc]
pub trait DebugNamespaceT {
    #[rpc(name = "debug_traceBlockByNumber")]
    fn trace_block_by_number(
        &self,
        block: BlockNumber,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Vec<ResultDebugCall>>>;

    #[rpc(name = "debug_traceBlockByHash")]
    fn trace_block_by_hash(
        &self,
        hash: H256,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Vec<ResultDebugCall>>>;

    #[rpc(name = "debug_traceCall")]
    fn trace_call(
        &self,
        request: CallRequest,
        block: Option<BlockId>,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<DebugCall>>;

    #[rpc(name = "debug_traceTransaction")]
    fn trace_transaction(
        &self,
        tx_hash: H256,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Option<DebugCall>>>;
}
