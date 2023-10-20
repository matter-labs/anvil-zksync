use zksync_basic_types::U256;

use crate::{
    fork::ForkSource,
    namespaces::{NetNamespaceT, Result},
    node::{InMemoryNode, TEST_NODE_NETWORK_ID},
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> NetNamespaceT
    for InMemoryNode<S>
{
    fn net_version(&self) -> Result<String> {
        Ok(TEST_NODE_NETWORK_ID.to_string())
    }

    fn net_peer_count(&self) -> Result<U256> {
        Ok(U256::from(0))
    }

    fn net_listening(&self) -> Result<bool> {
        Ok(false)
    }
}
