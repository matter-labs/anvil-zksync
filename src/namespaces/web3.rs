use jsonrpc_derive::rpc;

use crate::namespaces::Result;

#[rpc]
pub trait Web3NamespaceT {
    #[rpc(name = "web3_clientVersion", returns = "String")]
    fn web3_client_version(&self) -> Result<String>;
}
