use crate::error::RpcError;
use anvil_zksync_api_decl::NetNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use jsonrpsee::core::RpcResult;
use zksync_types::U256;

pub struct NetNamespace {
    node: InMemoryNode,
}

impl NetNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

impl NetNamespaceServer for NetNamespace {
    fn version(&self) -> RpcResult<String> {
        let chain_id = self.node.get_chain_id().map_err(RpcError::from)?;
        Ok(chain_id.to_string())
    }

    fn peer_count(&self) -> RpcResult<U256> {
        Ok(U256::from(0))
    }

    fn is_listening(&self) -> RpcResult<bool> {
        Ok(false)
    }
}
