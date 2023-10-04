//! This file hold testing helpers for other unit tests.
//!
//! There is MockServer that can help simulate a forked network.
//!

#![cfg(test)]

use crate::node::{InMemoryNode, TxExecutionInfo};
use crate::{fork::ForkSource, node::compute_hash};

use ethers::contract;
use httptest::{
    matchers::{eq, json_decoded, request},
    responders::json_encoded,
    Expectation, Server,
};
use itertools::Itertools;
use std::str::FromStr;
use vm::VmExecutionResultAndLogs;
use zksync_basic_types::{H160, U64};
use zksync_types::api::Log;
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
    let next_miniblock = node
        .get_inner()
        .read()
        .map(|reader| reader.current_miniblock.saturating_add(1))
        .expect("failed getting current batch number");
    let produced_block_hash = compute_hash(next_miniblock, tx_hash);

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

/// Deploys a contract with the given bytecode.
pub fn deploy_contract<T: ForkSource + std::fmt::Debug>(
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
        zksync_basic_types::L2ChainId(260),
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
            removed: Default::default(),
        }
    }
}

/// Simple storage solidity contract that stores and retrieves two numbers
///
/// contract Storage {
///     uint256 number1 = 1024;
///     uint256 number2 = 115792089237316195423570985008687907853269984665640564039457584007913129639935;   // uint256::max
///
///     function retrieve1() public view returns (uint256) {
///         return number1;
///     }
///
///     function retrieve2() public view returns (uint256) {
///         return number2;
///     }
/// }
pub const STORAGE_CONTRACT_BYTECODE: &str = "000200000000000200010000000103550000006001100270000000160010019d0000008001000039000000400010043f0000000101200190000000280000c13d0000000001000031000000040110008c000000460000413d0000000101000367000000000101043b000000e001100270000000180210009c000000350000613d000000190110009c000000460000c13d0000000001000416000000000101004b000000460000c13d000000040100008a00000000011000310000001a02000041000000000301004b000000000300001900000000030240190000001a01100197000000000401004b000000000200a0190000001a0110009c00000000010300190000000001026019000000000101004b000000460000c13d0000000101000039000000000101041a000000800010043f0000001c01000041000000520001042e0000000001000416000000000101004b000000460000c13d0000040001000039000000000010041b000000010100008a0000000102000039000000000012041b0000002001000039000001000010044300000120000004430000001701000041000000520001042e0000000001000416000000000101004b000000460000c13d000000040100008a00000000011000310000001a02000041000000000301004b000000000300001900000000030240190000001a01100197000000000401004b000000000200a0190000001a0110009c00000000010300190000000001026019000000000101004b000000480000613d00000000010000190000005300010430000000400100043d000000000200041a00000000002104350000001602000041000000160310009c000000000102801900000040011002100000001b011001c7000000520001042e0000005100000432000000520001042e0000005300010430000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff000000020000000000000000000000000000004000000100000000000000000000000000000000000000000000000000000000000000000000000000ae2e2cce000000000000000000000000000000000000000000000000000000002711432d80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000dec5bcecb8ee3456d9f70206a2d3c7fe5879354667a3a4b473afaff3d289dc8";

