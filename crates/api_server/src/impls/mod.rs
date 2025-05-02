mod anvil;
mod anvil_zks;
mod config;
mod debug;
mod eth;
mod eth_test;
mod evm;
mod net;
mod web3;
mod zkos;
mod zks;

// TODO: @dutterbutter rename ZKOSNamespace
pub use self::{
    anvil::AnvilNamespace, anvil_zks::AnvilZksNamespace, config::ConfigNamespace,
    debug::DebugNamespace, eth::EthNamespace, eth_test::EthTestNamespace, evm::EvmNamespace,
    net::NetNamespace, web3::Web3Namespace, zks::ZksNamespace, zkos::ZKOSNamespace
};
