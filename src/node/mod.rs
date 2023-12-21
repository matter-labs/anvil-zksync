//! In-memory node, that supports forking other networks.

mod config;
mod debug;
mod eth;
mod evm;
mod hardhat;
mod in_memory;
mod in_memory_ext;
mod net;
mod storage_logs;
mod web3;
mod zks;

pub use in_memory::*;
