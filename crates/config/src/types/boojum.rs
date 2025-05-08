use clap::Parser;
use serde::Deserialize;

/// Genesis
#[derive(Deserialize, Clone, Debug, Parser, Default)]
pub struct BoojumConfig {
    #[arg(long, help_heading = "Experimental Configuration")]
    /// Enables boojum (experimental).
    pub use_boojum: bool,

    #[arg(long, help_heading = "Experimental Configuration")]
    /// Path to boojum binary (if you need to compute witnesses).
    pub boojum_bin_path: Option<String>,
}
