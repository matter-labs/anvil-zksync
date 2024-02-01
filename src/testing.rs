//! This file hold testing helpers for other unit tests.
//!
//! There is MockServer that can help simulate a forked network.
//!

#![cfg(test)]

use crate::deps::InMemoryStorage;
use crate::node::{InMemoryNode, TxExecutionInfo};
use crate::{fork::ForkSource, node::compute_hash};

use ethabi::{ParamType, Token};
use ethers::contract;
use httptest::{
    matchers::{eq, json_decoded, request},
    responders::json_encoded,
    Expectation, Server,
};
use itertools::Itertools;
use multivm::interface::{ExecutionResult, VmExecutionResultAndLogs};
use std::str::FromStr;
use zksync_basic_types::{AccountTreeId, MiniblockNumber, H160, U64};
use zksync_types::api::{BlockIdVariant, BridgeAddresses, DebugCall, DebugCallType, Log};
use zksync_types::block::pack_block_info;
use zksync_types::StorageKey;
use zksync_types::{fee::Fee, l2::L2Tx, Address, L2ChainId, Nonce, ProtocolVersionId, H256, U256};
use zksync_utils::u256_to_h256;

/// Configuration for the [MockServer]'s initial block.
#[derive(Default, Debug, Clone)]
pub struct ForkBlockConfig {
    pub number: u64,
    pub hash: H256,
    pub transaction_count: u8,
}

/// A HTTP server that can be used to mock a fork source.
pub struct MockServer {
    /// The implementation for [httptest::Server].
    pub inner: Server,
}

impl MockServer {
    /// Start the mock server.
    pub fn run() -> Self {
        MockServer {
            inner: Server::run(),
        }
    }

    /// Start the mock server with pre-defined calls used to fetch the fork's state.
    /// The input config can be used to set the initial block's number, hash and transactions.
    pub fn run_with_config(block_config: ForkBlockConfig) -> Self {
        let server = Server::run();

        // setup initial fork calls
        server.expect(
            Expectation::matching(request::body(json_decoded(eq(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_blockNumber",
            })))))
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "result": format!("{:#x}", block_config.number),
            }))),
        );
        server.expect(
            Expectation::matching(request::body(json_decoded(eq(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "zks_getBlockDetails",
                "params": [ block_config.number ],
            })))))
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "number": block_config.number,
                    "l1BatchNumber": 1,
                    "timestamp": 1676461082u64,
                    "l1TxCount": 0,
                    "l2TxCount": 0,
                    "rootHash": format!("{:#x}", block_config.hash),
                    "status": "verified",
                    "commitTxHash": "0x9f5b07e968787514667fae74e77ecab766be42acd602c85cfdbda1dc3dd9902f",
                    "committedAt": "2023-02-15T11:40:39.326104Z",
                    "proveTxHash": "0xac8fe9fdcbeb5f1e59c41e6bd33b75d405af84e4b968cd598c2d3f59c9c925c8",
                    "provenAt": "2023-02-15T12:42:40.073918Z",
                    "executeTxHash": "0x65d50174b214b05e82936c4064023cbea5f6f8135e30b4887986b316a2178a39",
                    "executedAt": "2023-02-15T12:43:20.330052Z",
                    "l1GasPrice": 29860969933u64,
                    "l2FairGasPrice": 500000000u64,
                    "baseSystemContractsHashes": {
                      "bootloader": "0x0100038581be3d0e201b3cc45d151ef5cc59eb3a0f146ad44f0f72abf00b594c",
                      "default_aa": "0x0100038dc66b69be75ec31653c64cb931678299b9b659472772b2550b703f41c"
                    },
                    "operatorAddress": "0xfeee860e7aae671124e9a4e61139f3a5085dfeee",
                    "protocolVersion": ProtocolVersionId::Version15,
                  },
            }))),
        );
        server.expect(
            Expectation::matching(request::body(json_decoded(eq(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "eth_getBlockByHash",
                "params": [format!("{:#x}", block_config.hash), true],
            }))))).times(0..)
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {
                    "hash": format!("{:#x}", block_config.hash),
                    "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                    "miner": "0x0000000000000000000000000000000000000000",
                    "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "transactionsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "receiptsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "number": format!("{:#x}", block_config.number),
                    "l1BatchNumber": "0x6",
                    "gasUsed": "0x0",
                    "gasLimit": "0xffffffff",
                    "baseFeePerGas": "0x1dcd6500",
                    "extraData": "0x",
                    "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                    "timestamp": "0x63ecc41a",
                    "l1BatchTimestamp": "0x63ecbd12",
                    "difficulty": "0x0",
                    "totalDifficulty": "0x0",
                    "sealFields": [],
                    "uncles": [],
                    "transactions": (0..block_config.transaction_count)
                        .map(|index| {
                            TransactionResponseBuilder::new()
                                .set_hash(H256::repeat_byte(index))
                                .build_result()
                        })
                    .collect::<Vec<_>>(),
                    "size": "0x0",
                    "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "nonce": "0x0000000000000000"
                }
            }))),
        );

        MockServer { inner: server }
    }

    /// Retrieve the mock server's url.
    pub fn url(&self) -> String {
        self.inner.url("").to_string()
    }

    /// Assert an exactly single call expectation with a given request and the provided response.
    pub fn expect(&self, request: serde_json::Value, response: serde_json::Value) {
        self.inner.expect(
            Expectation::matching(request::body(json_decoded(eq(request))))
                .respond_with(json_encoded(response)),
        );
    }
}

