use anvil_zksync_types::LogLevel;
use std::sync::{Arc, RwLock};
use std::{fs::File, sync::Mutex};
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, reload, util::SubscriberInitExt, EnvFilter, Registry,
};

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
        disabled: bool,
    ) -> Result<Self, anyhow::Error> {
        let directives = binary_names
            .iter()
            .map(|x| format!("{}={}", x, log_level_filter.to_string().to_lowercase()))
            .collect::<Vec<String>>()
            .join(",");
        let filter = if disabled {
            EnvFilter::new("off")
        } else {
            Self::parse_filter(&directives)?
        };
        let (filter, reload_handle) = reload::Layer::<EnvFilter, Registry>::new(filter);

        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .event_format(tracing_subscriber::fmt::format().without_time())
                    .with_ansi(false),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .event_format(tracing_subscriber::fmt::format().without_time())
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
        let new_filter = Self::parse_filter(last_directives)?;
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
