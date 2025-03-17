use crate::zkstack_config::ZkstackConfig;
use alloy::network::EthereumWallet;
use alloy::providers::{Provider, ProviderBuilder};
use anyhow::Context;
use semver::Version;
use std::fs::File;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child as AsyncChild, Command as AsyncCommand};

/// Representation of an anvil process spawned onto an event loop.
///
/// Process will be killed once `AnvilHandle` handle has been dropped.
pub struct AnvilHandle {
    /// Underlying L1's environment that ensures anvil can continue running normally until this
    /// handle is dropped.
    env: L1AnvilEnv,
}

impl AnvilHandle {
    /// Runs anvil and its services. Returns on anvil exiting.
    pub async fn run(self) -> anyhow::Result<()> {
        match self.env {
            L1AnvilEnv::Process(ProcessAnvil { mut node_child, .. }) => {
                node_child.wait().await?;
            }
        }
        Ok(())
    }
}

async fn ensure_anvil_1_x_x() -> anyhow::Result<()> {
    let child = AsyncCommand::new("anvil")
        .arg("--version")
        .stdout(Stdio::piped())
        .spawn()
        .context("could not detect `anvil` version; make sure it is installed on your machine")?;
    let output = child.wait_with_output().await?;
    let output = std::str::from_utf8(&output.stdout)?;
    let version_line = output
        .lines()
        .next()
        .with_context(|| format!("`anvil --version` output did not contain any lines: {output}"))?;
    let version = version_line
        .strip_prefix("anvil Version: ")
        .with_context(|| {
            format!("`anvil --version` output started with unexpected prefix: {version_line}")
        })?;
    let version = Version::parse(version)?;
    tracing::debug!(%version, "detected installed anvil version");
    // Allow any version above `1.0.0-rc` (including `1.0.0-stable`)
    if version > Version::parse("1.0.0-rc")? {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "unsupported `anvil` version ({}), please upgrade to >1.0.0-rc",
            version
        ))
    }
}

/// Spawns an anvil instance using the system-provided `anvil` command and built-in precomputed state.
pub async fn spawn_process(
    port: u16,
    zkstack_config: &ZkstackConfig,
) -> anyhow::Result<(AnvilHandle, Arc<dyn Provider + 'static>)> {
    ensure_anvil_1_x_x().await?;

    let tmpdir = tempfile::Builder::new()
        .prefix("anvil_zksync_l1")
        .tempdir()?;
    let anvil_state_path = tmpdir.path().join("l1-state.json");
    let mut anvil_state_file = tokio::fs::File::create(&anvil_state_path).await?;
    anvil_state_file
        .write_all(include_bytes!("../../../l1-setup/state/l1-state.json"))
        .await?;
    anvil_state_file.flush().await?;
    drop(anvil_state_file);

    tracing::debug!(
        ?anvil_state_path,
        "unpacked anvil state into a temporary directory"
    );

    // TODO: Make log location configurable
    let log_file = File::create("./anvil-zksync-l1.log")?;
    let node_child = AsyncCommand::new("anvil")
        .arg("--port")
        .arg(port.to_string())
        .arg("--load-state")
        .arg(anvil_state_path)
        .stdout(log_file)
        .spawn()?;

    let env = L1AnvilEnv::Process(ProcessAnvil {
        node_child,
        _tmpdir: tmpdir,
    });
    let provider = setup_provider(port, zkstack_config).await?;

    Ok((AnvilHandle { env }, provider))
}

/// An environment that holds live resources that were used to spawn an anvil node.
///
/// This is not supposed to be dropped until anvil has finished running.
enum L1AnvilEnv {
    Process(ProcessAnvil),
}

/// An [anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil) instance running in
/// a separate process spawned from anvil-zksync.
struct ProcessAnvil {
    /// A handle to the spawned anvil node and its tasks.
    node_child: AsyncChild,
    /// Temporary directory containing state file. Holding it to ensure it does not get deleted prematurely.
    _tmpdir: TempDir,
}

async fn setup_provider(
    port: u16,
    config: &ZkstackConfig,
) -> anyhow::Result<Arc<dyn Provider + 'static>> {
    let blob_operator_wallet =
        EthereumWallet::from(config.wallets.blob_operator.private_key.clone());
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(blob_operator_wallet)
        .on_builtin(&format!("http://localhost:{port}"))
        .await?;

    // Wait for anvil to be up
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            match provider.get_accounts().await {
                Ok(_) => {
                    return anyhow::Ok(());
                }
                Err(err) if err.is_transport_error() => {
                    tracing::debug!(?err, "L1 Anvil is not up yet; sleeping");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(err) => return Err(err.into()),
            }
        }
    })
    .await
    .context("L1 anvil failed to start")?
    .context("unexpected response from L1 anvil")?;

    Ok(Arc::new(provider))
}
