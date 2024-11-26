use clap::ValueEnum;
use core::fmt;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::{fs::File, sync::Mutex};
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, reload, util::SubscriberInitExt, EnvFilter, Registry,
};

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

/// A sharable reference to the observability stack.
#[derive(Debug, Clone)]
pub struct Observability {
    binary_names: Vec<String>,
    reload_handle: reload::Handle<EnvFilter, Registry>,
    /// Last directives used to reload the underlying `EnvFilter` instance.
    last_directives: Arc<RwLock<String>>,
}

impl Observability {
    /// Initialize the tracing subscriber.
    pub fn init(
        binary_names: Vec<String>,
        log_level_filter: LevelFilter,
        log_file: File,
    ) -> Result<Self, anyhow::Error> {
        let directives = binary_names
            .iter()
            .map(|x| format!("{}={}", x, log_level_filter.to_string().to_lowercase()))
            .collect::<Vec<String>>()
            .join(",");
        let filter = Self::parse_filter(&directives)?;
        let (filter, reload_handle) = reload::Layer::new(filter);

        let timer_format =
            time::format_description::parse("[hour]:[minute]:[second]").expect("Cataplum");
        let time_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
        let timer = tracing_subscriber::fmt::time::OffsetTime::new(time_offset, timer_format);

        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer().event_format(
                    tracing_subscriber::fmt::format()
                        .compact()
                        .with_timer(timer.clone())
                        .with_target(false),
                ),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .event_format(
                        tracing_subscriber::fmt::format()
                            .compact()
                            .with_timer(timer.clone())
                            .with_target(false),
                    )
                    .with_writer(Mutex::new(log_file))
                    .with_ansi(false),
            )
            .init();

        Ok(Self {
            binary_names,
            reload_handle,
            last_directives: Arc::new(RwLock::new(directives)),
        })
    }

    /// Set the log level for the binary.
    pub fn set_log_level(&self, level: LogLevel) -> anyhow::Result<()> {
        let level = LevelFilter::from(level);
        let directives = self
            .binary_names
            .join(&format!("={},", level.to_string().to_lowercase()));
        let new_filter = Self::parse_filter(&directives)?;
        self.reload_handle.reload(new_filter)?;
        *self
            .last_directives
            .write()
            .expect("Observability lock is poisoned") = directives;

        Ok(())
    }

    /// Sets advanced logging directive.
    /// Example:
    ///     * "my_crate=debug"
    ///     * "my_crate::module=trace"
    ///     * "my_crate=debug,other_crate=warn"
    pub fn set_logging(&self, directives: String) -> Result<(), anyhow::Error> {
        let new_filter = Self::parse_filter(&directives)?;
        self.reload_handle.reload(new_filter)?;
        *self
            .last_directives
            .write()
            .expect("Observability lock is poisoned") = directives;

        Ok(())
    }

    /// Parses a directive and builds an [EnvFilter] from it.
    /// Example:
    ///     * "my_crate=debug"
    ///     * "my_crate::module=trace"
    ///     * "my_crate=debug,other_crate=warn"
    fn parse_filter(directives: &str) -> Result<EnvFilter, anyhow::Error> {
        let mut filter = EnvFilter::from_default_env();
        for directive in directives.split(',') {
            filter = filter.add_directive(directive.parse()?);
        }

        Ok(filter)
    }

    /// Enables logging with the latest used directives.
    pub fn enable_logging(&self) -> Result<(), anyhow::Error> {
        let last_directives = &*self
            .last_directives
            .read()
            .expect("Observability lock is poisoned");
        let new_filter = Self::parse_filter(&last_directives)?;
        self.reload_handle.reload(new_filter)?;

        Ok(())
    }

    /// Disables all logging.
    pub fn disable_logging(&self) -> Result<(), anyhow::Error> {
        let new_filter = EnvFilter::new("off");
        self.reload_handle.reload(new_filter)?;

        Ok(())
    }
}
