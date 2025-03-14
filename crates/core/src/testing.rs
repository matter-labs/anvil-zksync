//! This file hold testing helpers for other unit tests.
//!
//! There is MockServer that can help simulate a forked network.
//!

#![cfg(test)]

use crate::node::{InMemoryNode, TxBatch, TxExecutionInfo};

use anvil_zksync_config::constants::DEFAULT_ACCOUNT_BALANCE;
use httptest::{
    matchers::{eq, json_decoded, request},
    responders::json_encoded,
    Expectation, Server,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use url::Url;
use zksync_types::api::{BridgeAddresses, DebugCall, DebugCallType, Log, TransactionRequest};
use zksync_types::bytecode::BytecodeHash;
use zksync_types::fee::Fee;
use zksync_types::l2::L2Tx;
use zksync_types::{
    Address, ExecuteTransactionCommon, K256PrivateKey, L2BlockNumber, L2ChainId, Nonce,
    PackedEthSignature, ProtocolVersionId, Transaction, H160, H256, U256, U64,
};
use zksync_web3_decl::jsonrpsee::types::TwoPointZero;

/// Configuration for the [MockServer]'s initial block.
#[derive(Default, Debug, Clone)]
pub struct ForkBlockConfig {
    pub number: u64,
    pub hash: H256,
    pub transaction_count: u8,
}

#[derive(Serialize, Deserialize, Debug)]
struct RpcRequest {
    pub jsonrpc: TwoPointZero,
    pub id: u64,
    pub method: String,
    pub params: Option<serde_json::Value>,
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
                "method": "eth_chainId",
            })))))
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x104",
            }))),
        );
        server.expect(
            Expectation::matching(request::body(json_decoded(eq(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "zks_getBlockDetails",
                "params": [ block_config.number ],
            })))))
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
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
                "id": 3,
                "method": "eth_getBlockByHash",
                "params": [format!("{:#x}", block_config.hash), true],
            }))))).times(0..)
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
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
                "method": "zks_getFeeParams",
            })))))
            .respond_with(json_encoded(serde_json::json!(
            {
              "jsonrpc": "2.0",
              "result": {
                "V2": {
                  "config": {
                    "minimal_l2_gas_price": 25000000,
                    "compute_overhead_part": 0,
                    "pubdata_overhead_part": 1,
                    "batch_overhead_l1_gas": 800000,
                    "max_gas_per_batch": 200000000,
                    "max_pubdata_per_batch": 240000
                  },
                  "l1_gas_price": 46226388803u64,
                  "l1_pubdata_price": 100780475095u64,
                  "conversion_ratio": {
                    "numerator": 1,
                    "denominator": 1
                  }
                }
              },
              "id": 4
            }))),
        );

        MockServer { inner: server }
    }

    /// Retrieve the mock server's url.
    pub fn url(&self) -> Url {
        self.inner.url("").to_string().parse().unwrap()
    }

    /// Assert an exactly single call expectation with a given request and the provided response.
    pub fn expect(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
        result: serde_json::Value,
    ) {
        let method = method.to_string();
        let id_matcher = Arc::new(RwLock::new(0));
        let id_matcher_clone = id_matcher.clone();
        self.inner.expect(
            Expectation::matching(request::body(json_decoded(move |request: &RpcRequest| {
                let result = request.method == method && request.params == params;
                if result {
                    let mut writer = id_matcher.write().unwrap();
                    *writer = request.id;
                }
                result
            })))
            .respond_with(move || {
                let id = *id_matcher_clone.read().unwrap();
                json_encoded(serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": result,
                    "id": id
                }))
            }),
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
    from_account_private_key: K256PrivateKey,
    gas_limit: U256,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self {
            from_account_private_key: K256PrivateKey::from_bytes(H256::random()).unwrap(),
            gas_limit: U256::from(4_000_000),
            max_fee_per_gas: U256::from(50_000_000),
            max_priority_fee_per_gas: U256::from(50_000_000),
        }
    }
}

