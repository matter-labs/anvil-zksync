use crate::utils::parse_genesis_file;
use alloy::signers::local::coins_bip39::{English, Mnemonic};
use anvil_zksync_common::{sh_eprintln, sh_err};
use anvil_zksync_config::constants::{
    DEFAULT_DISK_CACHE_DIR, DEFAULT_MNEMONIC, TEST_NODE_NETWORK_ID,
};
use anvil_zksync_config::types::{
    AccountGenerator, CacheConfig, CacheType, Genesis, SystemContractsOptions,
};
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_core::node::fork::ForkConfig;
use anvil_zksync_core::{
    node::{InMemoryNode, VersionedState},
    utils::write_json_file,
};
use anvil_zksync_types::{
    LogLevel, ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails, TransactionOrder,
};
use anyhow::Result;
use clap::{arg, command, Parser, Subcommand};
use flate2::read::GzDecoder;
use futures::FutureExt;
use rand::{rngs::StdRng, SeedableRng};
use std::env;
use std::io::Read;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::time::{Instant, Interval};
use url::Url;
use zksync_types::{H256, U256};

#[derive(Debug, Parser, Clone)]
#[command(
    author = "Matter Labs",
    version,
    about = "A fast and extensible local ZKsync test node.",
    long_about = "anvil-zksync\n\nA developer-friendly ZKsync local node for testing."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    // General Options
    #[arg(long, help_heading = "General Options")]
    /// Run in offline mode (disables all network requests).
    pub offline: bool,

    #[arg(long, help_heading = "General Options")]
    /// Enable health check endpoint.
    /// It will be available for GET requests at /health.
    /// The endpoint will return 200 OK if the node is healthy.
    pub health_check_endpoint: bool,

    /// Writes output of `anvil-zksync` as json to user-specified file.
    #[arg(long, value_name = "OUT_FILE", help_heading = "General Options")]
    pub config_out: Option<String>,

    #[arg(long, default_value = "8011", help_heading = "Network Options")]
    /// Port to listen on (default: 8011).
    pub port: Option<u16>,

    /// The hosts the server will listen on.
    #[arg(
        long,
        value_name = "IP_ADDR",
        env = "ANVIL_ZKSYNC_IP_ADDR",
        default_value = "0.0.0.0",
        value_delimiter = ',',
        help_heading = "Network Options"
    )]
    pub host: Vec<IpAddr>,

    #[arg(long, help_heading = "Network Options")]
    /// Specify chain ID (default: 260).
    pub chain_id: Option<u32>,

    #[arg(short, long, help_heading = "Debugging Options")]
    /// Enable default settings for debugging contracts.
    pub debug_mode: bool,

    #[arg(long, default_value = "true", default_missing_value = "true", num_args(0..=1), help_heading = "Debugging Options")]
    /// If true, prints node config on startup.
    pub show_node_config: Option<bool>,

    #[arg(long, default_value = "true", default_missing_value = "true", num_args(0..=1), help_heading = "Debugging Options")]
    /// If true, prints transactions and calls summary.
    pub show_tx_summary: Option<bool>,

    #[arg(long, alias = "no-console-log", default_missing_value = "true", num_args(0..=1), help_heading = "Debugging Options")]
    /// Disables printing of `console.log` invocations to stdout if true.
    pub disable_console_log: Option<bool>,

    #[arg(long, default_missing_value = "true", num_args(0..=1), help_heading = "Debugging Options")]
    /// If true, logs events.
    pub show_event_logs: Option<bool>,

    // Debugging Options
    #[arg(long, help_heading = "Debugging Options")]
    /// Show call debug information.
    pub show_calls: Option<ShowCalls>,

    #[arg(
        default_missing_value = "true", num_args(0..=1),
        long,
        requires = "show_calls",
        help_heading = "Debugging Options"
    )]
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

    #[arg(long, default_missing_value = "true", num_args(0..=1), help_heading = "Debugging Options")]
    /// If true, the tool will try to resolve ABI and topic names for better readability.
    /// May decrease performance.
    pub resolve_hashes: Option<bool>,

    // Gas Configuration
    #[arg(long, help_heading = "Gas Configuration")]
    /// Custom L1 gas price (in wei).
    pub l1_gas_price: Option<u64>,

    #[arg(long, alias = "gas-price", help_heading = "Gas Configuration")]
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
    /// Log file path (default: anvil-zksync.log).
    pub log_file_path: Option<String>,

    #[arg(long, alias = "quiet", default_missing_value = "true", num_args(0..=1), help_heading = "Logging Configuration")]
    /// If true, the tool will not print anything on startup.
    pub silent: Option<bool>,

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

    /// The timestamp of the genesis block.
    #[arg(long, value_name = "NUM")]
    pub timestamp: Option<u64>,

    /// Initialize the genesis block with the given `genesis.json` file.
    #[arg(long, value_name = "PATH", value_parser= parse_genesis_file)]
    pub init: Option<Genesis>,

    /// This is an alias for both --load-state and --dump-state.
    ///
    /// It initializes the chain with the state and block environment stored at the file, if it
    /// exists, and dumps the chain's state on exit.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = &[
            "init",
            "dump_state",
            "load_state"
        ]
    )]
    pub state: Option<PathBuf>,

    /// Interval in seconds at which the state and block environment is to be dumped to disk.
    ///
    /// See --state and --dump-state
    #[arg(short, long, value_name = "SECONDS")]
    pub state_interval: Option<u64>,

    /// Dump the state and block environment of chain on exit to the given file.
    ///
    /// If the value is a directory, the state will be written to `<VALUE>/state.json`.
    #[arg(long, value_name = "PATH", conflicts_with = "init")]
    pub dump_state: Option<PathBuf>,

    /// Preserve historical state snapshots when dumping the state.
    ///
    /// This will save the in-memory states of the chain at particular block hashes.
    ///
    /// These historical states will be loaded into the memory when `--load-state` / `--state`, and
    /// aids in RPC calls beyond the block at which state was dumped.
    #[arg(long, conflicts_with = "init", default_value = "false")]
    pub preserve_historical_states: bool,

    /// Initialize the chain from a previously saved state snapshot.
    #[arg(long, value_name = "PATH", conflicts_with = "init")]
    pub load_state: Option<PathBuf>,

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

    /// Enables automatic impersonation on startup. This allows any transaction sender to be
    /// simulated as different accounts, which is useful for testing contract behavior.
    #[arg(
        long,
        visible_alias = "auto-unlock",
        help_heading = "Account Configuration"
    )]
    pub auto_impersonate: bool,

    /// Block time in seconds for interval sealing.
    /// If unset, node seals a new block as soon as there is at least one transaction.
    #[arg(short, long, value_name = "SECONDS", value_parser = duration_from_secs_f64, help_heading = "Block Sealing")]
    pub block_time: Option<Duration>,

    /// Disable auto and interval mining, and mine on demand instead.
    #[arg(long, visible_alias = "no-mine", conflicts_with = "block_time")]
    pub no_mining: bool,

    /// The cors `allow_origin` header
    #[arg(long, default_value = "*", help_heading = "Server options")]
    pub allow_origin: String,

    /// Disable CORS.
    #[arg(long, conflicts_with = "allow_origin", help_heading = "Server options")]
    pub no_cors: bool,

    /// Transaction ordering in the mempool.
    #[arg(long, default_value = "fifo")]
    pub order: TransactionOrder,
}

