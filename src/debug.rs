use crate::{
    fork::ForkSource,
    node::{InMemoryNodeInner, MAX_TX_SIZE},
};
use jsonrpc_core::{BoxFuture, Result};
use once_cell::sync::OnceCell;
use std::sync::{Arc, RwLock};
use vm::{CallTracer, ExecutionResult, HistoryDisabled, TxExecutionMode, Vm};
use zksync_basic_types::H256;
use zksync_core::api_server::web3::backend_jsonrpc::{
    error::into_jsrpc_error, namespaces::debug::DebugNamespaceT,
};
use zksync_state::StorageView;
use zksync_types::{
    api::{BlockId, BlockNumber, DebugCall, DebugCallType, ResultDebugCall, TracerConfig},
    l2::L2Tx,
    transaction_request::CallRequest,
    PackedEthSignature, Transaction,
};
use zksync_web3_decl::error::Web3Error;

/// Implementation of DebugNamespaceImpl
pub struct DebugNamespaceImpl<S> {
    node: Arc<RwLock<InMemoryNodeInner<S>>>,
}

impl<S> DebugNamespaceImpl<S> {
    /// Creates a new `Debug` instance with the given `node`.
    pub fn new(node: Arc<RwLock<InMemoryNodeInner<S>>>) -> Self {
        Self { node }
    }
}