impl TransactionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn deploy_contract(
        private_key: &K256PrivateKey,
        bytecode: Vec<u8>,
        calldata: Option<Vec<u8>>,
        nonce: Nonce,
    ) -> L2Tx {
        use alloy::dyn_abi::{DynSolValue, JsonAbiExt};
        use alloy::json_abi::{Function, Param, StateMutability};

        let salt = [0u8; 32];
        let bytecode_hash = BytecodeHash::for_bytecode(&bytecode).value().0;
        let call_data = calldata.unwrap_or_default();

        let create = Function {
            name: "create".to_string(),
            inputs: vec![
                Param {
                    name: "_salt".to_string(),
                    ty: "bytes32".to_string(),
                    components: vec![],
                    internal_type: None,
                },
                Param {
                    name: "_bytecodeHash".to_string(),
                    ty: "bytes32".to_string(),
                    components: vec![],
                    internal_type: None,
                },
                Param {
                    name: "_input".to_string(),
                    ty: "bytes".to_string(),
                    components: vec![],
                    internal_type: None,
                },
            ],
            outputs: vec![Param {
                name: "".to_string(),
                ty: "address".to_string(),
                components: vec![],
                internal_type: None,
            }],
            state_mutability: StateMutability::Payable,
        };

        let data = create
            .abi_encode_input(&[
                DynSolValue::FixedBytes(salt.into(), salt.len()),
                DynSolValue::FixedBytes(
                    bytecode_hash[..].try_into().expect("invalid hash length"),
                    bytecode_hash.len(),
                ),
                DynSolValue::Bytes(call_data),
            ])
            .expect("failed to encode function data");

        L2Tx::new_signed(
            Some(zksync_types::CONTRACT_DEPLOYER_ADDRESS),
            data.to_vec(),
            nonce,
            Fee {
                gas_limit: U256::from(400_000_000),
                max_fee_per_gas: U256::from(50_000_000),
                max_priority_fee_per_gas: U256::from(50_000_000),
                gas_per_pubdata_limit: U256::from(50000),
            },
            U256::from(0),
            zksync_types::L2ChainId::from(260),
            private_key,
            vec![bytecode],
            Default::default(),
        )
        .expect("failed signing tx")
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
        L2Tx::new_signed(
            Some(Address::random()),
            vec![],
            Nonce(0),
            Fee {
                gas_limit: self.gas_limit,
                max_fee_per_gas: self.max_fee_per_gas,
                max_priority_fee_per_gas: self.max_priority_fee_per_gas,
                gas_per_pubdata_limit: U256::from(50000),
            },
            U256::from(1),
            L2ChainId::from(260),
            &self.from_account_private_key,
            vec![],
            Default::default(),
        )
        .unwrap()
    }

    pub fn impersonate(&mut self, to_impersonate: Address) -> L2Tx {
        let mut tx = L2Tx::new(
            Some(Address::random()),
            vec![],
            Nonce(0),
            Fee {
                gas_limit: self.gas_limit,
                max_fee_per_gas: self.max_fee_per_gas,
                max_priority_fee_per_gas: self.max_priority_fee_per_gas,
                gas_per_pubdata_limit: U256::from(50000),
            },
            to_impersonate,
            U256::one(),
            vec![],
            Default::default(),
        );

        let mut req: TransactionRequest = tx.clone().into();
        req.chain_id = Some(260);
        let data = req.get_default_signed_message().unwrap();
        let sig = PackedEthSignature::sign_raw(&K256PrivateKey::random(), &data).unwrap();
        let raw = req.get_signed_bytes(&sig).unwrap();
        let (_, hash) = TransactionRequest::from_bytes_unverified(&raw).unwrap();

        tx.set_input(vec![], hash);
        tx.common_data.signature = sig.serialize_packed().into();
        tx
    }
}

/// Applies a transaction with a given hash to the node and returns the block hash.
pub async fn apply_tx(node: &InMemoryNode) -> (H256, L2BlockNumber, L2Tx) {
    let tx = TransactionBuilder::new().build();
    node.set_rich_account(tx.initiator_account(), U256::from(DEFAULT_ACCOUNT_BALANCE))
        .await;
    let block_number = node
        .node_handle
        .seal_block_sync(TxBatch {
            txs: vec![tx.clone().into()],
            impersonating: false,
        })
        .await
        .unwrap();

    let block_hash = node
        .blockchain
        .get_block_hash_by_number(block_number)
        .await
        .unwrap();

    (block_hash, block_number, tx)
}

/// Deploys a contract with the given bytecode.
pub async fn deploy_contract(
    node: &InMemoryNode,
    private_key: &K256PrivateKey,
    bytecode: Vec<u8>,
    calldata: Option<Vec<u8>>,
    nonce: Nonce,
) -> H256 {
    let tx = TransactionBuilder::deploy_contract(private_key, bytecode, calldata, nonce);
    let block_number = node
        .node_handle
        .seal_block_sync(TxBatch {
            txs: vec![tx.into()],
            impersonating: false,
        })
        .await
        .unwrap();

    node.blockchain
        .get_block_hash_by_number(block_number)
        .await
        .unwrap()
}

/// Builds transaction logs
#[derive(Debug, Default, Clone)]
pub struct LogBuilder {
    block_number: U64,
    block_timestamp: U64,
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
            block_timestamp: Some(self.block_timestamp),
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
        tx: Transaction {
            common_data: ExecuteTransactionCommon::L2(Default::default()),
            execute: Default::default(),
            received_timestamp_ms: 0,
            raw_bytes: None,
        },
        batch_number: Default::default(),
        miniblock_number: Default::default(),
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

mod test {
    use super::*;
    use crate::node::compute_hash;
    use zksync_types::block::L2BlockHasher;

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
        let node = InMemoryNode::test(None);
        let (actual_block_hash, actual_block_number, tx) = apply_tx(&node).await;

        assert_eq!(
            compute_hash(
                L2BlockNumber(1),
                1001,
                L2BlockHasher::legacy_hash(L2BlockNumber(0)),
                [&tx.hash()]
            ),
            actual_block_hash,
        );
        assert_eq!(L2BlockNumber(1), actual_block_number);

        assert!(
            node.blockchain
                .get_block_by_hash(&actual_block_hash)
                .await
                .is_some(),
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
