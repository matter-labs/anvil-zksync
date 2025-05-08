use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

/// API bindings for the `zkos` experimental namespace.
#[rpc(server, namespace = "zkos")]
pub trait ZKOSNamespace {
    /// Returns the witness for a given batch.
    ///
    /// # Returns
    /// Bytes with the witness that can be passed to proving system.
    #[method(name = "getWitness")]
    async fn get_witness(&self, batch: u32) -> RpcResult<Option<String>>;
}
