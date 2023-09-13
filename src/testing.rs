//! This file hold testing helpers for other unit tests.
//!
//! There is MockServer that can help simulate a forked network.
//!

#![cfg(test)]

use httptest::{
    matchers::{eq, json_decoded, request},
    responders::json_encoded,
    Expectation, Server,
};
use zksync_basic_types::H256;

/// A HTTP server that can be used to mock a fork source.
pub struct MockServer {
    /// The implementation for [httptest::Server].
    pub inner: Server,
}

impl MockServer {
    /// Start the mock server with pre-defined calls used to fetch the fork's state.
    pub fn run() -> Self {
        Self::run_with_config(10, H256::repeat_byte(0xab))
    }

    /// Start the mock server with pre-defined calls used to fetch the fork's state.
    /// The input can be used to set the initial block's number and hash.
    pub fn run_with_config(block_number: u64, block_hash: H256) -> Self {
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
                "result": format!("{:#x}", block_number),
            }))),
        );
        server.expect(
            Expectation::matching(request::body(json_decoded(eq(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "zks_getBlockDetails",
                "params": [ block_number ],
            })))))
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "number": block_number,
                    "l1BatchNumber": 1,
                    "timestamp": 1676461082u64,
                    "l1TxCount": 0,
                    "l2TxCount": 0,
                    "rootHash": format!("{:#x}", block_hash),
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
                    "protocolVersion": null
                  },
            }))),
        );
        server.expect(
            Expectation::matching(request::body(json_decoded(eq(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "eth_getBlockByHash",
                "params": [format!("{:#x}", block_hash), true],
            }))))).times(0..)
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {
                    "hash": format!("{:#x}", block_hash),
                    "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                    "miner": "0x0000000000000000000000000000000000000000",
                    "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "transactionsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "receiptsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "number": format!("{:#x}", block_number),
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

    /// Builds the json response
    pub fn build(&mut self) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": {
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
            },
        })
    }
}

mod test {
    use super::*;

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
}
