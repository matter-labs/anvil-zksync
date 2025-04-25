use alloy::primitives::{hex, Address};
use alloy::{sol, sol_types::SolCall};
use eyre::Result;

sol! {
    /// EVM-predeploy manager function
    function deployPredeployedContract(
        address contractAddress,
        bytes constructorInput
    ) external;
}

pub fn encode_predeploy_manager(addr: Address, ctor_hex: &str) -> Result<Vec<u8>> {
    let ctor_bytes = hex::decode(ctor_hex.trim_start_matches("0x"))?;

    let call = deployPredeployedContractCall {
        contractAddress: addr,
        constructorInput: ctor_bytes.into(),
    };

    Ok(call.abi_encode())
}
