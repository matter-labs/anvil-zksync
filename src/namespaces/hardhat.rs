use jsonrpc_derive::rpc;
use zksync_basic_types::{Address, U256, U64};

use super::RpcResult;

#[rpc]
pub trait HardhatNamespaceT {
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
    #[rpc(name = "hardhat_setBalance")]
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
    #[rpc(name = "hardhat_setNonce")]
    fn set_nonce(&self, address: Address, balance: U256) -> RpcResult<bool>;

    /// Sometimes you may want to advance the latest block number of the network by a large number of blocks.
    /// One way to do this would be to call the evm_mine RPC method multiple times, but this is too slow if you want to mine thousands of blocks.
    /// The hardhat_mine method can mine any number of blocks at once, in constant time. (It exhibits the same performance no matter how many blocks are mined.)
    ///
    /// # Arguments
    ///
    /// * `num_blocks` - The number of blocks to mine, defaults to 1
    /// * `interval` - The interval between the timestamps of each block, in seconds, and it also defaults to 1
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "hardhat_mine")]
    fn hardhat_mine(&self, num_blocks: Option<U64>, interval: Option<U64>) -> RpcResult<bool>;

    /// Hardhat Network allows you to send transactions impersonating specific account and contract addresses.
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
    #[rpc(name = "hardhat_impersonateAccount")]
    fn impersonate_account(&self, address: Address) -> RpcResult<bool>;

    /// Use this method to stop impersonating an account after having previously used `hardhat_impersonateAccount`
    /// The method returns `true` if the account was being impersonated and `false` otherwise.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to stop impersonating.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "hardhat_stopImpersonatingAccount")]
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
    #[rpc(name = "hardhat_setCode")]
    fn set_code(&self, address: Address, code: Vec<u8>) -> RpcResult<()>;
}
