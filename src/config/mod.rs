use std::{env, fs::read_to_string, path::PathBuf};

use cache::CacheConfig;
use cli::{CacheType, Cli, DevSystemContracts};
use gas::GasConfig;
use log::LogConfig;
use node::InMemoryNodeConfig;
use serde::Deserialize;

use crate::system_contracts;

pub mod cli;

pub const CONFIG_DIR: &str = ".era_test_node";
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// Defines the configuration parameters for the [InMemoryNode].
#[derive(Deserialize, Default, Debug, Clone)]
pub struct TestNodeConfig {
    pub node: InMemoryNodeConfig,
    // The values to be used when calculating gas.
    pub gas: Option<GasConfig>,
    // Logging configuration.
    pub log: LogConfig,
    // Caching configuration.
    pub cache: CacheConfig,
}

impl TestNodeConfig {
    /// Try to load a configuration file from either a provided path or the `$HOME` directory.
    pub fn try_load(file_path: &Option<String>) -> eyre::Result<TestNodeConfig> {
        let path = if let Some(path) = file_path {
            PathBuf::from(path)
        } else {
            // NOTE: `env::home_dir` is not compatible with Windows.
            #[allow(deprecated)]
            let mut path = env::home_dir().expect("failed to get home directory");

            path.push(CONFIG_DIR);
            path.push(CONFIG_FILE_NAME);
            path
        };

        let toml = read_to_string(path)?;
        let config = toml::from_str(&toml)?;

        Ok(config)
    }

    /// Override the config with values provided by [`Cli`].
    pub fn override_with_opts(&mut self, opt: &Cli) {
        // [`NodeConfig`].
        if let Some(port) = &opt.port {
            self.node.port = *port;
        }

        if let Some(show_calls) = &opt.show_calls {
            self.node.show_calls = *show_calls;
        }
        if let Some(show_outputs) = &opt.show_outputs {
            self.node.show_outputs = *show_outputs;
        }
        if let Some(show_storage_logs) = &opt.show_storage_logs {
            self.node.show_storage_logs = *show_storage_logs;
        }
        if let Some(show_vm_details) = &opt.show_vm_details {
            self.node.show_vm_details = *show_vm_details;
        }
        if let Some(show_gas_details) = &opt.show_gas_details {
            self.node.show_gas_details = *show_gas_details;
        }
        if let Some(resolve_hashes) = &opt.resolve_hashes {
            self.node.resolve_hashes = *resolve_hashes;
        }

        if let Some(contract_options) = &opt.dev_system_contracts {
            self.node.system_contracts_options = match contract_options {
                DevSystemContracts::BuiltIn => system_contracts::Options::BuiltIn,
                DevSystemContracts::BuiltInNoVerify => {
                    system_contracts::Options::BuiltInWithoutSecurity
                }
                DevSystemContracts::Local => system_contracts::Options::Local,
            };
        }

        // [`GasConfig`]
        if let Some(l1_gas_price) = &opt.l1_gas_price {
            let mut gas = self.gas.unwrap_or_default();
            gas.l1_gas_price = Some(*l1_gas_price);
            self.gas = Some(gas);
        }
        if let Some(l2_gas_price) = &opt.l2_gas_price {
            let mut gas = self.gas.unwrap_or_default();
            gas.l2_gas_price = Some(*l2_gas_price);
            self.gas = Some(gas);
        }

        // [`LogConfig`].
        if let Some(log_level) = &opt.log {
            self.log.level = *log_level;
        }
        if let Some(file_path) = &opt.log_file_path {
            self.log.file_path = file_path.to_string();
        }

        // [`CacheConfig`].
        if let Some(cache_type) = &opt.cache {
            self.cache = match cache_type {
                CacheType::None => CacheConfig::None,
                CacheType::Memory => CacheConfig::Memory,
                CacheType::Disk => CacheConfig::Disk {
                    dir: opt.cache_dir.clone().expect("missing --cache-dir argument"),
                    reset: opt.reset_cache.unwrap_or_default(),
                },
            };
        }
    }
}

pub mod node {
    use clap::Parser;
    use serde::Deserialize;
    use std::{fmt::Display, str::FromStr};

    use crate::system_contracts;

    #[derive(Deserialize, Debug, Copy, Clone)]
    pub struct InMemoryNodeConfig {
        pub port: u16,
        pub show_calls: ShowCalls,
        pub show_outputs: bool,
        pub show_storage_logs: ShowStorageLogs,
        pub show_vm_details: ShowVMDetails,
        pub show_gas_details: ShowGasDetails,
        pub resolve_hashes: bool,
        pub system_contracts_options: system_contracts::Options,
    }

    impl Default for InMemoryNodeConfig {
        fn default() -> Self {
            Self {
                port: 8011,
                show_calls: Default::default(),
                show_outputs: Default::default(),
                show_storage_logs: Default::default(),
                show_vm_details: Default::default(),
                show_gas_details: Default::default(),
                resolve_hashes: Default::default(),
                system_contracts_options: Default::default(),
            }
        }
    }