impl<S: Send + Sync + 'static + ForkSource + std::fmt::Debug> DebugNamespaceT
    for DebugNamespaceImpl<S>
{
    fn trace_block_by_number(
        &self,
        _block: BlockNumber,
        _options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Vec<ResultDebugCall>>> {
        todo!()
    }

    fn trace_block_by_hash(
        &self,
        _hash: H256,
        _options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Vec<ResultDebugCall>>> {
        todo!()
    }

    fn trace_call(
        &self,
        request: CallRequest,
        block: Option<BlockId>,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<DebugCall>> {
        let only_top = options.is_some_and(|o| o.tracer_config.only_top_call);
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            if !matches!(block, Some(BlockId::Number(BlockNumber::Latest))) {
                return Err(jsonrpc_core::Error::invalid_params(
                    "tracing only supported at `latest` block",
                ));
            }

            let inner = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;

            let mut l2_tx = match L2Tx::from_request(request.into(), MAX_TX_SIZE) {
                Ok(tx) => tx,
                Err(e) => {
                    let error = Web3Error::SerializationError(e);
                    return Err(into_jsrpc_error(error));
                }
            };

            let execution_mode = TxExecutionMode::EthCall;
            let storage = StorageView::new(&inner.fork_storage).to_rc_ptr();

            let bootloader_code = inner.system_contracts.contacts_for_l2_call();

            // init vm
            let batch_env = inner.create_l1_batch_env(storage.clone());
            let system_env = inner.create_system_env(bootloader_code.clone(), execution_mode);
            let mut vm = Vm::new(batch_env, system_env, storage, HistoryDisabled);

            // We must inject *some* signature (otherwise bootloader code fails to generate hash).
            if l2_tx.common_data.signature.is_empty() {
                l2_tx.common_data.signature =
                    PackedEthSignature::default().serialize_packed().into();
            }

            let tx: Transaction = l2_tx.clone().into();
            vm.push_transaction(tx);

            let call_tracer_result = Arc::new(OnceCell::default());
            let tracer = CallTracer::new(call_tracer_result.clone(), HistoryDisabled);
            let tx_result = vm.inspect(vec![Box::new(tracer)], vm::VmExecutionMode::OneTx);

            let call_traces = if only_top {
                vec![]
            } else {
                Arc::try_unwrap(call_tracer_result)
                    .unwrap()
                    .take()
                    .unwrap_or_default()
            };

            let result = match &tx_result.result {
                ExecutionResult::Success { output } => DebugCall {
                    gas_used: tx_result.statistics.gas_used.into(),
                    output: output.clone().into(),
                    r#type: DebugCallType::Call,
                    from: l2_tx.initiator_account(),
                    to: l2_tx.recipient_account(),
                    gas: l2_tx.common_data.fee.gas_limit,
                    value: l2_tx.execute.value.clone(),
                    input: l2_tx.execute.calldata().into(),
                    error: None,
                    revert_reason: None,
                    calls: call_traces.into_iter().map(Into::into).collect(),
                },
                ExecutionResult::Revert { output } => DebugCall {
                    gas_used: tx_result.statistics.gas_used.into(),
                    output: output.encoded_data().into(),
                    r#type: DebugCallType::Call,
                    from: l2_tx.initiator_account(),
                    to: l2_tx.recipient_account(),
                    gas: l2_tx.common_data.fee.gas_limit,
                    value: l2_tx.execute.value.clone(),
                    input: l2_tx.execute.calldata().into(),
                    error: Some(output.to_string()),
                    revert_reason: Some(output.to_string()),
                    calls: call_traces.into_iter().map(Into::into).collect(),
                },
                ExecutionResult::Halt { reason } => DebugCall {
                    gas_used: tx_result.statistics.gas_used.into(),
                    output: vec![].into(),
                    r#type: DebugCallType::Call,
                    from: l2_tx.initiator_account(),
                    to: l2_tx.recipient_account(),
                    gas: l2_tx.common_data.fee.gas_limit,
                    value: l2_tx.execute.value.clone(),
                    input: l2_tx.execute.calldata().into(),
                    error: Some(reason.to_string()),
                    revert_reason: Some(reason.to_string()),
                    calls: call_traces.into_iter().map(Into::into).collect(),
                },
            };

            Ok(result)
        })
    }

    fn trace_transaction(
        &self,
        tx_hash: H256,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Option<DebugCall>>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use zksync_types::transaction_request::CallRequestBuilder;

    use crate::{
        cache::CacheConfig,
        fork::ForkDetails,
        http_fork_source::HttpForkSource,
        node::{InMemoryNode, ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails},
    };

    use super::*;

    #[tokio::test]
    async fn test_trace_call_simple() {
        let fork = ForkDetails::from_network("mainnet", None, CacheConfig::Memory).await;
        let node: InMemoryNode<HttpForkSource> = InMemoryNode::<HttpForkSource>::new(
            Some(fork),
            ShowCalls::None,
            ShowStorageLogs::None,
            ShowVMDetails::None,
            ShowGasDetails::None,
            false,
            &crate::system_contracts::Options::BuiltIn,
        );
        let debug = DebugNamespaceImpl::new(node.get_inner());

        let request = CallRequestBuilder::default()
            .to("0x36615Cf349d7F6344891B1e7CA7C72883F5dc049"
                .parse()
                .unwrap())
            .gas(80_000_000.into())
            .build();
        let trace = debug
            .trace_call(request.clone(), None, None)
            .await
            .expect("trace call");

        assert!(trace.error.is_none());
        assert!(trace.revert_reason.is_none());

        // passing block number is not supported
        let resp = debug
            .trace_call(request, Some(BlockId::Number(1.into())), None)
            .await;
        assert!(resp.is_err());
    }

    #[tokio::test]
    async fn test_trace_call_weth() {
        let fork = ForkDetails::from_network("mainnet", None, CacheConfig::Memory).await;
        let node: InMemoryNode<HttpForkSource> = InMemoryNode::<HttpForkSource>::new(
            Some(fork),
            ShowCalls::None,
            ShowStorageLogs::None,
            ShowVMDetails::None,
            ShowGasDetails::None,
            false,
            &crate::system_contracts::Options::BuiltIn,
        );

        let debug = DebugNamespaceImpl::new(node.get_inner());
        let request = CallRequestBuilder::default()
            .to("0x5AEa5775959fBC2557Cc8789bC1bf90A239D9a91"
                .parse()
                .unwrap())
            // $ cast cd 'name()'
            .data(hex::decode("06fdde03").unwrap().into())
            .gas(80_000_000.into())
            .build();
        let trace = debug
            .trace_call(request, None, None)
            .await
            .expect("trace call");

        // output is encoded, so we can't compare it directly
        assert_eq!(&trace.output.0[64..77], "Wrapped Ether".as_bytes());

        assert!(trace.error.is_none());
        assert!(trace.revert_reason.is_none());
    }
}
