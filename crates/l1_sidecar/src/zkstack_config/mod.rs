use crate::zkstack_config::contracts::ContractsConfig;
use crate::zkstack_config::genesis::GenesisConfig;
use crate::zkstack_config::wallets::WalletsConfig;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use zksync_types::ProtocolVersionId;

pub mod contracts;
pub mod genesis;
pub mod wallets;

static BUILTIN_ZKSTACK_CONFIGS: Lazy<HashMap<ProtocolVersionId, ZkstackConfig>> = Lazy::new(|| {
    let wallets: WalletsConfig =
        serde_yaml::from_slice(include_bytes!("../../../../l1-setup/wallets.yaml")).unwrap();
    HashMap::from_iter([
        (
            ProtocolVersionId::Version26,
            ZkstackConfig {
                contracts: serde_yaml::from_slice(include_bytes!(
                    "../../../../l1-setup/configs/v26-contracts.yaml"
                ))
                .unwrap(),
                genesis: serde_yaml::from_slice(include_bytes!(
                    "../../../../l1-setup/configs/v26-genesis.yaml"
                ))
                .unwrap(),
                wallets: wallets.clone(),
            },
        ),
        (
            ProtocolVersionId::Version27,
            ZkstackConfig {
                contracts: serde_yaml::from_slice(include_bytes!(
                    "../../../../l1-setup/configs/v27-contracts.yaml"
                ))
                .unwrap(),
                genesis: serde_yaml::from_slice(include_bytes!(
                    "../../../../l1-setup/configs/v27-genesis.yaml"
                ))
                .unwrap(),
                wallets: wallets.clone(),
            },
        ),
        (
            ProtocolVersionId::Version28,
            ZkstackConfig {
                contracts: serde_yaml::from_slice(include_bytes!(
                    "../../../../l1-setup/configs/v28-contracts.yaml"
                ))
                .unwrap(),
                genesis: serde_yaml::from_slice(include_bytes!(
                    "../../../../l1-setup/configs/v28-genesis.yaml"
                ))
                .unwrap(),
                wallets,
            },
        ),
    ])
});

#[derive(Clone, Debug)]
pub struct ZkstackConfig {
    pub contracts: ContractsConfig,
    pub genesis: GenesisConfig,
    pub wallets: WalletsConfig,
}

impl ZkstackConfig {
    pub fn builtin(protocol_version: ProtocolVersionId) -> Self {
        BUILTIN_ZKSTACK_CONFIGS
            .get(&protocol_version)
            .expect("unsupported protocol version")
            .clone()
    }
}
