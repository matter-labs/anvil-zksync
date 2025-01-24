use clap::Parser;
use serde::Deserialize;

/// Genesis
#[derive(Deserialize, Clone, Debug, Parser, Default)]
pub struct ZKOSConfig {
    #[arg(long, help_heading = "Experimental Configuration")]
    /// Enables zkos (experimental).
    pub use_zkos: bool,

    #[arg(long, help_heading = "Experimental Configuration")]
    /// Path to zkos binary (if you need to compute witnesses).
    pub zkos_bin_path: Option<String>,
}
