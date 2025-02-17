use crate::constants::DEFAULT_DISK_CACHE_DIR;
use clap::ValueEnum;
use serde::Deserialize;

/// Cache type for the node.
#[derive(ValueEnum, Deserialize, Default, Debug, Copy, Clone, serde::Serialize, PartialEq)]
pub enum CacheType {
    None,
    Memory,
    #[default]
    Disk,
}

/// Cache configuration options.
#[derive(Deserialize, Debug, Clone)]
pub enum CacheConfig {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "memory")]
    Memory,
    #[serde(rename = "disk")]
    Disk { dir: String, reset: bool },
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self::Disk {
            dir: String::from(DEFAULT_DISK_CACHE_DIR),
            reset: false,
        }
    }
}