/// A mock response builder for a block
#[derive(Default, Debug, Clone)]
pub struct BlockResponseBuilder {
    hash: H256,
    number: u64,
}

impl BlockResponseBuilder {
    /// Create a new instance of [BlockResponseBuilder]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the block hash
    pub fn set_hash(&mut self, hash: H256) -> &mut Self {
        self.hash = hash;
        self
    }

    /// Sets the block number
    pub fn set_number(&mut self, number: u64) -> &mut Self {
        self.number = number;
        self
    }

    /// Builds the block json result response
    pub fn build_result(&mut self) -> serde_json::Value {
        serde_json::json!({
            "hash": format!("{:#x}", self.hash),
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "miner": "0x0000000000000000000000000000000000000000",
            "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "transactionsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "receiptsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "number": format!("{:#x}", self.number),
            "l1BatchNumber": "0x6",
            "gasUsed": "0x0",
            "gasLimit": "0xffffffff",
            "baseFeePerGas": "0x1dcd6500",
            "extraData": "0x",
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "timestamp": "0x63ecc41a",
            "l1BatchTimestamp": "0x63ecbd12",
            "difficulty": "0x0",
            "totalDifficulty": "0x0",
            "sealFields": [],
            "uncles": [],
            "transactions": [],
            "size": "0x0",
            "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "nonce": "0x0000000000000000"
        })
    }

    /// Builds the json response
    pub fn build(&mut self) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": self.build_result(),
        })
    }
}

/// A mock response builder for a transaction
#[derive(Default, Debug, Clone)]
pub struct TransactionResponseBuilder {
    hash: H256,
    block_hash: H256,
    block_number: U64,
}

impl TransactionResponseBuilder {
    /// Create a new instance of [TransactionResponseBuilder]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the transaction hash
    pub fn set_hash(&mut self, hash: H256) -> &mut Self {
        self.hash = hash;
        self
    }

    /// Sets the block hash
    pub fn set_block_hash(&mut self, hash: H256) -> &mut Self {
        self.block_hash = hash;
        self
    }

    /// Sets the block number
    pub fn set_block_number(&mut self, number: U64) -> &mut Self {
        self.block_number = number;
        self
    }

