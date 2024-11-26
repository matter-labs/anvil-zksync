use jsonrpc_derive::rpc;
use zksync_types::{Address, U256, U64};

use super::{ResetRequest, RpcResult};
use crate::utils::Numeric;

#[rpc]
pub trait AnvilNamespaceT {
    /// Set the minimum gas price for the node. Unsupported by design.
    ///
    /// # Arguments
    ///
    /// * `gas` - The minimum gas price to be set
    #[rpc(name = "anvil_setMinGasPrice")]
    fn set_min_gas_price(&self, gas: U256) -> RpcResult<()>;

    /// Snapshot the state of the blockchain at the current block. Takes no parameters. Returns the id of the snapshot
    /// that was created. A snapshot can only be reverted once. After a successful `anvil_revert`, the same snapshot id cannot
    /// be used again. Consider creating a new snapshot after each `anvil_revert` if you need to revert to the same
    /// point multiple times.
    ///
    /// # Returns
    /// The `U64` identifier for this snapshot.
    #[rpc(name = "anvil_snapshot")]
    fn snapshot(&self) -> RpcResult<U64>;

    /// Revert the state of the blockchain to a previous snapshot. Takes a single parameter,
    /// which is the snapshot id to revert to. This deletes the given snapshot, as well as any snapshots
    /// taken after (e.g.: reverting to id 0x1 will delete snapshots with ids 0x1, 0x2, etc.)
    ///
    /// # Arguments
    ///
    /// * `id` - The snapshot id to revert
    ///
    /// # Returns
    /// `true` if a snapshot was reverted, otherwise `false`.
    #[rpc(name = "anvil_revert")]
    fn revert(&self, id: U64) -> RpcResult<bool>;

    /// Set the current timestamp for the node.
    /// Warning: This will allow you to move backwards in time, which may cause new blocks to appear to be
    /// mined before old blocks. This will result in an invalid state.
    ///
    /// # Arguments
    ///
    /// * `time` - The timestamp to set the time to
    ///
    /// # Returns
    /// The difference between the current timestamp and the new timestamp.
    #[rpc(name = "anvil_setTime")]
    fn set_time(&self, timestamp: Numeric) -> RpcResult<i128>;

    /// Increase the current timestamp for the node
    ///
    /// # Arguments
    ///
    /// * `seconds` - The number of seconds to increase time by
    ///
    /// # Returns
    /// The applied time delta to the current timestamp in seconds.
    #[rpc(name = "anvil_increaseTime")]
    fn increase_time(&self, seconds: Numeric) -> RpcResult<u64>;

    /// Set timestamp for the next block. The timestamp must be in future.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp to set the time to
    #[rpc(name = "anvil_setNextBlockTimestamp")]
    fn set_next_block_timestamp(&self, timestamp: Numeric) -> RpcResult<()>;

    /// Sets auto impersonation status.
    ///
    /// # Arguments
    ///
    /// * `enabled` - `true` makes every account impersonated, `false` disables this behavior
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` representing the success of the operation.
    #[rpc(name = "anvil_autoImpersonateAccount")]
    fn auto_impersonate_account(&self, enabled: bool) -> RpcResult<()>;

    /// Sets the balance of the given address to the given balance.
    ///
    /// # Arguments
    ///
    /// * `address` - The `Address` whose balance will be edited
    /// * `balance` - The new balance to set for the given address, in wei
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "anvil_setBalance")]
    fn set_balance(&self, address: Address, balance: U256) -> RpcResult<bool>;

    /// Modifies an account's nonce by overwriting it.
    ///
    /// # Arguments
    ///
    /// * `address` - The `Address` whose nonce is to be changed
    /// * `nonce` - The new nonce
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "anvil_setNonce")]
    fn set_nonce(&self, address: Address, nonce: U256) -> RpcResult<bool>;

    /// Sometimes you may want to advance the latest block number of the network by a large number of blocks.
    /// One way to do this would be to call the evm_mine RPC method multiple times, but this is too slow if you want to mine thousands of blocks.
    /// The `anvil_mine` method can mine any number of blocks at once, in constant time. (It exhibits the same performance no matter how many blocks are mined.)
    ///
    /// # Arguments
    ///
    /// * `num_blocks` - The number of blocks to mine, defaults to 1
    /// * `interval` - The interval between the timestamps of each block, in seconds, and it also defaults to 1
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "anvil_mine")]
    fn anvil_mine(&self, num_blocks: Option<U64>, interval: Option<U64>) -> RpcResult<()>;

    /// Reset the state of the network back to a fresh forked state, or disable forking.
    ///
    /// # Arguments
    ///
    /// * `reset_spec` - The requested state, defaults to resetting the current network.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "anvil_reset")]
    fn reset_network(&self, reset_spec: Option<ResetRequest>) -> RpcResult<bool>;

    /// Era Test Node allows transactions impersonating specific account and contract addresses.
    /// To impersonate an account use this method, passing the address to impersonate as its parameter.
    /// After calling this method, any transactions with this sender will be executed without verification.
    /// Multiple addresses can be impersonated at once.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to impersonate
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "anvil_impersonateAccount")]
    fn impersonate_account(&self, address: Address) -> RpcResult<bool>;

    /// Use this method to stop impersonating an account after having previously used `anvil_impersonateAccount`
    /// The method returns `true` if the account was being impersonated and `false` otherwise.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to stop impersonating.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "anvil_stopImpersonatingAccount")]
    fn stop_impersonating_account(&self, address: Address) -> RpcResult<bool>;

    /// Modifies the bytecode stored at an account's address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address where the given code should be stored.
    /// * `code` - The code to be stored.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "anvil_setCode")]
    fn set_code(&self, address: Address, code: String) -> RpcResult<()>;

    /// Directly modifies the storage of a contract at a specified slot.
    ///
    /// # Arguments
    ///
    /// * `address` - The contract address whose storage is to be modified.
    /// * `slot` - The storage slot to modify.
    /// * `value` - The value to be set at the specified slot.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "anvil_setStorageAt")]
    fn set_storage_at(&self, address: Address, slot: U256, value: U256) -> RpcResult<bool>;
}
