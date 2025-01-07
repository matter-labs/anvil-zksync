use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

/// API bindings for the `evm` namespace. Note that most of the methods are covered by aliases in
/// [`AnvilNamespace`]. This namespace exclusively contains other methods.
#[rpc(server, namespace = "evm")]
pub trait EvmNamespace {
    /// Force a single block to be mined.
    ///
    /// # Returns
    /// The string "0x0".
    #[method(name = "mine")]
    async fn mine(&self) -> RpcResult<String>;
}
