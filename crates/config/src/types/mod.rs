mod account_generator;
mod genesis;
mod zksync_os;

pub use account_generator::AccountGenerator;
use clap::ValueEnum;
pub use genesis::Genesis;
use serde::Deserialize;
pub use zksync_os::ZKsyncOsConfig;

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
