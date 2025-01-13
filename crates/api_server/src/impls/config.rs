use crate::error::RpcError;
use anvil_zksync_api_decl::ConfigNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use anvil_zksync_types::{LogLevel, ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use jsonrpsee::core::{async_trait, RpcResult};

pub struct ConfigNamespace {
    node: InMemoryNode,
}

impl ConfigNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

#[async_trait]
impl ConfigNamespaceServer for ConfigNamespace {
    async fn get_show_calls(&self) -> RpcResult<String> {
        Ok(self.node.get_show_calls().await.map_err(RpcError::from)?)
    }

    async fn get_show_outputs(&self) -> RpcResult<bool> {
        Ok(self.node.get_show_outputs().await.map_err(RpcError::from)?)
    }

    async fn get_current_timestamp(&self) -> RpcResult<u64> {
        Ok(self.node.get_current_timestamp().map_err(RpcError::from)?)
    }

    async fn set_show_calls(&self, value: ShowCalls) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_calls(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_show_outputs(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_show_outputs(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_show_storage_logs(&self, value: ShowStorageLogs) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_storage_logs(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_show_vm_details(&self, value: ShowVMDetails) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_vm_details(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_show_gas_details(&self, value: ShowGasDetails) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_gas_details(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_resolve_hashes(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_resolve_hashes(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_show_node_config(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_show_node_config(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_show_tx_summary(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_show_tx_summary(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_show_event_logs(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_show_event_logs(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_disable_console_log(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_disable_console_log(value)
            .await
            .map_err(RpcError::from)?)
    }

    async fn set_log_level(&self, level: LogLevel) -> RpcResult<bool> {
        Ok(self.node.set_log_level(level).map_err(RpcError::from)?)
    }

    async fn set_logging(&self, directive: String) -> RpcResult<bool> {
        Ok(self.node.set_logging(directive).map_err(RpcError::from)?)
    }
}