    /// Builds the transaction json result
    pub fn build_result(&mut self) -> serde_json::Value {
        serde_json::json!({
            "hash": format!("{:#x}", self.hash),
            "nonce": "0x0",
            "blockHash": format!("{:#x}", self.block_hash),
            "blockNumber": format!("{:#x}", self.block_number),
            "transactionIndex": "0x0",
            "from": "0x29df43f75149d0552475a6f9b2ac96e28796ed0b",
            "to": "0x0000000000000000000000000000000000008006",
            "value": "0x0",
            "gasPrice": "0x0",
            "gas": "0x44aa200",
            "input": "0x3cda33510000000000000000000000000000000000000000000000000000000000000000010000553109a66f1432eb2286c54694784d1b6993bc24a168be0a49b4d0fd4500000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000",
            "type": "0xff",
            "maxFeePerGas": "0x0",
            "maxPriorityFeePerGas": "0x0",
            "chainId": "0x104",
            "l1BatchNumber": "0x1",
            "l1BatchTxIndex": "0x0",
        })
    }

    /// Builds the json response
    pub fn build(&mut self) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": self.build_result(),
        })
    }
}

/// A mock response builder for a transaction
#[derive(Default, Debug, Clone)]
pub struct RawTransactionsResponseBuilder {
    serial_ids: Vec<u64>,
}

impl RawTransactionsResponseBuilder {
    /// Create a new instance of [RawTransactionsResponseBuilder]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a new raw transaction with a serial id
    pub fn add(&mut self, serial_id: u64) -> &mut Self {
        self.serial_ids.push(serial_id);
        self
    }

    /// Builds the raw transaction json result
    pub fn build_result(&mut self) -> serde_json::Value {
        serde_json::json!(
            self.serial_ids
                .iter()
                .map(|serial_id| serde_json::json!({
                    "common_data": {
                        "L1": {
                            "sender": "0xcca8009f5e09f8c5db63cb0031052f9cb635af62",
                            "serialId": serial_id,
                            "deadlineBlock": 0,
                            "layer2TipFee": "0x0",
                            "fullFee": "0x0",
                            "maxFeePerGas": "0x0",
                            "gasLimit": "0x989680",
                            "gasPerPubdataLimit": "0x320",
                            "opProcessingType": "Common",
                            "priorityQueueType": "Deque",
                            "ethHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                            "ethBlock": 16631249u64,
                            "canonicalTxHash": "0xaaf9514a005ba59e29b53e1dc84d234d909c5202b44c5179f9c67d8e3cad0636",
                            "toMint": "0x470de4df820000",
                            "refundRecipient": "0xcca8009f5e09f8c5db63cb0031052f9cb635af62"
                        }
                    },
                    "execute": {
                        "contractAddress": "0xcca8009f5e09f8c5db63cb0031052f9cb635af62",
                        "calldata": "0x",
                        "value": "0x470de4df820000",
                        "factoryDeps": []
                    },
                    "received_timestamp_ms": 1676429272816u64,
                    "raw_bytes": null
                }))
                .collect_vec()
        )
    }

    /// Builds the json response
    pub fn build(&mut self) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": self.build_result(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct TransactionBuilder {
    tx_hash: H256,
    from_account_private_key: H256,
    gas_limit: U256,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self {
            tx_hash: H256::repeat_byte(0x01),
            from_account_private_key: H256::random(),
            gas_limit: U256::from(1_000_000),
            max_fee_per_gas: U256::from(250_000_000),
            max_priority_fee_per_gas: U256::from(250_000_000),
        }
    }
}

