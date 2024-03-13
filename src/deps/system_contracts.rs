use once_cell::sync::Lazy;
use serde_json::Value;
use zksync_basic_types::{AccountTreeId, Address, H160};
use zksync_types::{
    block::DeployedContract, ACCOUNT_CODE_STORAGE_ADDRESS, BOOTLOADER_ADDRESS,
    BOOTLOADER_UTILITIES_ADDRESS, COMPRESSOR_ADDRESS, CONTRACT_DEPLOYER_ADDRESS,
    ECRECOVER_PRECOMPILE_ADDRESS, EVENT_WRITER_ADDRESS, IMMUTABLE_SIMULATOR_STORAGE_ADDRESS,
    KECCAK256_PRECOMPILE_ADDRESS, KNOWN_CODES_STORAGE_ADDRESS, L1_MESSENGER_ADDRESS,
    L2_ETH_TOKEN_ADDRESS, MSG_VALUE_SIMULATOR_ADDRESS, NONCE_HOLDER_ADDRESS,
    SHA256_PRECOMPILE_ADDRESS, SYSTEM_CONTEXT_ADDRESS, PUBDATA_CHUNK_PUBLISHER_ADDRESS,
};

/// The `ecAdd` system contract address.
pub const ECADD_PRECOMPILE_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x06,
]);

/// The `ecMul` system contract address.
pub const ECMUL_PRECOMPILE_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x07,
]);

pub fn bytecode_from_slice(artifact_name: &str, contents: &[u8]) -> Vec<u8> {
    let artifact: Value = serde_json::from_slice(contents).expect(artifact_name);
    let bytecode = artifact["bytecode"]
        .as_str()
        .unwrap_or_else(|| panic!("Bytecode not found in {:?}", artifact_name))
        .strip_prefix("0x")
        .unwrap_or_else(|| panic!("Bytecode in {:?} is not hex", artifact_name));

    hex::decode(bytecode)
        .unwrap_or_else(|err| panic!("Can't decode bytecode in {:?}: {}", artifact_name, err))
}

pub static COMPILED_IN_SYSTEM_CONTRACTS: Lazy<Vec<DeployedContract>> = Lazy::new(|| {
    let mut deployed_system_contracts = [
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
            "L2EthToken",
            L2_ETH_TOKEN_ADDRESS,
            include_bytes!("contracts/L2EthToken.json").to_vec(),
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
            "Compressor",
            COMPRESSOR_ADDRESS,
            include_bytes!("contracts/Compressor.json").to_vec(),
        ),
        (
            "PubdataChunkPublisher",
            PUBDATA_CHUNK_PUBLISHER_ADDRESS,
            include_bytes!("contracts/PubdataChunkPublisher.json").to_vec(),
        ),
    ]
    .map(|(pname, address, contents)| DeployedContract {
        account_id: AccountTreeId::new(address),

        bytecode: bytecode_from_slice(pname, &contents),
    })
    .to_vec();

    let yul_contracts = [
        (
            "Keccak256",
            KECCAK256_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/Keccak256.yul.zbin").to_vec(),
        ),
        (
            "SHA256",
            SHA256_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/SHA256.yul.zbin").to_vec(),
        ),
        (
            "Ecrecover",
            ECRECOVER_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/Ecrecover.yul.zbin").to_vec(),
        ),
        (
            "EventWriter",
            EVENT_WRITER_ADDRESS,
            include_bytes!("contracts/EventWriter.yul.zbin").to_vec(),
        ),
        (
            "ECADD_PRECOMPILE_ADDRESS",
            ECADD_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/EcAdd.yul.zbin").to_vec(),
        ),
        (
            "ECMUL_PRECOMPILE_ADDRESS",
            ECMUL_PRECOMPILE_ADDRESS,
            include_bytes!("contracts/EcMul.yul.zbin").to_vec(),
        ),
    ]
    .map(|(_pname, address, contents)| DeployedContract {
        account_id: AccountTreeId::new(address),
        bytecode: contents,
    });

    deployed_system_contracts.extend(yul_contracts);

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
