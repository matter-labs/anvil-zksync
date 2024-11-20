use clap::{arg, command, Parser, Subcommand};
use std::time::Duration;
use zksync_types::H256;

use crate::config::{
    CacheConfig, CacheType, ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails,
    TestNodeConfig,
};
use crate::observability::LogLevel;
use crate::system_contracts::Options as SystemContractsOptions;

use super::DEFAULT_DISK_CACHE_DIR;

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
    #[arg(short, long, help_heading = "General Options")]
    /// Path to the configuration file. If not supplied, defaults will be used.
    pub config: Option<String>,

    #[arg(long, default_value = "8011", help_heading = "Network Options")]
    /// Port to listen on (default: 8011).
    pub port: Option<u16>,

    #[arg(long, help_heading = "Network Options")]
    /// Specify chain ID (default: 260).
    pub chain_id: Option<u32>,

    #[arg(long, help_heading = "General Options")]
    /// Run in offline mode (disables all network requests).
    pub offline: bool,

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

    #[arg(long, value_parser = duration_from_secs_f64, help_heading = "Block Sealing")]
    /// Block time in seconds for interval sealing.
    /// If unset, node seals a new block as soon as there is at least one transaction.
    pub block_time: Option<Duration>,
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
    /// Converts the CLI arguments into a `TestNodeConfig`.
    pub fn into_test_node_config(self) -> eyre::Result<TestNodeConfig> {
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
}

fn duration_from_secs_f64(s: &str) -> Result<Duration, String> {
    let s = s.parse::<f64>().map_err(|e| e.to_string())?;
    if s == 0.0 {
        return Err("Duration must be greater than 0".to_string());
    }
    Duration::try_from_secs_f64(s).map_err(|e| e.to_string())
}