    #[derive(
        Deserialize, Debug, Default, clap::Parser, Copy, Clone, clap::ValueEnum, PartialEq, Eq,
    )]
    pub enum ShowCalls {
        #[default]
        None,
        User,
        System,
        All,
    }

    impl FromStr for ShowCalls {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_ref() {
                "none" => Ok(ShowCalls::None),
                "user" => Ok(ShowCalls::User),
                "system" => Ok(ShowCalls::System),
                "all" => Ok(ShowCalls::All),
                _ => Err(format!(
                    "Unknown ShowCalls value {} - expected one of none|user|system|all.",
                    s
                )),
            }
        }
    }

    impl Display for ShowCalls {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
            write!(f, "{:?}", self)
        }
    }

    #[derive(Deserialize, Debug, Default, Parser, Copy, Clone, clap::ValueEnum, PartialEq, Eq)]
    pub enum ShowStorageLogs {
        #[default]
        None,
        Read,
        Write,
        Paid,
        All,
    }

    impl FromStr for ShowStorageLogs {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_ref() {
                "none" => Ok(ShowStorageLogs::None),
                "read" => Ok(ShowStorageLogs::Read),
                "write" => Ok(ShowStorageLogs::Write),
                "paid" => Ok(ShowStorageLogs::Paid),
                "all" => Ok(ShowStorageLogs::All),
                _ => Err(format!(
                    "Unknown ShowStorageLogs value {} - expected one of none|read|write|paid|all.",
                    s
                )),
            }
        }
    }

    impl Display for ShowStorageLogs {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
            write!(f, "{:?}", self)
        }
    }

    #[derive(Deserialize, Debug, Default, Parser, Copy, Clone, clap::ValueEnum, PartialEq, Eq)]
    pub enum ShowVMDetails {
        #[default]
        None,
        All,
    }

    impl FromStr for ShowVMDetails {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_ref() {
                "none" => Ok(ShowVMDetails::None),
                "all" => Ok(ShowVMDetails::All),
                _ => Err(format!(
                    "Unknown ShowVMDetails value {} - expected one of none|all.",
                    s
                )),
            }
        }
    }

    impl Display for ShowVMDetails {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
            write!(f, "{:?}", self)
        }
    }

    #[derive(Deserialize, Debug, Default, Parser, Copy, Clone, clap::ValueEnum, PartialEq, Eq)]
    pub enum ShowGasDetails {
        #[default]
        None,
        All,
    }

    impl FromStr for ShowGasDetails {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_ref() {
                "none" => Ok(ShowGasDetails::None),
                "all" => Ok(ShowGasDetails::All),
                _ => Err(format!(
                    "Unknown ShowGasDetails value {} - expected one of none|all.",
                    s
                )),
            }
        }
    }

    impl Display for ShowGasDetails {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
            write!(f, "{:?}", self)
        }
    }
}

pub mod gas {
    use serde::Deserialize;

    /// L1 Gas Price.
    pub const DEFAULT_L1_GAS_PRICE: u64 = 50_000_000_000;
    // TODO: for now, that's fine, as computation overhead is set to zero, but we may consider using calculated fee input everywhere.
    /// The default L2 Gas Price to be used if not supplied via the CLI argument.
    pub const DEFAULT_L2_GAS_PRICE: u64 = 25_000_000;
    /// L1 Gas Price Scale Factor for gas estimation.
    pub const DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR: f64 = 1.5;
    /// The factor by which to scale the gasLimit.
    pub const DEFAULT_ESTIMATE_GAS_SCALE_FACTOR: f32 = 1.3;

    #[derive(Deserialize, Debug, Default, Copy, Clone)]
    pub struct GasConfig {
        /// L1 gas price.
        pub l1_gas_price: Option<u64>,
        /// L2 gas price.
        pub l2_gas_price: Option<u64>,
        /// Factors used in estimating gas.
        pub estimation: Option<Estimation>,
    }

    #[derive(Deserialize, Debug, Default, Copy, Clone)]
    pub struct Estimation {
        /// L1 gas price scale factor for gas estimation.
        pub price_scale_factor: Option<f64>,
        /// The factor by which to scale the gasLimit.
        pub limit_scale_factor: Option<f32>,
    }
}

pub mod log {
    use serde::Deserialize;

    use crate::observability::LogLevel;

    pub const DEFAULT_LOG_FILE_PATH: &str = "era_test_node.log";

    #[derive(Deserialize, Debug, Clone)]
    pub struct LogConfig {
        pub level: LogLevel,
        pub file_path: String,
    }

    impl Default for LogConfig {
        fn default() -> Self {
            Self {
                level: Default::default(),
                file_path: String::from(DEFAULT_LOG_FILE_PATH),
            }
        }
    }
}

pub mod cache {
    use serde::Deserialize;

    pub const DEFAULT_DISK_CACHE_DIR: &str = ".cache";

    /// Cache configuration. Can be one of:
    ///
    /// None    : Caching is disabled
    /// Memory  : Caching is provided in-memory and not persisted across runs
    /// Disk    : Caching is persisted on disk in the provided directory and can be reset
    #[derive(Deserialize, Debug, Clone)]
    pub enum CacheConfig {
        #[serde(rename = "none")]
        None,
        #[serde(rename = "memory")]
        Memory,
        #[serde(rename = "disk")]
        Disk { dir: String, reset: bool },
    }

    impl Default for CacheConfig {
        fn default() -> Self {
            Self::Disk {
                dir: String::from(DEFAULT_DISK_CACHE_DIR),
                reset: false,
            }
        }
    }
}
