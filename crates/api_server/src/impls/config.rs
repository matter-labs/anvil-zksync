use crate::error::RpcError;
use anvil_zksync_api_decl::ConfigNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use anvil_zksync_types::{LogLevel, ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use jsonrpsee::core::RpcResult;

pub struct ConfigNamespace {
    node: InMemoryNode,
}

impl ConfigNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

impl ConfigNamespaceServer for ConfigNamespace {
    fn get_show_calls(&self) -> RpcResult<String> {
        Ok(self.node.get_show_calls().map_err(RpcError::from)?)
    }

    fn get_show_outputs(&self) -> RpcResult<bool> {
        Ok(self.node.get_show_outputs().map_err(RpcError::from)?)
    }

    fn get_current_timestamp(&self) -> RpcResult<u64> {
        Ok(self.node.get_current_timestamp().map_err(RpcError::from)?)
    }

    fn set_show_calls(&self, value: ShowCalls) -> RpcResult<String> {
        Ok(self.node.set_show_calls(value).map_err(RpcError::from)?)
    }

    fn set_show_outputs(&self, value: bool) -> RpcResult<bool> {
        Ok(self.node.set_show_outputs(value).map_err(RpcError::from)?)
    }

    fn set_show_storage_logs(&self, value: ShowStorageLogs) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_storage_logs(value)
            .map_err(RpcError::from)?)
    }

    fn set_show_vm_details(&self, value: ShowVMDetails) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_vm_details(value)
            .map_err(RpcError::from)?)
    }

    fn set_show_gas_details(&self, value: ShowGasDetails) -> RpcResult<String> {
        Ok(self
            .node
            .set_show_gas_details(value)
            .map_err(RpcError::from)?)
    }

    fn set_resolve_hashes(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_resolve_hashes(value)
            .map_err(RpcError::from)?)
    }

    fn set_show_node_config(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_show_node_config(value)
            .map_err(RpcError::from)?)
    }

    fn set_show_tx_summary(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_show_tx_summary(value)
            .map_err(RpcError::from)?)
    }

    fn set_show_event_logs(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_show_event_logs(value)
            .map_err(RpcError::from)?)
    }

    fn set_disable_console_log(&self, value: bool) -> RpcResult<bool> {
        Ok(self
            .node
            .set_disable_console_log(value)
            .map_err(RpcError::from)?)
    }

    fn set_log_level(&self, level: LogLevel) -> RpcResult<bool> {
        Ok(self.node.set_log_level(level).map_err(RpcError::from)?)
    }

    fn set_logging(&self, directive: String) -> RpcResult<bool> {
        Ok(self.node.set_logging(directive).map_err(RpcError::from)?)
    }
}
