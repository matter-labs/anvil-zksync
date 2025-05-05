use anvil_zksync_api_decl::NetNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use anvil_zksync_core::utils::block_on;
use jsonrpsee::core::RpcResult;
use zksync_types::U256;

use crate::error::JsonRpcAdapter;

pub struct NetNamespace {
    node: InMemoryNode,
}

impl NetNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

// TODO: Make this namespace async in zksync-era
impl NetNamespaceServer for NetNamespace {
    fn version(&self) -> RpcResult<String> {
        let node = self.node.clone();
        let chain_id =
            block_on(async move { node.get_chain_id().await.map_err(JsonRpcAdapter::from) })?;
        Ok(chain_id.to_string())
    }

    fn peer_count(&self) -> RpcResult<U256> {
        Ok(U256::from(0))
    }

    fn is_listening(&self) -> RpcResult<bool> {
        Ok(false)
    }
}
