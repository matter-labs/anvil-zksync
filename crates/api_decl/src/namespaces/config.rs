use anvil_zksync_types::{LogLevel, ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

#[rpc(server, namespace = "config")]
pub trait ConfigNamespace {
    /// Get the InMemoryNodeInner's show_calls property as a string
    ///
    /// # Returns
    /// The current `show_calls` value for the InMemoryNodeInner.
    #[method(name = "getShowCalls")]
    fn get_show_calls(&self) -> RpcResult<String>;

    /// Get the InMemoryNodeInner's show_outputs property as a boolean
    ///
    /// # Returns
    /// The current `show_outputs` value for the InMemoryNodeInner.
    #[method(name = "getShowOutputs")]
    fn get_show_outputs(&self) -> RpcResult<bool>;

    /// Get the InMemoryNodeInner's current_timestamp property
    ///
    /// # Returns
    /// The current `current_timestamp` value for the InMemoryNodeInner.
    #[method(name = "getCurrentTimestamp")]
    fn get_current_timestamp(&self) -> RpcResult<u64>;

    /// Set show_calls for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A ShowCalls enum to update show_calls to
    ///
    /// # Returns
    /// The updated/current `show_calls` value for the InMemoryNodeInner.
    #[method(name = "setShowCalls")]
    fn set_show_calls(&self, value: ShowCalls) -> RpcResult<String>;

    /// Set show_outputs for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: a bool value to update show_outputs to
    ///
    /// # Returns
    /// The updated/current `show_outputs` value for the InMemoryNodeInner.
    #[method(name = "setShowOutputs")]
    fn set_show_outputs(&self, value: bool) -> RpcResult<bool>;

    /// Set show_storage_logs for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A ShowStorageLogs enum to update show_storage_logs to
    ///
    /// # Returns
    /// The updated/current `show_storage_logs` value for the InMemoryNodeInner.
    #[method(name = "setShowStorageLogs")]
    fn set_show_storage_logs(&self, value: ShowStorageLogs) -> RpcResult<String>;

    /// Set show_vm_details for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A ShowVMDetails enum to update show_vm_details to
    ///
    /// # Returns
    /// The updated/current `show_vm_details` value for the InMemoryNodeInner.
    #[method(name = "setShowVmDetails")]
    fn set_show_vm_details(&self, value: ShowVMDetails) -> RpcResult<String>;

    /// Set show_gas_details for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A ShowGasDetails enum to update show_gas_details to
    ///
    /// # Returns
    /// The updated/current `show_gas_details` value for the InMemoryNodeInner.
    #[method(name = "setShowGasDetails")]
    fn set_show_gas_details(&self, value: ShowGasDetails) -> RpcResult<String>;

    /// Set resolve_hashes for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A bool to update resolve_hashes to
    ///
    /// # Returns
    /// The updated `resolve_hashes` value for the InMemoryNodeInner.
    #[method(name = "setResolveHashes")]
    fn set_resolve_hashes(&self, value: bool) -> RpcResult<bool>;

    /// Set show_node_config for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A bool to update show_node_config to
    ///
    /// # Returns
    /// The updated/current `show_node_config` value for the InMemoryNodeInner.
    #[method(name = "setShowNodeConfig")]
    fn set_show_node_config(&self, value: bool) -> RpcResult<bool>;

    /// Set show_tx_summary for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A bool to update show_tx_summary to
    ///
    /// # Returns
    /// The updated/current `show_tx_summary` value for the InMemoryNodeInner.
    #[method(name = "setShowTxSummary")]
    fn set_show_tx_summary(&self, value: bool) -> RpcResult<bool>;

    /// Set show_event_logs for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A bool to update show_event_logs to
    ///
    /// # Returns
    /// The updated/current `show_event_logs` value for the InMemoryNodeInner.
    #[method(name = "setShowEventLogs")]
    fn set_show_event_logs(&self, value: bool) -> RpcResult<bool>;

    /// Set disable_console_log for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `value`: A bool to update disable_console_log to
    ///
    /// # Returns
    /// The updated/current `disable_console_log` value for the InMemoryNodeInner.
    #[method(name = "setDisableConsoleLog")]
    fn set_disable_console_log(&self, value: bool) -> RpcResult<bool>;

    /// Set the logging for the InMemoryNodeInner
    ///
    /// # Parameters
    /// - `level`: The log level to set. One of: ["trace", "debug", "info", "warn", "error"]
    ///
    /// # Returns
    /// `true` if the operation succeeded, `false` otherwise.
    #[method(name = "setLogLevel")]
    fn set_log_level(&self, level: LogLevel) -> RpcResult<bool>;

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
    fn set_logging(&self, directive: String) -> RpcResult<bool>;
}
