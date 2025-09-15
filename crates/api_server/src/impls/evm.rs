use anvil_zksync_api_decl::EvmNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use jsonrpsee::core::{RpcResult, async_trait};

use crate::error::RpcErrorAdapter;

pub struct EvmNamespace {
    node: InMemoryNode,
}

impl EvmNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

#[async_trait]
impl EvmNamespaceServer for EvmNamespace {
    async fn mine(&self) -> RpcResult<String> {
        self.node
            .mine_block()
            .await
            .map_err(RpcErrorAdapter::into)?;
        Ok("0x0".to_string())
    }
}
