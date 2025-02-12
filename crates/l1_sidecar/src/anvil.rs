use crate::zkstack_config::ZkstackConfig;
use alloy::network::EthereumWallet;
use alloy::providers::{Provider, ProviderBuilder};
use anyhow::Context;
use foundry_anvil::{NodeConfig, NodeHandle};
use foundry_common::Shell;
use std::time::Duration;
use tempdir::TempDir;
use tokio::io::AsyncWriteExt;

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
            L1AnvilEnv::Builtin(BuiltinAnvil { node_handle, .. }) => {
                node_handle.await??;
            }
        }
        Ok(())
    }
}

/// Spawns an anvil instance using the built-in binary and built-in precomputed state.
pub async fn spawn_builtin(
    port: u16,
    zkstack_config: &ZkstackConfig,
) -> anyhow::Result<(AnvilHandle, Box<dyn Provider>)> {
    let tmpdir = TempDir::new("anvil_zksync_l1")?;
    let anvil_state_path = tmpdir.path().join("l1-state.json");
    let mut anvil_state_file = tokio::fs::File::create(&anvil_state_path).await?;
    anvil_state_file
        .write_all(include_bytes!("../../../l1-setup/state/l1-state.json"))
        .await?;
    anvil_state_file.flush().await?;
    drop(anvil_state_file);

    tracing::debug!(
        ?anvil_state_path,
        "unpacked built-in anvil state into a temporary directory"
    );

    // Set up empty shell to disable anvil's stdout. Ideally we would re-direct it into a file but
    // this doesn't seem possible right now.
    Shell::empty().set();
    let (_, node_handle) = foundry_anvil::spawn(
        NodeConfig::default()
            .with_port(port)
            .with_init_state_path(anvil_state_path),
    )
    .await;

    let env = L1AnvilEnv::Builtin(BuiltinAnvil {
        node_handle,
        _tmpdir: tmpdir,
    });
    let provider = setup_provider(port, zkstack_config).await?;

    Ok((AnvilHandle { env }, provider))
}

/// An environment that holds live resources that were used to spawn an anvil node.
///
/// This is not supposed to be dropped until anvil has finished running.
enum L1AnvilEnv {
    Builtin(BuiltinAnvil),
}

/// A built-in [anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil) instance
/// bundled with anvil-zksync.
struct BuiltinAnvil {
    /// A handle to the spawned anvil node and its tasks.
    node_handle: NodeHandle,
    /// Temporary directory containing state file. Holding it to ensure it does not get deleted prematurely.
    _tmpdir: TempDir,
}

async fn setup_provider(port: u16, config: &ZkstackConfig) -> anyhow::Result<Box<dyn Provider>> {
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

    Ok(Box::new(provider))
}
