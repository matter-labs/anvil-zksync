use anvil_zksync_core::node::zkos_get_batch_witness;
use jsonrpsee::core::{async_trait, RpcResult};

use anvil_zksync_api_decl::ZKOSNamespaceServer;

pub struct ZKOSNamespace {}

impl ZKOSNamespace {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ZKOSNamespaceServer for ZKOSNamespace {
    async fn get_witness(&self, batch: u32) -> RpcResult<Option<String>> {
        Ok(zkos_get_batch_witness(&batch).map(hex::encode))
    }
}
