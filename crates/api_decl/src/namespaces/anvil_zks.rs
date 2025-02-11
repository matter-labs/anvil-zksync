use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use zksync_types::{L1BatchNumber, H256};

/// Custom namespace that contains anvil-zksync specific methods.
#[rpc(server, namespace = "anvil_zks")]
pub trait AnvilZksNamespace {
    #[method(name = "commitBatch")]
    async fn commit_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256>;
}
