//! Validation that anvil-zksync conforms to the official Ethereum Spec

use anvil_zksync_spec_tests::process::AnvilZKsyncRunner;
use anvil_zksync_spec_tests::{EraApi, EthSpecPatch};
use jsonschema::Validator;
use openrpc_types::resolved::{Method, OpenRPC};
use schemars::visit::Visitor;
use serde_json::{json, Value};
use std::path::Path;
use zksync_basic_types::U256;

// We expect that the git submodule under root directory is initialized and has been built.
const ETH_OPENRPC_PATH: &str = "../execution-apis/openrpc.json";

fn resolve_method_spec(method_name: &str) -> anyhow::Result<Method> {
    let path = Path::new(ETH_OPENRPC_PATH);
    if !path.exists() {
        // TODO: Explore whether it would make sense to do it automatically for the user
        anyhow::bail!(
            "Expected ETH execution api OpenRPC spec to be available at '{}'. \
            Please make sure that git submodule is initialized and run \
            `npm install && npm run build` inside the submodule.",
            ETH_OPENRPC_PATH
        );
    }
    let openrpc: OpenRPC = serde_json::from_slice(&std::fs::read(path)?)?;
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
        .unwrap_or_else(|| panic!("method '{method_name}' not found"));
    Ok(method)
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
    // Start anvil-zksync as an OS process with a randomly selected RPC port
    let node_handle = AnvilZKsyncRunner::default().run().await?;
    // Connect to it via JSON-RPC API
    let era_api = EraApi::local(node_handle.config.rpc_port)?;

    // Resolve the method of interest from the official Ethereum Specification.
    // Assumes you have a locally built openrpc.json from https://github.com/ethereum/execution-apis
    // (see TODO in resolve_method_spec).
    let method = resolve_method_spec("eth_getBlockByNumber")?;
    // Resolve the expected result's JSON Schema (should be self-contained with no references).
    let mut result_schema = method.result.unwrap().schema;
    // Patch the schema with the **known** differences between Ethereum Specification and ZKsync.
    // In this case it is three extra fields relating to L1 batches and seal criteria.
    EthSpecPatch::for_block().visit_schema(&mut result_schema);
    // Build JSON Schema validator based on the resulting schema.
    let validator = jsonschema::options().build(&serde_json::to_value(result_schema)?)?;
    // Make a real request to the running anvil-zksync and get its response as a JSON value.
    let result = era_api
        .make_request("eth_getBlockByNumber", vec![json!("0x0"), json!(false)])
        .await?;
    // Validate the JSON response against the schema.
    validate_schema(validator, result);

    Ok(())
}

#[test_log::test(tokio::test)]
async fn validate_eth_get_block_with_txs_legacy() -> anyhow::Result<()> {
    let node_handle = AnvilZKsyncRunner::default().run().await?;
    let era_api = EraApi::local(node_handle.config.rpc_port)?;

    era_api.transfer_eth_legacy(U256::from("100")).await?;

    let method = resolve_method_spec("eth_getBlockByNumber")?;
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
