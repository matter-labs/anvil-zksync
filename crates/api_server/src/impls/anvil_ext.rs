use crate::error::RpcError;
use anvil_zksync_api_decl::AnvilExtNamespaceServer;
use anvil_zksync_l1_sidecar::L1Sidecar;
use jsonrpsee::core::{async_trait, RpcResult};
use zksync_types::{L1BatchNumber, H256};

pub struct AnvilExtNamespace {
    l1_sidecar: L1Sidecar,
}

impl AnvilExtNamespace {
    pub fn new(l1_sidecar: L1Sidecar) -> Self {
        Self { l1_sidecar }
    }
}

#[async_trait]
impl AnvilExtNamespaceServer for AnvilExtNamespace {
    async fn commit_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256> {
        Ok(self
            .l1_sidecar
            .commit_batch(batch_number)
            .await
            .map_err(RpcError::from)?)
    }
}
