use crate::node::{InMemoryNode, MAX_TX_SIZE};
use crate::utils::create_debug_output;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use zksync_multivm::interface::storage::StorageView;
use zksync_multivm::interface::{VmFactory, VmInterface};
use zksync_multivm::tracers::CallTracer;
use zksync_multivm::vm_latest::constants::ETH_CALL_GAS_LIMIT;
use zksync_multivm::vm_latest::{HistoryDisabled, ToTracerPointer, Vm};
use zksync_types::l2::L2Tx;
use zksync_types::transaction_request::CallRequest;
use zksync_types::{api, api::BlockId, web3::Bytes, PackedEthSignature, Transaction, H256};
use zksync_web3_decl::error::Web3Error;

impl InMemoryNode {
    pub async fn trace_block_impl(
        &self,
        block_id: api::BlockId,
        options: Option<api::TracerConfig>,
    ) -> anyhow::Result<api::CallTracerBlockResult> {
        let only_top = options.is_some_and(|o| o.tracer_config.only_top_call);
        let tx_hashes = self
            .blockchain
            .get_block_tx_hashes_by_id(block_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Block (id={block_id}) not found"))?;

        let mut debug_calls = Vec::with_capacity(tx_hashes.len());
        for tx_hash in tx_hashes {
            let result = self.blockchain
                .get_tx_debug_info(&tx_hash, only_top)
                .await
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Unexpectedly transaction (hash={tx_hash}) belongs to a block but could not be found"
                    )
                })?;
            debug_calls.push(api::ResultDebugCall { result });
        }

        Ok(api::CallTracerBlockResult::CallTrace(debug_calls))
    }

    pub async fn trace_call_impl(
        &self,
        request: CallRequest,
        block: Option<api::BlockId>,
        options: Option<api::TracerConfig>,
    ) -> Result<api::CallTracerResult, Web3Error> {
        let only_top = options.is_some_and(|o| o.tracer_config.only_top_call);
        let inner = self.inner.read().await;
        let system_contracts = self.system_contracts.contracts_for_l2_call();
        if block.is_some() && !matches!(block, Some(api::BlockId::Number(api::BlockNumber::Latest)))
        {
            return Err(Web3Error::InternalError(anyhow::anyhow!(
                "tracing only supported at `latest` block"
            )));
        }

        let mut l2_tx = L2Tx::from_request(
            request.into(),
            MAX_TX_SIZE,
            self.system_contracts.allow_no_target(),
        )
        .map_err(Web3Error::SerializationError)?;
        let execution_mode = zksync_multivm::interface::TxExecutionMode::EthCall;

        // init vm
        let (mut l1_batch_env, _block_context) = inner.create_l1_batch_env().await;

        // update the enforced_base_fee within l1_batch_env to match the logic in zksync_core
        l1_batch_env.enforced_base_fee = Some(l2_tx.common_data.fee.max_fee_per_gas.as_u64());
        let system_env = inner.create_system_env(system_contracts.clone(), execution_mode);
        let storage = StorageView::new(inner.read_storage()).to_rc_ptr();
        let mut vm: Vm<_, HistoryDisabled> = Vm::new(l1_batch_env, system_env, storage);

        // We must inject *some* signature (otherwise bootloader code fails to generate hash).
        if l2_tx.common_data.signature.is_empty() {
            l2_tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        // Match behavior of zksync_core:
        // Protection against infinite-loop eth_calls and alike:
        // limiting the amount of gas the call can use.
        l2_tx.common_data.fee.gas_limit = ETH_CALL_GAS_LIMIT.into();

        let tx: Transaction = l2_tx.clone().into();
        vm.push_transaction(tx);

        let call_tracer_result = Arc::new(OnceCell::default());
        let tracer = CallTracer::new(call_tracer_result.clone()).into_tracer_pointer();

        let tx_result = vm.inspect(
            &mut tracer.into(),
            zksync_multivm::interface::InspectExecutionMode::OneTx,
        );
        let call_traces = if only_top {
            vec![]
        } else {
            Arc::try_unwrap(call_tracer_result)
                .unwrap()
                .take()
                .unwrap_or_default()
        };

        let debug = create_debug_output(&l2_tx.into(), &tx_result, call_traces)?;

        Ok(api::CallTracerResult::CallTrace(debug))
    }

    pub async fn trace_transaction_impl(
        &self,
        tx_hash: H256,
        options: Option<api::TracerConfig>,
    ) -> anyhow::Result<Option<api::CallTracerResult>> {
        let only_top = options.is_some_and(|o| o.tracer_config.only_top_call);
        Ok(self
            .blockchain
            .get_tx_debug_info(&tx_hash, only_top)
            .await
            .map(api::CallTracerResult::CallTrace))
    }

    pub async fn get_raw_transaction_impl(&self, tx_hash: H256) -> anyhow::Result<Option<Bytes>> {
        // Ok(self.blockchain.get_raw_transaction(&tx_hash).await)
        Ok(None)
    }

    pub async fn get_raw_transactions_impl(&self, block: BlockId) -> anyhow::Result<Vec<Bytes>> {
        // Ok(self.blockchain.get_raw_transactions(&block).await)
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use alloy::dyn_abi::{DynSolValue, FunctionExt, JsonAbiExt};
    use alloy::json_abi::{Function, Param, StateMutability};
    use alloy::primitives::{Address as AlloyAddress, U256 as AlloyU256};
    use anvil_zksync_config::constants::DEFAULT_ACCOUNT_BALANCE;
    use zksync_types::{
        transaction_request::CallRequestBuilder, utils::deployed_address_create, Address,
        K256PrivateKey, L2BlockNumber, Nonce, H160, U256,
    };

    use super::*;
    use crate::{
        deps::system_contracts::bytecode_from_slice,
        node::{InMemoryNode, TransactionResult},
        testing::{self, LogBuilder},
    };

    async fn deploy_test_contracts(node: &InMemoryNode) -> (Address, Address) {
        let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xee)).unwrap();
        let from_account = private_key.address();
        node.set_rich_account(from_account, U256::from(DEFAULT_ACCOUNT_BALANCE))
            .await;

        // first, deploy secondary contract
        let secondary_bytecode = bytecode_from_slice(
            "Secondary",
            include_bytes!("../deps/test-contracts/Secondary.json"),
        );
        let secondary_deployed_address = deployed_address_create(from_account, U256::zero());
        let alloy_secondary_address = AlloyAddress::from(secondary_deployed_address.0);
        let secondary_constructor_calldata =
            DynSolValue::Uint(AlloyU256::from(2), 256).abi_encode();

        testing::deploy_contract(
            node,
            &private_key,
            secondary_bytecode,
            Some(secondary_constructor_calldata),
            Nonce(0),
        )
        .await;

        // deploy primary contract using the secondary contract address as a constructor parameter
        let primary_bytecode = bytecode_from_slice(
            "Primary",
            include_bytes!("../deps/test-contracts/Primary.json"),
        );
        let primary_deployed_address = deployed_address_create(from_account, U256::one());
        let primary_constructor_calldata =
            DynSolValue::Address(alloy_secondary_address).abi_encode();

        testing::deploy_contract(
            node,
            &private_key,
            primary_bytecode,
            Some(primary_constructor_calldata),
            Nonce(1),
        )
        .await;

        (primary_deployed_address, secondary_deployed_address)
    }

    #[tokio::test]
    async fn test_trace_deployed_contract() {
        let node = InMemoryNode::test(None);

        let (primary_deployed_address, secondary_deployed_address) =
            deploy_test_contracts(&node).await;

        let func = Function {
            name: "calculate".to_string(),
            inputs: vec![Param {
                name: "value".to_string(),
                ty: "uint256".to_string(),
                components: vec![],
                internal_type: None,
            }],
            outputs: vec![Param {
                name: "".to_string(),
                ty: "uint256".to_string(),
                components: vec![],
                internal_type: None,
            }],
            state_mutability: StateMutability::NonPayable,
        };

        let calldata = func
            .abi_encode_input(&[DynSolValue::Uint(AlloyU256::from(42), 256)])
            .expect("failed to encode function input");

        let request = CallRequestBuilder::default()
            .to(Some(primary_deployed_address))
            .data(calldata.clone().into())
            .gas(80_000_000.into())
            .build();
        let trace = node
            .trace_call_impl(request.clone(), None, None)
            .await
            .expect("trace call")
            .unwrap_default();

        // call should not revert
        assert!(trace.error.is_none());
        assert!(trace.revert_reason.is_none());

        // check that the call was successful
        let output = func
            .abi_decode_output(trace.output.0.as_slice(), true)
            .expect("failed to decode output");
        assert_eq!(
            output[0],
            DynSolValue::Uint(AlloyU256::from(84), 256),
            "unexpected output"
        );

        // find the call to primary contract in the trace
        let contract_call = trace.calls.last().unwrap().calls.first().unwrap();

        assert_eq!(contract_call.to, primary_deployed_address);
        assert_eq!(contract_call.input, calldata.into());

        // check that it contains a call to secondary contract
        let subcall = contract_call.calls.first().unwrap();
        assert_eq!(subcall.to, secondary_deployed_address);
        assert_eq!(subcall.from, primary_deployed_address);
        assert_eq!(
            subcall.output,
            func.abi_encode_output(&[DynSolValue::Uint(AlloyU256::from(84), 256)])
                .expect("failed to encode function output")
                .into()
        );
    }

    #[tokio::test]
    async fn test_trace_only_top() {
        let node = InMemoryNode::test(None);

        let (primary_deployed_address, _) = deploy_test_contracts(&node).await;

        // trace a call to the primary contract
        let func = Function {
            name: "calculate".to_string(),
            inputs: vec![Param {
                name: "value".to_string(),
                ty: "uint256".to_string(),
                components: vec![],
                internal_type: None,
            }],
            outputs: vec![],
            state_mutability: StateMutability::NonPayable,
        };

        let calldata = func
            .abi_encode_input(&[DynSolValue::Uint(AlloyU256::from(42), 256)])
            .expect("failed to encode function input");

        let request = CallRequestBuilder::default()
            .to(Some(primary_deployed_address))
            .data(calldata.into())
            .gas(80_000_000.into())
            .build();

        // if we trace with onlyTopCall=true, we should get only the top-level call
        let trace = node
            .trace_call_impl(
                request,
                None,
                Some(api::TracerConfig {
                    tracer: api::SupportedTracers::CallTracer,
                    tracer_config: api::CallTracerConfig {
                        only_top_call: true,
                    },
                }),
            )
            .await
            .expect("trace call")
            .unwrap_default();
        // call should not revert
        assert!(trace.error.is_none());
        assert!(trace.revert_reason.is_none());

        // call should not contain any subcalls
        assert!(trace.calls.is_empty());
    }

    #[tokio::test]
    async fn test_trace_reverts() {
        let node = InMemoryNode::test(None);

        let (primary_deployed_address, _) = deploy_test_contracts(&node).await;

        let func = Function {
            name: "shouldRevert".to_string(),
            inputs: vec![],
            outputs: vec![],
            state_mutability: StateMutability::NonPayable,
        };

        // trace a call to the primary contract
        let request = CallRequestBuilder::default()
            .to(Some(primary_deployed_address))
            .data(func.selector().to_vec().into())
            .gas(80_000_000.into())
            .build();
        let trace = node
            .trace_call_impl(request, None, None)
            .await
            .expect("trace call")
            .unwrap_default();

        // call should revert
        assert!(trace.revert_reason.is_some());
        // find the call to primary contract in the trace
        let contract_call = trace.calls.last().unwrap().calls.first().unwrap();

        // the contract subcall should have reverted
        assert!(contract_call.revert_reason.is_some());
    }

    #[tokio::test]
    async fn test_trace_transaction_impl() {
        let node = InMemoryNode::test(None);
        {
            let mut writer = node.inner.write().await;
            writer
                .insert_tx_result(
                    H256::repeat_byte(0x1),
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        new_bytecodes: vec![],
                        receipt: api::TransactionReceipt {
                            logs: vec![LogBuilder::new()
                                .set_address(H160::repeat_byte(0xa1))
                                .build()],
                            ..Default::default()
                        },
                        debug: testing::default_tx_debug_info(),
                    },
                )
                .await;
        }
        let result = node
            .trace_transaction_impl(H256::repeat_byte(0x1), None)
            .await
            .unwrap()
            .unwrap()
            .unwrap_default();
        assert_eq!(result.calls.len(), 1);
    }

    #[tokio::test]
    async fn test_trace_transaction_only_top() {
        let node = InMemoryNode::test(None);
        node.inner
            .write()
            .await
            .insert_tx_result(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    new_bytecodes: vec![],
                    receipt: api::TransactionReceipt {
                        logs: vec![LogBuilder::new()
                            .set_address(H160::repeat_byte(0xa1))
                            .build()],
                        ..Default::default()
                    },
                    debug: testing::default_tx_debug_info(),
                },
            )
            .await;
        let result = node
            .trace_transaction_impl(
                H256::repeat_byte(0x1),
                Some(api::TracerConfig {
                    tracer: api::SupportedTracers::CallTracer,
                    tracer_config: api::CallTracerConfig {
                        only_top_call: true,
                    },
                }),
            )
            .await
            .unwrap()
            .unwrap()
            .unwrap_default();
        assert!(result.calls.is_empty());
    }

    #[tokio::test]
    async fn test_trace_transaction_not_found() {
        let node = InMemoryNode::test(None);
        let result = node
            .trace_transaction_impl(H256::repeat_byte(0x1), None)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_trace_block_by_hash_empty() {
        let node = InMemoryNode::test(None);
        let block = api::Block::<api::TransactionVariant>::default();
        node.inner
            .write()
            .await
            .insert_block(H256::repeat_byte(0x1), block)
            .await;
        let result = node
            .trace_block_impl(api::BlockId::Hash(H256::repeat_byte(0x1)), None)
            .await
            .unwrap()
            .unwrap_default();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_trace_block_by_hash_impl() {
        let node = InMemoryNode::test(None);
        let tx = api::Transaction::default();
        let tx_hash = tx.hash;
        let mut block = api::Block::<api::TransactionVariant>::default();
        block.transactions.push(api::TransactionVariant::Full(tx));
        {
            let mut writer = node.inner.write().await;
            writer.insert_block(H256::repeat_byte(0x1), block).await;
            writer
                .insert_tx_result(
                    tx_hash,
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        new_bytecodes: vec![],
                        receipt: api::TransactionReceipt::default(),
                        debug: testing::default_tx_debug_info(),
                    },
                )
                .await;
        }
        let result = node
            .trace_block_impl(api::BlockId::Hash(H256::repeat_byte(0x1)), None)
            .await
            .unwrap()
            .unwrap_default();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].result.calls.len(), 1);
    }

    #[tokio::test]
    async fn test_trace_block_by_number_impl() {
        let node = InMemoryNode::test(None);
        let tx = api::Transaction::default();
        let tx_hash = tx.hash;
        let mut block = api::Block::<api::TransactionVariant>::default();
        block.transactions.push(api::TransactionVariant::Full(tx));
        {
            let mut writer = node.inner.write().await;
            writer.insert_block(H256::repeat_byte(0x1), block).await;
            writer
                .insert_block_hash(L2BlockNumber(0), H256::repeat_byte(0x1))
                .await;
            writer
                .insert_tx_result(
                    tx_hash,
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        new_bytecodes: vec![],
                        receipt: api::TransactionReceipt::default(),
                        debug: testing::default_tx_debug_info(),
                    },
                )
                .await;
        }
        // check `latest` alias
        let result = node
            .trace_block_impl(api::BlockId::Number(api::BlockNumber::Latest), None)
            .await
            .unwrap()
            .unwrap_default();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].result.calls.len(), 1);

        // check block number
        let result = node
            .trace_block_impl(
                api::BlockId::Number(api::BlockNumber::Number(0.into())),
                None,
            )
            .await
            .unwrap()
            .unwrap_default();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].result.calls.len(), 1);
    }
}
