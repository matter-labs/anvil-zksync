use std::sync::{Arc, RwLock};

use crate::{fork::ForkSource, node::InMemoryNodeInner};
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_web3_decl::error::Web3Error;

/// Implementation of EvmNamespace
pub struct EvmNamespaceImpl<S> {
    node: Arc<RwLock<InMemoryNodeInner<S>>>,
}

impl<S> EvmNamespaceImpl<S> {
    /// Creates a new `Evm` instance with the given `node`.
    pub fn new(node: Arc<RwLock<InMemoryNodeInner<S>>>) -> Self {
        Self { node }
    }
}

#[rpc]
pub trait EvmNamespaceT {
    /// Increase the current timestamp for the node
    ///
    /// # Parameters
    /// - `time_delta`: The number of seconds to increase time by
    ///
    /// # Returns
    /// The applied time delta to `current_timestamp` value for the InMemoryNodeInner.
    #[rpc(name = "evm_increaseTime")]
    fn increase_time(&self, time_delta_seconds: u64) -> BoxFuture<Result<u64>>;

    /// Set the current timestamp for the node. The timestamp must be in future.
    ///
    /// # Parameters
    /// - `timestamp`: The timestamp to set the time to
    ///
    /// # Returns
    /// The new timestamp value for the InMemoryNodeInner.
    #[rpc(name = "evm_setNextBlockTimestamp")]
    fn set_next_block_timestamp(&self, timestamp: u64) -> BoxFuture<Result<u64>>;

    /// Set the current timestamp for the node.
    /// Warning: This will allow you to move backwards in time, which may cause new blocks to appear to be
    /// mined before old blocks. This will result in an invalid state.
    ///
    /// # Parameters
    /// - `time`: The timestamp to set the time to
    ///
    /// # Returns
    /// The difference between the `current_timestamp` and the new timestamp for the InMemoryNodeInner.
    #[rpc(name = "evm_setTime")]
    fn set_time(&self, time: u64) -> BoxFuture<Result<i128>>;
}

impl<S: Send + Sync + 'static + ForkSource + std::fmt::Debug> EvmNamespaceT
    for EvmNamespaceImpl<S>
{
    fn increase_time(&self, time_delta_seconds: u64) -> BoxFuture<Result<u64>> {
        let inner = Arc::clone(&self.node);

        Box::pin(async move {
            if time_delta_seconds == 0 {
                return Ok(time_delta_seconds);
            }

            let time_delta = time_delta_seconds.saturating_mul(1000);
            match inner.write() {
                Ok(mut inner_guard) => {
                    inner_guard.current_timestamp =
                        inner_guard.current_timestamp.saturating_add(time_delta);
                    Ok(time_delta_seconds)
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn set_next_block_timestamp(&self, timestamp: u64) -> BoxFuture<Result<u64>> {
        let inner = Arc::clone(&self.node);

        Box::pin(async move {
            match inner.write() {
                Ok(mut inner_guard) => {
                    if timestamp < inner_guard.current_timestamp {
                        Err(into_jsrpc_error(Web3Error::InternalError))
                    } else {
                        inner_guard.current_timestamp = timestamp;
                        Ok(timestamp)
                    }
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn set_time(&self, time: u64) -> BoxFuture<Result<i128>> {
        let inner = Arc::clone(&self.node);

        Box::pin(async move {
            match inner.write() {
                Ok(mut inner_guard) => {
                    let time_diff =
                        (time as i128).saturating_sub(inner_guard.current_timestamp as i128);
                    inner_guard.current_timestamp = time;
                    Ok(time_diff)
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};

    use super::*;

    #[tokio::test]
    async fn test_increase_time_zero_value() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let increase_value_seconds = 0u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        let expected_response = increase_value_seconds;

        let actual_response = evm
            .increase_time(increase_value_seconds)
            .await
            .expect("failed increasing timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            increase_value_seconds.saturating_mul(1000u64),
            timestamp_after.saturating_sub(timestamp_before),
            "timestamp did not increase by the specified amount",
        );
    }

    #[tokio::test]
    async fn test_increase_time_max_value() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let increase_value_seconds = u64::MAX;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(0, timestamp_before, "initial timestamp must be non zero",);
        let expected_response = increase_value_seconds;

        let actual_response = evm
            .increase_time(increase_value_seconds)
            .await
            .expect("failed increasing timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            u64::MAX,
            timestamp_after,
            "timestamp did not saturate upon increase",
        );
    }

    #[tokio::test]
    async fn test_increase_time() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let increase_value_seconds = 100u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        let expected_response = increase_value_seconds;

        let actual_response = evm
            .increase_time(increase_value_seconds)
            .await
            .expect("failed increasing timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            increase_value_seconds.saturating_mul(1000u64),
            timestamp_after.saturating_sub(timestamp_before),
            "timestamp did not increase by the specified amount",
        );
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_future() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let new_timestamp = 10_000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(
            timestamp_before, new_timestamp,
            "timestamps must be different"
        );
        let expected_response = new_timestamp;

        let actual_response = evm
            .set_next_block_timestamp(new_timestamp)
            .await
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            new_timestamp, timestamp_after,
            "timestamp was not set correctly",
        );
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_past_fails() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        let new_timestamp = timestamp_before + 500;
        evm.set_next_block_timestamp(new_timestamp)
            .await
            .expect("failed setting timestamp");

        let result = evm.set_next_block_timestamp(timestamp_before).await;

        assert!(result.is_err(), "expected an error for timestamp in past");
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_same_value() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let new_timestamp = 1000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_eq!(timestamp_before, new_timestamp, "timestamps must be same");
        let expected_response = new_timestamp;

        let actual_response = evm
            .set_next_block_timestamp(new_timestamp)
            .await
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            timestamp_before, timestamp_after,
            "timestamp must not change",
        );
    }

    #[tokio::test]
    async fn test_set_time_future() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let new_time = 10_000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = 9000;

        let actual_response = evm
            .set_time(new_time)
            .await
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(new_time, timestamp_after, "timestamp was not set correctly",);
    }

    #[tokio::test]
    async fn test_set_time_past() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let new_time = 10u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = -990;

        let actual_response = evm
            .set_time(new_time)
            .await
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(new_time, timestamp_after, "timestamp was not set correctly",);
    }

    #[tokio::test]
    async fn test_set_time_same_value() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let new_time = 1000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_eq!(timestamp_before, new_time, "timestamps must be same");
        let expected_response = 0;

        let actual_response = evm
            .set_time(new_time)
            .await
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            timestamp_before, timestamp_after,
            "timestamp must not change",
        );
    }

    #[tokio::test]
    async fn test_set_time_edges() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        for new_time in [0, u64::MAX] {
            let timestamp_before = node
                .get_inner()
                .read()
                .map(|inner| inner.current_timestamp)
                .unwrap_or_else(|_| panic!("case {}: failed reading timestamp", new_time));
            assert_ne!(
                timestamp_before, new_time,
                "case {new_time}: timestamps must be different"
            );
            let expected_response = (new_time as i128).saturating_sub(timestamp_before as i128);

            let actual_response = evm
                .set_time(new_time)
                .await
                .expect("failed setting timestamp");
            let timestamp_after = node
                .get_inner()
                .read()
                .map(|inner| inner.current_timestamp)
                .unwrap_or_else(|_| panic!("case {}: failed reading timestamp", new_time));

            assert_eq!(
                expected_response, actual_response,
                "case {new_time}: erroneous response"
            );
            assert_eq!(
                new_time, timestamp_after,
                "case {new_time}: timestamp was not set correctly",
            );
        }
    }
}
