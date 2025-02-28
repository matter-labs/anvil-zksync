use crate::zkstack_config::contracts::ContractsConfig;
use crate::zkstack_config::genesis::GenesisConfig;
use crate::zkstack_config::wallets::WalletsConfig;

pub mod contracts;
pub mod genesis;
pub mod wallets;

#[derive(Clone, Debug)]
pub struct ZkstackConfig {
    pub contracts: ContractsConfig,
    pub genesis: GenesisConfig,
    pub wallets: WalletsConfig,
}

impl ZkstackConfig {
    pub fn builtin() -> Self {
        let contracts: ContractsConfig = serde_yaml::from_slice(include_bytes!(
            "../../../../l1-setup/configs/contracts.yaml"
        ))
        .unwrap();
        let genesis: GenesisConfig =
            serde_yaml::from_slice(include_bytes!("../../../../l1-setup/configs/genesis.yaml"))
                .unwrap();
        let wallets: WalletsConfig =
            serde_yaml::from_slice(include_bytes!("../../../../l1-setup/wallets.yaml")).unwrap();
        Self {
            contracts,
            genesis,
            wallets,
        }
    }
}
