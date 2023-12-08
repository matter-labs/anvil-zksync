use crate::{
    fork::ForkSource,
    namespaces::{Result, Web3NamespaceT},
    node::InMemoryNode,
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> Web3NamespaceT
    for InMemoryNode<S>
{
    fn web3_client_version(&self) -> Result<String> {
        Ok("zkSync/v2.0".to_string())
    }
}
