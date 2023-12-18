use crate::{
    fork::ForkSource,
    node::{InMemoryNodeInner, MAX_TX_SIZE},
    utils::{create_debug_output, storage_view::StorageView, to_real_block_number},
};
use itertools::Itertools;
use jsonrpc_core::{BoxFuture, Result};
use multivm::interface::VmInterface;
use multivm::vm_latest::{constants::ETH_CALL_GAS_LIMIT, CallTracer, Vm};
use once_cell::sync::OnceCell;
use std::sync::{Arc, RwLock};
use zksync_basic_types::H256;
use zksync_core::api_server::web3::backend_jsonrpc::{
    error::into_jsrpc_error, namespaces::debug::DebugNamespaceT,
};
use zksync_types::{
    api::{BlockId, BlockNumber, DebugCall, ResultDebugCall, TracerConfig, TransactionVariant},
    l2::L2Tx,
    transaction_request::CallRequest,
    PackedEthSignature, Transaction, U64,
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
        block: BlockNumber,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Vec<ResultDebugCall>>> {
        let only_top = options.is_some_and(|o| o.tracer_config.only_top_call);
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            let inner = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;

            let block = {
                let number =
                    to_real_block_number(block, U64::from(inner.current_miniblock)).as_u64();

                inner
                    .block_hashes
                    .get(&number)
                    .and_then(|hash| inner.blocks.get(hash))
                    .ok_or_else(|| {
                        into_jsrpc_error(Web3Error::SubmitTransactionError(
                            "Block not found".to_string(),
                            vec![],
                        ))
                    })?
            };

            let tx_hashes = block
                .transactions
                .iter()
                .map(|tx| match tx {
                    TransactionVariant::Full(tx) => tx.hash,
                    TransactionVariant::Hash(hash) => *hash,
                })
                .collect_vec();

            let debug_calls = tx_hashes
                .into_iter()
                .map(|tx_hash| {
                    let tx = inner.tx_results.get(&tx_hash).ok_or_else(|| {
                        into_jsrpc_error(Web3Error::SubmitTransactionError(
                            "Transaction not found".to_string(),
                            vec![],
                        ))
                    })?;
                    Ok(tx.debug_info(only_top))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .map(|result| ResultDebugCall { result })
                .collect_vec();

            Ok(debug_calls)
        })
    }

    fn trace_block_by_hash(
        &self,
        hash: H256,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Vec<ResultDebugCall>>> {
        let only_top = options.is_some_and(|o| o.tracer_config.only_top_call);
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            let inner = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;

            let block = inner.blocks.get(&hash).ok_or_else(|| {
                into_jsrpc_error(Web3Error::SubmitTransactionError(
                    "Block not found".to_string(),
                    vec![],
                ))
            })?;

            let tx_hashes = block
                .transactions
                .iter()
                .map(|tx| match tx {
                    TransactionVariant::Full(tx) => tx.hash,
                    TransactionVariant::Hash(hash) => *hash,
                })
                .collect_vec();

            let debug_calls = tx_hashes
                .into_iter()
                .map(|tx_hash| {
                    let tx = inner.tx_results.get(&tx_hash).ok_or_else(|| {
                        into_jsrpc_error(Web3Error::SubmitTransactionError(
                            "Transaction not found".to_string(),
                            vec![],
                        ))
                    })?;
                    Ok(tx.debug_info(only_top))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .map(|result| ResultDebugCall { result })
                .collect_vec();

            Ok(debug_calls)
        })
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

            let execution_mode = multivm::interface::TxExecutionMode::EthCall;
            let storage = StorageView::new(&inner.fork_storage).into_rc_ptr();

            let bootloader_code = inner.system_contracts.contracts_for_l2_call();

            // init vm
            let (mut l1_batch_env, _block_context) = inner.create_l1_batch_env(storage.clone());

            // update the enforced_base_fee within l1_batch_env to match the logic in zksync_core
            l1_batch_env.enforced_base_fee = Some(l2_tx.common_data.fee.max_fee_per_gas.as_u64());
            let system_env = inner.create_system_env(bootloader_code.clone(), execution_mode);
            let mut vm: Vm<_, HistoryDisabled> = Vm::new(l1_batch_env, system_env, storage);

            // We must inject *some* signature (otherwise bootloader code fails to generate hash).
            if l2_tx.common_data.signature.is_empty() {
                l2_tx.common_data.signature =
                    PackedEthSignature::default().serialize_packed().into();
            }

            // Match behavior of zksync_core:
            // Protection against infinite-loop eth_calls and alike:
            // limiting the amount of gas the call can use.
            l2_tx.common_data.fee.gas_limit = ETH_CALL_GAS_LIMIT.into();

            let tx: Transaction = l2_tx.clone().into();
            vm.push_transaction(tx);

            let call_tracer_result = Arc::new(OnceCell::default());
            let tracer = CallTracer::new(call_tracer_result.clone()).into_tracer_pointer();
            let tx_result = vm.inspect(tracer.into(), multivm::interface::VmExecutionMode::OneTx);

            let call_traces = if only_top {
                vec![]
            } else {
                Arc::try_unwrap(call_tracer_result)
                    .unwrap()
                    .take()
                    .unwrap_or_default()
            };

            let debug =
                create_debug_output(&l2_tx, &tx_result, call_traces).map_err(into_jsrpc_error)?;

            Ok(debug)
        })
    }

    fn trace_transaction(
        &self,
        tx_hash: H256,
        options: Option<TracerConfig>,
    ) -> BoxFuture<Result<Option<DebugCall>>> {
        let only_top = options.is_some_and(|o| o.tracer_config.only_top_call);
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            let inner = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;

            Ok(inner
                .tx_results
                .get(&tx_hash)
                .map(|tx| tx.debug_info(only_top)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        deps::system_contracts::bytecode_from_slice,
        http_fork_source::HttpForkSource,
        node::{InMemoryNode, TransactionResult},
        testing::{self, LogBuilder},
    };
    use ethers::abi::{short_signature, AbiEncode, HumanReadableParser, ParamType, Token};
    use zksync_basic_types::{Address, Nonce, H160, U256};
    use zksync_types::{
        api::{Block, CallTracerConfig, SupportedTracers, TransactionReceipt},
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

    #[tokio::test]
    async fn test_trace_transaction() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            writer.tx_results.insert(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![LogBuilder::new()
                            .set_address(H160::repeat_byte(0xa1))
                            .build()],
                        ..Default::default()
                    },
                    debug: testing::default_tx_debug_info(),
                },
            );
        }
        let result = DebugNamespaceImpl::new(inner)
            .trace_transaction(H256::repeat_byte(0x1), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.calls.len(), 1);
    }

    #[tokio::test]
    async fn test_trace_transaction_only_top() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            writer.tx_results.insert(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![LogBuilder::new()
                            .set_address(H160::repeat_byte(0xa1))
                            .build()],
                        ..Default::default()
                    },
                    debug: testing::default_tx_debug_info(),
                },
            );
        }
        let result = DebugNamespaceImpl::new(inner)
            .trace_transaction(
                H256::repeat_byte(0x1),
                Some(TracerConfig {
                    tracer: SupportedTracers::CallTracer,
                    tracer_config: CallTracerConfig {
                        only_top_call: true,
                    },
                }),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(result.calls.is_empty());
    }

    #[tokio::test]
    async fn test_trace_transaction_not_found() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        let result = DebugNamespaceImpl::new(inner)
            .trace_transaction(H256::repeat_byte(0x1), None)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_trace_block_by_hash_empty() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            let block = Block::<TransactionVariant>::default();
            writer.blocks.insert(H256::repeat_byte(0x1), block);
        }
        let result = DebugNamespaceImpl::new(inner)
            .trace_block_by_hash(H256::repeat_byte(0x1), None)
            .await
            .unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_trace_block_by_hash() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            let tx = zksync_types::api::Transaction::default();
            let tx_hash = tx.hash;
            let mut block = Block::<TransactionVariant>::default();
            block.transactions.push(TransactionVariant::Full(tx));
            writer.blocks.insert(H256::repeat_byte(0x1), block);
            writer.tx_results.insert(
                tx_hash,
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt::default(),
                    debug: testing::default_tx_debug_info(),
                },
            );
        }
        let result = DebugNamespaceImpl::new(inner)
            .trace_block_by_hash(H256::repeat_byte(0x1), None)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].result.calls.len(), 1);
    }

    #[tokio::test]
    async fn test_trace_block_by_number() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            let tx = zksync_types::api::Transaction::default();
            let tx_hash = tx.hash;
            let mut block = Block::<TransactionVariant>::default();
            block.transactions.push(TransactionVariant::Full(tx));
            writer.blocks.insert(H256::repeat_byte(0x1), block);
            writer.block_hashes.insert(0, H256::repeat_byte(0x1));
            writer.tx_results.insert(
                tx_hash,
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt::default(),
                    debug: testing::default_tx_debug_info(),
                },
            );
        }
        // check `latest` alias
        let result = DebugNamespaceImpl::new(node.get_inner())
            .trace_block_by_number(BlockNumber::Latest, None)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].result.calls.len(), 1);

        // check block number
        let result = DebugNamespaceImpl::new(node.get_inner())
            .trace_block_by_number(BlockNumber::Number(0.into()), None)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].result.calls.len(), 1);
    }
}