#[derive(Debug, Subcommand, Clone)]
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

#[derive(Debug, Parser, Clone)]
pub struct ForkArgs {
    /// Whether to fork from existing network.
    /// If not set - will start a new network from genesis.
    /// If set - will try to fork a remote network. Possible values:
    ///  - mainnet
    ///  - sepolia-testnet
    ///  - http://XXX:YY
    #[arg(
        long,
        alias = "network",
        help = "Network to fork from (e.g., http://XXX:YY, mainnet, sepolia-testnet)."
    )]
    pub fork_url: ForkUrl,
    // Fork at a given L2 miniblock height.
    // If not set - will use the current finalized block from the network.
    #[arg(
        long,
        value_name = "BLOCK",
        long_help = "Fetch state from a specific block number over a remote endpoint.",
        alias = "fork-at"
    )]
    pub fork_block_number: Option<u64>,

    /// Fetch state from a specific transaction hash over a remote endpoint.
    ///
    /// See --fork-url.
    #[arg(
        long,
        requires = "fork_url",
        value_name = "TRANSACTION",
        conflicts_with = "fork_block_number"
    )]
    pub fork_transaction_hash: Option<H256>,
}

#[derive(Clone, Debug)]
pub enum ForkUrl {
    Mainnet,
    SepoliaTestnet,
    Other(Url),
}

