use crate::error::RpcError;
use anvil_zksync_api_decl::EvmNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use jsonrpsee::core::RpcResult;

pub struct EvmNamespace {
    node: InMemoryNode,
}

impl EvmNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

impl EvmNamespaceServer for EvmNamespace {
    fn mine(&self) -> RpcResult<String> {
        self.node.mine_block().map_err(RpcError::from)?;
        Ok("0x0".to_string())
    }
}