pub const STORAGE_TRACE_PRIMARY_BYTECODE: &str = "00030000000000020001000000000002000200000001035500000060011002700000004a0010019d000100000000001f0000000101200190000000490000c13d0000008001000039000000400010043f0000000001000031000000040110008c0000009e0000413d0000000201000367000000000101043b000000e001100270000000500210009c0000008d0000613d000000510110009c0000009e0000c13d0000000001000416000000000101004b0000009e0000c13d000000040100008a00000000011000310000004c02000041000000000301004b000000000300001900000000030240190000004c01100197000000000401004b000000000200a0190000004c0110009c00000000010300190000000001026019000000000101004b0000009e0000c13d000000c001000039000000400010043f0000000701000039000000800010043f0000005701000041000000a00010043f0000002002000039000000400100043d0000000003210436000000800200043d00000000002304350000004003100039000000000402004b0000003b0000613d00000000040000190000000005340019000000a006400039000000000606043300000000006504350000002004400039000000000524004b000000340000413d000000000332001900000000000304350000005f02200039000000200300008a000000000232016f0000004a030000410000004a0420009c00000000020380190000004a0410009c000000000103801900000040011002100000006002200210000000000112019f000001230001042e0000000001000416000000000101004b0000009e0000c13d00000000010000310000009f02100039000000200300008a000000000232016f0000004b0320009c000000580000413d000000560100004100000000001004350000004101000039000000040010043f00000053010000410000012400010430000000400020043f0000001f0210018f00000002030003670000000504100272000000660000613d00000000050000190000000506500210000000000763034f000000000707043b000000800660003900000000007604350000000105500039000000000645004b0000005e0000413d000000000502004b000000750000613d0000000504400210000000000343034f00000003022002100000008004400039000000000504043300000000052501cf000000000525022f000000000303043b0000010002200089000000000323022f00000000022301cf000000000252019f00000000002404350000004c02000041000000200310008c000000000300001900000000030240190000004c01100197000000000401004b000000000200a0190000004c0110009c00000000010300190000000001026019000000000101004b0000009e0000c13d000000800100043d0000004d0210009c0000009e0000213d000000000200041a0000004e02200197000000000112019f000000000010041b0000002001000039000001000010044300000120000004430000004f01000041000001230001042e0000000001000416000000000101004b0000009e0000c13d000000040100008a00000000011000310000004c02000041000000200310008c000000000300001900000000030240190000004c01100197000000000401004b000000000200a0190000004c0110009c00000000010300190000000001026019000000000101004b000000a00000613d00000000010000190000012400010430000000000200041a000000400a00043d000000520100004100000000001a043500000004010000390000000201100367000000000101043b0000000403a00039000000000013043500000000010004140000004d02200197000000040320008c000000b20000c13d0000000103000031000000200130008c00000020040000390000000004034019000000e20000013d0000004a030000410000004a0410009c00000000010380190000004a04a0009c00000000030a40190000004003300210000000c001100210000000000131019f00000053011001c700010000000a001d0122011d0000040f000000010a000029000000000301001900000060033002700000004a03300197000000200430008c000000200400003900000000040340190000001f0540018f0000000506400272000000d00000613d0000000007000019000000050870021000000000098a0019000000000881034f000000000808043b00000000008904350000000107700039000000000867004b000000c80000413d000000000705004b000000df0000613d0000000506600210000000000761034f00000000066a00190000000305500210000000000806043300000000085801cf000000000858022f000000000707043b0000010005500089000000000757022f00000000055701cf000000000585019f0000000000560435000100000003001f0000000102200190000000f70000613d0000001f01400039000000600210018f0000000001a20019000000000221004b00000000020000190000000102004039000000540410009c000000520000213d0000000102200190000000520000c13d000000400010043f000000200230008c0000009e0000413d00000000020a043300000000002104350000004a020000410000004a0310009c0000000001028019000000400110021000000055011001c7000001230001042e000000400200043d0000001f0430018f0000000503300272000001040000613d000000000500001900000005065002100000000007620019000000000661034f000000000606043b00000000006704350000000105500039000000000635004b000000fc0000413d000000000504004b000001130000613d0000000503300210000000000131034f00000000033200190000000304400210000000000503043300000000054501cf000000000545022f000000000101043b0000010004400089000000000141022f00000000014101cf000000000151019f00000000001304350000004a0100004100000001030000310000004a0430009c00000000030180190000004a0420009c000000000102401900000040011002100000006002300210000000000112019f000001240001043000000120002104210000000102000039000000000001042d0000000002000019000000000001042d0000012200000432000001230001042e000001240001043000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff00000000000000000000000000000000000000000000000100000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000000000000000000000000000000000020000000000000000000000000000004000000100000000000000000000000000000000000000000000000000000000000000000000000000b9d92de80000000000000000000000000000000000000000000000000000000006fdde03c6888fa1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000024000000000000000000000000000000000000000000000000000000000000000000000000ffffffffffffffff00000000000000000000000000000000000000200000000000000000000000004e487b71000000000000000000000000000000000000000000000000000000005072696d61727900000000000000000000000000000000000000000000000000cf40272a3c1f2b4fb6c7818f5725de9625a811b4c35176dd77458bb8597631c7";
pub const STORAGE_TRACE_SECONDARY_BYTECODE: &str = "0002000000000002000100000001035500000060011002700000002e0010019d0000000101200190000000470000c13d0000008001000039000000400010043f0000000001000031000000040110008c000000970000413d0000000101000367000000000101043b000000e001100270000000340210009c000000860000613d000000350110009c000000970000c13d0000000001000416000000000101004b000000970000c13d000000040100008a00000000011000310000003002000041000000000301004b000000000300001900000000030240190000003001100197000000000401004b000000000200a019000000300110009c00000000010300190000000001026019000000000101004b000000970000c13d000000c001000039000000400010043f0000000901000039000000800010043f0000003701000041000000a00010043f0000002002000039000000400100043d0000000003210436000000800200043d00000000002304350000004003100039000000000402004b000000390000613d00000000040000190000000005340019000000a006400039000000000606043300000000006504350000002004400039000000000524004b000000320000413d000000000332001900000000000304350000005f02200039000000200300008a000000000232016f0000002e030000410000002e0420009c00000000020380190000002e0410009c000000000103801900000040011002100000006002200210000000000112019f000000b30001042e0000000001000416000000000101004b000000970000c13d00000000010000310000009f02100039000000200300008a000000000232016f0000002f0320009c000000560000413d000000320100004100000000001004350000004101000039000000040010043f0000003301000041000000b400010430000000400020043f0000001f0210018f00000001030003670000000504100272000000640000613d00000000050000190000000506500210000000000763034f000000000707043b000000800660003900000000007604350000000105500039000000000645004b0000005c0000413d000000000502004b000000730000613d0000000504400210000000000343034f00000003022002100000008004400039000000000504043300000000052501cf000000000525022f000000000303043b0000010002200089000000000323022f00000000022301cf000000000252019f00000000002404350000003002000041000000200310008c000000000300001900000000030240190000003001100197000000000401004b000000000200a019000000300110009c00000000010300190000000001026019000000000101004b000000970000c13d000000800100043d000000000010041b0000002001000039000001000010044300000120000004430000003101000041000000b30001042e0000000001000416000000000101004b000000970000c13d000000040100008a00000000011000310000003002000041000000200310008c000000000300001900000000030240190000003001100197000000000401004b000000000200a019000000300110009c00000000010300190000000001026019000000000101004b000000990000613d0000000001000019000000b40001043000000004010000390000000101100367000000000201043b000000000300041a00000000412300a9000000000403004b000000a30000613d00000000433100d9000000000232004b000000ac0000c13d000000000010041b000000400200043d00000000001204350000002e010000410000002e0320009c0000000001024019000000400110021000000036011001c7000000b30001042e000000320100004100000000001004350000001101000039000000040010043f0000003301000041000000b400010430000000b200000432000000b30001042e000000b40001043000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff0000000000000000000000000000000000000000000000010000000000000000800000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000400000010000000000000000004e487b7100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002400000000000000000000000000000000000000000000000000000000000000000000000000000000c6888fa10000000000000000000000000000000000000000000000000000000006fdde0300000000000000000000000000000000000000200000000000000000000000005365636f6e6461727900000000000000000000000000000000000000000000001d03f8da4dfd31da347f55ce5a97529b23aa02f21e6f855b898d7b5d5090eb83";

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
            result: vm::ExecutionResult::Success { output: vec![] },
            logs: Default::default(),
            statistics: Default::default(),
            refunds: Default::default(),
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
            H256::from_str("0xd97ba6a5ab0f2d7fbfc697251321cce20bff3da2b0ddaf12c80f80f0ab270b15")
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
