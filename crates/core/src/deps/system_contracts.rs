use anvil_zksync_config::types::SystemContractsOptions;
use flate2::read::GzDecoder;
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use zksync_types::system_contracts::get_system_smart_contracts;
use zksync_types::{
    block::DeployedContract, ProtocolVersionId, ACCOUNT_CODE_STORAGE_ADDRESS, BOOTLOADER_ADDRESS,
    BOOTLOADER_UTILITIES_ADDRESS, CODE_ORACLE_ADDRESS, COMPLEX_UPGRADER_ADDRESS,
    COMPRESSOR_ADDRESS, CONTRACT_DEPLOYER_ADDRESS, CREATE2_FACTORY_ADDRESS,
    ECRECOVER_PRECOMPILE_ADDRESS, EC_ADD_PRECOMPILE_ADDRESS, EC_MUL_PRECOMPILE_ADDRESS,
    EC_PAIRING_PRECOMPILE_ADDRESS, EVENT_WRITER_ADDRESS, IMMUTABLE_SIMULATOR_STORAGE_ADDRESS,
    KECCAK256_PRECOMPILE_ADDRESS, KNOWN_CODES_STORAGE_ADDRESS, L1_MESSENGER_ADDRESS,
    L2_ASSET_ROUTER_ADDRESS, L2_BASE_TOKEN_ADDRESS, L2_BRIDGEHUB_ADDRESS,
    L2_GENESIS_UPGRADE_ADDRESS, L2_MESSAGE_ROOT_ADDRESS, L2_NATIVE_TOKEN_VAULT_ADDRESS,
    L2_WRAPPED_BASE_TOKEN_IMPL, MSG_VALUE_SIMULATOR_ADDRESS, NONCE_HOLDER_ADDRESS,
    PUBDATA_CHUNK_PUBLISHER_ADDRESS, SECP256R1_VERIFY_PRECOMPILE_ADDRESS,
    SHA256_PRECOMPILE_ADDRESS, SLOAD_CONTRACT_ADDRESS, SYSTEM_CONTEXT_ADDRESS,
};
use zksync_types::{AccountTreeId, Address, H160};

pub const TIMESTAMP_ASSERTER_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x80, 0x80, 0x12,
]);

static BUILTIN_CONTRACT_ARCHIVES: [(ProtocolVersionId, &[u8]); 2] = [
    (
        ProtocolVersionId::Version26,
        include_bytes!("contracts/builtin-contracts-v26.tar.gz"),
    ),
    (
        ProtocolVersionId::Version27,
        include_bytes!("contracts/builtin-contracts-v27.tar.gz"),
    ),
];

static BUILTIN_CONTRACT_ARTIFACTS: Lazy<HashMap<ProtocolVersionId, HashMap<String, Vec<u8>>>> =
    Lazy::new(|| {
        let mut result = HashMap::new();
        for (protocol_version, built_in_contracts) in BUILTIN_CONTRACT_ARCHIVES {
            let decoder = GzDecoder::new(built_in_contracts);
            let mut archive = tar::Archive::new(decoder);
            let mut contract_artifacts = HashMap::new();
            for file in archive
                .entries()
                .expect("failed to decompress built-in contracts")
            {
                let mut file = file.expect("failed to read a built-in contract entry");
                let path = file
                    .header()
                    .path()
                    .expect("contract path is malformed")
                    .file_name()
                    .expect("built-in contract entry does not have a filename")
                    .to_string_lossy()
                    .to_string();

                let mut contents = Vec::with_capacity(
                    file.header().size().expect("contract size is corrupted") as usize,
                );
                file.read_to_end(&mut contents).unwrap();
                contract_artifacts.insert(path, contents);
            }
            result.insert(protocol_version, contract_artifacts);
        }
        result
    });

pub fn bytecode_from_slice(artifact_name: &str, contents: &[u8]) -> Vec<u8> {
    let artifact: Value = serde_json::from_slice(contents).expect(artifact_name);
    let bytecode = artifact["bytecode"]
        .as_object()
        .unwrap_or_else(|| panic!("Bytecode not found in {:?}", artifact_name))
        .get("object")
        .unwrap_or_else(|| panic!("Bytecode object not found in {:?}", artifact_name))
        .as_str()
        .unwrap_or_else(|| panic!("Bytecode object is not a string in {:?}", artifact_name));

    hex::decode(bytecode)
        .unwrap_or_else(|err| panic!("Can't decode bytecode in {:?}: {}", artifact_name, err))
}

pub fn load_builtin_contract(protocol_version: ProtocolVersionId, artifact_name: &str) -> Vec<u8> {
    let artifact_path = format!("{artifact_name}.json");
    bytecode_from_slice(
        artifact_name,
        BUILTIN_CONTRACT_ARTIFACTS
            .get(&protocol_version)
            .unwrap_or_else(|| panic!("protocol version '{protocol_version}' is not supported"))
            .get(&artifact_path)
            .unwrap_or_else(|| {
                panic!("failed to find built-in contract artifact at '{artifact_path}'")
            }),
    )
}

