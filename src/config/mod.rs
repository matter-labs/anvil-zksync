use crate::fork::ForkDetails;
use crate::{observability, system_contracts};
use anyhow::anyhow;
use std::net::{IpAddr, Ipv4Addr};

use crate::config::{
    cache::{CacheConfig, CacheType},
    constants::*,
    show_details::*,
};
use crate::utils::{format_eth, format_gwei};
use alloy_signer::Signer;
use alloy_signer_local::{
    coins_bip39::{English, Mnemonic},
    MnemonicBuilder, PrivateKeySigner,
};
use colored::{Colorize, CustomColor};
use observability::LogLevel;
use rand::thread_rng;
use serde::Deserialize;
use serde_json::{json, to_writer, Value};
use std::collections::HashMap;
use std::fs::File;
use zksync_types::fee_model::FeeModelConfigV2;
use zksync_types::U256;

pub mod cache;
pub mod cli;
pub mod constants;
pub mod show_details;

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
    pub fee_model_config_v2: Option<FeeModelConfigV2>,
}

/// Defines the configuration parameters for the [InMemoryNode].
#[derive(Debug, Clone)]
pub struct TestNodeConfig {
    /// Filename to write era-test-node output as json
    pub config_out: Option<String>,
    /// Port the node will listen on
    pub port: u16,
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
    /// Configuration for system contracts
    pub system_contracts_options: system_contracts::Options,
    /// Directory to override bytecodes
    pub override_bytecodes_dir: Option<String>,
    /// Enables EVM emulation mode
    pub use_evm_emulator: bool,
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
    /// Enable auto impersonation of accounts on startup
    pub enable_auto_impersonate: bool,
    /// Whether the node operates in offline mode
    pub offline: bool,
    /// The host the server will listen on
    pub host: Vec<IpAddr>,
    /// Whether we need to enable the health check endpoint.
    pub health_check_endpoint: bool,
}

impl Default for TestNodeConfig {
    fn default() -> Self {
        // generate some random wallets
        let genesis_accounts = AccountGenerator::new(10).phrase(DEFAULT_MNEMONIC).gen();
        Self {
            // Node configuration defaults
            config_out: None,
            port: NODE_PORT,
            show_calls: Default::default(),
            show_outputs: false,
            show_storage_logs: Default::default(),
            show_vm_details: Default::default(),
            show_gas_details: Default::default(),
            resolve_hashes: false,
            system_contracts_options: Default::default(),
            override_bytecodes_dir: None,
            use_evm_emulator: false,
            chain_id: Some(TEST_NODE_NETWORK_ID),

            // Gas configuration defaults
            l1_gas_price: Some(DEFAULT_L1_GAS_PRICE),
            l2_gas_price: Some(DEFAULT_L2_GAS_PRICE),
            l1_pubdata_price: Some(DEFAULT_FAIR_PUBDATA_PRICE),
            price_scale_factor: Some(DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR),
            limit_scale_factor: Some(DEFAULT_ESTIMATE_GAS_SCALE_FACTOR),

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

            // Offline mode disabled by default
            offline: false,
            host: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            health_check_endpoint: false,
        }
    }
}

