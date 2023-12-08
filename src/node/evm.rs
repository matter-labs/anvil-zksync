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