impl TransactionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_hash(&mut self, hash: H256) -> &mut Self {
        self.tx_hash = hash;
        self
    }

    pub fn set_gas_limit(&mut self, gas_limit: U256) -> &mut Self {
        self.gas_limit = gas_limit;
        self
    }

    pub fn set_max_fee_per_gas(&mut self, max_fee_per_gas: U256) -> &mut Self {
        self.max_fee_per_gas = max_fee_per_gas;
        self
    }

    pub fn set_max_priority_fee_per_gas(&mut self, max_priority_fee_per_gas: U256) -> &mut Self {
        self.max_priority_fee_per_gas = max_priority_fee_per_gas;
        self
    }

    pub fn build(&mut self) -> L2Tx {
        let mut tx = L2Tx::new_signed(
            Address::random(),
            vec![],
            Nonce(0),
            Fee {
                gas_limit: self.gas_limit,
                max_fee_per_gas: self.max_fee_per_gas,
                max_priority_fee_per_gas: self.max_priority_fee_per_gas,
                gas_per_pubdata_limit: U256::from(20000),
            },
            U256::from(1),
            L2ChainId::from(260),
            &self.from_account_private_key,
            None,
            Default::default(),
        )
        .unwrap();
        tx.set_input(vec![], self.tx_hash);
        tx
    }
}

/// Applies a transaction with a given hash to the node and returns the block hash.
pub fn apply_tx<T: ForkSource + std::fmt::Debug + Clone>(
    node: &InMemoryNode<T>,
    tx_hash: H256,
) -> (H256, U64) {
    let next_miniblock = node
        .get_inner()
        .read()
        .map(|reader| reader.current_miniblock.saturating_add(1))
        .expect("failed getting current batch number");
    let produced_block_hash = compute_hash(next_miniblock, tx_hash);

    let tx = TransactionBuilder::new().set_hash(tx_hash).build();
    node.set_rich_account(tx.common_data.initiator_address);
    node.apply_txs(vec![tx]).expect("failed applying tx");

    (produced_block_hash, U64::from(next_miniblock))
}

/// Deploys a contract with the given bytecode.
pub fn deploy_contract<T: ForkSource + std::fmt::Debug + Clone>(
    node: &InMemoryNode<T>,
    tx_hash: H256,
    private_key: H256,
    bytecode: Vec<u8>,
    calldata: Option<Vec<u8>>,
    nonce: Nonce,
) -> H256 {
    use ethers::abi::Function;
    use ethers::types::Bytes;
    use zksync_web3_rs::eip712;

    let next_miniblock = node
        .get_inner()
        .read()
        .map(|reader| reader.current_miniblock.saturating_add(1))
        .expect("failed getting current batch number");
    let produced_block_hash = compute_hash(next_miniblock, tx_hash);

    let salt = [0u8; 32];
    let bytecode_hash = eip712::hash_bytecode(&bytecode).expect("invalid bytecode");
    let call_data: Bytes = calldata.unwrap_or_default().into();
    let create: Function = serde_json::from_str(
        r#"{
            "inputs": [
              {
                "internalType": "bytes32",
                "name": "_salt",
                "type": "bytes32"
              },
              {
                "internalType": "bytes32",
                "name": "_bytecodeHash",
                "type": "bytes32"
              },
              {
                "internalType": "bytes",
                "name": "_input",
                "type": "bytes"
              }
            ],
            "name": "create",
            "outputs": [
              {
                "internalType": "address",
                "name": "",
                "type": "address"
              }
            ],
            "stateMutability": "payable",
            "type": "function"
          }"#,
    )
    .unwrap();

    let data = contract::encode_function_data(&create, (salt, bytecode_hash, call_data))
        .expect("failed encoding function data");

    let mut tx = L2Tx::new_signed(
        zksync_types::CONTRACT_DEPLOYER_ADDRESS,
        data.to_vec(),
        nonce,
        Fee {
            gas_limit: U256::from(82511299),
            max_fee_per_gas: U256::from(250_000_000),
            max_priority_fee_per_gas: U256::from(250_000_000),
            gas_per_pubdata_limit: U256::from(50000),
        },
        U256::from(0),
        zksync_basic_types::L2ChainId::from(260),
        &private_key,
        Some(vec![bytecode]),
        Default::default(),
    )
    .expect("failed signing tx");
    tx.set_input(vec![], tx_hash);
    node.apply_txs(vec![tx]).expect("failed deploying contract");

    produced_block_hash
}

/// Builds transaction logs
#[derive(Debug, Default, Clone)]
pub struct LogBuilder {
    block_number: U64,
    address: Option<H160>,
    topics: Option<Vec<H256>>,
}

