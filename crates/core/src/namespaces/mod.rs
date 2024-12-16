mod eth;
mod eth_test;

pub use eth::EthNamespaceT;
pub use eth_test::EthTestNodeNamespaceT;

pub type Result<T> = jsonrpc_core::Result<T>;
pub type RpcResult<T> = jsonrpc_core::BoxFuture<Result<T>>;
