use anvil_zksync_api_decl::AnvilZksNamespaceServer;
use anvil_zksync_core::node::zksync_os_get_batch_witness;
use anvil_zksync_l1_sidecar::L1Sidecar;
use jsonrpsee::core::{RpcResult, async_trait};
use zksync_types::web3::Bytes;
use zksync_types::{H256, L1BatchNumber};

use crate::error::{RpcErrorAdapter, rpc_invalid_params};

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
        self.l1_sidecar
            .commit_batch(batch_number)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn prove_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256> {
        self.l1_sidecar
            .prove_batch(batch_number)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn execute_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256> {
        self.l1_sidecar
            .execute_batch(batch_number)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn get_witness(&self, batch_number: L1BatchNumber) -> RpcResult<Bytes> {
        Ok(zksync_os_get_batch_witness(&batch_number)
            .ok_or(rpc_invalid_params(
                "Batch with this number doesn't exist yet".to_string(),
            ))?
            .into())
    }
}
