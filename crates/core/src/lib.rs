#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(allocator_api)]
#![feature(array_chunks)]
#![feature(get_mut_unchecked)]
#![feature(const_type_id)]
#![feature(vec_push_within_capacity)]
#![feature(ptr_alignment_type)]
#![feature(btreemap_alloc)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(ptr_metadata)]
#![feature(alloc_layout_extra)]
#![feature(array_windows)]
#![feature(btree_cursors)]
#![feature(slice_from_ptr_range)]
#![feature(const_trait_impl)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::double_must_use)]
#![allow(clippy::explicit_auto_deref)]

//! anvil-zksync
//!
//! The `anvil-zksync` crate provides an in-memory node designed primarily for local testing.
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
//! To start the node, use the command `anvil-zksync run`. For more advanced functionalities like forking or
//! replaying transactions, refer to the [official documentation](https://era.zksync.io/docs/tools/testing/anvil-zksync.html).
//!
//! ## Contributions
//!
//! Contributions to improve `anvil-zksync` are welcome. Please refer to the [contribution guidelines](https://github.com/matter-labs/anvil-zksync/blob/main/.github/CONTRIBUTING.md) for more details.

pub mod bootloader_debug;
pub mod deps;
pub mod filters;
pub mod formatter;
pub mod node;
pub mod observability;
pub mod system_contracts;
pub mod utils;

mod testing;