static BUILTIN_CONTRACT_LOCATIONS: [(&str, Address); 33] = [
    // *************************************************
    // *     Kernel contracts (base offset 0x8000)     *
    // *************************************************
    ("AccountCodeStorage", ACCOUNT_CODE_STORAGE_ADDRESS),
    ("NonceHolder", NONCE_HOLDER_ADDRESS),
    ("KnownCodesStorage", KNOWN_CODES_STORAGE_ADDRESS),
    ("ImmutableSimulator", IMMUTABLE_SIMULATOR_STORAGE_ADDRESS),
    ("ContractDeployer", CONTRACT_DEPLOYER_ADDRESS),
    ("L1Messenger", L1_MESSENGER_ADDRESS),
    ("MsgValueSimulator", MSG_VALUE_SIMULATOR_ADDRESS),
    ("L2BaseToken", L2_BASE_TOKEN_ADDRESS),
    ("SystemContext", SYSTEM_CONTEXT_ADDRESS),
    ("BootloaderUtilities", BOOTLOADER_UTILITIES_ADDRESS),
    ("EventWriter", EVENT_WRITER_ADDRESS),
    ("Compressor", COMPRESSOR_ADDRESS),
    ("ComplexUpgrader", COMPLEX_UPGRADER_ADDRESS),
    ("PubdataChunkPublisher", PUBDATA_CHUNK_PUBLISHER_ADDRESS),
    // *************************************************
    // *  Non-kernel contracts (base offset 0x010000)  *
    // *************************************************
    ("Create2Factory", CREATE2_FACTORY_ADDRESS),
    ("L2GenesisUpgrade", L2_GENESIS_UPGRADE_ADDRESS),
    ("Bridgehub", L2_BRIDGEHUB_ADDRESS),
    ("L2AssetRouter", L2_ASSET_ROUTER_ADDRESS),
    ("L2NativeTokenVault", L2_NATIVE_TOKEN_VAULT_ADDRESS),
    ("MessageRoot", L2_MESSAGE_ROOT_ADDRESS),
    ("SloadContract", SLOAD_CONTRACT_ADDRESS),
    ("L2WrappedBaseToken", L2_WRAPPED_BASE_TOKEN_IMPL),
    // *************************************************
    // *                 Precompiles                   *
    // *************************************************
    ("Keccak256", KECCAK256_PRECOMPILE_ADDRESS),
    ("SHA256", SHA256_PRECOMPILE_ADDRESS),
    ("Ecrecover", ECRECOVER_PRECOMPILE_ADDRESS),
    ("EcAdd", EC_ADD_PRECOMPILE_ADDRESS),
    ("EcMul", EC_MUL_PRECOMPILE_ADDRESS),
    ("EcPairing", EC_PAIRING_PRECOMPILE_ADDRESS),
    ("CodeOracle", CODE_ORACLE_ADDRESS),
    ("P256Verify", SECP256R1_VERIFY_PRECOMPILE_ADDRESS),
    // TODO: It might make more sense to source address for these from zkstack config
    // *************************************************
    // *                L2 contracts                   *
    // *************************************************
    ("TimestampAsserter", TIMESTAMP_ASSERTER_ADDRESS),
    // *************************************************
    // *               Empty contracts                 *
    // *************************************************
    // For now, only zero address and the bootloader address have empty bytecode at the init
    // In the future, we might want to set all of the system contracts this way.
    ("EmptyContract", Address::zero()),
    ("EmptyContract", BOOTLOADER_ADDRESS),
];

static BUILTIN_CONTRACTS: Lazy<HashMap<ProtocolVersionId, Vec<DeployedContract>>> =
    Lazy::new(|| {
        let mut result = HashMap::new();
        for (protocol_version, _) in BUILTIN_CONTRACT_ARCHIVES {
            result.insert(
                protocol_version,
                BUILTIN_CONTRACT_LOCATIONS
                    .map(|(artifact_name, address)| DeployedContract {
                        account_id: AccountTreeId::new(address),
                        bytecode: load_builtin_contract(protocol_version, artifact_name),
                    })
                    .to_vec(),
            );
        }
        result
    });

pub fn get_deployed_contracts(
    options: SystemContractsOptions,
    protocol_version: ProtocolVersionId,
) -> Vec<DeployedContract> {
    match options {
        SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
            BUILTIN_CONTRACTS
                .get(&protocol_version)
                .unwrap_or_else(|| panic!("protocol version '{protocol_version}' is not supported"))
                .clone()
        }
        SystemContractsOptions::Local => get_system_smart_contracts(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_v26_contracts() {
        let contracts = get_deployed_contracts(
            SystemContractsOptions::BuiltIn,
            ProtocolVersionId::Version26,
        );
        assert_eq!(contracts.len(), BUILTIN_CONTRACT_LOCATIONS.len());
    }

    #[test]
    fn load_v27_contracts() {
        let contracts = get_deployed_contracts(
            SystemContractsOptions::BuiltIn,
            ProtocolVersionId::Version27,
        );
        assert_eq!(contracts.len(), BUILTIN_CONTRACT_LOCATIONS.len());
    }
}
