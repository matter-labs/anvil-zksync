use jsonrpc_derive::rpc;
use zksync_basic_types::U256;

use crate::namespaces::Result;

#[rpc]
pub trait NetNamespaceT {
    #[rpc(name = "net_version", returns = "String")]
    fn net_version(&self) -> Result<String>;

    #[rpc(name = "net_peerCount", returns = "U256")]
    fn net_peer_count(&self) -> Result<U256>;

    #[rpc(name = "net_listening", returns = "bool")]
    fn net_listening(&self) -> Result<bool>;
}
