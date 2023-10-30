use jsonrpc_derive::rpc;
use zksync_basic_types::U64;

use crate::namespaces::RpcResult;

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
    fn increase_time(&self, time_delta_seconds: u64) -> RpcResult<u64>;

    /// Force a single block to be mined.
    ///
    /// Will mine an empty block (containing zero transactions)
    ///
    /// # Returns
    /// The string "0x0".
    #[rpc(name = "evm_mine")]
    fn evm_mine(&self) -> RpcResult<String>;

    /// Set the current timestamp for the node. The timestamp must be in future.
    ///
    /// # Parameters
    /// - `timestamp`: The timestamp to set the time to
    ///
    /// # Returns
    /// The new timestamp value for the InMemoryNodeInner.
    #[rpc(name = "evm_setNextBlockTimestamp")]
    fn set_next_block_timestamp(&self, timestamp: u64) -> RpcResult<u64>;

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
    fn set_time(&self, time: u64) -> RpcResult<i128>;

    /// Snapshot the state of the blockchain at the current block. Takes no parameters. Returns the id of the snapshot
    /// that was created. A snapshot can only be reverted once. After a successful evm_revert, the same snapshot id cannot
    /// be used again. Consider creating a new snapshot after each evm_revert if you need to revert to the same
    /// point multiple times.
    ///
    /// # Returns
    /// The `U64` identifier for this snapshot.
    #[rpc(name = "evm_snapshot")]
    fn snapshot(&self) -> RpcResult<U64>;

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
    fn revert_snapshot(&self, snapshot_id: U64) -> RpcResult<bool>;
}
