use crate::zkstack_config::ZkstackConfig;
use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::providers::{Provider, ProviderBuilder, WalletProvider};
use alloy::rpc::types::TransactionRequest;
use std::ffi::OsStr;
use std::fs::File;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use tempdir::TempDir;
use tokio::fs::OpenOptions as AsyncOpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child as AsyncChild, Command as AsyncCommand};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const ANVIL_BIN: &[u8] = include_bytes!("../../../l1-setup/bin/anvil_v0.3.0_linux_amd64");
#[cfg(all(target_os = "linux", target_arch = "arm"))]
const ANVIL_BIN: &[u8] = include_bytes!("../../../l1-setup/bin/anvil_v0.3.0_linux_arm64");
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const ANVIL_BIN: &[u8] = include_bytes!("../../../l1-setup/bin/anvil_v0.3.0_darwin_amd64");
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const ANVIL_BIN: &[u8] = include_bytes!("../../../l1-setup/bin/anvil_v0.3.0_darwin_arm64");

/// Representation of an anvil process spawned onto an event loop.
///
/// Process will be killed once `AnvilHandle` handle has been dropped.
pub struct AnvilHandle {
    child: AsyncChild,
    // Hold environment to ensure anvil can continue running normally until this handle is dropped.
    _env: L1AnvilEnv,
}

impl AnvilHandle {
    /// Waits for anvil to exit completely, returning the status that it exited with.
    pub async fn wait(mut self) -> anyhow::Result<ExitStatus> {
        Ok(self.child.wait().await?)
    }
}

/// Spawns an anvil instance using the built-in binary and built-in precomputed state.
pub async fn spawn_builtin(
    port: u16,
    zkstack_config: &ZkstackConfig,
) -> anyhow::Result<(AnvilHandle, Box<dyn Provider>)> {
    let tmpdir = TempDir::new("anvil_zksync_l1")?;
    let anvil_bin_path = tmpdir.path().join("anvil");
    let mut anvil_bin_file = AsyncOpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o755)
        .open(&anvil_bin_path)
        .await?;
    anvil_bin_file.write_all(ANVIL_BIN).await?;
    anvil_bin_file.flush().await?;
    drop(anvil_bin_file);

    let anvil_state_path = tmpdir.path().join("l1-state.json");
    let mut anvil_state_file = tokio::fs::File::create(&anvil_state_path).await?;
    anvil_state_file
        .write_all(include_bytes!("../../../l1-setup/state/l1-state.json"))
        .await?;
    anvil_state_file.flush().await?;
    drop(anvil_state_file);

    tracing::debug!(
        ?anvil_state_path,
        ?anvil_bin_path,
        "unpacked built-in anvil to a temporary directory"
    );

    let env = L1AnvilEnv::Builtin(BuiltinAnvil {
        anvil_bin_path,
        anvil_state_path,
        _tmpdir: tmpdir,
    });
    env.spawn(port, zkstack_config).await
}

/// A configured environment that can be used to spawn an anvil node as a valid L1 for anvil-zksync.
///
/// This is not supposed to be dropped until anvil process has finished running.
enum L1AnvilEnv {
    Builtin(BuiltinAnvil),
}

/// A built-in [anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil) instance
/// bundled inside anvil-zksync binary.
///
/// While it might be possible to compile anvil as a position-independent executable and then
/// (dlopen)[https://docs.rs/dlopen/latest/dlopen] it directly from address space of anvil-zksync,
/// it would be a massive overkill for what we are trying to achieve. Instead, we simply include
/// anvil in the `.rodata` section and then write it into a temporary directory in runtime. Same for
/// anvil state file.
///
/// Another alternative is to use [memfd_create](https://man7.org/linux/man-pages/man2/memfd_create.2.html)
/// but that would make code Linux-specific.
struct BuiltinAnvil {
    // Path to unpacked anvil binary shipped with anvil-zksync.
    anvil_bin_path: PathBuf,
    // Path to unpacked anvil L1 state shipped with anvil-zksync.
    anvil_state_path: PathBuf,
    // Hold temporary directory to ensure it does not get deleted prematurely.
    _tmpdir: TempDir,
}

impl L1AnvilEnv {
    async fn spawn(
        self,
        port: u16,
        config: &ZkstackConfig,
    ) -> anyhow::Result<(AnvilHandle, Box<dyn Provider>)> {
        let (anvil_bin_path, anvil_state_path) = match &self {
            Self::Builtin(BuiltinAnvil {
                _tmpdir,
                anvil_bin_path,
                anvil_state_path,
            }) => (anvil_bin_path, anvil_state_path),
        };
        // TODO: Make log location configurable
        let log_file = File::create("./anvil-zksync-l1.log")?;
        let child = AsyncCommand::new(anvil_bin_path)
            .args(vec![
                OsStr::new("--port"),
                OsStr::new(&port.to_string()),
                OsStr::new("--load-state"),
                anvil_state_path.as_os_str(),
            ])
            .stdout(log_file)
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;

        let blob_operator_wallet =
            EthereumWallet::from(config.wallets.blob_operator.private_key.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(blob_operator_wallet)
            .on_builtin(&format!("http://localhost:{port}"))
            .await?;

        // Wait for anvil to be up
        loop {
            match provider.get_accounts().await {
                Ok(_) => {
                    break;
                }
                Err(err) if err.is_transport_error() => {
                    tracing::debug!(?err, "L1 Anvil is not up yet; sleeping");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(err) => return Err(err.into()),
            }
        }

        // Send a dummy transaction with big gas price to refresh anvil's fee estimator
        let tx = TransactionRequest::default()
            .with_to(provider.default_signer_address())
            .with_max_fee_per_gas(10_000_000_000)
            .with_max_priority_fee_per_gas(1);
        provider.send_transaction(tx).await?.watch().await?;

        Ok((AnvilHandle { child, _env: self }, Box::new(provider)))
    }
}