impl LogBuilder {
    /// Create a new instance of [LogBuilder]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the log's block number
    pub fn set_block(&mut self, number: U64) -> &mut Self {
        self.block_number = number;
        self
    }

    /// Sets the log address
    pub fn set_address(&mut self, address: H160) -> &mut Self {
        self.address = Some(address);
        self
    }

    /// Sets the log topics
    pub fn set_topics(&mut self, topics: Vec<H256>) -> &mut Self {
        self.topics = Some(topics);
        self
    }

    /// Builds the [Log] object
    pub fn build(&mut self) -> Log {
        Log {
            address: self.address.unwrap_or_default(),
            topics: self.topics.clone().unwrap_or_default(),
            data: Default::default(),
            block_hash: Some(H256::zero()),
            block_number: Some(self.block_number),
            l1_batch_number: Default::default(),
            transaction_hash: Default::default(),
            transaction_index: Default::default(),
            log_index: Default::default(),
            transaction_log_index: Default::default(),
            log_type: Default::default(),
            removed: Some(false),
        }
    }
}

/// Simple storage solidity contract that stores and retrieves two numbers
///
/// contract Storage {
///   uint256 number1 = 1024;
///   uint256 number2 = 115792089237316195423570985008687907853269984665640564039457584007913129639935; // uint256::max
///
///   function retrieve1() public view returns (uint256) {
///     return number1;
///   }
///
///   function retrieve2() public view returns (uint256) {
///     return number2;
///   }
///
///   function transact_retrieve1() public returns (uint256) {
///     return number1;
///   }
/// }
pub const STORAGE_CONTRACT_BYTECODE: &str    = "0000008003000039000000400030043f0000000102200190000000150000c13d00000000020100190000000d02200198000000290000613d000000000101043b000000e0011002700000000e0210009c000000220000613d0000000f0210009c000000220000613d000000100110009c000000290000c13d0000000001000416000000000101004b000000290000c13d0000000101000039000000000101041a000000260000013d0000000001000416000000000101004b000000290000c13d0000040001000039000000000010041b000000010100008a0000000102000039000000000012041b0000002001000039000001000010044300000120000004430000000c010000410000002c0001042e0000000001000416000000000101004b000000290000c13d000000000100041a000000800010043f00000011010000410000002c0001042e00000000010000190000002d000104300000002b000004320000002c0001042e0000002d0001043000000000000000000000000000000000000000020000000000000000000000000000004000000100000000000000000000000000000000000000000000000000fffffffc00000000000000000000000000000000000000000000000000000000000000000000000000000000bbf5533500000000000000000000000000000000000000000000000000000000ae2e2cce000000000000000000000000000000000000000000000000000000002711432d0000000000000000000000000000000000000020000000800000000000000000ccac83652a1e8701e76052e8662f8e7889170c68883ae295c1c984f22be3560f";

/// Returns a default instance for a successful [TxExecutionInfo]
pub fn default_tx_execution_info() -> TxExecutionInfo {
    TxExecutionInfo {
        tx: L2Tx {
            execute: zksync_types::Execute {
                contract_address: Default::default(),
                calldata: Default::default(),
                value: Default::default(),
                factory_deps: Default::default(),
            },
            common_data: Default::default(),
            received_timestamp_ms: Default::default(),
            raw_bytes: None,
        },
        batch_number: Default::default(),
        miniblock_number: Default::default(),
        result: VmExecutionResultAndLogs {
            result: ExecutionResult::Success { output: vec![] },
            logs: Default::default(),
            statistics: Default::default(),
            refunds: Default::default(),
        },
    }
}

