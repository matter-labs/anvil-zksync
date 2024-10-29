//! Validation that zkSync Era In-Memory Node conforms to the official Ethereum Spec

use crate::tests::api::EraApi;
use crate::tests::patch::Patch;
use crate::tests::process;
use openrpc_types::resolved::{Method, OpenRPC};
use schemars::visit::Visitor;
use serde_json::json;
use zksync_basic_types::U256;

fn resolve_method_spec(method_name: &str) -> Method {
    // Load Ethereum OpenRPC spec
    // TODO: Add https://github.com/ethereum/execution-apis to git submodules and make sure that the
    // output file exists. Give user helpful guide on how to generate it if it doesn't:
    // npm install && npm run build.
    let openrpc: OpenRPC =
        serde_json::from_str(include_str!("../../../../execution-apis/openrpc.json")).unwrap();
    let method = openrpc
        .methods
        .into_iter()
        .find_map(|method| {
            if method.name == method_name {
                Some(method)
            } else {
                None
            }
        })
        .expect(&format!("method '{method_name}' not found"));
    method
}

#[tokio::test]
async fn validate_eth_get_block_genesis() -> anyhow::Result<()> {
    // Start era-test-node as an OS process with a randomly selected RPC port
    let node_handle = process::run_default().await?;
    // Connect to it via JSON-RPC API
    let era_api = EraApi::local(node_handle.config.rpc_port)?;

    // Resolve the method of interest from the official Ethereum Specification.
    // Assumes you have a locally built openrpc.json from https://github.com/ethereum/execution-apis
    // (see TODO in resolve_method_spec).
    let method = resolve_method_spec("eth_getBlockByNumber");
    // Resolve the expected result's JSON Schema (shoulb be self contained with no references).
    let mut result_schema = method.result.unwrap().schema;
    // Patch the schema with the **known** differences between Ethereum Specification and ZKsync.
    // In this case it is three extra fields relating to L1 batches and seal criteria.
    let mut patch = Patch::for_block();
    patch.visit_schema(&mut result_schema);
    // Build JSON Schema validator based on the resulting schema.
    let validator = jsonschema::options().build(&serde_json::to_value(result_schema)?)?;
    // Make a real request to the running era-test-node and get its response as a JSON value.
    let result = era_api
        .make_request("eth_getBlockByNumber", vec![json!("0x0"), json!(false)])
        .await?;
    // Validate the JSON response against the schema.
    validator.validate(&result).unwrap();

    Ok(())
}

// FIXME: Does not work yet, need to fix TX receipts
#[ignore]
#[test_log::test(tokio::test)]
async fn validate_eth_get_block_with_txs() -> anyhow::Result<()> {
    // Start era-test-node process
    let node_handle = process::run_default().await?;
    // Connect to it via JSON-RPC API
    let era_api = EraApi::local(node_handle.config.rpc_port)?;

    era_api.transfer_eth(U256::from("100")).await?;

    let method = resolve_method_spec("eth_getBlockByNumber");
    let mut result_schema = method.result.unwrap().schema;
    let mut patch = Patch::for_block();
    patch.visit_schema(&mut result_schema);
    let validator = jsonschema::options().build(&serde_json::to_value(result_schema)?)?;
    let result = era_api
        .make_request("eth_getBlockByNumber", vec![json!("0x1"), json!(false)])
        .await?;
    validator.validate(&result).unwrap();

    Ok(())
}
