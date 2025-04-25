use alloy::primitives::Address;
use once_cell::sync::Lazy;
use serde::Deserialize;

/// Pre-deploy contract data
pub static PREDEPLOYS: Lazy<Vec<Predeploy>> = Lazy::new(|| {
    const RAW: &str = include_str!("../../common/src/data/predeploys.json");
    serde_json::from_str(RAW).expect("invalid predeploys.json")
});

#[derive(Debug, Deserialize)]
pub struct Predeploy {
    pub address: Address,
    pub constructor_input: String,
}
