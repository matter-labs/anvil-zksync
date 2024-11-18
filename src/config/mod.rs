use crate::{observability, system_contracts};

use crate::config::{
    cache::{CacheConfig, CacheType},
    constants::*,
    show_details::*,
};
use alloy_signer::Signer;
use alloy_signer_local::{
    coins_bip39::{English, Mnemonic},
    MnemonicBuilder, PrivateKeySigner,
};
use cli::Cli;
use observability::LogLevel;
use rand::thread_rng;
use serde::Deserialize;
use zksync_types::U256;

pub mod cache;
pub mod cli;
pub mod constants;
pub mod show_details;

/// Defines the configuration parameters for the [InMemoryNode].
#[derive(Debug, Clone)]
pub struct TestNodeConfig {
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
    // @dev these are essentially the RICH WALLETS
    /// Signer accounts that will be initialized with `genesis_balance` in the genesis block.
    pub genesis_accounts: Vec<PrivateKeySigner>,
    /// Native token balance of every genesis account in the genesis block
    pub genesis_balance: U256,
    /// The generator used to generate the dev accounts
    pub account_generator: Option<AccountGenerator>,
    /// Signer accounts that can sign messages/transactions
    pub signer_accounts: Vec<PrivateKeySigner>,
    /// Whether the node operates in offline mode
    pub offline: bool,
}

impl Default for TestNodeConfig {
    fn default() -> Self {
        // generate some random wallets
        // TODO: this should be RICH WALLETS?
        let genesis_accounts = AccountGenerator::new(10).phrase(DEFAULT_MNEMONIC).gen();
        Self {
            // Node configuration defaults
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
            // TODO: switch order here
            genesis_accounts: genesis_accounts.clone(),
            // 100ETH default balance
            genesis_balance: U256::from(100u128 * 10u128.pow(18)),
            account_generator: None,
            signer_accounts: genesis_accounts,
            offline: false,
        }
    }
}

impl TestNodeConfig {
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

    /// Sets the genesis accounts. TODO: this should be RICH WALLETS
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

    /// Override the config with values provided by [`Cli`].
    pub fn override_with_opts(&mut self, opt: &Cli) {
        // [`NodeConfig`].
        if let Some(port) = &opt.port {
            self.port = *port;
        }

        if opt.debug_mode {
            self.show_calls = ShowCalls::All;
            self.show_outputs = true;
            self.show_gas_details = ShowGasDetails::All;
            self.resolve_hashes = true;
        }
        if let Some(show_calls) = &opt.show_calls {
            self.show_calls = *show_calls;
        }
        if let Some(show_outputs) = &opt.show_outputs {
            self.show_outputs = *show_outputs;
        }
        if let Some(show_storage_logs) = &opt.show_storage_logs {
            self.show_storage_logs = *show_storage_logs;
        }
        if let Some(show_vm_details) = &opt.show_vm_details {
            self.show_vm_details = *show_vm_details;
        }
        if let Some(show_gas_details) = &opt.show_gas_details {
            self.show_gas_details = *show_gas_details;
        }
        if let Some(resolve_hashes) = &opt.resolve_hashes {
            self.resolve_hashes = *resolve_hashes;
        }

        if opt.chain_id.is_some() {
            self.chain_id = opt.chain_id;
        }

        if let Some(contract_options) = opt.dev_system_contracts {
            self.system_contracts_options = contract_options;
        }

        if opt.emulate_evm {
            assert_eq!(
                self.system_contracts_options,
                system_contracts::Options::Local,
                "EVM emulation currently requires using local contracts"
            );
            self.use_evm_emulator = true;
        }

        // [`GasConfig`]
        if let Some(l1_gas_price) = &opt.l1_gas_price {
            self.l1_gas_price = Some(*l1_gas_price);
        }
        if let Some(l2_gas_price) = &opt.l2_gas_price {
            self.l2_gas_price = Some(*l2_gas_price);
        }
        if let Some(l1_pubdata_price) = &opt.l1_pubdata_price {
            self.l1_pubdata_price = Some(*l1_pubdata_price);
        }
        if let Some(price_scale) = &opt.price_scale_factor {
            self.price_scale_factor = Some(*price_scale);
        }
        if let Some(limit_scale) = &opt.limit_scale_factor {
            self.limit_scale_factor = Some(*limit_scale);
        }

        // [`LogConfig`].
        if let Some(log_level) = &opt.log {
            self.log_level = *log_level;
        }
        if let Some(file_path) = &opt.log_file_path {
            self.log_file_path = file_path.to_string();
        }

        // [`CacheConfig`].
        if let Some(cache_type) = &opt.cache {
            self.cache_config = match cache_type {
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

/// Can create dev accounts
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
            chain_id: TEST_NODE_NETWORK_ID.into(),
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

    pub fn gen(&self) -> Vec<PrivateKeySigner> {
        let builder = MnemonicBuilder::<English>::default().phrase(self.phrase.as_str());

        let derivation_path = self.derivation_path.as_deref().unwrap_or("m/44'/60'/0'/0/");

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
