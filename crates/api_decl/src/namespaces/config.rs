use anvil_zksync_types::{LogLevel, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

#[rpc(server, namespace = "config")]
pub trait ConfigNamespace {
    /// Get the InMemoryNodeInner's current_timestamp property
    ///
    /// # Returns
    /// The current `current_timestamp` value for the InMemoryNodeInner.
    #[method(name = "getCurrentTimestamp")]
    async fn get_current_timestamp(&self) -> RpcResult<u64>;

    /// Set show_storage_logs for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A ShowStorageLogs enum to update show_storage_logs to
    ///
    /// # Returns
    /// The updated/current `show_storage_logs` value for the InMemoryNodeInner.
    #[method(name = "setShowStorageLogs")]
    async fn set_show_storage_logs(&self, value: ShowStorageLogs) -> RpcResult<String>;

    /// Set show_vm_details for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A ShowVMDetails enum to update show_vm_details to
    ///
    /// # Returns
    /// The updated/current `show_vm_details` value for the InMemoryNodeInner.
    #[method(name = "setShowVmDetails")]
    async fn set_show_vm_details(&self, value: ShowVMDetails) -> RpcResult<String>;

    /// Set show_gas_details for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A ShowGasDetails enum to update show_gas_details to
    ///
    /// # Returns
    /// The updated/current `show_gas_details` value for the InMemoryNodeInner.
    #[method(name = "setShowGasDetails")]
    async fn set_show_gas_details(&self, value: ShowGasDetails) -> RpcResult<String>;

    /// Set show_node_config for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A bool to update show_node_config to
    ///
    /// # Returns
    /// The updated/current `show_node_config` value for the InMemoryNodeInner.
    #[method(name = "setShowNodeConfig")]
    async fn set_show_node_config(&self, value: bool) -> RpcResult<bool>;

    /// Set the logging for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `level`: The log level to set. One of: ["trace", "debug", "info", "warn", "error"]
    ///
    /// # Returns
    /// `true` if the operation succeeded, `false` otherwise.
    #[method(name = "setLogLevel")]
    async fn set_log_level(&self, level: LogLevel) -> RpcResult<bool>;

    /// Set the logging for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `level`: The logging directive to set. Example:
    ///     * "my_crate=debug"
    ///     * "my_crate::module=trace"
    ///     * "my_crate=debug,other_crate=warn"
    ///
    /// # Returns
    /// `true` if the operation succeeded, `false` otherwise.
    #[method(name = "setLogging")]
    async fn set_logging(&self, directive: String) -> RpcResult<bool>;
}
