use serde::Deserialize;

/// Boojum OS configuration.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct BoojumConfig {
    /// Enables boojum (experimental).
    pub use_boojum: bool,

    /// Path to boojum binary (if you need to compute witnesses).
    pub boojum_bin_path: Option<String>,
}
