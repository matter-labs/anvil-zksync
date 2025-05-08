use anvil_zksync_core::node::boojumos_get_batch_witness;
use jsonrpsee::core::{async_trait, RpcResult};

use anvil_zksync_api_decl::BoojumOSNamespaceServer;

pub struct BoojumOSNamespace {}

impl BoojumOSNamespace {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BoojumOSNamespaceServer for BoojumOSNamespace {
    async fn get_witness(&self, batch: u32) -> RpcResult<Option<String>> {
        Ok(boojumos_get_batch_witness(&batch).map(hex::encode))
    }
}
