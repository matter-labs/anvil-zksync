use zksync_types::{Address, U256, U64};
use zksync_web3_decl::error::Web3Error;

use crate::utils::Numeric;
use crate::{
    fork::ForkSource,
    namespaces::{EvmNamespaceT, RpcResult},
    node::InMemoryNode,
    utils::{into_jsrpc_error, IntoBoxedFuture},
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> EvmNamespaceT
    for InMemoryNode<S>
{
    fn increase_time(&self, time_delta_seconds: Numeric) -> RpcResult<u64> {
        self.increase_time(time_delta_seconds)
            .map_err(|err| {
                tracing::error!("failed increasing time: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn set_nonce(&self, address: Address, balance: U256) -> RpcResult<bool> {
        self.set_nonce(address, balance)
            .map_err(|err| {
                tracing::error!("failed setting nonce: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }

    fn evm_mine(&self) -> RpcResult<String> {
        self.mine_block()
            .map_err(|err| {
                tracing::error!("failed mining block: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .map(|_| "0x0".to_string())
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

    fn set_time(&self, timestamp: Numeric) -> RpcResult<i128> {
        self.set_time(timestamp)
            .map_err(|err| {
                tracing::error!("failed setting time: {:?}", err);
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

    fn revert_snapshot(&self, snapshot_id: U64) -> RpcResult<bool> {
        self.revert_snapshot(snapshot_id)
            .map_err(|err| {
                tracing::error!("failed reverting snapshot: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            })
            .into_boxed_future()
    }
}
