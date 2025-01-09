use clap::ValueEnum;
use core::fmt;
use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;

/// Log filter level for the node.
#[derive(Default, Debug, Copy, Clone, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LogLevel::Trace => f.pad("TRACE"),
            LogLevel::Debug => f.pad("DEBUG"),
            LogLevel::Info => f.pad("INFO"),
            LogLevel::Warn => f.pad("WARN"),
            LogLevel::Error => f.pad("ERROR"),
        }
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}
