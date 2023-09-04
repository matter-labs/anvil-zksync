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

/// A HTTP server that can be used to mock a fork source.
pub struct MockServer {
    /// The implementation for [httptest::Server].
    pub inner: Server,
}

impl MockServer {
    /// Start the mock server with pre-defined calls used to fetch the fork's state.
    pub fn run() -> Self {
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
                "result": "0xa",
            }))),
        );
        server.expect(
            Expectation::matching(request::body(json_decoded(eq(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "zks_getBlockDetails",
                "params": vec![ 10 ],
            })))))
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "number": 10,
                    "l1BatchNumber": 5,
                    "timestamp": 1676461082u64,
                    "l1TxCount": 0,
                    "l2TxCount": 0,
                    "rootHash": "0x086c9487350539c884510044efce5e3f2aaffca4215c12b9044506375097fecd",
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
                "id": 0,
                "method": "eth_getStorageAt",
                "params": vec!["0x000000000000000000000000000000000000800a","0xe9472b134a1b5f7b935d5debff2691f95801214eafffdeabbf0e366da383104e","0xa"],
            }))))).times(0..)
            .respond_with(json_encoded(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
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
