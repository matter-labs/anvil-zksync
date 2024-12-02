mod anvil;
mod config;
mod debug;
mod eth;
mod eth_test;
mod evm;
mod hardhat;
mod net;
mod web3;
mod zks;

pub use anvil::{AnvilNamespaceT, DetailedTransaction};
pub use config::ConfigurationApiNamespaceT;
pub use debug::DebugNamespaceT;
pub use eth::EthNamespaceT;
pub use eth_test::EthTestNodeNamespaceT;
pub use evm::EvmNamespaceT;
pub use hardhat::{HardhatNamespaceT, ResetRequest};
pub use net::NetNamespaceT;
pub use web3::Web3NamespaceT;
pub use zks::ZksNamespaceT;

pub type Result<T> = jsonrpc_core::Result<T>;
pub type RpcResult<T> = jsonrpc_core::BoxFuture<Result<T>>;