impl ForkUrl {
    const MAINNET_URL: &'static str = "https://mainnet.era.zksync.io:443";
    const SEPOLIA_TESTNET_URL: &'static str = "https://sepolia.era.zksync.dev:443";

    pub fn to_config(&self) -> ForkConfig {
        match self {
            ForkUrl::Mainnet => ForkConfig {
                url: Self::MAINNET_URL.parse().unwrap(),
                estimate_gas_price_scale_factor: 1.5,
                estimate_gas_scale_factor: 1.4,
            },
            ForkUrl::SepoliaTestnet => ForkConfig {
                url: Self::SEPOLIA_TESTNET_URL.parse().unwrap(),
                estimate_gas_price_scale_factor: 2.0,
                estimate_gas_scale_factor: 1.3,
            },
            ForkUrl::Other(url) => ForkConfig::unknown(url.clone()),
        }
    }
}

impl FromStr for ForkUrl {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s == "mainnet" {
            Ok(ForkUrl::Mainnet)
        } else if s == "sepolia-testnet" {
            Ok(ForkUrl::SepoliaTestnet)
        } else {
            Ok(Url::from_str(s).map(ForkUrl::Other)?)
        }
    }
}

#[derive(Debug, Parser, Clone)]
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
        alias = "network",
        help = "Network to fork from (e.g., http://XXX:YY, mainnet, sepolia-testnet)."
    )]
    pub fork_url: ForkUrl,
    /// Transaction hash to replay.
    #[arg(help = "Transaction hash to replay.")]
    pub tx: H256,
}

