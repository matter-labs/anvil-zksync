use std::env;

use clap::{arg, command, Parser, Subcommand};
use rand::{rngs::StdRng, SeedableRng};
use zksync_types::{H256, U256};

use crate::config::constants::{DEFAULT_MNEMONIC, TEST_NODE_NETWORK_ID};
use crate::config::{
    AccountGenerator, CacheConfig, CacheType, ShowCalls, ShowGasDetails, ShowStorageLogs,
    ShowVMDetails, TestNodeConfig,
};
use crate::observability::LogLevel;
use crate::system_contracts::Options as SystemContractsOptions;

use super::DEFAULT_DISK_CACHE_DIR;
use alloy_signer_local::coins_bip39::{English, Mnemonic};

#[derive(Debug, Parser)]
#[command(
    author = "Matter Labs",
    version,
    about = "A fast and extensible local ZKsync test node.",
    long_about = "Era Test Node\n\nA developer-friendly ZKsync local node for testing."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    // General Options
    #[arg(long, help_heading = "General Options")]
    /// Run in offline mode (disables all network requests).
    pub offline: bool,

    #[arg(long, default_value = "8011", help_heading = "Network Options")]
    /// Port to listen on (default: 8011).
    pub port: Option<u16>,

    #[arg(long, help_heading = "Network Options")]
    /// Specify chain ID (default: 260).
    pub chain_id: Option<u32>,

    #[arg(short, long, help_heading = "Debugging Options")]
    /// Enable default settings for debugging contracts.
    pub debug_mode: bool,

    // Debugging Options
    #[arg(long, help_heading = "Debugging Options")]
    /// Show call debug information.
    pub show_calls: Option<ShowCalls>,

    #[arg(long, help_heading = "Debugging Options")]
    /// Show call output information.
    pub show_outputs: Option<bool>,

    #[arg(long, help_heading = "Debugging Options")]
    /// Show storage log information.
    pub show_storage_logs: Option<ShowStorageLogs>,

    #[arg(long, help_heading = "Debugging Options")]
    /// Show VM details information.
    pub show_vm_details: Option<ShowVMDetails>,

    #[arg(long, help_heading = "Debugging Options")]
    /// Show gas details information.
    pub show_gas_details: Option<ShowGasDetails>,

    #[arg(long, help_heading = "Debugging Options")]
    /// If true, the tool will try to resolve ABI and topic names for better readability.
    /// May decrease performance.
    pub resolve_hashes: Option<bool>,

    // Gas Configuration
    #[arg(long, help_heading = "Gas Configuration")]
    /// Custom L1 gas price (in wei).
    pub l1_gas_price: Option<u64>,

    #[arg(long, help_heading = "Gas Configuration")]
    /// Custom L2 gas price (in wei).
    pub l2_gas_price: Option<u64>,

    #[arg(long, help_heading = "Gas Configuration")]
    /// Custom L1 pubdata price (in wei).
    pub l1_pubdata_price: Option<u64>,

    #[arg(long, help_heading = "Gas Configuration")]
    /// Gas price estimation scale factor.
    pub price_scale_factor: Option<f64>,

    #[arg(long, help_heading = "Gas Configuration")]
    /// Gas limit estimation scale factor.
    pub limit_scale_factor: Option<f32>,

    #[arg(long, help_heading = "System Configuration")]
    /// Directory to override bytecodes.
    pub override_bytecodes_dir: Option<String>,

    // System Configuration
    #[arg(long, help_heading = "System Configuration")]
    /// Option for system contracts (default: built-in).
    pub dev_system_contracts: Option<SystemContractsOptions>,

    #[arg(
        long,
        requires = "dev_system_contracts",
        help_heading = "System Configuration"
    )]
    /// Enables EVM emulation. Requires local system contracts.
    pub emulate_evm: bool,

    // Logging Configuration
    #[arg(long, help_heading = "Logging Configuration")]
    /// Log level (default: info).
    pub log: Option<LogLevel>,

    #[arg(long, help_heading = "Logging Configuration")]
    /// Log file path (default: era_test_node.log).
    pub log_file_path: Option<String>,

    // Cache Options
    #[arg(long, help_heading = "Cache Options")]
    /// Cache type (none, memory, or disk). Default: "disk".
    pub cache: Option<CacheType>,

    #[arg(long, help_heading = "Cache Options")]
    /// Reset the local disk cache.
    pub reset_cache: Option<bool>,

    #[arg(long, help_heading = "Cache Options")]
    /// Cache directory location for disk cache (default: .cache).
    pub cache_dir: Option<String>,

    /// Number of dev accounts to generate and configure.
    #[arg(
        long,
        short,
        default_value = "10",
        value_name = "NUM",
        help_heading = "Account Configuration"
    )]
    pub accounts: u64,

    /// The balance of every dev account in Ether.
    #[arg(
        long,
        default_value = "10000",
        value_name = "NUM",
        help_heading = "Account Configuration"
    )]
    pub balance: u64,

    /// BIP39 mnemonic phrase used for generating accounts.
    /// Cannot be used if `mnemonic_random` or `mnemonic_seed` are used.
    #[arg(long, short, conflicts_with_all = &["mnemonic_seed", "mnemonic_random"], help_heading = "Account Configuration")]
    pub mnemonic: Option<String>,

    /// Automatically generates a BIP39 mnemonic phrase and derives accounts from it.
    /// Cannot be used with other `mnemonic` options.
    /// You can specify the number of words you want in the mnemonic.
    /// [default: 12]
    #[arg(long, conflicts_with_all = &["mnemonic", "mnemonic_seed"], default_missing_value = "12", num_args(0..=1), help_heading = "Account Configuration")]
    pub mnemonic_random: Option<usize>,

    /// Generates a BIP39 mnemonic phrase from a given seed.
    /// Cannot be used with other `mnemonic` options.
    /// CAREFUL: This is NOT SAFE and should only be used for testing.
    /// Never use the private keys generated in production.
    #[arg(long = "mnemonic-seed-unsafe", conflicts_with_all = &["mnemonic", "mnemonic_random"],  help_heading = "Account Configuration")]
    pub mnemonic_seed: Option<u64>,

    /// Sets the derivation path of the child key to be derived.
    /// [default: m/44'/60'/0'/0/]
    #[arg(long, help_heading = "Account Configuration")]
    pub derivation_path: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Starts a new empty local network.
    #[command(name = "run")]
    Run,
    /// Starts a local network that is a fork of another network.
    #[command(name = "fork")]
    Fork(ForkArgs),
    /// Starts a local network that is a fork of another network, and replays a given TX on it.
    #[command(name = "replay_tx")]
    ReplayTx(ReplayArgs),
}

