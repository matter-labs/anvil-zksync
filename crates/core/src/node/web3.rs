use crate::{
    namespaces::{Result, Web3NamespaceT},
    node::InMemoryNode,
};

impl Web3NamespaceT for InMemoryNode {
    fn web3_client_version(&self) -> Result<String> {
        Ok("zkSync/v2.0".to_string())
    }
}