/// Returns a default instance for a successful [DebugCall]
pub fn default_tx_debug_info() -> DebugCall {
    DebugCall {
        r#type: DebugCallType::Call,
        from: Address::zero(),
        to: Address::zero(),
        gas: U256::zero(),
        gas_used: U256::zero(),
        value: U256::zero(),
        output: Default::default(),
        input: Default::default(),
        error: None,
        revert_reason: None,
        calls: vec![DebugCall {
            r#type: DebugCallType::Call,
            from: Address::zero(),
            to: Address::zero(),
            gas: U256::zero(),
            gas_used: U256::zero(),
            value: U256::zero(),
            output: Default::default(),
            input: Default::default(),
            error: None,
            revert_reason: None,
            calls: vec![],
        }],
    }
}

/// Decodes a `bytes` tx result to its concrete parameter type.
pub fn decode_tx_result(output: &[u8], param_type: ParamType) -> Token {
    let result = ethabi::decode(&[ParamType::Bytes], output).expect("failed decoding output");
    if result.is_empty() {
        panic!("result was empty");
    }

    let result_bytes = result[0]
        .clone()
        .into_bytes()
        .expect("failed converting result to bytes");
    let result = ethabi::decode(&[param_type], &result_bytes).expect("failed converting output");
    if result.is_empty() {
        panic!("decoded result was empty");
    }

    result[0].clone()
}

/// Asserts that two instances of [BridgeAddresses] are equal
pub fn assert_bridge_addresses_eq(
    expected_bridge_addresses: &BridgeAddresses,
    actual_bridge_addresses: &BridgeAddresses,
) {
    assert_eq!(
        expected_bridge_addresses.l1_erc20_default_bridge,
        actual_bridge_addresses.l1_erc20_default_bridge
    );
    assert_eq!(
        expected_bridge_addresses.l2_erc20_default_bridge,
        actual_bridge_addresses.l2_erc20_default_bridge
    );
    assert_eq!(
        expected_bridge_addresses.l1_weth_bridge,
        actual_bridge_addresses.l1_weth_bridge
    );
    assert_eq!(
        expected_bridge_addresses.l2_weth_bridge,
        actual_bridge_addresses.l2_weth_bridge
    );
}

/// Represents a read-only fork source that is backed by the provided [InMemoryStorage].
#[derive(Debug, Clone)]
pub struct ExternalStorage {
    pub raw_storage: InMemoryStorage,
}

impl ForkSource for &ExternalStorage {
    fn get_storage_at(
        &self,
        address: H160,
        idx: U256,
        _block: Option<BlockIdVariant>,
    ) -> eyre::Result<H256> {
        let key = StorageKey::new(AccountTreeId::new(address), u256_to_h256(idx));
        Ok(self
            .raw_storage
            .state
            .get(&key)
            .cloned()
            .unwrap_or_default())
    }

    fn get_raw_block_transactions(
        &self,
        _block_number: MiniblockNumber,
    ) -> eyre::Result<Vec<zksync_types::Transaction>> {
        todo!()
    }

    fn get_bytecode_by_hash(&self, hash: H256) -> eyre::Result<Option<Vec<u8>>> {
        Ok(self.raw_storage.factory_deps.get(&hash).cloned())
    }

    fn get_transaction_by_hash(
        &self,
        _hash: H256,
    ) -> eyre::Result<Option<zksync_types::api::Transaction>> {
        todo!()
    }

    fn get_transaction_details(
        &self,
        _hash: H256,
    ) -> eyre::Result<std::option::Option<zksync_types::api::TransactionDetails>> {
        todo!()
    }

    fn get_block_by_hash(
        &self,
        _hash: H256,
        _full_transactions: bool,
    ) -> eyre::Result<Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>> {
        todo!()
    }

    fn get_block_by_number(
        &self,
        _block_number: zksync_types::api::BlockNumber,
        _full_transactions: bool,
    ) -> eyre::Result<Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>> {
        todo!()
    }

    fn get_block_details(
        &self,
        _miniblock: MiniblockNumber,
    ) -> eyre::Result<Option<zksync_types::api::BlockDetails>> {
        todo!()
    }

    fn get_block_transaction_count_by_hash(&self, _block_hash: H256) -> eyre::Result<Option<U256>> {
        todo!()
    }

