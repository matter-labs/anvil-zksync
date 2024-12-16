use crate::{
    deps::system_contracts::bytecode_from_slice,
    node::{InMemoryNode, TransactionResult},
    testing::{self, LogBuilder},
};
use anvil_zksync_config::constants::DEFAULT_ACCOUNT_BALANCE;
use ethers::abi::{short_signature, AbiEncode, HumanReadableParser, ParamType, Token};
use zksync_types::{
    api::{Block, CallTracerConfig, SupportedTracers, TransactionReceipt},
    transaction_request::CallRequestBuilder,
    utils::deployed_address_create,
    Address, K256PrivateKey, Nonce, H160, U256,
};
use zksync_types::{
    api::{BlockNumber, TracerConfig, TransactionVariant},
    H256,
};

fn deploy_test_contracts(node: &InMemoryNode) -> (Address, Address) {
    let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xee)).unwrap();
    let from_account = private_key.address();
    node.set_rich_account(from_account, U256::from(DEFAULT_ACCOUNT_BALANCE));

    // first, deploy secondary contract
    let secondary_bytecode = bytecode_from_slice(
        "Secondary",
        include_bytes!("../../deps/test-contracts/Secondary.json"),
    );
    let secondary_deployed_address = deployed_address_create(from_account, U256::zero());
    testing::deploy_contract(
        node,
        H256::repeat_byte(0x1),
        &private_key,
        secondary_bytecode,
        Some((U256::from(2),).encode()),
        Nonce(0),
    );

    // deploy primary contract using the secondary contract address as a constructor parameter
    let primary_bytecode = bytecode_from_slice(
        "Primary",
        include_bytes!("../../deps/test-contracts/Primary.json"),
    );
    let primary_deployed_address = deployed_address_create(from_account, U256::one());
    testing::deploy_contract(
        node,
        H256::repeat_byte(0x1),
        &private_key,
        primary_bytecode,
        Some((secondary_deployed_address).encode()),
        Nonce(1),
    );
    (primary_deployed_address, secondary_deployed_address)
}

#[tokio::test]
async fn test_trace_deployed_contract() {
    let node = InMemoryNode::default();

    let (primary_deployed_address, secondary_deployed_address) = deploy_test_contracts(&node);
    // trace a call to the primary contract
    let func = HumanReadableParser::parse_function("calculate(uint)").unwrap();
    let calldata = func.encode_input(&[Token::Uint(U256::from(42))]).unwrap();
    let request = CallRequestBuilder::default()
        .to(Some(primary_deployed_address))
        .data(calldata.clone().into())
        .gas(80_000_000.into())
        .build();
    let trace = node
        .trace_call(request.clone(), None, None)
        .await
        .expect("trace call")
        .unwrap_default();

    // call should not revert
    assert!(trace.error.is_none());
    assert!(trace.revert_reason.is_none());

    // check that the call was successful
    let output = ethers::abi::decode(&[ParamType::Uint(256)], trace.output.0.as_slice()).unwrap();
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
    let node = InMemoryNode::default();

    let (primary_deployed_address, _) = deploy_test_contracts(&node);

    // trace a call to the primary contract
    let func = HumanReadableParser::parse_function("calculate(uint)").unwrap();
    let calldata = func.encode_input(&[Token::Uint(U256::from(42))]).unwrap();
    let request = CallRequestBuilder::default()
        .to(Some(primary_deployed_address))
        .data(calldata.into())
        .gas(80_000_000.into())
        .build();

    // if we trace with onlyTopCall=true, we should get only the top-level call
    let trace = node
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
    let node = InMemoryNode::default();

    let (primary_deployed_address, _) = deploy_test_contracts(&node);

    // trace a call to the primary contract
    let request = CallRequestBuilder::default()
        .to(Some(primary_deployed_address))
        .data(short_signature("shouldRevert()", &[]).into())
        .gas(80_000_000.into())
        .build();
    let trace = node
        .trace_call(request, None, None)
        .await
        .expect("trace call")
        .unwrap_default();

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
    let node = InMemoryNode::default();
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
    let result = node
        .trace_transaction(H256::repeat_byte(0x1), None)
        .await
        .unwrap()
        .unwrap()
        .unwrap_default();
    assert_eq!(result.calls.len(), 1);
}

#[tokio::test]
async fn test_trace_transaction_only_top() {
    let node = InMemoryNode::default();
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
    let result = node
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
        .unwrap()
        .unwrap_default();
    assert!(result.calls.is_empty());
}

#[tokio::test]
async fn test_trace_transaction_not_found() {
    let node = InMemoryNode::default();
    let result = node
        .trace_transaction(H256::repeat_byte(0x1), None)
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_trace_block_by_hash_empty() {
    let node = InMemoryNode::default();
    let inner = node.get_inner();
    {
        let mut writer = inner.write().unwrap();
        let block = Block::<TransactionVariant>::default();
        writer.blocks.insert(H256::repeat_byte(0x1), block);
    }
    let result = node
        .trace_block_by_hash(H256::repeat_byte(0x1), None)
        .await
        .unwrap()
        .unwrap_default();
    assert_eq!(result.len(), 0);
}

#[tokio::test]
async fn test_trace_block_by_hash() {
    let node = InMemoryNode::default();
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
    let result = node
        .trace_block_by_hash(H256::repeat_byte(0x1), None)
        .await
        .unwrap()
        .unwrap_default();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].result.calls.len(), 1);
}

#[tokio::test]
async fn test_trace_block_by_number() {
    let node = InMemoryNode::default();
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
    let result = node
        .trace_block_by_number(BlockNumber::Latest, None)
        .await
        .unwrap()
        .unwrap_default();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].result.calls.len(), 1);

    // check block number
    let result = node
        .trace_block_by_number(BlockNumber::Number(0.into()), None)
        .await
        .unwrap()
        .unwrap_default();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].result.calls.len(), 1);
}
