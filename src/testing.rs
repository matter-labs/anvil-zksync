//! This file hold testing helpers for other unit tests.
//!
//! There is MockServer that can help simulate a forked network.
//!

#![cfg(test)]

use crate::node::{InMemoryNode, TxExecutionInfo};
use crate::{fork::ForkSource, node::compute_hash};

use httptest::{
    matchers::{eq, json_decoded, request},
    responders::json_encoded,
    Expectation, Server,
};
use itertools::Itertools;
use std::str::FromStr;
use vm::vm::{VmPartialExecutionResult, VmTxExecutionResult};
use zksync_basic_types::{H160, U64};
use zksync_types::api::Log;
use zksync_types::tx::tx_execution_info::TxExecutionStatus;
use zksync_types::{
    fee::Fee, l2::L2Tx, Address, L2ChainId, Nonce, PackedEthSignature, ProtocolVersionId, H256,
    U256,
};

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
                    "protocolVersion": ProtocolVersionId::latest(),
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
        server.expect(
            Expectation::matching(request::body(json_decoded(eq(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "eth_getStorageAt",
                "params": vec!["0x000000000000000000000000000000000000800a","0xe9472b134a1b5f7b935d5debff2691f95801214eafffdeabbf0e366da383104e","0xa"],
            }))))).times(0..)
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "result": "0x0000000000000000000000000000000000000000000000000000000000000000",
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
}

impl TransactionResponseBuilder {
    /// Create a new instance of [TransactionResponseBuilder]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the block hash
    pub fn set_hash(&mut self, hash: H256) -> &mut Self {
        self.hash = hash;
        self
    }

    /// Builds the transaction json result
    pub fn build_result(&mut self) -> serde_json::Value {
        serde_json::json!({
            "hash": format!("{:#x}", self.hash),
            "nonce": "0x0",
            "blockHash": "0x51f81bcdfc324a0dff2b5bec9d92e21cbebc4d5e29d3a3d30de3e03fbeab8d7f",
            "blockNumber": "0x1",
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
            "chainId": "0x144",
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

/// Applies a transaction with a given hash to the node and returns the block hash.
pub fn apply_tx<T: ForkSource + std::fmt::Debug>(node: &InMemoryNode<T>, tx_hash: H256) -> H256 {
    let current_batch = node
        .get_inner()
        .read()
        .map(|reader| reader.current_batch)
        .expect("failed getting current batch number");
    let produced_block_hash = compute_hash(current_batch, tx_hash);

    let private_key = H256::random();
    let from_account = PackedEthSignature::address_from_private_key(&private_key)
        .expect("failed generating address");
    node.set_rich_account(from_account);
    let mut tx = L2Tx::new_signed(
        Address::random(),
        vec![],
        Nonce(0),
        Fee {
            gas_limit: U256::from(1_000_000),
            max_fee_per_gas: U256::from(250_000_000),
            max_priority_fee_per_gas: U256::from(250_000_000),
            gas_per_pubdata_limit: U256::from(20000),
        },
        U256::from(1),
        L2ChainId(260),
        &private_key,
        None,
        Default::default(),
    )
    .unwrap();
    tx.set_input(vec![], tx_hash);
    node.apply_txs(vec![tx]).expect("failed applying tx");

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
            address: self.address.clone().unwrap_or_default(),
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
            removed: Default::default(),
        }
    }
}

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
        },
        batch_number: Default::default(),
        miniblock_number: Default::default(),
        result: VmTxExecutionResult {
            status: TxExecutionStatus::Success,
            result: VmPartialExecutionResult {
                logs: Default::default(),
                revert_reason: Default::default(),
                contracts_used: Default::default(),
                cycles_used: Default::default(),
                computational_gas_used: Default::default(),
            },
            call_traces: Default::default(),
            gas_refunded: Default::default(),
            operator_suggested_refund: Default::default(),
        },
    }
}

mod test {
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
        let actual_block_hash = apply_tx(&node, H256::repeat_byte(0x01));

        assert_eq!(
            H256::from_str("0x89c0aa770eba1f187235bdad80de9c01fe81bca415d442ca892f087da56fa109")
                .unwrap(),
            actual_block_hash,
        );

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
}
