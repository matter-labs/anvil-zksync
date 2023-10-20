use zksync_basic_types::U64;
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_web3_decl::error::Web3Error;

use crate::{
    fork::ForkSource,
    namespaces::{EvmNamespaceT, RpcResult},
    node::InMemoryNode,
    utils::IntoBoxedFuture,
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> EvmNamespaceT
    for InMemoryNode<S>
{
    fn increase_time(&self, time_delta_seconds: u64) -> RpcResult<u64> {
        self.increase_time(time_delta_seconds)
            .map_err(|err| {
                tracing::error!("failed increasing time: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn evm_mine(&self) -> RpcResult<String> {
        self.mine_block()
            .map_err(|err| {
                tracing::error!("failed mining block: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn set_next_block_timestamp(&self, timestamp: u64) -> RpcResult<u64> {
        self.set_next_block_timestamp(timestamp)
            .map_err(|err| {
                tracing::error!("failed setting time for next timestamp: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn set_time(&self, time: u64) -> RpcResult<i128> {
        self.set_time(time)
            .map_err(|err| {
                tracing::error!("failed setting time: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn snapshot(&self) -> RpcResult<U64> {
        self.snapshot()
            .map_err(|err| {
                tracing::error!("failed creating snapshot: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn revert_snapshot(&self, snapshot_id: U64) -> RpcResult<bool> {
        self.revert_snapshot(snapshot_id)
            .map_err(|err| {
                tracing::error!("failed reverting snapshot: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
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

        let increase_value_seconds = 0u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        let expected_response = increase_value_seconds;

        let actual_response = EvmNamespaceT::increase_time(&node, increase_value_seconds)
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

        let increase_value_seconds = u64::MAX;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(0, timestamp_before, "initial timestamp must be non zero",);
        let expected_response = increase_value_seconds;

        let actual_response = EvmNamespaceT::increase_time(&node, increase_value_seconds)
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

        let increase_value_seconds = 100u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        let expected_response = increase_value_seconds;

        let actual_response = EvmNamespaceT::increase_time(&node, increase_value_seconds)
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

        let actual_response = EvmNamespaceT::set_next_block_timestamp(&node, new_timestamp)
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

        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        let new_timestamp = timestamp_before + 500;
        EvmNamespaceT::set_next_block_timestamp(&node, new_timestamp)
            .await
            .expect("failed setting timestamp");

        let result = EvmNamespaceT::set_next_block_timestamp(&node, timestamp_before).await;

        assert!(result.is_err(), "expected an error for timestamp in past");
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_same_value() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_timestamp = 1000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_eq!(timestamp_before, new_timestamp, "timestamps must be same");
        let expected_response = new_timestamp;

        let actual_response = EvmNamespaceT::set_next_block_timestamp(&node, new_timestamp)
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

        let new_time = 10_000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = 9000;

        let actual_response = EvmNamespaceT::set_time(&node, new_time)
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

        let new_time = 10u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = -990;

        let actual_response = EvmNamespaceT::set_time(&node, new_time)
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

        let new_time = 1000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_eq!(timestamp_before, new_time, "timestamps must be same");
        let expected_response = 0;

        let actual_response = EvmNamespaceT::set_time(&node, new_time)
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

            let actual_response = EvmNamespaceT::set_time(&node, new_time)
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

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");
        let result = node.evm_mine().await.expect("evm_mine");
        assert_eq!(&result, "0x0");

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 1, current_block.number);
        assert_eq!(start_block.timestamp + 1, current_block.timestamp);

        let result = node.evm_mine().await.expect("evm_mine");
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

        let snapshot_id_1 = EvmNamespaceT::snapshot(&node)
            .await
            .expect("failed creating snapshot 1");
        let snapshot_id_2 = EvmNamespaceT::snapshot(&node)
            .await
            .expect("failed creating snapshot 2");

        assert_eq!(snapshot_id_1, U64::from(1));
        assert_eq!(snapshot_id_2, U64::from(2));
    }

    #[tokio::test]
    async fn test_evm_revert_snapshot_restores_state() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let initial_block = node
            .get_block_number()
            .await
            .expect("failed fetching block number");
        let snapshot_id = EvmNamespaceT::snapshot(&node)
            .await
            .expect("failed creating snapshot");
        node.evm_mine().await.expect("evm_mine");
        let current_block = node
            .get_block_number()
            .await
            .expect("failed fetching block number");
        assert_eq!(current_block, initial_block + 1);

        let reverted = EvmNamespaceT::revert_snapshot(&node, snapshot_id)
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

        let _snapshot_id_1 = EvmNamespaceT::snapshot(&node)
            .await
            .expect("failed creating snapshot");
        let snapshot_id_2 = EvmNamespaceT::snapshot(&node)
            .await
            .expect("failed creating snapshot");
        let _snapshot_id_3 = EvmNamespaceT::snapshot(&node)
            .await
            .expect("failed creating snapshot");
        assert_eq!(3, node.snapshots.read().unwrap().len());

        let reverted = EvmNamespaceT::revert_snapshot(&node, snapshot_id_2)
            .await
            .expect("failed reverting snapshot");
        assert!(reverted);

        assert_eq!(1, node.snapshots.read().unwrap().len());
    }

    #[tokio::test]
    async fn test_evm_revert_snapshot_fails_for_invalid_snapshot_id() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let result = EvmNamespaceT::revert_snapshot(&node, U64::from(100)).await;
        assert!(result.is_err());
    }
}
