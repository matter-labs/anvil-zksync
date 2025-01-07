use std::fs;

use anvil_zksync_core::node::InMemoryNode;
use eyre::Context;
use hex::FromHex;
use serde::Deserialize;
use std::str::FromStr;
use zksync_types::Address;

#[derive(Debug, Deserialize)]
struct ContractJson {
    bytecode: Bytecode,
}

#[derive(Debug, Deserialize)]
struct Bytecode {
    object: String,
}

// Loads a list of bytecodes and addresses from the directory and then inserts them directly
// into the Node's storage.
pub async fn override_bytecodes(node: &InMemoryNode, bytecodes_dir: String) -> eyre::Result<()> {
    for entry in fs::read_dir(bytecodes_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let filename = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name,
                None => eyre::bail!("Invalid filename {}", path.display().to_string()),
            };

            // Look only at .json files.
            if let Some(filename) = filename.strip_suffix(".json") {
                let address = Address::from_str(filename)
                    .wrap_err(format!("Cannot parse {} as address", filename))?;

                let file_content = fs::read_to_string(&path)?;
                let contract: ContractJson = serde_json::from_str(&file_content)
                    .wrap_err(format!("Failed to  parse json file {:?}", path))?;

                let bytecode = Vec::from_hex(contract.bytecode.object)
                    .wrap_err(format!("Failed to parse hex from {:?}", path))?;

                node.override_bytecode(&address, &bytecode)
                    .await
                    .expect("Failed to override bytecode");
                tracing::info!("+++++ Replacing bytecode at address {:?} +++++", address);
            }
        }
    }
    Ok(())
}
