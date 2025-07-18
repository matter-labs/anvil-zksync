use serde::Deserialize;

/// ZKsync OS configuration.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct ZKsyncOsConfig {
    /// Enables ZKsync OS (experimental).
    pub zksync_os: bool,

    /// Path to ZKsync OS binary (if you need to compute witnesses).
    pub zksync_os_bin_path: Option<String>,
}
