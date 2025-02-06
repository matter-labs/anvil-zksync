use std::{ffi::OsStr, fmt::Display, path::Path, time::Duration};

use anyhow::Context;
use chrono::{DateTime, Local};
use tokio::process::{Child, Command};

use crate::utils::LockedPort;

const ANVIL_ZKSYNC_BINARY_DEFAULT_PATH: &str = "../target/release/anvil-zksync";
const ANVIL_ZKSYNC_SRC_PATH: &str = "../src";

pub struct EraRunConfig {
    pub rpc_port: u16,
}

pub struct EraRunHandle {
    pub config: EraRunConfig,
    process: Option<Child>,
}

impl Drop for EraRunHandle {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            tracing::info!("Cleaning up anvil-zksync process: pid={:?}", process.id());

            process.start_kill().expect("failed to kill anvil-zksync");
            let _ = process.try_wait();
        }
    }
}

pub fn run<S: AsRef<OsStr> + Clone + Display>(
    bin_path: S,
    config: EraRunConfig,
) -> anyhow::Result<EraRunHandle> {
    let mut options = Vec::new();
    options.push(format!("--port={}", config.rpc_port));
    // TODO: parametrize log file, cache file etc so simultaneous nodes don't compete
    options.push("run".to_string());
    tracing::info!(bin_path = %bin_path, rpc_port = config.rpc_port, "Starting anvil-zksync");
    let process = Command::new(bin_path.clone())
        .args(options)
        .spawn()
        .with_context(|| format!("failed to run anvil-zksync using '{}'", bin_path))?;
    Ok(EraRunHandle {
        config,
        process: Some(process),
    })
}

/// Ensures that the anvil-zksync binary was built after the last source file got modified.
fn ensure_binary_is_fresh() -> anyhow::Result<()> {
    if !Path::new(ANVIL_ZKSYNC_BINARY_DEFAULT_PATH).exists() {
        anyhow::bail!(
            "Expected anvil-zksync binary to be built and present at '{}'. Please run `make all` in the root directory.",
            ANVIL_ZKSYNC_BINARY_DEFAULT_PATH
        );
    }
    let metadata = std::fs::metadata(ANVIL_ZKSYNC_BINARY_DEFAULT_PATH)?;
    match metadata.modified() {
        Ok(binary_mod_time) => {
            let binary_mod_time = DateTime::<Local>::from(binary_mod_time);
            tracing::info!(
                %binary_mod_time,
                path = ANVIL_ZKSYNC_BINARY_DEFAULT_PATH,
                "Resolved when binary file was last modified"
            );
            let source_mod_time = std::fs::read_dir(ANVIL_ZKSYNC_SRC_PATH)
                .context("couldn't access anvil-zksync source directory")?
                .map(|entry| entry.and_then(|f| f.metadata()).and_then(|f| f.modified()))
                .collect::<std::io::Result<Vec<_>>>()
                .context("couldn't get one of the anvil-zksync source file's metadata")?
                .into_iter()
                .max();
            if let Some(source_mod_time) = source_mod_time {
                let source_mod_time = DateTime::<Local>::from(source_mod_time);
                tracing::info!(
                    %source_mod_time,
                    path = ANVIL_ZKSYNC_SRC_PATH,
                    "Resolved when source files were last modified"
                );
                if binary_mod_time < source_mod_time {
                    // TODO: invoke `make all` for the user automatically?
                    anyhow::bail!(
                        "Source files have been recently modified (source last modified at '{}', binary last modified at '{}'). \
                        Please re-build anvil-zksync binary by running `make all` in the root directory.",
                        source_mod_time,
                        binary_mod_time,
                    );
                }
            } else {
                tracing::warn!(
                    path = ANVIL_ZKSYNC_SRC_PATH,
                    "No files found under the source directory"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                %error,
                path = ANVIL_ZKSYNC_BINARY_DEFAULT_PATH,
                "Could not get modification time from file (your platform might not support it, refer to the attached error). \
                Make sure that your binary has been built against the code you are working with."
            );
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct AnvilZKsyncRunner {
    path: Option<String>,
    rpc_port: Option<u16>,
}

impl AnvilZKsyncRunner {
    pub fn path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    pub fn rpc_port(mut self, rpc_port: u16) -> Self {
        self.rpc_port = Some(rpc_port);
        self
    }

    pub async fn run(self) -> anyhow::Result<EraRunHandle> {
        let path = match self.path {
            Some(path) => path,
            None => {
                if let Ok(path) = std::env::var("ANVIL_ZKSYNC_BINARY_PATH") {
                    path
                } else {
                    // Default to the binary taken from the target directory
                    ensure_binary_is_fresh()?;
                    ANVIL_ZKSYNC_BINARY_DEFAULT_PATH.to_string()
                }
            }
        };
        let rpc_port_lock = match self.rpc_port {
            Some(rpc_port) => LockedPort::acquire(rpc_port).await?,
            None => {
                if let Ok(rpc_port) = std::env::var("ANVIL_ZKSYNC_RPC_PORT") {
                    LockedPort::acquire(rpc_port.parse().context(
                        "failed to parse `ANVIL_ZKSYNC_RPC_PORT` var as a valid port number",
                    )?)
                    .await?
                } else {
                    LockedPort::acquire_unused().await?
                }
            }
        };

        let config = EraRunConfig {
            rpc_port: rpc_port_lock.port,
        };
        let handle = run(path, config)?;

        // TODO: Wait for anvil-zksync healthcheck instead
        tokio::time::sleep(Duration::from_secs(1)).await;

        Ok(handle)
    }
}
