use anvil_zksync_config::types::SystemContractsOptions;
use flate2::read::GzDecoder;
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use zksync_types::system_contracts::{
    get_system_smart_contracts, get_system_smart_contracts_from_dir,
};
use zksync_types::{
    block::DeployedContract, ProtocolVersionId, ACCOUNT_CODE_STORAGE_ADDRESS, BOOTLOADER_ADDRESS,
    BOOTLOADER_UTILITIES_ADDRESS, CODE_ORACLE_ADDRESS, COMPLEX_UPGRADER_ADDRESS,
    COMPRESSOR_ADDRESS, CONTRACT_DEPLOYER_ADDRESS, CREATE2_FACTORY_ADDRESS,
    ECRECOVER_PRECOMPILE_ADDRESS, EC_ADD_PRECOMPILE_ADDRESS, EC_MUL_PRECOMPILE_ADDRESS,
    EC_PAIRING_PRECOMPILE_ADDRESS, EVENT_WRITER_ADDRESS, EVM_GAS_MANAGER_ADDRESS,
    EVM_HASHES_STORAGE_ADDRESS, EVM_PREDEPLOYS_MANAGER_ADDRESS, IDENTITY_ADDRESS,
    IMMUTABLE_SIMULATOR_STORAGE_ADDRESS, KECCAK256_PRECOMPILE_ADDRESS, KNOWN_CODES_STORAGE_ADDRESS,
    L1_MESSENGER_ADDRESS, L2_ASSET_ROUTER_ADDRESS, L2_BASE_TOKEN_ADDRESS, L2_BRIDGEHUB_ADDRESS,
    L2_GENESIS_UPGRADE_ADDRESS, L2_MESSAGE_ROOT_ADDRESS, L2_NATIVE_TOKEN_VAULT_ADDRESS,
    L2_WRAPPED_BASE_TOKEN_IMPL, MODEXP_PRECOMPILE_ADDRESS, MSG_VALUE_SIMULATOR_ADDRESS,
    NONCE_HOLDER_ADDRESS, PUBDATA_CHUNK_PUBLISHER_ADDRESS, SECP256R1_VERIFY_PRECOMPILE_ADDRESS,
    SHA256_PRECOMPILE_ADDRESS, SLOAD_CONTRACT_ADDRESS, SYSTEM_CONTEXT_ADDRESS,
};
use zksync_types::{AccountTreeId, Address, H160};

pub const TIMESTAMP_ASSERTER_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x80, 0x80, 0x12,
]);