impl Cli {
    /// Checks for deprecated options and warns users.
    pub fn deprecated_config_option() {
        if env::args().any(|arg| arg == "--config" || arg.starts_with("--config=")) {
            sh_eprintln!(
                "Warning: The '--config' option has been removed. \
                Please migrate to using other configuration options or defaults."
            );
        }
    }
    /// Converts the CLI arguments to a `TestNodeConfig`.
    pub fn into_test_node_config(self) -> eyre::Result<TestNodeConfig> {
        let genesis_balance = U256::from(self.balance as u128 * 10u128.pow(18));

        let mut config = TestNodeConfig::default()
            .with_port(self.port)
            .with_offline(if self.offline { Some(true) } else { None })
            .with_l1_gas_price(self.l1_gas_price)
            .with_l2_gas_price(self.l2_gas_price)
            .with_l1_pubdata_price(self.l1_pubdata_price)
            .with_show_tx_summary(self.show_tx_summary)
            .with_show_event_logs(self.show_event_logs)
            .with_disable_console_log(self.disable_console_log)
            .with_show_calls(self.show_calls)
            .with_vm_log_detail(self.show_vm_details)
            .with_show_storage_logs(self.show_storage_logs)
            .with_show_gas_details(self.show_gas_details)
            .with_show_outputs(self.show_outputs)
            .with_show_event_logs(self.show_event_logs)
            .with_resolve_hashes(self.resolve_hashes)
            .with_gas_limit_scale(self.limit_scale_factor)
            .with_price_scale(self.price_scale_factor)
            .with_resolve_hashes(self.resolve_hashes)
            .with_show_node_config(self.show_node_config)
            .with_silent(self.silent)
            .with_system_contracts(self.dev_system_contracts)
            .with_override_bytecodes_dir(self.override_bytecodes_dir.clone()) // Added
            .with_log_level(self.log)
            .with_log_file_path(self.log_file_path.clone())
            .with_account_generator(self.account_generator())
            .with_auto_impersonate(self.auto_impersonate)
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
            .with_genesis_timestamp(self.timestamp)
            .with_genesis(self.init)
            .with_chain_id(self.chain_id)
            .set_config_out(self.config_out)
            .with_host(self.host)
            .with_evm_emulator(if self.emulate_evm { Some(true) } else { None })
            .with_health_check_endpoint(if self.health_check_endpoint {
                Some(true)
            } else {
                None
            })
            .with_block_time(self.block_time)
            .with_no_mining(self.no_mining)
            .with_allow_origin(self.allow_origin)
            .with_no_cors(self.no_cors)
            .with_transaction_order(self.order)
            .with_state(self.state)
            .with_state_interval(self.state_interval)
            .with_dump_state(self.dump_state)
            .with_preserve_historical_states(self.preserve_historical_states)
            .with_load_state(self.load_state);

        if self.emulate_evm && self.dev_system_contracts != Some(SystemContractsOptions::Local) {
            return Err(eyre::eyre!(
                "EVM emulation requires the 'local' system contracts option."
            ));
        }

        if self.debug_mode {
            config = config.with_debug_mode();
        }

        Ok(config)
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

fn duration_from_secs_f64(s: &str) -> Result<Duration, String> {
    let s = s.parse::<f64>().map_err(|e| e.to_string())?;
    if s == 0.0 {
        return Err("Duration must be greater than 0".to_string());
    }
    Duration::try_from_secs_f64(s).map_err(|e| e.to_string())
}

// Implementation adapted from: https://github.com/foundry-rs/foundry/blob/206dab285437bd6889463ab006b6a5fb984079d8/crates/anvil/src/cmd.rs#L606
/// Helper type to periodically dump the state of the chain to disk
pub struct PeriodicStateDumper {
    in_progress_dump: Option<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>>>,
    node: InMemoryNode,
    dump_state: Option<PathBuf>,
    preserve_historical_states: bool,
    interval: Interval,
}

impl PeriodicStateDumper {
    pub fn new(
        node: InMemoryNode,
        dump_state: Option<PathBuf>,
        interval: Duration,
        preserve_historical_states: bool,
    ) -> Self {
        let dump_state = dump_state.map(|mut dump_state| {
            if dump_state.is_dir() {
                dump_state = dump_state.join("state.json");
            }
            dump_state
        });

        let interval = tokio::time::interval_at(Instant::now() + interval, interval);
        Self {
            in_progress_dump: None,
            node,
            dump_state,
            preserve_historical_states,
            interval,
        }
    }

    #[allow(dead_code)] // TODO: Remove this once the method is used
    pub async fn dump(&self) {
        if let Some(state) = self.dump_state.clone() {
            Self::dump_state(self.node.clone(), state, self.preserve_historical_states).await
        }
    }

    /// Infallible state dump
    async fn dump_state(node: InMemoryNode, dump_path: PathBuf, preserve_historical_states: bool) {
        tracing::trace!(path=?dump_path, "Dumping state");

        // Spawn a blocking task for state dumping
        let state_bytes = match node.dump_state(preserve_historical_states).await {
            Ok(bytes) => bytes,
            Err(err) => {
                sh_err!("Failed to dump state: {:?}", err);
                return;
            }
        };

        let mut decoder = GzDecoder::new(&state_bytes.0[..]);
        let mut json_str = String::new();
        if let Err(err) = decoder.read_to_string(&mut json_str) {
            sh_err!("Failed to decompress state bytes: {}", err);
            return;
        }

        let state = match serde_json::from_str::<VersionedState>(&json_str) {
            Ok(state) => state,
            Err(err) => {
                sh_err!("Failed to parse state JSON: {}", err);
                return;
            }
        };

        if let Err(err) = write_json_file(&dump_path, &state) {
            sh_err!("Failed to write state to file: {}", err);
        } else {
            tracing::trace!(path = ?dump_path, "Dumped state successfully");
        }
    }
}

// An endless future that periodically dumps the state to disk if configured.
// Implementation adapted from: https://github.com/foundry-rs/foundry/blob/206dab285437bd6889463ab006b6a5fb984079d8/crates/anvil/src/cmd.rs#L658
impl Future for PeriodicStateDumper {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.dump_state.is_none() {
            return Poll::Pending;
        }

        loop {
            if let Some(mut flush) = this.in_progress_dump.take() {
                match flush.poll_unpin(cx) {
                    Poll::Ready(_) => {
                        this.interval.reset();
                    }
                    Poll::Pending => {
                        this.in_progress_dump = Some(flush);
                        return Poll::Pending;
                    }
                }
            }

            if this.interval.poll_tick(cx).is_ready() {
                let api = this.node.clone();
                let path = this.dump_state.clone().expect("exists; see above");
                this.in_progress_dump = Some(Box::pin(Self::dump_state(
                    api,
                    path,
                    this.preserve_historical_states,
                )));
            } else {
                break;
            }
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::PeriodicStateDumper;

    use super::Cli;
    use anvil_zksync_core::node::InMemoryNode;
    use clap::Parser;
    use serde_json::Value;
    use std::{
        env,
        net::{IpAddr, Ipv4Addr},
    };
    use tempdir::TempDir;
    use zksync_types::{H160, U256};

    #[test]
    fn can_parse_host() {
        // Test adapted from https://github.com/foundry-rs/foundry/blob/398ef4a3d55d8dd769ce86cada5ec845e805188b/crates/anvil/src/cmd.rs#L895
        let args = Cli::parse_from(["anvil-zksync"]);
        assert_eq!(args.host, vec![IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))]);

        let args = Cli::parse_from([
            "anvil-zksync",
            "--host",
            "::1",
            "--host",
            "1.1.1.1",
            "--host",
            "2.2.2.2",
        ]);
        assert_eq!(
            args.host,
            ["::1", "1.1.1.1", "2.2.2.2"]
                .map(|ip| ip.parse::<IpAddr>().unwrap())
                .to_vec()
        );

        let args = Cli::parse_from(["anvil-zksync", "--host", "::1,1.1.1.1,2.2.2.2"]);
        assert_eq!(
            args.host,
            ["::1", "1.1.1.1", "2.2.2.2"]
                .map(|ip| ip.parse::<IpAddr>().unwrap())
                .to_vec()
        );

        env::set_var("ANVIL_ZKSYNC_IP_ADDR", "1.1.1.1");
        let args = Cli::parse_from(["anvil-zksync"]);
        assert_eq!(args.host, vec!["1.1.1.1".parse::<IpAddr>().unwrap()]);

        env::set_var("ANVIL_ZKSYNC_IP_ADDR", "::1,1.1.1.1,2.2.2.2");
        let args = Cli::parse_from(["anvil-zksync"]);
        assert_eq!(
            args.host,
            ["::1", "1.1.1.1", "2.2.2.2"]
                .map(|ip| ip.parse::<IpAddr>().unwrap())
                .to_vec()
        );
    }

    #[tokio::test]
    async fn test_dump_state() -> anyhow::Result<()> {
        let temp_dir = TempDir::new("state-test").expect("failed creating temporary dir");
        let dump_path = temp_dir.path().join("state.json");

        let config = anvil_zksync_config::TestNodeConfig {
            dump_state: Some(dump_path.clone()),
            state_interval: Some(1),
            preserve_historical_states: true,
            ..Default::default()
        };

        let node = InMemoryNode::test_config(None, config.clone());

        let mut state_dumper = PeriodicStateDumper::new(
            node.clone(),
            config.dump_state.clone(),
            std::time::Duration::from_secs(1),
            config.preserve_historical_states,
        );

        // Spawn the state dumper as a task:
        let dumper_handle = tokio::spawn(async move {
            tokio::select! {
                _ = &mut state_dumper => {}
            }
            state_dumper.dump().await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        dumper_handle.abort();
        let _ = dumper_handle.await;

        let dumped_data =
            std::fs::read_to_string(&dump_path).expect("Expected state file to be created");
        let _: Value =
            serde_json::from_str(&dumped_data).expect("Failed to parse dumped state as JSON");

        Ok(())
    }

    #[tokio::test]
    async fn test_load_state() -> anyhow::Result<()> {
        let temp_dir = TempDir::new("state-load-test").expect("failed creating temporary dir");
        let state_path = temp_dir.path().join("state.json");

        let config = anvil_zksync_config::TestNodeConfig {
            dump_state: Some(state_path.clone()),
            state_interval: Some(1),
            preserve_historical_states: true,
            ..Default::default()
        };

        let node = InMemoryNode::test_config(None, config.clone());
        let test_address = H160::from_low_u64_be(12345);
        node.set_rich_account(test_address, U256::from(1000000u64))
            .await;

        let mut state_dumper = PeriodicStateDumper::new(
            node.clone(),
            config.dump_state.clone(),
            std::time::Duration::from_secs(1),
            config.preserve_historical_states,
        );

        let dumper_handle = tokio::spawn(async move {
            tokio::select! {
                _ = &mut state_dumper => {}
            }
            state_dumper.dump().await;
        });

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        dumper_handle.abort();
        let _ = dumper_handle.await;

        // assert the state json file was created
        std::fs::read_to_string(&state_path).expect("Expected state file to be created");

        let new_config = anvil_zksync_config::TestNodeConfig::default();
        let new_node = InMemoryNode::test_config(None, new_config.clone());

        new_node
            .load_state(zksync_types::web3::Bytes(std::fs::read(&state_path)?))
            .await?;

        // assert the balance from the loaded state is correctly applied
        let balance = new_node.get_balance_impl(test_address, None).await?;
        assert_eq!(balance, U256::from(1000000u64));

        Ok(())
    }
}
