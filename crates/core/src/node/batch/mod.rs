//! Main implementation of ZKsync VM [batch executor](crate::interface::BatchExecutor).
//!
//! This implementation is used by various ZKsync components, like the state keeper and components based on the VM runner.

pub use self::factory::{MainBatchExecutorFactory, TraceCalls};

mod executor;
mod factory;
mod shared;
