//! zkSync Era In-Memory Node
//!
//! The `era-test-node` crate provides an in-memory node designed primarily for local testing.
//! It supports forking the state from other networks, making it a valuable tool for integration testing,
//! bootloader and system contract testing, and prototyping.
//!
//! ## Overview
//!
//! - **In-Memory Database**: The node uses an in-memory database for storing state information,
//!   and employs simplified hashmaps for tracking blocks and transactions.
//!
//! - **Forking**: In fork mode, the node fetches missing storage data from a remote source if not available locally.
//!
//! - **Remote Server Interaction**: The node can use the remote server (openchain) to resolve the ABI and topics
//!   to human-readable names.
//!
//! - **Local Testing**: Designed for local testing, this node is not intended for production use.
//!
//! ## Features
//!
//! - Fork the state of mainnet, testnet, or a custom network.
//! - Replay existing mainnet or testnet transactions.
//! - Use local bootloader and system contracts.
//! - Operate deterministically in non-fork mode.
//! - Start quickly with pre-configured 'rich' accounts.
//! - Resolve names of ABI functions and Events using openchain.
//!
//! ## Limitations
//!
//! - No communication between Layer 1 and Layer 2.
//! - Many APIs are not yet implemented.
//! - No support for accessing historical data.
//! - Only one transaction allowed per Layer 1 batch.
//!
//! ## Usage
//!
//! To start the node, use the command `era_test_node run`. For more advanced functionalities like forking or
//! replaying transactions, refer to the [official documentation](https://era.zksync.io/docs/tools/testing/era-test-node.html).
//!
//! ## Contributions
//!
//! Contributions to improve `era-test-node` are welcome. Please refer to the [contribution guidelines](https://github.com/matter-labs/era-test-node/blob/main/.github/CONTRIBUTING.md) for more details.

pub mod bootloader_debug;
pub mod console_log;
pub mod deps;
pub mod filters;
pub mod fork;
pub mod formatter;
pub mod http_fork_source;
pub mod namespaces;
pub mod node;
pub mod observability;
pub mod resolver;
pub mod system_contracts;
pub mod utils;

mod cache;
mod testing;
