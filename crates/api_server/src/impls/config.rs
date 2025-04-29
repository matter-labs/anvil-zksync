use anvil_zksync_api_decl::ConfigNamespaceServer;
use crate::node::InMemoryNode;
use anvil_zksync_types::{LogLevel, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use jsonrpsee::core::{async_trait, RpcResult};

use crate::error::JsonRPCAdapter;

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
    async fn get_current_timestamp(&self) -> RpcResult<u64> {
        Ok(self
            .node
            .get_current_timestamp()
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn set_show_storage_logs(&self, value: ShowStorageLogs) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_storage_logs(value)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn set_show_vm_details(&self, value: ShowVMDetails) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_vm_details(value)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn set_show_gas_details(&self, value: ShowGasDetails) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_gas_details(value)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn set_show_node_config(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_show_node_config(value)
            .await
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn set_log_level(&self, level: LogLevel) -> RpcResult<bool> {
        Ok(self
            .node
            .set_log_level(level)
            .map_err(JsonRPCAdapter::from)?)
    }

    async fn set_logging(&self, directive: String) -> RpcResult<bool> {
        Ok(self
            .node
            .set_logging(directive)
            .map_err(JsonRPCAdapter::from)?)
    }
}
