use anvil_zksync_config::types::SystemContractsOptions;
use once_cell::sync::Lazy;
use serde_json::Value;
use zksync_types::system_contracts::get_system_smart_contracts;
use zksync_types::{
    block::DeployedContract, ACCOUNT_CODE_STORAGE_ADDRESS, BOOTLOADER_ADDRESS,
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

pub static COMPILED_IN_SYSTEM_CONTRACTS: Lazy<Vec<DeployedContract>> = Lazy::new(|| {
    let mut deployed_system_contracts = [
        // *************************************************
        // *     Kernel contracts (base offset 0x8000)     *
        // *************************************************
        (
            "AccountCodeStorage",
            ACCOUNT_CODE_STORAGE_ADDRESS,
            include_bytes!("contracts/AccountCodeStorage.json").to_vec(),
        ),
        (
            "NonceHolder",
            NONCE_HOLDER_ADDRESS,
            include_bytes!("contracts/NonceHolder.json").to_vec(),
        ),
        (
            "KnownCodesStorage",
            KNOWN_CODES_STORAGE_ADDRESS,
            include_bytes!("contracts/KnownCodesStorage.json").to_vec(),
        ),
        (
            "ImmutableSimulator",
            IMMUTABLE_SIMULATOR_STORAGE_ADDRESS,
            include_bytes!("contracts/ImmutableSimulator.json").to_vec(),
        ),
        (
            "ContractDeployer",
            CONTRACT_DEPLOYER_ADDRESS,
            include_bytes!("contracts/ContractDeployer.json").to_vec(),
        ),
        (
            "L1Messenger",
            L1_MESSENGER_ADDRESS,
            include_bytes!("contracts/L1Messenger.json").to_vec(),
        ),
        (
            "MsgValueSimulator",
            MSG_VALUE_SIMULATOR_ADDRESS,
            include_bytes!("contracts/MsgValueSimulator.json").to_vec(),
        ),
        (
            "L2BaseToken",
            L2_BASE_TOKEN_ADDRESS,
            include_bytes!("contracts/L2BaseToken.json").to_vec(),
        ),
        (
            "SystemContext",
            SYSTEM_CONTEXT_ADDRESS,
            include_bytes!("contracts/SystemContext.json").to_vec(),
        ),
        (
            "BootloaderUtilities",
            BOOTLOADER_UTILITIES_ADDRESS,
            include_bytes!("contracts/BootloaderUtilities.json").to_vec(),
        ),
        (
            "EventWriter",
            EVENT_WRITER_ADDRESS,
            include_bytes!("contracts/EventWriter.json").to_vec(),
        ),
        (
            "Compressor",
            COMPRESSOR_ADDRESS,
            include_bytes!("contracts/Compressor.json").to_vec(),
        ),
        (
            "ComplexUpgrader",
            COMPLEX_UPGRADER_ADDRESS,
            include_bytes!("contracts/ComplexUpgrader.json").to_vec(),
        ),
        (
            "PubdataChunkPublisher",
            PUBDATA_CHUNK_PUBLISHER_ADDRESS,
            include_bytes!("contracts/PubdataChunkPublisher.json").to_vec(),
        ),
        // *************************************************
        // *  Non-kernel contracts (base offset 0x010000)  *
        // *************************************************
        (
            "Create2Factory",
            CREATE2_FACTORY_ADDRESS,
            include_bytes!("contracts/Create2Factory.json").to_vec(),
        ),
        (
            "L2GenesisUpgrade",
            L2_GENESIS_UPGRADE_ADDRESS,
            include_bytes!("contracts/L2GenesisUpgrade.json").to_vec(),
        ),
        (
            "Bridgehub",
            L2_BRIDGEHUB_ADDRESS,
            include_bytes!("contracts/Bridgehub.json").to_vec(),
        ),
        (
            "L2AssetRouter",
            L2_ASSET_ROUTER_ADDRESS,
            include_bytes!("contracts/L2AssetRouter.json").to_vec(),
        ),
        (
            "L2NativeTokenVault",
            L2_NATIVE_TOKEN_VAULT_ADDRESS,
            include_bytes!("contracts/L2NativeTokenVault.json").to_vec(),
        ),
        (
            "MessageRoot",
            L2_MESSAGE_ROOT_ADDRESS,
            include_bytes!("contracts/MessageRoot.json").to_vec(),
        ),
        (
            "SloadContract",
            SLOAD_CONTRACT_ADDRESS,
            include_bytes!("contracts/SloadContract.json").to_vec(),
        ),
        (
            "L2WrappedBaseToken",
            L2_WRAPPED_BASE_TOKEN_IMPL,
            include_bytes!("contracts/L2WrappedBaseToken.json").to_vec(),
        ),
        // *************************************************
        // *                 Precompiles                   *
        // *************************************************
        (
            "Keccak256",
            KECCAK256_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/Keccak256.json").to_vec(),
        ),
        (
            "SHA256",
            SHA256_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/SHA256.json").to_vec(),
        ),
        (
            "Ecrecover",
            ECRECOVER_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/Ecrecover.json").to_vec(),
        ),
        (
            "EcAdd",
            EC_ADD_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/EcAdd.json").to_vec(),
        ),
        (
            "EcMul",
            EC_MUL_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/EcMul.json").to_vec(),
        ),
        (
            "EcPairing",
            EC_PAIRING_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/EcPairing.json").to_vec(),
        ),
        (
            "CodeOracle",
            CODE_ORACLE_ADDRESS,
            include_bytes!("contracts/CodeOracle.json").to_vec(),
        ),
        (
            "P256Verify",
            SECP256R1_VERIFY_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/P256Verify.json").to_vec(),
        ),
        // TODO: It might make more sense to source address for these from zkstack config
        // *************************************************
        // *                L2 contracts                   *
        // *************************************************
        (
            "TimestampAsserter",
            TIMESTAMP_ASSERTER_ADDRESS,
            include_bytes!("contracts/TimestampAsserter.json").to_vec(),
        ),
    ]
    .map(|(pname, address, contents)| DeployedContract {
        account_id: AccountTreeId::new(address),
        bytecode: bytecode_from_slice(pname, &contents),
    })
    .to_vec();

    let empty_bytecode = bytecode_from_slice(
        "EmptyContract",
        include_bytes!("contracts/EmptyContract.json"),
    );
    // For now, only zero address and the bootloader address have empty bytecode at the init
    // In the future, we might want to set all of the system contracts this way.
    let empty_system_contracts =
        [Address::zero(), BOOTLOADER_ADDRESS].map(|address| DeployedContract {
            account_id: AccountTreeId::new(address),
            bytecode: empty_bytecode.clone(),
        });

    deployed_system_contracts.extend(empty_system_contracts);
    deployed_system_contracts
});

pub fn get_deployed_contracts(
    options: &SystemContractsOptions,
) -> Vec<zksync_types::block::DeployedContract> {
    match options {
        SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
            COMPILED_IN_SYSTEM_CONTRACTS.clone()
        }
        SystemContractsOptions::Local => get_system_smart_contracts(),
    }
}
