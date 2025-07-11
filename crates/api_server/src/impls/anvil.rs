use anvil_zksync_api_decl::AnvilNamespaceServer;
use anvil_zksync_common::sh_warn;
use anvil_zksync_core::node::InMemoryNode;
use anvil_zksync_types::Numeric;
use anvil_zksync_types::api::{DetailedTransaction, ResetRequest};
use jsonrpsee::core::{RpcResult, async_trait};
use zksync_types::api::Block;
use zksync_types::web3::Bytes;
use zksync_types::{Address, H256, U64, U256};

use crate::error::{RpcErrorAdapter, rpc_unsupported};

pub struct AnvilNamespace {
    node: InMemoryNode,
}

impl AnvilNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

#[async_trait]
impl AnvilNamespaceServer for AnvilNamespace {
    async fn dump_state(&self, preserve_historical_states: Option<bool>) -> RpcResult<Bytes> {
        self.node
            .dump_state(preserve_historical_states.unwrap_or(false))
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn load_state(&self, bytes: Bytes) -> RpcResult<bool> {
        self.node
            .load_state(bytes)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn mine_detailed(&self) -> RpcResult<Block<DetailedTransaction>> {
        self.node
            .mine_detailed()
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_rpc_url(&self, url: String) -> RpcResult<()> {
        self.node
            .set_rpc_url(url)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_next_block_base_fee_per_gas(&self, base_fee: U256) -> RpcResult<()> {
        self.node
            .set_next_block_base_fee_per_gas(base_fee)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn drop_transaction(&self, hash: H256) -> RpcResult<Option<H256>> {
        self.node
            .drop_transaction(hash)
            .map_err(RpcErrorAdapter::into)
    }

    async fn drop_all_transactions(&self) -> RpcResult<()> {
        self.node
            .drop_all_transactions()
            .map_err(RpcErrorAdapter::into)
    }

    async fn remove_pool_transactions(&self, address: Address) -> RpcResult<()> {
        self.node
            .remove_pool_transactions(address)
            .map_err(RpcErrorAdapter::into)
    }

    async fn get_auto_mine(&self) -> RpcResult<bool> {
        self.node
            .get_immediate_sealing()
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_auto_mine(&self, enable: bool) -> RpcResult<()> {
        self.node
            .set_immediate_sealing(enable)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_interval_mining(&self, seconds: u64) -> RpcResult<()> {
        self.node
            .set_interval_sealing(seconds)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_block_timestamp_interval(&self, seconds: u64) -> RpcResult<()> {
        self.node
            .set_block_timestamp_interval(seconds)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn remove_block_timestamp_interval(&self) -> RpcResult<bool> {
        self.node
            .remove_block_timestamp_interval()
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_min_gas_price(&self, _gas: U256) -> RpcResult<()> {
        sh_warn!("Setting minimum gas price is unsupported as ZKsync is a post-EIP1559 chain");
        rpc_unsupported("set_min_gas_price")
    }

    async fn set_logging_enabled(&self, enable: bool) -> RpcResult<()> {
        self.node
            .set_logging_enabled(enable)
            .map_err(RpcErrorAdapter::into)
    }

    async fn snapshot(&self) -> RpcResult<U64> {
        self.node.snapshot().await.map_err(RpcErrorAdapter::into)
    }

    async fn revert(&self, id: U64) -> RpcResult<bool> {
        self.node
            .revert_snapshot(id)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_time(&self, timestamp: Numeric) -> RpcResult<i128> {
        self.node
            .set_time(timestamp.into())
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn increase_time(&self, seconds: Numeric) -> RpcResult<u64> {
        self.node
            .increase_time(seconds.into())
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_next_block_timestamp(&self, timestamp: Numeric) -> RpcResult<()> {
        self.node
            .set_next_block_timestamp(timestamp.into())
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn auto_impersonate_account(&self, enabled: bool) -> RpcResult<()> {
        self.node.auto_impersonate_account(enabled);
        Ok(())
    }

    async fn set_balance(&self, address: Address, balance: U256) -> RpcResult<bool> {
        self.node
            .set_balance(address, balance)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_nonce(&self, address: Address, nonce: U256) -> RpcResult<bool> {
        self.node
            .set_nonce(address, nonce)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn anvil_mine(&self, num_blocks: Option<U64>, interval: Option<U64>) -> RpcResult<()> {
        self.node
            .mine_blocks(num_blocks, interval)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn reset_network(&self, reset_spec: Option<ResetRequest>) -> RpcResult<bool> {
        self.node
            .reset_network(reset_spec)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn impersonate_account(&self, address: Address) -> RpcResult<()> {
        self.node
            .impersonate_account(address)
            .map(|_| ())
            .map_err(RpcErrorAdapter::into)
    }

    async fn stop_impersonating_account(&self, address: Address) -> RpcResult<()> {
        self.node
            .stop_impersonating_account(address)
            .map(|_| ())
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_code(&self, address: Address, code: String) -> RpcResult<()> {
        self.node
            .set_code(address, code)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_storage_at(&self, address: Address, slot: U256, value: U256) -> RpcResult<bool> {
        self.node
            .set_storage_at(address, slot, value)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn set_chain_id(&self, id: u32) -> RpcResult<()> {
        self.node
            .set_chain_id(id)
            .await
            .map_err(RpcErrorAdapter::into)
    }
}
