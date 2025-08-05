use anvil_zksync_api_decl::EthTestNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use jsonrpsee::core::{RpcResult, async_trait};
use zksync_types::H256;
use zksync_types::transaction_request::CallRequest;

use crate::error::RpcErrorAdapter;

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
        self.node
            .send_transaction_impl(tx)
            .await
            .map_err(RpcErrorAdapter::into)
    }
}
