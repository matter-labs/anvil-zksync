mod config;
mod eth;
mod eth_test;
mod zks;

pub use config::ConfigurationApiNamespaceT;
pub use eth::EthNamespaceT;
pub use eth_test::EthTestNodeNamespaceT;
pub use zks::ZksNamespaceT;

pub type Result<T> = jsonrpc_core::Result<T>;
pub type RpcResult<T> = jsonrpc_core::BoxFuture<Result<T>>;
