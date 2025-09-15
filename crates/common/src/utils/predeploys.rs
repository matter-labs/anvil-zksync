use alloy::primitives::{Address, hex};
use alloy::{sol, sol_types::SolCall};
use eyre::{Result, eyre};
use once_cell::sync::Lazy;
use serde::Deserialize;

/// Pre-deploy contract data
pub static PREDEPLOYS: Lazy<Vec<Predeploy>> = Lazy::new(|| {
    const RAW: &str = include_str!("../data/predeploys.json");
    serde_json::from_str(RAW).expect("invalid predeploys.json")
});

#[derive(Debug, Deserialize)]
pub struct Predeploy {
    pub address: Address,
    pub constructor_input: String,
}

sol! {
    /// EVM‐predeploy manager function
    function deployPredeployedContract(
        address contractAddress,
        bytes constructorInput
    ) external;
}

impl Predeploy {
    pub fn encode_manager_call(&self) -> Result<Vec<u8>> {
        let ctor_bytes =
            hex::decode(self.constructor_input.trim_start_matches("0x")).map_err(|e| {
                eyre!(
                    "invalid hex in constructor_input for {}: {}",
                    self.address,
                    e
                )
            })?;

        let call = deployPredeployedContractCall {
            contractAddress: self.address,
            constructorInput: ctor_bytes.into(),
        };

        Ok(call.abi_encode())
    }
}
