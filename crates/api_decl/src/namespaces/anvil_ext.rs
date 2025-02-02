//! This module extends the Anvil JSON-RPC API with anvil-zksync specific RPC methods.

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use zksync_types::{L1BatchNumber, H256};

#[rpc(server, namespace = "anvil")]
pub trait AnvilExtNamespace {
    #[method(name = "commitBatch")]
    async fn commit_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256>;
}