static BUILTIN_CONTRACT_ARCHIVES: [(ProtocolVersionId, &[u8]); 3] = [
    (
        ProtocolVersionId::Version26,
        include_bytes!("contracts/builtin-contracts-v26.tar.gz"),
    ),
    (
        ProtocolVersionId::Version27,
        include_bytes!("contracts/builtin-contracts-v27.tar.gz"),
    ),
    (
        ProtocolVersionId::Version28,
        include_bytes!("contracts/builtin-contracts-v28.tar.gz"),
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

const V26: ProtocolVersionId = ProtocolVersionId::Version26;
const V27: ProtocolVersionId = ProtocolVersionId::Version27;
const V28: ProtocolVersionId = ProtocolVersionId::Version28;

/// Triple containing a name of a contract, its L2 address and minimum supported protocol version
static BUILTIN_CONTRACT_LOCATIONS: [(&str, Address, ProtocolVersionId); 38] = [
    // *************************************************
    // *     Kernel contracts (base offset 0x8000)     *
    // *************************************************
    ("AccountCodeStorage", ACCOUNT_CODE_STORAGE_ADDRESS, V26),
    ("NonceHolder", NONCE_HOLDER_ADDRESS, V26),
    ("KnownCodesStorage", KNOWN_CODES_STORAGE_ADDRESS, V26),
    (
        "ImmutableSimulator",
        IMMUTABLE_SIMULATOR_STORAGE_ADDRESS,
        V26,
    ),
    ("ContractDeployer", CONTRACT_DEPLOYER_ADDRESS, V26),
    ("L1Messenger", L1_MESSENGER_ADDRESS, V26),
    ("MsgValueSimulator", MSG_VALUE_SIMULATOR_ADDRESS, V26),
    ("L2BaseToken", L2_BASE_TOKEN_ADDRESS, V26),
    ("SystemContext", SYSTEM_CONTEXT_ADDRESS, V26),
    ("BootloaderUtilities", BOOTLOADER_UTILITIES_ADDRESS, V26),
    ("EventWriter", EVENT_WRITER_ADDRESS, V26),
    ("Compressor", COMPRESSOR_ADDRESS, V26),
    ("ComplexUpgrader", COMPLEX_UPGRADER_ADDRESS, V26),
    (
        "PubdataChunkPublisher",
        PUBDATA_CHUNK_PUBLISHER_ADDRESS,
        V26,
    ),
    ("EvmGasManager", EVM_GAS_MANAGER_ADDRESS, V27),
    ("EvmPredeploysManager", EVM_PREDEPLOYS_MANAGER_ADDRESS, V27),
    ("EvmHashesStorage", EVM_HASHES_STORAGE_ADDRESS, V27),
    // *************************************************
    // *  Non-kernel contracts (base offset 0x010000)  *
    // *************************************************
    ("Create2Factory", CREATE2_FACTORY_ADDRESS, V26),
    ("L2GenesisUpgrade", L2_GENESIS_UPGRADE_ADDRESS, V26),
    ("Bridgehub", L2_BRIDGEHUB_ADDRESS, V26),
    ("L2AssetRouter", L2_ASSET_ROUTER_ADDRESS, V26),
    ("L2NativeTokenVault", L2_NATIVE_TOKEN_VAULT_ADDRESS, V26),
    ("MessageRoot", L2_MESSAGE_ROOT_ADDRESS, V26),
    ("SloadContract", SLOAD_CONTRACT_ADDRESS, V26),
    ("L2WrappedBaseToken", L2_WRAPPED_BASE_TOKEN_IMPL, V26),
    // *************************************************
    // *                 Precompiles                   *
    // *************************************************
    ("Keccak256", KECCAK256_PRECOMPILE_ADDRESS, V26),
    ("SHA256", SHA256_PRECOMPILE_ADDRESS, V26),
    ("Ecrecover", ECRECOVER_PRECOMPILE_ADDRESS, V26),
    ("EcAdd", EC_ADD_PRECOMPILE_ADDRESS, V26),
    ("EcMul", EC_MUL_PRECOMPILE_ADDRESS, V26),
    ("EcPairing", EC_PAIRING_PRECOMPILE_ADDRESS, V26),
    ("CodeOracle", CODE_ORACLE_ADDRESS, V26),
    ("P256Verify", SECP256R1_VERIFY_PRECOMPILE_ADDRESS, V26),
    ("Identity", IDENTITY_ADDRESS, V27),
    ("Modexp", MODEXP_PRECOMPILE_ADDRESS, V28),
    // TODO: It might make more sense to source address for these from zkstack config
    // *************************************************
    // *                L2 contracts                   *
    // *************************************************
    ("TimestampAsserter", TIMESTAMP_ASSERTER_ADDRESS, V26),
    // *************************************************
    // *               Empty contracts                 *
    // *************************************************
    // For now, only zero address and the bootloader address have empty bytecode at the init
    // In the future, we might want to set all of the system contracts this way.
    ("EmptyContract", Address::zero(), V26),
    ("EmptyContract", BOOTLOADER_ADDRESS, V26),
];

static BUILTIN_CONTRACTS: Lazy<HashMap<ProtocolVersionId, Vec<DeployedContract>>> =
    Lazy::new(|| {
        let mut result = HashMap::new();
        for (protocol_version, _) in BUILTIN_CONTRACT_ARCHIVES {
            result.insert(
                protocol_version,
                BUILTIN_CONTRACT_LOCATIONS
                    .iter()
                    .filter(|(_, _, min_version)| &protocol_version >= min_version)
                    .map(|(artifact_name, address, _)| DeployedContract {
                        account_id: AccountTreeId::new(*address),
                        bytecode: load_builtin_contract(protocol_version, artifact_name),
                    })
                    .collect(),
            );
        }
        result
    });

pub fn get_deployed_contracts(
    options: SystemContractsOptions,
    protocol_version: ProtocolVersionId,
    system_contracts_path: Option<&Path>,
) -> Vec<DeployedContract> {
    match options {
        SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
            BUILTIN_CONTRACTS
                .get(&protocol_version)
                .unwrap_or_else(|| panic!("protocol version '{protocol_version}' is not supported"))
                .clone()
        }
        SystemContractsOptions::Local => {
            // checks if system contracts path is provided
            if let Some(path) = system_contracts_path {
                get_system_smart_contracts_from_dir(path.to_path_buf())
            } else {
                get_system_smart_contracts()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_protocol_contracts(protocol_version: ProtocolVersionId) -> usize {
        BUILTIN_CONTRACT_LOCATIONS
            .iter()
            .filter(|(_, _, min_version)| &protocol_version >= min_version)
            .count()
    }

    #[test]
    fn load_v26_contracts() {
        let contracts = get_deployed_contracts(
            SystemContractsOptions::BuiltIn,
            ProtocolVersionId::Version26,
            None,
        );
        assert_eq!(
            contracts.len(),
            count_protocol_contracts(ProtocolVersionId::Version26)
        );
    }

    #[test]
    fn load_v27_contracts() {
        let contracts = get_deployed_contracts(
            SystemContractsOptions::BuiltIn,
            ProtocolVersionId::Version27,
            None,
        );
        assert_eq!(
            contracts.len(),
            count_protocol_contracts(ProtocolVersionId::Version27)
        );
    }

    #[test]
    fn load_v28_contracts() {
        let contracts = get_deployed_contracts(
            SystemContractsOptions::BuiltIn,
            ProtocolVersionId::Version28,
            None,
        );
        assert_eq!(
            contracts.len(),
            count_protocol_contracts(ProtocolVersionId::Version28)
        );
    }
}
