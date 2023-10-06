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
    PackedEthSignature, Transaction, CONTRACT_DEPLOYER_ADDRESS,
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

    /// Trace execution of a transaction.
    fn trace_call(
        &self,
        request: CallRequest,
        block: Option<BlockId>,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<DebugCall>> {
        let only_top = options.is_some_and(|o| o.tracer_config.only_top_call);
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            if block.is_some() && !matches!(block, Some(BlockId::Number(BlockNumber::Latest))) {
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

            let bootloader_code = inner.system_contracts.contracts_for_l2_call();

            // init vm
            let (l1_batch_env, _block_context) = inner.create_l1_batch_env(storage.clone());
            let system_env = inner.create_system_env(bootloader_code.clone(), execution_mode);
            let mut vm = Vm::new(l1_batch_env, system_env, storage, HistoryDisabled);

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

            let calltype = if l2_tx.recipient_account() == CONTRACT_DEPLOYER_ADDRESS {
                DebugCallType::Create
            } else {
                DebugCallType::Call
            };

            let result = match &tx_result.result {
                ExecutionResult::Success { output } => DebugCall {
                    gas_used: tx_result.statistics.gas_used.into(),
                    output: output.clone().into(),
                    r#type: calltype,
                    from: l2_tx.initiator_account(),
                    to: l2_tx.recipient_account(),
                    gas: l2_tx.common_data.fee.gas_limit,
                    value: l2_tx.execute.value,
                    input: l2_tx.execute.calldata().into(),
                    error: None,
                    revert_reason: None,
                    calls: call_traces.into_iter().map(Into::into).collect(),
                },
                ExecutionResult::Revert { output } => DebugCall {
                    gas_used: tx_result.statistics.gas_used.into(),
                    output: output.encoded_data().into(),
                    r#type: calltype,
                    from: l2_tx.initiator_account(),
                    to: l2_tx.recipient_account(),
                    gas: l2_tx.common_data.fee.gas_limit,
                    value: l2_tx.execute.value,
                    input: l2_tx.execute.calldata().into(),
                    error: Some(output.to_string()),
                    revert_reason: Some(output.to_string()),
                    calls: call_traces.into_iter().map(Into::into).collect(),
                },
                ExecutionResult::Halt { reason } => DebugCall {
                    gas_used: tx_result.statistics.gas_used.into(),
                    output: vec![].into(),
                    r#type: calltype,
                    from: l2_tx.initiator_account(),
                    to: l2_tx.recipient_account(),
                    gas: l2_tx.common_data.fee.gas_limit,
                    value: l2_tx.execute.value,
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
        _tx_hash: H256,
        _options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Option<DebugCall>>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        deps::system_contracts::bytecode_from_slice, http_fork_source::HttpForkSource,
        node::InMemoryNode, testing,
    };
    use ethers::abi::{short_signature, AbiEncode, HumanReadableParser, ParamType, Token};
    use zksync_basic_types::{Address, Nonce, U256};
    use zksync_types::{
        api::{CallTracerConfig, SupportedTracers},
        transaction_request::CallRequestBuilder,
        utils::deployed_address_create,
    };

    fn deploy_test_contracts(node: &InMemoryNode<HttpForkSource>) -> (Address, Address) {
        let private_key = H256::repeat_byte(0xee);
        let from_account = zksync_types::PackedEthSignature::address_from_private_key(&private_key)
            .expect("failed generating address");
        node.set_rich_account(from_account);

        // first, deploy secondary contract
        let secondary_bytecode = bytecode_from_slice(
            "Secondary",
            include_bytes!("deps/test-contracts/Secondary.json"),
        );
        let secondary_deployed_address = deployed_address_create(from_account, U256::zero());
        testing::deploy_contract(
            &node,
            H256::repeat_byte(0x1),
            private_key,
            secondary_bytecode,
            Some((U256::from(2),).encode()),
            Nonce(0),
        );

        // deploy primary contract using the secondary contract address as a constructor parameter
        let primary_bytecode = bytecode_from_slice(
            "Primary",
            include_bytes!("deps/test-contracts/Primary.json"),
        );
        let primary_deployed_address = deployed_address_create(from_account, U256::one());
        testing::deploy_contract(
            &node,
            H256::repeat_byte(0x1),
            private_key,
            primary_bytecode,
            Some((secondary_deployed_address).encode()),
            Nonce(1),
        );
        (primary_deployed_address, secondary_deployed_address)
    }

    #[tokio::test]
    async fn test_trace_deployed_contract() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let debug = DebugNamespaceImpl::new(node.get_inner());

        let (primary_deployed_address, secondary_deployed_address) = deploy_test_contracts(&node);

        // trace a call to the primary contract
        let func = HumanReadableParser::parse_function("calculate(uint)").unwrap();
        let calldata = func.encode_input(&[Token::Uint(U256::from(42))]).unwrap();
        let request = CallRequestBuilder::default()
            .to(primary_deployed_address)
            .data(calldata.clone().into())
            .gas(80_000_000.into())
            .build();
        let trace = debug
            .trace_call(request.clone(), None, None)
            .await
            .expect("trace call");

        // call should not revert
        assert!(trace.error.is_none());
        assert!(trace.revert_reason.is_none());

        // check that the call was successful
        let output =
            ethers::abi::decode(&[ParamType::Uint(256)], &trace.output.0.as_slice()).unwrap();
        assert_eq!(output[0], Token::Uint(U256::from(84)));

        // find the call to primary contract in the trace
        let contract_call = trace
            .calls
            .first()
            .unwrap()
            .calls
            .last()
            .unwrap()
            .calls
            .first()
            .unwrap();

        assert_eq!(contract_call.to, primary_deployed_address);
        assert_eq!(contract_call.input, calldata.into());

        // check that it contains a call to secondary contract
        let subcall = contract_call.calls.first().unwrap();
        assert_eq!(subcall.to, secondary_deployed_address);
        assert_eq!(subcall.from, primary_deployed_address);
        assert_eq!(subcall.output, U256::from(84).encode().into());
    }

    #[tokio::test]
    async fn test_trace_only_top() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let debug = DebugNamespaceImpl::new(node.get_inner());

        let (primary_deployed_address, _) = deploy_test_contracts(&node);

        // trace a call to the primary contract
        let func = HumanReadableParser::parse_function("calculate(uint)").unwrap();
        let calldata = func.encode_input(&[Token::Uint(U256::from(42))]).unwrap();
        let request = CallRequestBuilder::default()
            .to(primary_deployed_address)
            .data(calldata.into())
            .gas(80_000_000.into())
            .build();

        // if we trace with onlyTopCall=true, we should get only the top-level call
        let trace = debug
            .trace_call(
                request,
                None,
                Some(TracerConfig {
                    tracer: SupportedTracers::CallTracer,
                    tracer_config: CallTracerConfig {
                        only_top_call: true,
                    },
                }),
            )
            .await
            .expect("trace call");
        // call should not revert
        assert!(trace.error.is_none());
        assert!(trace.revert_reason.is_none());

        // call should not contain any subcalls
        assert!(trace.calls.is_empty());
    }

    #[tokio::test]
    async fn test_trace_reverts() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let debug = DebugNamespaceImpl::new(node.get_inner());

        let (primary_deployed_address, _) = deploy_test_contracts(&node);

        // trace a call to the primary contract
        let request = CallRequestBuilder::default()
            .to(primary_deployed_address)
            .data(short_signature("shouldRevert()", &[]).into())
            .gas(80_000_000.into())
            .build();
        let trace = debug
            .trace_call(request, None, None)
            .await
            .expect("trace call");

        // call should revert
        assert!(trace.revert_reason.is_some());

        // find the call to primary contract in the trace
        let contract_call = trace
            .calls
            .first()
            .unwrap()
            .calls
            .last()
            .unwrap()
            .calls
            .first()
            .unwrap();

        // the contract subcall should have reverted
        assert!(contract_call.revert_reason.is_some());
    }
}
