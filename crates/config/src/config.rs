use crate::constants::*;
use crate::types::*;
use crate::utils::{format_eth, format_gwei};
use alloy::signers::local::PrivateKeySigner;
use anvil_zksync_common::sh_println;
use anvil_zksync_types::{
    LogLevel, ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails, TransactionOrder,
};
use colored::{Colorize, CustomColor};
use serde_json::{json, to_writer, Value};
use std::collections::HashMap;
use std::fs::File;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::Duration;
use zksync_types::fee_model::FeeModelConfigV2;
use zksync_types::U256;

pub const VERSION_MESSAGE: &str = concat!(env!("CARGO_PKG_VERSION"));

const BANNER: &str = r#"
                      _  _         _____ _  __
  __ _  _ __  __   __(_)| |       |__  /| |/ / ___  _   _  _ __    ___
 / _` || '_ \ \ \ / /| || | _____   / / | ' / / __|| | | || '_ \  / __|
| (_| || | | | \ V / | || ||_____| / /_ | . \ \__ \| |_| || | | || (__
 \__,_||_| |_|  \_/  |_||_|       /____||_|\_\|___/ \__, ||_| |_| \___|
                                                    |___/
"#;
/// Struct to hold the details of the fork for display purposes
pub struct ForkPrintInfo {
    pub network_rpc: String,
    pub l1_block: String,
    pub l2_block: String,
    pub block_timestamp: String,
    pub fork_block_hash: String,
    pub fee_model_config_v2: FeeModelConfigV2,
}

/// Defines the configuration parameters for the [InMemoryNode].
#[derive(Debug, Clone)]
pub struct TestNodeConfig {
    /// Filename to write anvil-zksync output as json
    pub config_out: Option<String>,
    /// Port the node will listen on
    pub port: u16,
    /// Print node config on startup if true
    pub show_node_config: bool,
    /// Print transactions and calls summary if true
    pub show_tx_summary: bool,
    /// If true, logs events.
    pub show_event_logs: bool,
    /// Disables printing of `console.log` invocations to stdout if true
    pub disable_console_log: bool,
    /// Controls visibility of call logs
    pub show_calls: ShowCalls,
    /// Whether to show call output data
    pub show_outputs: bool,
    /// Level of detail for storage logs
    pub show_storage_logs: ShowStorageLogs,
    /// Level of detail for VM execution logs
    pub show_vm_details: ShowVMDetails,
    /// Level of detail for gas usage logs
    pub show_gas_details: ShowGasDetails,
    /// Whether to resolve hash references
    pub resolve_hashes: bool,
    /// Don’t print anything on startup if true
    pub silent: bool,
    /// Configuration for system contracts
    pub system_contracts_options: SystemContractsOptions,
    /// Directory to override bytecodes
    pub override_bytecodes_dir: Option<String>,
    /// Enable bytecode compression
    pub bytecode_compression: bool,
    /// Enables EVM emulation mode
    pub use_evm_emulator: bool,
    /// Enables ZKOS mode (experimental)
    pub use_zkos: bool,
    /// Optional chain ID for the node
    pub chain_id: Option<u32>,
    /// L1 gas price (optional override)
    pub l1_gas_price: Option<u64>,
    /// L2 gas price (optional override)
    pub l2_gas_price: Option<u64>,
    /// Price for pubdata on L1
    pub l1_pubdata_price: Option<u64>,
    /// L1 gas price scale factor for gas estimation
    pub price_scale_factor: Option<f64>,
    /// The factor by which to scale the gasLimit
    pub limit_scale_factor: Option<f32>,
    /// Logging verbosity level
    pub log_level: LogLevel,
    /// Path to the log file
    pub log_file_path: String,
    /// Cache configuration for the test node
    pub cache_config: CacheConfig,
    /// Signer accounts that will be initialized with `genesis_balance` in the genesis block.
    pub genesis_accounts: Vec<PrivateKeySigner>,
    /// Native token balance of every genesis account in the genesis block
    pub genesis_balance: U256,
    /// The generator used to generate the dev accounts
    pub account_generator: Option<AccountGenerator>,
    /// Signer accounts that can sign messages/transactions
    pub signer_accounts: Vec<PrivateKeySigner>,
    /// The genesis to use to initialize the node
    pub genesis: Option<Genesis>,
    /// Genesis block timestamp
    pub genesis_timestamp: Option<u64>,
    /// Enable auto impersonation of accounts on startup
    pub enable_auto_impersonate: bool,
    /// Whether the node operates in offline mode
    pub offline: bool,
    /// The host the server will listen on
    pub host: Vec<IpAddr>,
    /// Whether we need to enable the health check endpoint.
    pub health_check_endpoint: bool,
    /// Block time in seconds for interval sealing.
    /// If unset, node seals a new block as soon as there is at least one transaction.
    pub block_time: Option<Duration>,
    /// Maximum number of transactions per block
    pub max_transactions: usize,
    /// Disable automatic sealing mode and use `BlockSealer::Noop` instead
    pub no_mining: bool,
    /// The cors `allow_origin` header
    pub allow_origin: String,
    /// Disable CORS if true
    pub no_cors: bool,
    /// How transactions are sorted in the mempool
    pub transaction_order: TransactionOrder,
    /// Path to load/dump the state from
    pub state: Option<PathBuf>,
    /// Path to dump the state to
    pub dump_state: Option<PathBuf>,
    /// Interval to dump the state
    pub state_interval: Option<u64>,
    /// Preserve historical states
    pub preserve_historical_states: bool,
    /// State to load
    pub load_state: Option<PathBuf>,
    /// L1 configuration, disabled if `None`
    pub l1_config: Option<L1Config>,
}

#[derive(Debug, Clone)]
pub enum L1Config {
    /// Spawn a separate `anvil` process and initialize it to use as L1.
    Spawn {
        /// Port the spawned L1 anvil node will listen on
        port: u16,
    },
    /// Use externally set up L1.
    External {
        /// Address of L1 node's JSON-RPC endpoint
        address: String,
    },
}

impl Default for TestNodeConfig {
    fn default() -> Self {
        // generate some random wallets
        let genesis_accounts = AccountGenerator::new(10).phrase(DEFAULT_MNEMONIC).gen();
        Self {
            // Node configuration defaults
            config_out: None,
            port: NODE_PORT,
            show_node_config: true,
            show_tx_summary: true,
            show_event_logs: false,
            disable_console_log: false,
            show_calls: Default::default(),
            show_outputs: false,
            show_storage_logs: Default::default(),
            show_vm_details: Default::default(),
            show_gas_details: Default::default(),
            resolve_hashes: false,
            silent: false,
            system_contracts_options: Default::default(),
            override_bytecodes_dir: None,
            bytecode_compression: false,
            use_evm_emulator: false,
            use_zkos: false,
            chain_id: None,

            // Gas configuration defaults
            l1_gas_price: None,
            l2_gas_price: None,
            l1_pubdata_price: None,
            price_scale_factor: None,
            limit_scale_factor: None,

            // Log configuration defaults
            log_level: Default::default(),
            log_file_path: String::from(DEFAULT_LOG_FILE_PATH),

            // Cache configuration default
            cache_config: Default::default(),

            // Account generator
            account_generator: None,
            genesis_accounts: genesis_accounts.clone(),
            signer_accounts: genesis_accounts,
            enable_auto_impersonate: false,
            // 100ETH default balance
            genesis_balance: U256::from(100u128 * 10u128.pow(18)),
            genesis_timestamp: Some(NON_FORK_FIRST_BLOCK_TIMESTAMP),
            genesis: None,

            // Offline mode disabled by default
            offline: false,
            host: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            health_check_endpoint: false,

            // Block sealing configuration default
            block_time: None,
            no_mining: false,

            max_transactions: 1000,
            transaction_order: TransactionOrder::Fifo,

            // Server configuration
            allow_origin: "*".to_string(),
            no_cors: false,

            // state configuration
            state: None,
            dump_state: None,
            state_interval: None,
            preserve_historical_states: false,
            load_state: None,
            l1_config: None,
        }
    }
}

impl TestNodeConfig {
    pub fn print(&self, fork_details: Option<&ForkPrintInfo>) {
        if let Some(config_out) = self.config_out.as_deref() {
            let file = File::create(config_out)
                .expect("Unable to create anvil-zksync config description file");
            to_writer(&file, &self.as_json(fork_details)).expect("Failed writing json");
        }

        if self.silent || !self.show_node_config {
            return;
        }

        let color = CustomColor::new(13, 71, 198);

        // Banner, version and repository section.
        sh_println!(
            r#"
{} 
Version:        {}
Repository:     {}

"#,
            BANNER.custom_color(color),
            VERSION_MESSAGE.green(),
            "https://github.com/matter-labs/anvil-zksync".green()
        );

        // Rich Accounts.
        let balance = format_eth(self.genesis_balance);
        let mut rich_accounts = String::new();
        for (idx, account) in self.genesis_accounts.iter().enumerate() {
            rich_accounts.push_str(&format!("({}) {} ({})\n", idx, account.address(), balance));
        }
        sh_println!(
            r#"
Rich Accounts
========================
{}
"#,
            rich_accounts
        );

        // Private Keys.
        let mut private_keys = String::new();
        for (idx, account) in self.genesis_accounts.iter().enumerate() {
            let private_key = hex::encode(account.credential().to_bytes());
            private_keys.push_str(&format!("({}) 0x{}\n", idx, private_key));
        }
        sh_println!(
            r#"
Private Keys
========================
{}
"#,
            private_keys
        );

        // Wallet configuration.
        if let Some(ref generator) = self.account_generator {
            sh_println!(
                r#"
Wallet
========================
Mnemonic:            {}
Derivation path:     {}
"#,
                generator.get_phrase().green(),
                generator.get_derivation_path().green()
            );
        }

        // Either print Fork Details (if provided) or the Network Configuration.
        if let Some(fd) = fork_details {
            sh_println!(
                r#"
Fork Details
========================
Network RPC:               {}
Chain ID:                  {}
L1 Batch #:                {}
L2 Block #:                {}
Block Timestamp:           {}
Fork Block Hash:           {}
Compute Overhead Part:     {}
Pubdata Overhead Part:     {}
Batch Overhead L1 Gas:     {}
Max Gas Per Batch:         {}
Max Pubdata Per Batch:     {}
"#,
                fd.network_rpc.green(),
                self.get_chain_id().to_string().green(),
                fd.l1_block.green(),
                fd.l2_block.green(),
                fd.block_timestamp.to_string().green(),
                format!("{:#}", fd.fork_block_hash).green(),
                fd.fee_model_config_v2
                    .compute_overhead_part
                    .to_string()
                    .green(),
                fd.fee_model_config_v2
                    .pubdata_overhead_part
                    .to_string()
                    .green(),
                fd.fee_model_config_v2
                    .batch_overhead_l1_gas
                    .to_string()
                    .green(),
                fd.fee_model_config_v2.max_gas_per_batch.to_string().green(),
                fd.fee_model_config_v2
                    .max_pubdata_per_batch
                    .to_string()
                    .green()
            );
        } else {
            sh_println!(
                r#"
Network Configuration
========================
Chain ID: {}
"#,
                self.chain_id
                    .unwrap_or(TEST_NODE_NETWORK_ID)
                    .to_string()
                    .green()
            );
        }

        // Gas Configuration.
        sh_println!(
            r#"
Gas Configuration
========================
L1 Gas Price (gwei):               {}
L2 Gas Price (gwei):               {}
L1 Pubdata Price (gwei):           {}
Estimated Gas Price Scale Factor:  {}
Estimated Gas Limit Scale Factor:  {}
"#,
            format_gwei(self.get_l1_gas_price().into()).green(),
            format_gwei(self.get_l2_gas_price().into()).green(),
            format_gwei(self.get_l1_pubdata_price().into()).green(),
            self.get_price_scale().to_string().green(),
            self.get_gas_limit_scale().to_string().green()
        );

        // Genesis Timestamp.
        sh_println!(
            r#"
Genesis Timestamp
========================
{}
"#,
            self.get_genesis_timestamp().to_string().green()
        );

        // Node Configuration.
        sh_println!(
            r#"
Node Configuration
========================
Port:                  {}
EVM Emulator:          {}
Health Check Endpoint: {}
ZK OS:                 {}
L1:                    {}
"#,
            self.port,
            if self.use_evm_emulator {
                "Enabled".green()
            } else {
                "Disabled".red()
            },
            if self.health_check_endpoint {
                "Enabled".green()
            } else {
                "Disabled".red()
            },
            if self.use_zkos {
                "Enabled".green()
            } else {
                "Disabled".red()
            },
            if self.l1_config.is_some() {
                "Enabled".green()
            } else {
                "Disabled".red()
            }
        );

        // L1 Configuration
        match self.l1_config.as_ref() {
            Some(L1Config::Spawn { port }) => {
                sh_println!(
                    r#"
L1 Configuration (Spawned)
========================
Port: {port}
"#
                );
            }
            Some(L1Config::External { address }) => {
                sh_println!(
                    r#"
L1 Configuration (External)
========================
Address: {address}
"#
                );
            }
            None => {}
        }

        // Listening addresses.
        let mut listening = String::new();
        listening.push_str("\n========================================\n");
        for host in &self.host {
            listening.push_str(&format!(
                "  Listening on {}:{}\n",
                host.to_string().green(),
                self.port.to_string().green()
            ));
        }
        listening.push_str("========================================\n");
        sh_println!("{}", listening);
    }

    fn as_json(&self, fork: Option<&ForkPrintInfo>) -> Value {
        let mut wallet_description = HashMap::new();
        let mut available_accounts = Vec::with_capacity(self.genesis_accounts.len());
        let mut private_keys = Vec::with_capacity(self.genesis_accounts.len());

        for wallet in &self.genesis_accounts {
            available_accounts.push(format!("{:?}", wallet.address()));
            private_keys.push(format!("0x{}", hex::encode(wallet.credential().to_bytes())));
        }

        if let Some(ref gen) = self.account_generator {
            let phrase = gen.get_phrase().to_string();
            let derivation_path = gen.get_derivation_path().to_string();

            wallet_description.insert("derivation_path".to_string(), derivation_path);
            wallet_description.insert("mnemonic".to_string(), phrase);
        };

        if let Some(fork) = fork {
            json!({
              "available_accounts": available_accounts,
              "private_keys": private_keys,
              "endpoint": fork.network_rpc,
              "l1_block": fork.l1_block,
              "l2_block": fork.l2_block,
              "block_hash": fork.fork_block_hash,
              "chain_id": self.get_chain_id(),
              "wallet": wallet_description,
              "l1_gas_price": format!("{}", self.get_l1_gas_price()),
              "l2_gas_price": format!("{}", self.get_l2_gas_price()),
              "l1_pubdata_price": format!("{}", self.get_l1_pubdata_price()),
              "price_scale_factor": format!("{}", self.get_price_scale()),
              "limit_scale_factor": format!("{}", self.get_gas_limit_scale()),
              "fee_model_config_v2": fork.fee_model_config_v2,
            })
        } else {
            json!({
              "available_accounts": available_accounts,
              "private_keys": private_keys,
              "wallet": wallet_description,
              "chain_id": self.get_chain_id(),
              "l1_gas_price": format!("{}", self.get_l1_gas_price()),
              "l2_gas_price": format!("{}", self.get_l2_gas_price()),
              "l1_pubdata_price": format!("{}", self.get_l1_pubdata_price()),
              "price_scale_factor": format!("{}", self.get_price_scale()),
              "limit_scale_factor": format!("{}", self.get_gas_limit_scale()),
            })
        }
    }

    /// Sets the file path to write the anvil-zksync config info to.
    #[must_use]
    pub fn set_config_out(mut self, config_out: Option<String>) -> Self {
        self.config_out = config_out;
        self
    }

    /// Set the port for the test node
    #[must_use]
    pub fn with_port(mut self, port: Option<u16>) -> Self {
        if let Some(port) = port {
            self.port = port;
        }
        self
    }

    /// Get the port for the test node
    pub fn get_port(&self) -> u16 {
        self.port
    }

    /// Set the chain ID for the test node
    #[must_use]
    pub fn with_chain_id(mut self, chain_id: Option<u32>) -> Self {
        if let Some(chain_id) = chain_id {
            self.chain_id = Some(chain_id);
        }
        self
    }

    /// Get the chain ID for the test node
    pub fn get_chain_id(&self) -> u32 {
        self.chain_id.unwrap_or(TEST_NODE_NETWORK_ID)
    }

    /// Update the chain ID
    pub fn update_chain_id(&mut self, chain_id: Option<u32>) -> &mut Self {
        self.chain_id = chain_id;
        self
    }

    /// Set the system contracts configuration option
    #[must_use]
    pub fn with_system_contracts(mut self, option: Option<SystemContractsOptions>) -> Self {
        if let Some(option) = option {
            self.system_contracts_options = option;
        }
        self
    }

    /// Get the system contracts configuration option
    pub fn get_system_contracts(&self) -> SystemContractsOptions {
        self.system_contracts_options
    }

    /// Set the override bytecodes directory
    #[must_use]
    pub fn with_override_bytecodes_dir(mut self, dir: Option<String>) -> Self {
        if let Some(dir) = dir {
            self.override_bytecodes_dir = Some(dir);
        }
        self
    }

    /// Get the override bytecodes directory
    pub fn get_override_bytecodes_dir(&self) -> Option<&String> {
        self.override_bytecodes_dir.as_ref()
    }

    /// Set the bytecode compression option
    #[must_use]
    pub fn with_bytecode_compression(mut self, enable: Option<bool>) -> Self {
        if let Some(enable) = enable {
            self.bytecode_compression = enable;
        }
        self
    }

    /// Get the bytecode compression option
    pub fn is_bytecode_compression_enabled(&self) -> bool {
        self.bytecode_compression
    }

    /// Enable or disable EVM emulation
    #[must_use]
    pub fn with_evm_emulator(mut self, enable: Option<bool>) -> Self {
        if let Some(enable) = enable {
            self.use_evm_emulator = enable;
        }
        self
    }

    /// Get the EVM emulation status
    pub fn is_evm_emulator_enabled(&self) -> bool {
        self.use_evm_emulator
    }

    /// Set the L1 gas price
    #[must_use]
    pub fn with_l1_gas_price(mut self, price: Option<u64>) -> Self {
        if let Some(price) = price {
            self.l1_gas_price = Some(price);
        }
        self
    }

    /// Get the L1 gas price
    pub fn get_l1_gas_price(&self) -> u64 {
        self.l1_gas_price.unwrap_or(DEFAULT_L1_GAS_PRICE)
    }

    /// Update the L1 gas price
    pub fn update_l1_gas_price(&mut self, price: Option<u64>) -> &mut Self {
        self.l1_gas_price = price;
        self
    }

    /// Set the L2 gas price
    #[must_use]
    pub fn with_l2_gas_price(mut self, price: Option<u64>) -> Self {
        if let Some(price) = price {
            self.l2_gas_price = Some(price);
        }
        self
    }

    /// Get the L2 gas price
    pub fn get_l2_gas_price(&self) -> u64 {
        self.l2_gas_price.unwrap_or(DEFAULT_L2_GAS_PRICE)
    }

    /// Update the L2 gas price
    pub fn update_l2_gas_price(&mut self, price: Option<u64>) -> &mut Self {
        self.l2_gas_price = price;
        self
    }

    /// Set the L1 pubdata price
    #[must_use]
    pub fn with_l1_pubdata_price(mut self, price: Option<u64>) -> Self {
        self.l1_pubdata_price = price;
        self
    }

    /// Get the L1 pubdata price
    pub fn get_l1_pubdata_price(&self) -> u64 {
        self.l1_pubdata_price.unwrap_or(DEFAULT_FAIR_PUBDATA_PRICE)
    }

    /// Update the L1 pubdata price
    pub fn update_l1_pubdata_price(&mut self, price: Option<u64>) -> &mut Self {
        self.l1_pubdata_price = price;
        self
    }

    /// Set the log level
    #[must_use]
    pub fn with_log_level(mut self, level: Option<LogLevel>) -> Self {
        if let Some(level) = level {
            self.log_level = level;
        }
        self
    }

    /// Get the log level
    pub fn get_log_level(&self) -> LogLevel {
        self.log_level
    }

    /// Set the cache configuration
    #[must_use]
    pub fn with_cache_config(mut self, config: Option<CacheConfig>) -> Self {
        if let Some(config) = config {
            self.cache_config = config;
        }
        self
    }

    /// Get the cache configuration
    pub fn get_cache_config(&self) -> &CacheConfig {
        &self.cache_config
    }

    /// Set the log file path
    #[must_use]
    pub fn with_log_file_path(mut self, path: Option<String>) -> Self {
        if let Some(path) = path {
            self.log_file_path = path;
        }
        self
    }

    /// Get the log file path
    pub fn get_log_file_path(&self) -> &str {
        &self.log_file_path
    }

    /// Applies the defaults for debug mode.
    #[must_use]
    pub fn with_debug_mode(mut self) -> Self {
        self.show_calls = ShowCalls::User;
        self.resolve_hashes = true;
        self.show_gas_details = ShowGasDetails::All;
        self
    }

    /// Set the visibility of call logs
    #[must_use]
    pub fn with_show_calls(mut self, show_calls: Option<ShowCalls>) -> Self {
        if let Some(show_calls) = show_calls {
            self.show_calls = show_calls;
        }
        self
    }

    /// Get the visibility of call logs
    pub fn get_show_calls(&self) -> ShowCalls {
        self.show_calls
    }

    /// Enable or disable resolving hashes
    #[must_use]
    pub fn with_resolve_hashes(mut self, resolve: Option<bool>) -> Self {
        if let Some(resolve) = resolve {
            self.resolve_hashes = resolve;
        }
        self
    }

    /// Enable or disable silent mode
    #[must_use]
    pub fn with_silent(mut self, silent: Option<bool>) -> Self {
        if let Some(silent) = silent {
            self.silent = silent;
        }
        self
    }

    /// Enable or disable printing node config on startup
    #[must_use]
    pub fn with_show_node_config(mut self, show_node_config: Option<bool>) -> Self {
        if let Some(show_node_config) = show_node_config {
            self.show_node_config = show_node_config;
        }
        self
    }

    // Enable or disable printing transactions and calls summary
    #[must_use]
    pub fn with_show_tx_summary(mut self, show_tx_summary: Option<bool>) -> Self {
        if let Some(show_tx_summary) = show_tx_summary {
            self.show_tx_summary = show_tx_summary;
        }
        self
    }
    /// Enable or disable logging events
    #[must_use]
    pub fn with_show_event_logs(mut self, show_event_logs: Option<bool>) -> Self {
        if let Some(show_event_logs) = show_event_logs {
            self.show_event_logs = show_event_logs;
        }
        self
    }

    /// Get the visibility of event logs
    pub fn get_show_event_logs(&self) -> bool {
        self.show_event_logs
    }

    // Enable or disable printing of `console.log` invocations to stdout
    #[must_use]
    pub fn with_disable_console_log(mut self, disable_console_log: Option<bool>) -> Self {
        if let Some(disable_console_log) = disable_console_log {
            self.disable_console_log = disable_console_log;
        }
        self
    }

    /// Check if resolving hashes is enabled
    pub fn is_resolve_hashes_enabled(&self) -> bool {
        self.resolve_hashes
    }

    /// Set the visibility of storage logs
    #[must_use]
    pub fn with_show_storage_logs(mut self, show_storage_logs: Option<ShowStorageLogs>) -> Self {
        if let Some(show_storage_logs) = show_storage_logs {
            self.show_storage_logs = show_storage_logs;
        }
        self
    }

    /// Get the visibility of storage logs
    pub fn get_show_storage_logs(&self) -> ShowStorageLogs {
        self.show_storage_logs
    }

    /// Set the detail level of VM execution logs
    #[must_use]
    pub fn with_vm_log_detail(mut self, detail: Option<ShowVMDetails>) -> Self {
        if let Some(detail) = detail {
            self.show_vm_details = detail;
        }
        self
    }

    /// Get the detail level of VM execution logs
    pub fn get_vm_log_detail(&self) -> ShowVMDetails {
        self.show_vm_details
    }

    /// Set the visibility of gas usage logs
    #[must_use]
    pub fn with_show_gas_details(mut self, show_gas_details: Option<ShowGasDetails>) -> Self {
        if let Some(show_gas_details) = show_gas_details {
            self.show_gas_details = show_gas_details;
        }
        self
    }

    /// Get the visibility of gas usage logs
    pub fn get_show_gas_details(&self) -> ShowGasDetails {
        self.show_gas_details
    }

    /// Set show outputs
    #[must_use]
    pub fn with_show_outputs(mut self, show_outputs: Option<bool>) -> Self {
        if let Some(show_outputs) = show_outputs {
            self.show_outputs = show_outputs;
        }
        self
    }

    /// Get show outputs
    pub fn get_show_outputs(&self) -> bool {
        self.show_outputs
    }

    /// Set the gas limit scale factor
    #[must_use]
    pub fn with_gas_limit_scale(mut self, scale: Option<f32>) -> Self {
        if let Some(scale) = scale {
            self.limit_scale_factor = Some(scale);
        }
        self
    }

    /// Get the gas limit scale factor
    pub fn get_gas_limit_scale(&self) -> f32 {
        self.limit_scale_factor
            .unwrap_or(DEFAULT_ESTIMATE_GAS_SCALE_FACTOR)
    }

    /// Update the gas limit scale factor
    pub fn update_gas_limit_scale(&mut self, scale: Option<f32>) -> &mut Self {
        self.limit_scale_factor = scale;
        self
    }

    /// Set the price scale factor
    #[must_use]
    pub fn with_price_scale(mut self, scale: Option<f64>) -> Self {
        if let Some(scale) = scale {
            self.price_scale_factor = Some(scale);
        }
        self
    }

    /// Get the price scale factor
    pub fn get_price_scale(&self) -> f64 {
        self.price_scale_factor
            .unwrap_or(DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR)
    }

    /// Updates the price scale factor
    pub fn update_price_scale(&mut self, scale: Option<f64>) -> &mut Self {
        self.price_scale_factor = scale;
        self
    }

    /// Sets the balance of the genesis accounts in the genesis block
    #[must_use]
    pub fn with_genesis_balance<U: Into<U256>>(mut self, balance: U) -> Self {
        self.genesis_balance = balance.into();
        self
    }

    /// Sets the genesis accounts.
    #[must_use]
    pub fn with_genesis_accounts(mut self, accounts: Vec<PrivateKeySigner>) -> Self {
        self.genesis_accounts = accounts;
        self
    }

    /// Sets the signer accounts
    #[must_use]
    pub fn with_signer_accounts(mut self, accounts: Vec<PrivateKeySigner>) -> Self {
        self.signer_accounts = accounts;
        self
    }

    /// Sets both the genesis accounts and the signer accounts
    /// so that `genesis_accounts == accounts`
    #[must_use]
    pub fn with_account_generator(mut self, generator: AccountGenerator) -> Self {
        let accounts = generator.gen();
        self.account_generator = Some(generator);
        self.with_signer_accounts(accounts.clone())
            .with_genesis_accounts(accounts)
    }

    /// Sets the genesis timestamp
    #[must_use]
    pub fn with_genesis_timestamp(mut self, timestamp: Option<u64>) -> Self {
        self.genesis_timestamp = timestamp;
        self
    }

    /// Returns the genesis timestamp to use
    pub fn get_genesis_timestamp(&self) -> u64 {
        self.genesis_timestamp
            .unwrap_or(NON_FORK_FIRST_BLOCK_TIMESTAMP)
    }

    /// Sets the init genesis (genesis.json)
    #[must_use]
    pub fn with_genesis(mut self, genesis: Option<Genesis>) -> Self {
        self.genesis = genesis;
        self
    }

    /// Sets whether to enable autoImpersonate
    #[must_use]
    pub fn with_auto_impersonate(mut self, enable_auto_impersonate: bool) -> Self {
        self.enable_auto_impersonate = enable_auto_impersonate;
        self
    }

    /// Set the offline mode
    #[must_use]
    pub fn with_offline(mut self, offline: Option<bool>) -> Self {
        if let Some(offline) = offline {
            self.offline = offline;
        }
        self
    }

    /// Get the offline mode status
    pub fn is_offline(&self) -> bool {
        self.offline
    }

    /// Sets the host the server will listen on
    #[must_use]
    pub fn with_host(mut self, host: Vec<IpAddr>) -> Self {
        self.host = if host.is_empty() {
            vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]
        } else {
            host
        };
        self
    }
    /// Set the health check endpoint mode
    #[must_use]
    pub fn with_health_check_endpoint(mut self, health_check_endpoint: Option<bool>) -> Self {
        if let Some(health_check_endpoint) = health_check_endpoint {
            self.health_check_endpoint = health_check_endpoint;
        }
        self
    }

    /// Get the health check endpoint mode status
    pub fn is_health_check_endpoint_endpoint_enabled(&self) -> bool {
        self.health_check_endpoint
    }

    /// Set the block time
    #[must_use]
    pub fn with_block_time(mut self, block_time: Option<Duration>) -> Self {
        self.block_time = block_time;
        self
    }

    /// If set to `true` auto sealing will be disabled
    #[must_use]
    pub fn with_no_mining(mut self, no_mining: bool) -> Self {
        self.no_mining = no_mining;
        self
    }

    // Set transactions order in the mempool
    #[must_use]
    pub fn with_transaction_order(mut self, transaction_order: TransactionOrder) -> Self {
        self.transaction_order = transaction_order;
        self
    }

    /// Set allow_origin CORS header
    #[must_use]
    pub fn with_allow_origin(mut self, allow_origin: String) -> Self {
        self.allow_origin = allow_origin;
        self
    }

    /// Enable or disable CORS
    #[must_use]
    pub fn with_no_cors(mut self, no_cors: bool) -> Self {
        self.no_cors = no_cors;
        self
    }

    /// Set the state
    #[must_use]
    pub fn with_state(mut self, state: Option<PathBuf>) -> Self {
        self.state = state;
        self
    }

    /// Set the state dump path
    #[must_use]
    pub fn with_dump_state(mut self, dump_state: Option<PathBuf>) -> Self {
        self.dump_state = dump_state;
        self
    }

    /// Set the state dump interval
    #[must_use]
    pub fn with_state_interval(mut self, state_interval: Option<u64>) -> Self {
        self.state_interval = state_interval;
        self
    }

    /// Set preserve historical states
    #[must_use]
    pub fn with_preserve_historical_states(mut self, preserve_historical_states: bool) -> Self {
        self.preserve_historical_states = preserve_historical_states;
        self
    }

    /// Set the state to load
    #[must_use]
    pub fn with_load_state(mut self, load_state: Option<PathBuf>) -> Self {
        self.load_state = load_state;
        self
    }

    /// Set the L1 config
    #[must_use]
    pub fn with_l1_config(mut self, l1_config: Option<L1Config>) -> Self {
        self.l1_config = l1_config;
        self
    }
}
