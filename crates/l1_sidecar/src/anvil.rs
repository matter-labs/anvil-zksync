use crate::zkstack_config::ZkstackConfig;
use alloy::network::EthereumWallet;
use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::ext::AnvilApi;
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::transports::RpcError;
use anvil_zksync_common::sh_println;
use anyhow::Context;
use once_cell::sync::Lazy;
use semver::Version;
use std::collections::HashMap;
use std::fs::File;
use std::process::Stdio;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child as AsyncChild, Command as AsyncCommand};
use zksync_types::ProtocolVersionId;

static L1_STATES: Lazy<HashMap<ProtocolVersionId, &[u8]>> = Lazy::new(|| {
    HashMap::from_iter([
        (
            ProtocolVersionId::Version26,
            include_bytes!("../../../l1-setup/state/v26-l1-state.json").as_slice(),
        ),
        (
            ProtocolVersionId::Version27,
            include_bytes!("../../../l1-setup/state/v27-l1-state.json").as_slice(),
        ),
        (
            ProtocolVersionId::Version28,
            include_bytes!("../../../l1-setup/state/v28-l1-state.json").as_slice(),
        ),
        (
            ProtocolVersionId::Version29,
            include_bytes!("../../../l1-setup/state/v29-l1-state.json").as_slice(),
        ),
    ])
});

static L1_PAYLOADS: Lazy<HashMap<ProtocolVersionId, &str>> = Lazy::new(|| {
    HashMap::from_iter([
        (
            ProtocolVersionId::Version26,
            include_str!("../../../l1-setup/state/v26-l1-state-payload.txt"),
        ),
        (
            ProtocolVersionId::Version27,
            include_str!("../../../l1-setup/state/v27-l1-state-payload.txt"),
        ),
        (
            ProtocolVersionId::Version28,
            include_str!("../../../l1-setup/state/v28-l1-state-payload.txt"),
        ),
        (
            ProtocolVersionId::Version29,
            include_str!("../../../l1-setup/state/v29-l1-state-payload.txt"),
        ),
    ])
});

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
            L1AnvilEnv::External => tokio::signal::ctrl_c().await?,
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
    // Allow any non-0.x version (including `1.0.0-stable`, `1.0.0-nightly` and other pre-releases)
    if version.major >= 1 {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "unsupported `anvil` version ({}), please upgrade to >=1.0.0",
            version
        ))
    }
}

/// Spawns an anvil instance using the system-provided `anvil` command and built-in precomputed state.
pub async fn spawn_process(
    port: u16,
    zkstack_config: &ZkstackConfig,
) -> anyhow::Result<(AnvilHandle, DynProvider)> {
    ensure_anvil_1_x_x().await?;

    let tmpdir = tempfile::Builder::new()
        .prefix("anvil_zksync_l1")
        .tempdir()?;
    let anvil_state_path = tmpdir.path().join("l1-state.json");
    let mut anvil_state_file = tokio::fs::File::create(&anvil_state_path).await?;
    anvil_state_file
        .write_all(
            L1_STATES
                .get(&zkstack_config.genesis.genesis_protocol_version)
                .expect("zkstack config refers to an unsupported protocol version"),
        )
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
    let provider = setup_provider(&format!("http://localhost:{port}"), zkstack_config).await?;

    Ok((AnvilHandle { env }, DynProvider::new(provider)))
}

pub async fn external(
    address: &str,
    zkstack_config: &ZkstackConfig,
) -> anyhow::Result<(AnvilHandle, DynProvider)> {
    let env = L1AnvilEnv::External;
    let provider = setup_provider(address, zkstack_config).await?;
    inject_l1_state(zkstack_config.genesis.genesis_protocol_version, &provider).await?;

    // Submit a transaction with very high gas to refresh anvil's fee estimator. Seems like some
    // >=1.0.0 versions are still affected by this bug.
    let fees = provider.estimate_eip1559_fees().await?;
    provider
        .send_transaction(
            TransactionRequest::default()
                .to(Address::default())
                .value(U256::from(1))
                .max_fee_per_gas(fees.max_fee_per_gas * 1000000)
                .max_priority_fee_per_gas(fees.max_priority_fee_per_gas * 1000000),
        )
        .await?
        .get_receipt()
        .await?;

    Ok((AnvilHandle { env }, DynProvider::new(provider)))
}

/// An environment that holds live resources that were used to spawn an anvil node.
///
/// This is not supposed to be dropped until anvil has finished running.
enum L1AnvilEnv {
    Process(ProcessAnvil),
    External,
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
    address: &str,
    config: &ZkstackConfig,
) -> anyhow::Result<impl Provider + use<>> {
    let blob_operator_wallet =
        EthereumWallet::from(config.wallets.blob_operator.private_key.clone());
    let provider = ProviderBuilder::new()
        .wallet(blob_operator_wallet)
        .connect(address)
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
                    sh_println!("Waiting for L1 to become available at {address}...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(err) => return Err(err.into()),
            }
        }
    })
    .await
    .context("L1 anvil failed to start")?
    .context("unexpected response from L1 anvil")?;

    Ok(provider)
}

/// Injects pre-computed L1 state into provider.
async fn inject_l1_state(
    protocol_version: ProtocolVersionId,
    provider: &impl Provider,
) -> anyhow::Result<()> {
    // Trim trailing EOL and drop the `0x` prefix
    let state_payload = &L1_PAYLOADS
        .get(&protocol_version)
        .expect("zkstack config refers to an unsupported protocol version")
        .trim()[2..];
    let state_payload = Bytes::from(hex::decode(state_payload)?);
    match provider.anvil_load_state(state_payload).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(anyhow::anyhow!(
            "`anvil` refused to inject L1 state; see its logs for more details"
        )),
        Err(RpcError::ErrorResp(e))
            if e.code == -32600 && e.message.contains("Invalid request") =>
        {
            Err(anyhow::anyhow!(
                "`anvil` rejected `anvil_loadState` request; likely because of the request size limit - try running it with `--no-request-size-limit`: {e}"
            ))
        }
        Err(e) => Err(e.into()),
    }
}
