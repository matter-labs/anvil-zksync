use zksync_basic_types::{Address, U256, U64};
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_web3_decl::error::Web3Error;

use crate::{
    fork::ForkSource,
    namespaces::{HardhatNamespaceT, RpcResult},
    node::InMemoryNode,
    utils::IntoBoxedFuture,
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> HardhatNamespaceT
    for InMemoryNode<S>
{
    fn set_balance(&self, address: Address, balance: U256) -> RpcResult<bool> {
        self.set_balance(address, balance)
            .map_err(|err| {
                tracing::error!("failed setting balance : {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn set_nonce(&self, address: Address, balance: U256) -> RpcResult<bool> {
        self.set_nonce(address, balance)
            .map_err(|err| {
                tracing::error!("failed setting nonce: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn hardhat_mine(&self, num_blocks: Option<U64>, interval: Option<U64>) -> RpcResult<bool> {
        self.mine_blocks(num_blocks, interval)
            .map_err(|err| {
                tracing::error!("failed mining blocks: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn impersonate_account(&self, address: Address) -> RpcResult<bool> {
        self.impersonate_account(address)
            .map_err(|err| {
                tracing::error!("failed impersonating account: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn stop_impersonating_account(&self, address: Address) -> RpcResult<bool> {
        InMemoryNode::<S>::stop_impersonating_account(self, address)
            .map_err(|err| {
                tracing::error!("failed stopping to impersonate account: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn set_code(&self, address: Address, code: Vec<u8>) -> RpcResult<()> {
        self.set_code(address, code)
            .map_err(|err| {
                tracing::error!("failed setting code: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }
}
