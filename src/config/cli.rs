use clap::{arg, command, Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use zksync_types::H256;

use crate::observability::LogLevel;

use super::node::{ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};

/// Cache type config for the node.
#[derive(ValueEnum, Deserialize, Default, Debug, Copy, Clone)]
pub enum CacheType {
    None,
    Memory,
    #[default]
    Disk,
}

/// System contract options.
#[derive(ValueEnum, Debug, Clone)]
pub enum DevSystemContracts {
    BuiltIn,
    BuiltInNoVerify,
    Local,
}

#[derive(Debug, Parser)]
#[command(author = "Matter Labs", version, about = "Test Node", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[arg(short, long)]
    /// The file path to the config file. If not supplied, defaults will be used.
    pub config: Option<String>,

    #[arg(long)]
    /// Port to listen on - default: 8011
    pub port: Option<u16>,
    #[arg(long)]
    /// Show call debug information
    pub show_calls: Option<ShowCalls>,
    #[arg(long)]
    /// Show call output
    pub show_outputs: Option<bool>,
    #[arg(long)]
    /// Show storage log information
    pub show_storage_logs: Option<ShowStorageLogs>,
    #[arg(long)]
    /// Show VM details information
    pub show_vm_details: Option<ShowVMDetails>,

    #[arg(long)]
    /// Show Gas details information
    pub show_gas_details: Option<ShowGasDetails>,

    #[arg(long)]
    /// If provided, uses a custom value as the L1 gas price.
    pub l1_gas_price: Option<u64>,

    #[arg(long)]
    /// If provided, uses a custom value as the L2 gas price.
    pub l2_gas_price: Option<u64>,

    #[arg(long)]
    /// If true, the tool will try to contact openchain to resolve the ABI & topic names.
    /// It will make debug log more readable, but will decrease the performance.
    pub resolve_hashes: Option<bool>,

    /// Specifies the option for the system contracts (use compiled built-in with or without signature verification, or load locally).
    /// Default: built-in
    #[arg(long)]
    pub dev_system_contracts: Option<DevSystemContracts>,

    /// Log filter level - default: info
    #[arg(long)]
    pub log: Option<LogLevel>,

    /// Log file path - default: era_test_node.log
    #[arg(long)]
    pub log_file_path: Option<String>,

    /// Cache type, can be one of `none`, `memory`, or `disk` - default: "disk"
    #[arg(long)]
    pub cache: Option<CacheType>,

    /// If true, will reset the local `disk` cache.
    #[arg(long)]
    pub reset_cache: Option<bool>,

    /// Cache directory location for `disk` cache - default: ".cache"
    #[arg(long)]
    pub cache_dir: Option<String>,

    #[arg(long)]
    pub override_bytecodes_dir: Option<String>,
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
    ///  - testnet
    ///  - http://XXX:YY
    pub network: String,
    #[arg(long)]
    // Fork at a given L2 miniblock height.
    // If not set - will use the current finalized block from the network.
    pub fork_at: Option<u64>,
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
    pub network: String,
    /// Transaction hash to replay.
    pub tx: H256,
}
