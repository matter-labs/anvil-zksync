mod account_generator;
mod cache;
mod genesis;
mod log;
mod show_details;

pub use account_generator::AccountGenerator;
pub use cache::{CacheConfig, CacheType};
use clap::ValueEnum;
pub use genesis::Genesis;
pub use log::LogLevel;
use serde::Deserialize;
pub use show_details::{ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};

#[derive(Deserialize, Default, Debug, Copy, Clone, PartialEq, ValueEnum)]
pub enum SystemContractsOptions {
    // Use the compiled-in contracts
    #[default]
    BuiltIn,
    // Load the contracts bytecode at runtime from ZKSYNC_HOME
    Local,
    // Don't verify the signatures and return transaction result on calls (used only for testing - for example Forge).
    BuiltInWithoutSecurity,
}
