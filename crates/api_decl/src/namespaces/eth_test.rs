use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use zksync_types::H256;
use zksync_types::transaction_request::CallRequest;

/// API bindings for the `eth` namespace that are not normally supported by core ZKsync.
#[rpc(server, namespace = "eth")]
pub trait EthTestNamespace {
    #[method(name = "sendTransaction")]
    async fn send_transaction(&self, tx: CallRequest) -> RpcResult<H256>;
}