#[derive(Debug, Parser)]
pub struct ForkArgs {
    /// Whether to fork from existing network.
    /// If not set - will start a new network from genesis.
    /// If set - will try to fork a remote network. Possible values:
    ///  - mainnet
    ///  - sepolia-testnet
    ///  - http://XXX:YY
    #[arg(
        long,
        help = "Network to fork from (e.g., mainnet, sepolia-testnet, etc.)."
    )]
    pub network: String,
    // Fork at a given L2 miniblock height.
    // If not set - will use the current finalized block from the network.
    #[arg(
        long,
        value_name = "BLOCK",
        long_help = "Fetch state from a specific block number over a remote endpoint.",
        alias = "fork-at"
    )]
    pub fork_block_number: Option<u64>,
}

#[derive(Debug, Parser)]
pub struct ReplayArgs {
    /// Whether to fork from existing network.
    /// If not set - will start a new network from genesis.
    /// If set - will try to fork a remote network. Possible values:
    ///  - mainnet
    ///  - sepolia-testnet
    ///  - goerli-testnet
    ///  - http://XXX:YY
    #[arg(
        long,
        help = "Network to fork from (e.g., mainnet, sepolia-testnet, etc.)."
    )]
    pub network: String,
    /// Transaction hash to replay.
    #[arg(help = "Transaction hash to replay.")]
    pub tx: H256,
}

