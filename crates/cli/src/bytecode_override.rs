use anvil_zksync_core::node::InMemoryNode;
use anyhow::Context;
use hex::FromHex;
use serde::Deserialize;
use std::str::FromStr;
use tokio::fs;
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
pub async fn override_bytecodes(node: &InMemoryNode, bytecodes_dir: String) -> anyhow::Result<()> {
    let mut read_dir = fs::read_dir(bytecodes_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let path = entry.path();

        if path.is_file() {
            let filename = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name,
                None => anyhow::bail!("Invalid filename {}", path.display().to_string()),
            };

            // Look only at .json files.
            if let Some(filename) = filename.strip_suffix(".json") {
                let address = Address::from_str(filename)
                    .with_context(|| format!("Cannot parse {} as address", filename))?;

                let file_content = fs::read_to_string(&path).await?;
                let contract: ContractJson = serde_json::from_str(&file_content)
                    .with_context(|| format!("Failed to  parse json file {:?}", path))?;

                let bytecode = Vec::from_hex(contract.bytecode.object)
                    .with_context(|| format!("Failed to parse hex from {:?}", path))?;

                node.override_bytecode(address, bytecode)
                    .await
                    .expect("Failed to override bytecode");
                println!("Replacing bytecode at address {:?}", address);
            }
        }
    }
    Ok(())
}