impl TestNodeConfig {
    pub fn print(&self, fork_details: Option<&ForkPrintInfo>) {
        if self.config_out.is_some() {
            let config_out = self.config_out.as_deref().unwrap();
            to_writer(
                &File::create(config_out)
                    .expect("Unable to create era-test-node config description file"),
                &self.as_json(fork_details),
            )
            .expect("Failed writing json");
        }
        let color = CustomColor::new(13, 71, 198);

        println!("{}", BANNER.custom_color(color));
        tracing::info!("Version:        {}", VERSION_MESSAGE.green());
        tracing::info!(
            "Repository:     {}",
            "https://github.com/matter-labs/era-test-node".green()
        );
        println!("\n");

        tracing::info!("Rich Accounts");
        tracing::info!("========================");
        let balance = format_eth(self.genesis_balance);
        for (idx, account) in self.genesis_accounts.iter().enumerate() {
            tracing::info!("({}) {} ({balance})", idx, account.address());
        }
        println!("\n");

        tracing::info!("Private Keys");
        tracing::info!("========================");
        for (idx, account) in self.genesis_accounts.iter().enumerate() {
            let private_key = hex::encode(account.credential().to_bytes());
            tracing::info!("({}) 0x{}", idx, private_key);
        }
        println!("\n");

        if let Some(ref generator) = self.account_generator {
            tracing::info!("Wallet");
            tracing::info!("========================");
            tracing::info!("Mnemonic:            {}", generator.phrase.green());
            tracing::info!(
                "Derivation path:     {}",
                generator
                    .derivation_path
                    .as_deref()
                    .unwrap_or(DERIVATION_PATH)
                    .green()
            );
        }
        println!("\n");

        if let Some(fd) = fork_details {
            tracing::info!("Fork Details");
            tracing::info!("========================");
            tracing::info!("Network RPC:               {}", fd.network_rpc.green());
            tracing::info!(
                "Chain ID:                  {}",
                self.get_chain_id().to_string().green()
            );
            tracing::info!("L1 Batch #:                {}", fd.l1_block.green());
            tracing::info!("L2 Block #:                {}", fd.l2_block.green());
            tracing::info!(
                "Block Timestamp:           {}",
                fd.block_timestamp.to_string().green()
            );
            tracing::info!(
                "Fork Block Hash:           {}",
                format!("{:#}", fd.fork_block_hash).green()
            );
            if let Some(fee_config) = &fd.fee_model_config_v2 {
                tracing::info!(
                    "Compute Overhead Part:     {}",
                    fee_config.compute_overhead_part.to_string().green()
                );
                tracing::info!(
                    "Pubdata Overhead Part:     {}",
                    fee_config.pubdata_overhead_part.to_string().green()
                );
                tracing::info!(
                    "Batch Overhead L1 Gas:     {}",
                    fee_config.batch_overhead_l1_gas.to_string().green()
                );
                tracing::info!(
                    "Max Gas Per Batch:         {}",
                    fee_config.max_gas_per_batch.to_string().green()
                );
                tracing::info!(
                    "Max Pubdata Per Batch:     {}",
                    fee_config.max_pubdata_per_batch.to_string().green()
                );
            }
            println!("\n");
        } else {
            tracing::info!("Network Configuration");
            tracing::info!("========================");
            tracing::info!(
                "Chain ID: {}",
                self.chain_id
                    .unwrap_or(TEST_NODE_NETWORK_ID)
                    .to_string()
                    .green()
            );
            println!("\n");
        }
        tracing::info!("Gas Configuration");
        tracing::info!("========================");
        tracing::info!(
            "L1 Gas Price (gwei):               {}",
            format_gwei(self.get_l1_gas_price().into()).green()
        );
        tracing::info!(
            "L2 Gas Price (gwei):               {}",
            format_gwei(self.get_l2_gas_price().into()).green()
        );
        tracing::info!(
            "L1 Pubdata Price (gwei):           {}",
            format_gwei(self.get_l1_pubdata_price().into()).green()
        );
        tracing::info!(
            "Estimated Gas Price Scale Factor:  {}",
            self.get_price_scale().to_string().green()
        );
        tracing::info!(
            "Estimated Gas Limit Scale Factor:  {}",
            self.get_gas_limit_scale().to_string().green()
        );
        println!("\n");

        tracing::info!("Node Configuration");
        tracing::info!("========================");
        tracing::info!("Port:               {}", self.port);
        tracing::info!(
            "EVM Emulator:       {}",
            if self.use_evm_emulator {
                "Enabled".green()
            } else {
                "Disabled".red()
            }
        );
        tracing::info!(
            "Health Check Endpoint: {}",
            if self.health_check_endpoint {
                "Enabled".green()
            } else {
                "Disabled".red()
            }
        );
        println!("\n");
        tracing::info!("========================================");
        for host in &self.host {
            tracing::info!(
                "  Listening on {}:{}",
                host.to_string().green(),
                self.port.to_string().green()
            );
        }
        tracing::info!("========================================");
        println!("\n");
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

    /// Sets the file path to write the Era-test-node's config info to.
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
    pub fn with_system_contracts(mut self, option: Option<system_contracts::Options>) -> Self {
        if let Some(option) = option {
            self.system_contracts_options = option;
        }
        self
    }

    /// Get the system contracts configuration option
    pub fn get_system_contracts(&self) -> system_contracts::Options {
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

    /// Check if resolving hashes is enabled
    pub fn is_resolve_hashes_enabled(&self) -> bool {
        self.resolve_hashes
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

    /// Updates the configuration from fork details.
    pub async fn update_with_fork_details(
        &mut self,
        fork_details_result: Result<ForkDetails, eyre::Report>,
    ) -> Result<Option<ForkDetails>, anyhow::Error> {
        match fork_details_result {
            Ok(fd) => {
                self.update_l1_gas_price(Some(fd.l1_gas_price))
                    .update_l2_gas_price(Some(fd.l2_fair_gas_price))
                    .update_l1_pubdata_price(Some(fd.fair_pubdata_price))
                    .update_price_scale(Some(fd.estimate_gas_price_scale_factor))
                    .update_gas_limit_scale(Some(fd.estimate_gas_scale_factor))
                    .update_chain_id(Some(fd.chain_id.as_u64() as u32));
                Ok(Some(fd))
            }
            Err(error) => {
                tracing::error!("Error while attempting to fork: {:?}", error);
                Err(anyhow!(error))
            }
        }
    }
}

/// Account Generator
/// Manages the generation of accounts for era-test-node
#[derive(Clone, Debug, Deserialize)]
pub struct AccountGenerator {
    chain_id: u32,
    amount: usize,
    phrase: String,
    derivation_path: Option<String>,
}

impl AccountGenerator {
    pub fn new(amount: usize) -> Self {
        Self {
            chain_id: TEST_NODE_NETWORK_ID,
            amount,
            phrase: Mnemonic::<English>::new(&mut thread_rng()).to_phrase(),
            derivation_path: None,
        }
    }

    #[must_use]
    pub fn phrase(mut self, phrase: impl Into<String>) -> Self {
        self.phrase = phrase.into();
        self
    }

    fn get_phrase(&self) -> &str {
        &self.phrase
    }

    #[must_use]
    pub fn chain_id(mut self, chain_id: impl Into<u32>) -> Self {
        self.chain_id = chain_id.into();
        self
    }

    #[must_use]
    pub fn derivation_path(mut self, derivation_path: impl Into<String>) -> Self {
        let mut derivation_path = derivation_path.into();
        if !derivation_path.ends_with('/') {
            derivation_path.push('/');
        }
        self.derivation_path = Some(derivation_path);
        self
    }

    fn get_derivation_path(&self) -> &str {
        self.derivation_path.as_deref().unwrap_or(DERIVATION_PATH)
    }

    pub fn gen(&self) -> Vec<PrivateKeySigner> {
        let builder = MnemonicBuilder::<English>::default().phrase(self.phrase.as_str());

        let derivation_path = self.derivation_path.as_deref().unwrap_or(DERIVATION_PATH);

        (0..self.amount)
            .map(|idx| {
                let builder = builder
                    .clone()
                    .derivation_path(format!("{derivation_path}{idx}"))
                    .unwrap();
                builder
                    .build()
                    .unwrap()
                    .with_chain_id(Some(self.chain_id.into()))
            })
            .collect()
    }
}
