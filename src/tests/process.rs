use crate::tests::utils;
use anyhow::Context;
use fs2::FileExt;
use std::time::Duration;
use tokio::process::{Child, Command};

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
            tracing::info!("Cleaning up era-test-node process: pid={:?}", process.id());

            process.start_kill().expect("failed to kill era-test-node");
            let _ = process.try_wait();
        }
    }
}

pub fn run(bin_path: &str, config: EraRunConfig) -> anyhow::Result<EraRunHandle> {
    let mut options = Vec::new();
    options.push(format!("--port={}", config.rpc_port));
    // TODO: parametrize log file, cache file etc so simultaneous nodes don't compete
    options.push("run".to_string());
    let process = Command::new(bin_path)
        .args(options)
        .spawn()
        .with_context(|| format!("failed to run era-test-node using '{}'", bin_path))?;
    Ok(EraRunHandle {
        config,
        process: Some(process),
    })
}

pub async fn run_default() -> anyhow::Result<EraRunHandle> {
    let (rpc_port, rpc_port_lock) = utils::acquire_unused_port().await?;
    let config = EraRunConfig { rpc_port };
    // TODO: run checks that the binary exists and is up-to-date
    let handle = run("./target/release/era_test_node", config)?;

    // TODO: Wait for era-test-node healthcheck instead
    tokio::time::sleep(Duration::from_secs(1)).await;

    rpc_port_lock
        .unlock()
        .with_context(|| format!("failed to unlock lockfile for rpc_port={}", rpc_port))?;

    Ok(handle)
}
