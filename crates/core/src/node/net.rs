use crate::{
    namespaces::{NetNamespaceT, Result},
    node::InMemoryNode,
};
use anvil_zksync_config::constants::TEST_NODE_NETWORK_ID;
use zksync_types::U256;

impl NetNamespaceT for InMemoryNode {
    fn net_version(&self) -> Result<String> {
        let chain_id = self
            .get_inner()
            .read()
            .map(|reader| reader.config.chain_id.unwrap_or(TEST_NODE_NETWORK_ID))
            .expect("Failed to get lock");
        Ok(chain_id.to_string())
    }

    fn net_peer_count(&self) -> Result<U256> {
        Ok(U256::from(0))
    }

    fn net_listening(&self) -> Result<bool> {
        Ok(false)
    }
}
