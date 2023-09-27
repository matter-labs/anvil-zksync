use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_basic_types::H256;
use zksync_types::transaction_request::CallRequest;

///
/// ETH namespace extension for the test node.
///
#[rpc]
pub trait EthTestNodeNamespaceT {
    #[rpc(name = "eth_sendTransaction")]
    fn send_transaction(&self, tx: CallRequest) -> BoxFuture<Result<H256>>;
}