impl Cli {
    /// Checks for deprecated options and warns users.
    pub fn deprecated_config_option() {
        if env::args().any(|arg| arg == "--config" || arg.starts_with("--config=")) {
            eprintln!(
                "Warning: The '--config' option has been removed. \
                Please migrate to using other configuration options or defaults."
            );
        }
    }
    /// Converts the CLI arguments to a `TestNodeConfig`.
    pub fn to_test_node_config(&self) -> eyre::Result<TestNodeConfig> {
        let genesis_balance = U256::from(100u128 * 10u128.pow(18));

        let vm_log_detail = if let Some(output) = self.show_outputs {
            if output {
                Some(ShowVMDetails::All)
            } else {
                Some(ShowVMDetails::None)
            }
        } else if let Some(logs) = self.show_storage_logs {
            match logs {
                ShowStorageLogs::None => Some(ShowVMDetails::None),
                _ => Some(ShowVMDetails::All),
            }
        } else {
            None
        };

        let config = TestNodeConfig::default()
            .with_port(self.port)
            .with_l1_gas_price(self.l1_gas_price)
            .with_l2_gas_price(self.l2_gas_price)
            .with_l1_pubdata_price(self.l1_pubdata_price)
            .with_show_calls(self.show_calls)
            .with_vm_log_detail(vm_log_detail)
            .with_gas_limit_scale(self.limit_scale_factor)
            .with_price_scale(self.price_scale_factor)
            .with_resolve_hashes(self.resolve_hashes)
            .with_system_contracts(self.dev_system_contracts)
            .with_override_bytecodes_dir(self.override_bytecodes_dir.clone()) // Added
            .with_log_level(self.log)
            .with_log_file_path(self.log_file_path.clone())
            .with_account_generator(self.account_generator())
            .with_genesis_balance(genesis_balance)
            .with_cache_config(self.cache.map(|cache_type| {
                match cache_type {
                    CacheType::None => CacheConfig::None,
                    CacheType::Memory => CacheConfig::Memory,
                    CacheType::Disk => CacheConfig::Disk {
                        dir: self
                            .cache_dir
                            .clone()
                            .unwrap_or_else(|| DEFAULT_DISK_CACHE_DIR.to_string()),
                        reset: self.reset_cache.unwrap_or(false),
                    },
                }
            }))
            .with_chain_id(self.chain_id)
            .with_evm_emulator(if self.emulate_evm { Some(true) } else { None });

        if self.emulate_evm && self.dev_system_contracts != Some(SystemContractsOptions::Local) {
            return Err(eyre::eyre!(
                "EVM emulation requires the 'local' system contracts option."
            ));
        }

        if self.debug_mode {
            Ok(config
                .with_show_calls(Some(ShowCalls::All))
                .with_vm_log_detail(Some(ShowVMDetails::All))
                .with_resolve_hashes(Some(true)))
        } else {
            Ok(config)
        }
    }

    fn account_generator(&self) -> AccountGenerator {
        let mut gen = AccountGenerator::new(self.accounts as usize)
            .phrase(DEFAULT_MNEMONIC)
            .chain_id(self.chain_id.unwrap_or(TEST_NODE_NETWORK_ID));
        if let Some(ref mnemonic) = self.mnemonic {
            gen = gen.phrase(mnemonic);
        } else if let Some(count) = self.mnemonic_random {
            let mut rng = rand::thread_rng();
            let mnemonic = match Mnemonic::<English>::new_with_count(&mut rng, count) {
                Ok(mnemonic) => mnemonic.to_phrase(),
                Err(_) => DEFAULT_MNEMONIC.to_string(),
            };
            gen = gen.phrase(mnemonic);
        } else if let Some(seed) = self.mnemonic_seed {
            let mut seed = StdRng::seed_from_u64(seed);
            let mnemonic = Mnemonic::<English>::new(&mut seed).to_phrase();
            gen = gen.phrase(mnemonic);
        }
        if let Some(ref derivation) = self.derivation_path {
            gen = gen.derivation_path(derivation);
        }
        gen
    }
}
