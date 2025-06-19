use crate::error::RpcError;
use anvil_zksync_api_decl::DebugNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use jsonrpsee::core::{async_trait, RpcResult};
use zksync_types::api::{BlockNumber, CallTracerBlockResult, CallTracerResult, TracerConfig};
use zksync_types::transaction_request::CallRequest;
use zksync_types::{api, api::BlockId, web3::Bytes, H256};

pub struct DebugNamespace {
    node: InMemoryNode,
}

impl DebugNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

#[async_trait]
impl DebugNamespaceServer for DebugNamespace {
    async fn trace_block_by_number(
        &self,
        block: BlockNumber,
        options: Option<TracerConfig>,
    ) -> RpcResult<CallTracerBlockResult> {
        Ok(self
            .node
            .trace_block_impl(api::BlockId::Number(block), options)
            .await
            .map_err(RpcError::from)?)
    }

    async fn trace_block_by_hash(
        &self,
        hash: H256,
        options: Option<TracerConfig>,
    ) -> RpcResult<CallTracerBlockResult> {
        Ok(self
            .node
            .trace_block_impl(api::BlockId::Hash(hash), options)
            .await
            .map_err(RpcError::from)?)
    }

    async fn trace_call(
        &self,
        request: CallRequest,
        block: Option<api::BlockId>,
        options: Option<TracerConfig>,
    ) -> RpcResult<CallTracerResult> {
        Ok(self
            .node
            .trace_call_impl(request, block, options)
            .await
            .map_err(RpcError::from)?)
    }

    async fn trace_transaction(
        &self,
        tx_hash: H256,
        options: Option<TracerConfig>,
    ) -> RpcResult<Option<CallTracerResult>> {
        Ok(self
            .node
            .trace_transaction_impl(tx_hash, options)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_raw_transaction(&self, tx_hash: H256) -> RpcResult<Option<Bytes>> {
        Ok(self
            .node
            .get_raw_transaction_impl(tx_hash)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_raw_transactions(&self, block: BlockId) -> RpcResult<Vec<Bytes>> {
        Ok(self
            .node
            .get_raw_transactions_impl(block)
            .await
            .map_err(RpcError::from)?)
    }
}
