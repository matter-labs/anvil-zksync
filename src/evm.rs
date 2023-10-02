use std::sync::{Arc, RwLock};

use crate::{
    fork::ForkSource,
    node::{InMemoryNodeInner, Snapshot},
    utils::mine_empty_blocks,
};
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_basic_types::U64;
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_web3_decl::error::Web3Error;

/// The maximum number of [Snapshot]s to store. Each snapshot represents the node state
/// and can be used to revert the node to an earlier point in time.
const MAX_SNAPSHOTS: u8 = 100;

/// Implementation of EvmNamespace
pub struct EvmNamespaceImpl<S> {
    node: Arc<RwLock<InMemoryNodeInner<S>>>,
    /// List of snapshots of the [InMemoryNodeInner]. This is bounded at runtime by [MAX_SNAPSHOTS].
    snapshots: Arc<RwLock<Vec<Snapshot>>>,
}

impl<S> EvmNamespaceImpl<S> {
    /// Creates a new `Evm` instance with the given `node`.
    pub fn new(node: Arc<RwLock<InMemoryNodeInner<S>>>) -> Self {
        Self {
            node,
            snapshots: Default::default(),
        }
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

    /// Force a single block to be mined.
    ///
    /// Will mine an empty block (containing zero transactions)
    ///
    /// # Returns
    /// The string "0x0".
    #[rpc(name = "evm_mine")]
    fn evm_mine(&self) -> BoxFuture<Result<String>>;

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

    /// Snapshot the state of the blockchain at the current block. Takes no parameters. Returns the id of the snapshot
    /// that was created. A snapshot can only be reverted once. After a successful evm_revert, the same snapshot id cannot
    /// be used again. Consider creating a new snapshot after each evm_revert if you need to revert to the same
    /// point multiple times.
    ///
    /// # Returns
    /// The `U64` identifier for this snapshot.
    #[rpc(name = "evm_snapshot")]
    fn snapshot(&self) -> BoxFuture<Result<U64>>;

    /// Revert the state of the blockchain to a previous snapshot. Takes a single parameter,
    /// which is the snapshot id to revert to. This deletes the given snapshot, as well as any snapshots
    /// taken after (e.g.: reverting to id 0x1 will delete snapshots with ids 0x1, 0x2, etc.)
    ///
    /// # Parameters
    /// - `snapshot_id`: The snapshot id to revert.
    ///
    /// # Returns
    /// `true` if a snapshot was reverted, otherwise `false`.
    #[rpc(name = "evm_revert")]
    fn revert_snapshot(&self, snapshot_id: U64) -> BoxFuture<Result<bool>>;
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

    fn evm_mine(&self) -> BoxFuture<Result<String>> {
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            match inner.write() {
                Ok(mut inner) => {
                    mine_empty_blocks(&mut inner, 1, 1000);
                    log::info!("👷 Mined block #{}", inner.current_miniblock);
                    Ok("0x0".to_string())
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn snapshot(&self) -> BoxFuture<Result<U64>> {
        let inner = Arc::clone(&self.node);
        let snapshots = Arc::clone(&self.snapshots);

        Box::pin(async move {
            // validate max snapshots
            snapshots
                .read()
                .map_err(|err| {
                    log::error!("failed acquiring read lock for snapshot: {:?}", err);
                    into_jsrpc_error(Web3Error::InternalError)
                })
                .and_then(|snapshots| {
                    if snapshots.len() >= MAX_SNAPSHOTS as usize {
                        log::error!("maximum number of '{}' snapshots exceeded", MAX_SNAPSHOTS);
                        Err(into_jsrpc_error(Web3Error::InternalError))
                    } else {
                        Ok(())
                    }
                })?;

            // snapshot the node
            let snapshot = inner
                .read()
                .map_err(|err| {
                    format!("failed acquiring read lock to node for snapshot: {:?}", err)
                })
                .and_then(|reader| reader.snapshot())
                .map_err(|err| {
                    log::error!("failed creating snapshot: {:?}", err);
                    into_jsrpc_error(Web3Error::InternalError)
                })?;
            snapshots
                .write()
                .map(|mut snapshots| {
                    snapshots.push(snapshot);
                    snapshots.len()
                })
                .map_err(|err| {
                    log::error!("failed storing snapshot: {:?}", err);
                    into_jsrpc_error(Web3Error::InternalError)
                })
                .map(U64::from)
        })
    }

    fn revert_snapshot(&self, snapshot_id: U64) -> BoxFuture<Result<bool>> {
        let inner = Arc::clone(&self.node);
        let snapshots = Arc::clone(&self.snapshots);

        Box::pin(async move {
            let mut snapshots = snapshots.write().map_err(|err| {
                log::error!("failed acquiring read lock for snapshots: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })?;
            let snapshot_id_index = snapshot_id.as_usize().saturating_sub(1);
            if snapshot_id_index >= snapshots.len() {
                log::error!("no snapshot exists for the id '{}'", snapshot_id);
                return Err(into_jsrpc_error(Web3Error::InternalError));
            }

            // remove all snapshots following the index and use the first snapshot for restore
            let selected_snapshot = snapshots
                .drain(snapshot_id_index..)
                .next()
                .expect("unexpected failure, value must exist");

            inner
                .write()
                .map_err(|err| format!("failed acquiring read lock for snapshots: {:?}", err))
                .and_then(|mut writer| writer.restore_snapshot(selected_snapshot).map(|_| true))
                .or_else(|err| {
                    log::error!(
                        "failed restoring snapshot for id '{}': {}",
                        snapshot_id,
                        err
                    );
                    Ok(false)
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};
    use zksync_core::api_server::web3::backend_jsonrpc::namespaces::eth::EthNamespaceT;

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

    #[tokio::test]
    async fn test_evm_mine() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");
        let result = evm.evm_mine().await.expect("evm_mine");
        assert_eq!(&result, "0x0");

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 1, current_block.number);
        assert_eq!(start_block.timestamp + 1, current_block.timestamp);

        let result = evm.evm_mine().await.expect("evm_mine");
        assert_eq!(&result, "0x0");

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 2, current_block.number);
        assert_eq!(start_block.timestamp + 2, current_block.timestamp);
    }

    #[tokio::test]
    async fn test_evm_snapshot_creates_incrementing_ids() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let snapshot_id_1 = evm.snapshot().await.expect("failed creating snapshot 1");
        let snapshot_id_2 = evm.snapshot().await.expect("failed creating snapshot 2");

        assert_eq!(snapshot_id_1, U64::from(1));
        assert_eq!(snapshot_id_2, U64::from(2));
    }

    #[tokio::test]
    async fn test_evm_revert_snapshot_restores_state() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let initial_block = node
            .get_block_number()
            .await
            .expect("failed fetching block number");
        let snapshot_id = evm.snapshot().await.expect("failed creating snapshot");
        evm.evm_mine().await.expect("evm_mine");
        let current_block = node
            .get_block_number()
            .await
            .expect("failed fetching block number");
        assert_eq!(current_block, initial_block + 1);

        let reverted = evm
            .revert_snapshot(snapshot_id)
            .await
            .expect("failed reverting snapshot");
        assert!(reverted);

        let restored_block = node
            .get_block_number()
            .await
            .expect("failed fetching block number");
        assert_eq!(restored_block, initial_block);
    }

    #[tokio::test]
    async fn test_evm_revert_snapshot_removes_all_snapshots_following_the_reverted_one() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let _snapshot_id_1 = evm.snapshot().await.expect("failed creating snapshot");
        let snapshot_id_2 = evm.snapshot().await.expect("failed creating snapshot");
        let _snapshot_id_3 = evm.snapshot().await.expect("failed creating snapshot");
        assert_eq!(3, evm.snapshots.read().unwrap().len());

        let reverted = evm
            .revert_snapshot(snapshot_id_2)
            .await
            .expect("failed reverting snapshot");
        assert!(reverted);

        assert_eq!(1, evm.snapshots.read().unwrap().len());
    }

    #[tokio::test]
    async fn test_evm_revert_snapshot_fails_for_invalid_snapshot_id() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let result = evm.revert_snapshot(U64::from(100)).await;
        assert!(result.is_err());
    }
}
