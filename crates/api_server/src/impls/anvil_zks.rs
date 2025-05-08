use crate::error::RpcError;
use anvil_zksync_api_decl::AnvilZksNamespaceServer;
use anvil_zksync_core::node::boojumos_get_batch_witness;
use anvil_zksync_l1_sidecar::L1Sidecar;
use jsonrpsee::core::{async_trait, RpcResult};
use zksync_types::{L1BatchNumber, H256};

pub struct AnvilZksNamespace {
    l1_sidecar: L1Sidecar,
}

impl AnvilZksNamespace {
    pub fn new(l1_sidecar: L1Sidecar) -> Self {
        Self { l1_sidecar }
    }
}

#[async_trait]
impl AnvilZksNamespaceServer for AnvilZksNamespace {
    async fn commit_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256> {
        Ok(self
            .l1_sidecar
            .commit_batch(batch_number)
            .await
            .map_err(RpcError::from)?)
    }

    async fn prove_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256> {
        Ok(self
            .l1_sidecar
            .prove_batch(batch_number)
            .await
            .map_err(RpcError::from)?)
    }

    async fn execute_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256> {
        Ok(self
            .l1_sidecar
            .execute_batch(batch_number)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_boojum_witness(&self, batch: u32) -> RpcResult<Option<String>> {
        Ok(boojumos_get_batch_witness(&batch).map(hex::encode))
    }
}
