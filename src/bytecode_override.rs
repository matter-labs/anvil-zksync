use std::fs;

use crate::fork::ForkSource;
use crate::node::InMemoryNode;
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

pub fn override_bytecodes<T: Clone + ForkSource + std::fmt::Debug>(
    node: &InMemoryNode<T>,
    bytecodes_dir: String,
) -> Result<(), anyhow::Error> {
    for entry in fs::read_dir(bytecodes_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let filename = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name,
                None => anyhow::bail!("Invalid filename {}", path.display().to_string()),
            };

            let filename = filename.strip_suffix(".json").unwrap();

            let address = Address::from_str(filename).expect(&format!("Cannot parse {}", filename));

            let file_content = fs::read_to_string(&path)?;
            let contract: ContractJson = serde_json::from_str(&file_content).unwrap();

            let bytecode = Vec::from_hex(contract.bytecode.object).unwrap();

            node.override_bytecode(&address, &bytecode).unwrap();
            tracing::info!("+++++ Replacing bytecode at address {:?} +++++", address);
        }
    }
    Ok(())
}
