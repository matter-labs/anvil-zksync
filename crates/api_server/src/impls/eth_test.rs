use crate::error::RpcError;
use anvil_zksync_api_decl::EthTestNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use jsonrpsee::core::{async_trait, RpcResult};
use zksync_types::transaction_request::CallRequest;
use zksync_types::H256;

pub struct EthTestNamespace {
    node: InMemoryNode,
}

impl EthTestNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

#[async_trait]
impl EthTestNamespaceServer for EthTestNamespace {
    async fn send_transaction(&self, tx: CallRequest) -> RpcResult<H256> {
        Ok(self
            .node
            .send_transaction_impl(tx)
            .await
            .map_err(RpcError::from)?)
    }
}