    fn get_block_transaction_count_by_number(
        &self,
        _block_number: zksync_types::api::BlockNumber,
    ) -> eyre::Result<Option<U256>> {
        todo!()
    }

    fn get_transaction_by_block_hash_and_index(
        &self,
        _block_hash: H256,
        _index: zksync_basic_types::web3::types::Index,
    ) -> eyre::Result<Option<zksync_types::api::Transaction>> {
        todo!()
    }

    fn get_transaction_by_block_number_and_index(
        &self,
        _block_number: zksync_types::api::BlockNumber,
        _index: zksync_basic_types::web3::types::Index,
    ) -> eyre::Result<Option<zksync_types::api::Transaction>> {
        todo!()
    }

    fn get_bridge_contracts(&self) -> eyre::Result<zksync_types::api::BridgeAddresses> {
        todo!()
    }

    fn get_confirmed_tokens(
        &self,
        _from: u32,
        _limit: u8,
    ) -> eyre::Result<Vec<zksync_web3_decl::types::Token>> {
        todo!()
    }
}

mod test {
    use maplit::hashmap;
    use zksync_types::block::unpack_block_info;
    use zksync_utils::h256_to_u256;

    use super::*;
    use crate::http_fork_source::HttpForkSource;

    #[test]
    fn test_block_response_builder_set_hash() {
        let builder = BlockResponseBuilder::new()
            .set_hash(H256::repeat_byte(0x01))
            .build();

        let actual_value = builder
            .as_object()
            .and_then(|o| o.get("result").unwrap().as_object())
            .and_then(|o| o.get("hash").unwrap().as_str())
            .expect("failed retrieving value");

        assert_eq!(
            "0x0101010101010101010101010101010101010101010101010101010101010101",
            actual_value
        );
    }

    #[test]
    fn test_block_response_builder_set_number() {
        let builder = BlockResponseBuilder::new().set_number(255).build();

        let actual_value = builder
            .as_object()
            .and_then(|o| o.get("result").unwrap().as_object())
            .and_then(|o| o.get("number").unwrap().as_str())
            .expect("failed retrieving value");

        assert_eq!("0xff", actual_value);
    }

    #[test]
    fn test_transaction_response_builder_set_hash() {
        let builder = TransactionResponseBuilder::new()
            .set_hash(H256::repeat_byte(0x01))
            .build();

        let actual_value = builder
            .as_object()
            .and_then(|o| o.get("result").unwrap().as_object())
            .and_then(|o| o.get("hash").unwrap().as_str())
            .expect("failed retrieving value");

        assert_eq!(
            "0x0101010101010101010101010101010101010101010101010101010101010101",
            actual_value
        );
    }

    #[test]
    fn test_raw_transactions_response_builder_no_items() {
        let builder = RawTransactionsResponseBuilder::new().build();

        let actual_len = builder
            .as_object()
            .and_then(|o| o.get("result").unwrap().as_array())
            .map(|o| o.len())
            .expect("failed retrieving value");

        assert_eq!(0, actual_len);
    }

    #[test]
    fn test_raw_transactions_response_builder_added_items() {
        let builder = RawTransactionsResponseBuilder::new()
            .add(10)
            .add(11)
            .build();

        let actual_serial_ids = builder
            .as_object()
            .and_then(|o| o.get("result").unwrap().as_array())
            .map(|o| {
                o.iter()
                    .map(|o| o.get("common_data").unwrap().as_object().unwrap())
                    .map(|o| o.get("L1").unwrap().as_object().unwrap())
                    .map(|entry| entry.get("serialId").unwrap().as_u64().unwrap())
                    .collect_vec()
            })
            .expect("failed retrieving value");

        assert_eq!(vec![10, 11], actual_serial_ids);
    }

    #[tokio::test]
    async fn test_apply_tx() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let (actual_block_hash, actual_block_number) = apply_tx(&node, H256::repeat_byte(0x01));

