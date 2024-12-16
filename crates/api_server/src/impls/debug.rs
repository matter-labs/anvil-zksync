use crate::error::RpcError;
use anvil_zksync_api_decl::DebugNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use jsonrpsee::core::{async_trait, RpcResult};
use zksync_types::api::{
    BlockId, BlockNumber, CallTracerBlockResult, CallTracerResult, TracerConfig,
};
use zksync_types::transaction_request::CallRequest;
use zksync_types::H256;

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
            .trace_block_by_number(block, options)
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
            .trace_block_by_hash(hash, options)
            .await
            .map_err(RpcError::from)?)
    }

    async fn trace_call(
        &self,
        request: CallRequest,
        block: Option<BlockId>,
        options: Option<TracerConfig>,
    ) -> RpcResult<CallTracerResult> {
        Ok(self
            .node
            .trace_call(request, block, options)
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
            .trace_transaction(tx_hash, options)
            .await
            .map_err(RpcError::from)?)
    }
}
