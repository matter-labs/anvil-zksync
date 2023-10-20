mod configuration_api;
mod evm;
mod hardhat;
mod net;

use zksync_core::api_server::web3::backend_jsonrpc::namespaces::{debug, eth, zks};

pub use configuration_api::ConfigurationApiNamespaceT;
pub use debug::DebugNamespaceT;
pub use eth::EthNamespaceT;
pub use evm::EvmNamespaceT;
pub use hardhat::HardhatNamespaceT;
pub use net::NetNamespaceT;
pub use zks::ZksNamespaceT;

pub type Result<T> = jsonrpc_core::Result<T>;
pub type RpcResult<T> = jsonrpc_core::BoxFuture<Result<T>>;