        assert_eq!(
            H256::from_str("0xd97ba6a5ab0f2d7fbfc697251321cce20bff3da2b0ddaf12c80f80f0ab270b15")
                .unwrap(),
            actual_block_hash,
        );
        assert_eq!(U64::from(1), actual_block_number);

        assert!(
            node.get_inner()
                .read()
                .map(|inner| inner.blocks.contains_key(&actual_block_hash))
                .unwrap(),
            "block was not produced"
        );
    }

    #[test]
    fn test_log_builder_set_block() {
        let log = LogBuilder::new().set_block(U64::from(2)).build();

        assert_eq!(Some(U64::from(2)), log.block_number);
    }

    #[test]
    fn test_log_builder_set_address() {
        let log = LogBuilder::new()
            .set_address(H160::repeat_byte(0x1))
            .build();

        assert_eq!(H160::repeat_byte(0x1), log.address);
    }

    #[test]
    fn test_log_builder_set_topics() {
        let log = LogBuilder::new()
            .set_topics(vec![
                H256::repeat_byte(0x1),
                H256::repeat_byte(0x2),
                H256::repeat_byte(0x3),
                H256::repeat_byte(0x4),
            ])
            .build();

        assert_eq!(
            vec![
                H256::repeat_byte(0x1),
                H256::repeat_byte(0x2),
                H256::repeat_byte(0x3),
                H256::repeat_byte(0x4),
            ],
            log.topics
        );
    }

    #[test]
    fn test_external_storage() {
        let input_batch = 1;
        let input_l2_block = 2;
        let input_timestamp = 3;
        let input_bytecode = vec![0x4];
        let batch_key = StorageKey::new(
            AccountTreeId::new(zksync_types::SYSTEM_CONTEXT_ADDRESS),
            zksync_types::SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
        );
        let l2_block_key = StorageKey::new(
            AccountTreeId::new(zksync_types::SYSTEM_CONTEXT_ADDRESS),
            zksync_types::SYSTEM_CONTEXT_CURRENT_L2_BLOCK_INFO_POSITION,
        );

        let storage = &ExternalStorage {
            raw_storage: InMemoryStorage {
                state: hashmap! {
                    batch_key => u256_to_h256(U256::from(input_batch)),
                    l2_block_key => u256_to_h256(pack_block_info(
                        input_l2_block,
                        input_timestamp,
                    ))
                },
                factory_deps: hashmap! {
                    H256::repeat_byte(0x1) => input_bytecode.clone(),
                },
            },
        };

        let actual_batch = storage
            .get_storage_at(
                zksync_types::SYSTEM_CONTEXT_ADDRESS,
                h256_to_u256(zksync_types::SYSTEM_CONTEXT_BLOCK_INFO_POSITION),
                None,
            )
            .map(|value| h256_to_u256(value).as_u64())
            .expect("failed getting batch number");
        assert_eq!(input_batch, actual_batch);

        let (actual_l2_block, actual_timestamp) = storage
            .get_storage_at(
                zksync_types::SYSTEM_CONTEXT_ADDRESS,
                h256_to_u256(zksync_types::SYSTEM_CONTEXT_CURRENT_L2_BLOCK_INFO_POSITION),
                None,
            )
            .map(|value| unpack_block_info(h256_to_u256(value)))
            .expect("failed getting l2 block info");
        assert_eq!(input_l2_block, actual_l2_block);
        assert_eq!(input_timestamp, actual_timestamp);

        let zero_missing_value = storage
            .get_storage_at(
                zksync_types::SYSTEM_CONTEXT_ADDRESS,
                h256_to_u256(H256::repeat_byte(0x1e)),
                None,
            )
            .map(|value| h256_to_u256(value).as_u64())
            .expect("failed missing value");
        assert_eq!(0, zero_missing_value);

        let actual_bytecode = storage
            .get_bytecode_by_hash(H256::repeat_byte(0x1))
            .ok()
            .expect("failed getting bytecode")
            .expect("missing bytecode");
        assert_eq!(input_bytecode, actual_bytecode);
    }
}
