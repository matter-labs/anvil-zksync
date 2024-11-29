use zksync_types::{Address, H256, U256, U64};
use zksync_web3_decl::error::Web3Error;

use crate::utils::Numeric;
use crate::{
    fork::ForkSource,
    namespaces::{AnvilNamespaceT, ResetRequest, RpcResult},
    node::InMemoryNode,
    utils::{into_jsrpc_error, into_jsrpc_error_message, IntoBoxedFuture},
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> AnvilNamespaceT
    for InMemoryNode<S>
{
    fn set_next_block_base_fee_per_gas(&self, base_fee: U256) -> RpcResult<()> {
        self.set_next_block_base_fee_per_gas(base_fee)
            .map_err(|err| {
                tracing::error!("failed setting next block's base fee: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn drop_transaction(&self, hash: H256) -> RpcResult<Option<H256>> {
        self.drop_transaction(hash)
            .map_err(|err| {
                tracing::error!("failed dropping transaction: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn drop_all_transactions(&self) -> RpcResult<()> {
        self.drop_all_transactions()
            .map_err(|err| {
                tracing::error!("failed dropping all transactions: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn remove_pool_transactions(&self, address: Address) -> RpcResult<()> {
        self.remove_pool_transactions(address)
            .map_err(|err| {
                tracing::error!("failed removing pool transactions: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn get_auto_mine(&self) -> RpcResult<bool> {
        self.get_immediate_sealing()
            .map_err(|err| {
                tracing::error!("failed getting immediate sealing: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn set_auto_mine(&self, enable: bool) -> RpcResult<()> {
        self.set_immediate_sealing(enable)
            .map_err(|err| {
                tracing::error!("failed setting immediate sealing: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn set_interval_mining(&self, seconds: u64) -> RpcResult<()> {
        self.set_interval_sealing(seconds)
            .map_err(|err| {
                tracing::error!("failed setting interval sealing: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn set_block_timestamp_interval(&self, seconds: u64) -> RpcResult<()> {
        self.time.set_block_timestamp_interval(seconds);
        Ok(()).into_boxed_future()
    }

    fn remove_block_timestamp_interval(&self) -> RpcResult<bool> {
        Ok(self.time.remove_block_timestamp_interval()).into_boxed_future()
    }

    fn set_min_gas_price(&self, _gas: U256) -> RpcResult<()> {
        tracing::info!("anvil_setMinGasPrice is unsupported as ZKsync is a post-EIP1559 chain");
        Err(into_jsrpc_error(Web3Error::MethodNotImplemented)).into_boxed_future()
    }

    fn set_logging_enabled(&self, enable: bool) -> RpcResult<()> {
        self.set_logging_enabled(enable)
            .map_err(|err| {
                tracing::error!("failed creating snapshot: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn snapshot(&self) -> RpcResult<U64> {
        self.snapshot()
            .map_err(|err| {
                tracing::error!("failed creating snapshot: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn revert(&self, id: U64) -> RpcResult<bool> {
        self.revert_snapshot(id)
            .map_err(|err| {
                tracing::error!("failed reverting snapshot: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn set_time(&self, timestamp: Numeric) -> RpcResult<i128> {
        self.set_time(timestamp)
            .map_err(|err| {
                tracing::error!("failed setting time: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn increase_time(&self, seconds: Numeric) -> RpcResult<u64> {
        self.increase_time(seconds)
            .map_err(|err| {
                tracing::error!("failed increasing time: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn set_next_block_timestamp(&self, timestamp: Numeric) -> RpcResult<()> {
        self.set_next_block_timestamp(timestamp)
            .map_err(|err| {
                tracing::error!("failed setting time for next timestamp: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn auto_impersonate_account(&self, enabled: bool) -> RpcResult<()> {
        self.auto_impersonate_account(enabled);
        Ok(()).into_boxed_future()
    }

    fn set_balance(&self, address: Address, balance: U256) -> RpcResult<bool> {
        self.set_balance(address, balance)
            .map_err(|err| {
                tracing::error!("failed setting balance : {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn set_nonce(&self, address: Address, nonce: U256) -> RpcResult<bool> {
        self.set_nonce(address, nonce)
            .map_err(|err| {
                tracing::error!("failed setting nonce: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn anvil_mine(&self, num_blocks: Option<U64>, interval: Option<U64>) -> RpcResult<()> {
        self.mine_blocks(num_blocks, interval)
            .map_err(|err| {
                tracing::error!("failed mining blocks: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn reset_network(&self, reset_spec: Option<ResetRequest>) -> RpcResult<bool> {
        self.reset_network(reset_spec)
            .map_err(|err| {
                tracing::error!("failed reset: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn impersonate_account(&self, address: Address) -> RpcResult<bool> {
        self.impersonate_account(address)
            .map_err(|err| {
                tracing::error!("failed impersonating account: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn stop_impersonating_account(&self, address: Address) -> RpcResult<bool> {
        InMemoryNode::<S>::stop_impersonating_account(self, address)
            .map_err(|err| {
                tracing::error!("failed stopping to impersonate account: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn set_code(&self, address: Address, code: String) -> RpcResult<()> {
        self.set_code(address, code)
            .map_err(|err| {
                tracing::error!("failed setting code: {:?}", err);
                into_jsrpc_error_message(err.to_string())
            })
            .into_boxed_future()
    }

    fn set_storage_at(&self, address: Address, slot: U256, value: U256) -> RpcResult<bool> {
        self.set_storage_at(address, slot, value)
            .map_err(|err| {
                tracing::error!("failed setting storage: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }
}
