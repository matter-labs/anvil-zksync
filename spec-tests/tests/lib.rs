//! Validation that zkSync Era In-Memory Node conforms to the official Ethereum Spec

use era_test_node_spec_tests::{process, EraApi, EthSpecPatch};
use jsonschema::Validator;
use openrpc_types::resolved::{Method, OpenRPC};
use schemars::visit::Visitor;
use serde_json::{json, Value};
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

/// Validate result against JSON Schema validator.
///
/// Prints all occurring errors instead of panicking on the first one. Asserts there are no errors
/// at the end of the flow.
fn validate_schema(validator: Validator, result: Value) {
    let errors = validator.iter_errors(&result).collect::<Vec<_>>();
    for err in &errors {
        eprintln!(
            "=== Validation error while validating instance at '{}' against schema at '{}':",
            err.instance_path, err.schema_path
        );
        eprintln!("{}", err);
    }
    assert!(
        errors.is_empty(),
        "There were JSON Schema validation errors, see above for the full list"
    );
}

#[test_log::test(tokio::test)]
async fn validate_eth_get_block_genesis() -> anyhow::Result<()> {
    // Start era-test-node as an OS process with a randomly selected RPC port
    let node_handle = process::run_default().await?;
    // Connect to it via JSON-RPC API
    let era_api = EraApi::local(node_handle.config.rpc_port)?;

    // Resolve the method of interest from the official Ethereum Specification.
    // Assumes you have a locally built openrpc.json from https://github.com/ethereum/execution-apis
    // (see TODO in resolve_method_spec).
    let method = resolve_method_spec("eth_getBlockByNumber");
    // Resolve the expected result's JSON Schema (should be self-contained with no references).
    let mut result_schema = method.result.unwrap().schema;
    // Patch the schema with the **known** differences between Ethereum Specification and ZKsync.
    // In this case it is three extra fields relating to L1 batches and seal criteria.
    EthSpecPatch::for_block().visit_schema(&mut result_schema);
    // Build JSON Schema validator based on the resulting schema.
    let validator = jsonschema::options().build(&serde_json::to_value(result_schema)?)?;
    // Make a real request to the running era-test-node and get its response as a JSON value.
    let result = era_api
        .make_request("eth_getBlockByNumber", vec![json!("0x0"), json!(false)])
        .await?;
    // Validate the JSON response against the schema.
    validate_schema(validator, result);

    Ok(())
}

#[test_log::test(tokio::test)]
async fn validate_eth_get_block_with_txs() -> anyhow::Result<()> {
    let node_handle = process::run_default().await?;
    let era_api = EraApi::local(node_handle.config.rpc_port)?;

    era_api.transfer_eth_legacy(U256::from("100")).await?;

    let method = resolve_method_spec("eth_getBlockByNumber");
    let mut result_schema = method.result.unwrap().schema;
    EthSpecPatch::for_block().visit_schema(&mut result_schema);
    EthSpecPatch::for_tx_info().visit_schema(&mut result_schema);
    EthSpecPatch::for_legacy_tx().visit_schema(&mut result_schema);
    let validator = jsonschema::options().build(&serde_json::to_value(result_schema)?)?;
    let result = era_api
        .make_request("eth_getBlockByNumber", vec![json!("0x1"), json!(true)])
        .await?;
    // Asserts there is at least one transaction in the block
    assert!(!result
        .get("transactions")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    validate_schema(validator, result);

    Ok(())
}
