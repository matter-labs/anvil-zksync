use std::collections::HashMap;

use lazy_static::lazy_static;
use serde::Deserialize;
use zksync_types::{Address, H160};

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum ContractType {
    System,
    Precompile,
    Popular,
    Unknown,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KnownAddress {
    pub address: H160,
    pub name: String,
    pub contract_type: ContractType,
}

lazy_static! {
    /// Loads the known contact addresses from the JSON file.
    pub static ref KNOWN_ADDRESSES: HashMap<H160, KnownAddress> = {
        let json_value = serde_json::from_slice(include_bytes!("./data/address_map.json")).unwrap();
        let pairs: Vec<KnownAddress> = serde_json::from_value(json_value).unwrap();

        pairs
            .into_iter()
            .map(|entry| (entry.address, entry))
            .collect()
    };
}

/// Checks if the given address is a precompile based on `KNOWN_ADDRESSES`.
pub fn is_precompile(address: &Address) -> bool {
    if let Some(known) = KNOWN_ADDRESSES.get(address) {
        matches!(known.contract_type, ContractType::Precompile)
    } else {
        false
    }
}

/// Checks if the given address is a system contract based on `KNOWN_ADDRESSES`.
pub fn is_system(address: &Address) -> bool {
    if let Some(known) = KNOWN_ADDRESSES.get(address) {
        matches!(known.contract_type, ContractType::System)
    } else {
        false
    }
}
