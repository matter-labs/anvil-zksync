use crate::anvil::AnvilHandle;
use crate::commitment_generator::CommitmentGenerator;
use crate::l1_sender::{L1Sender, L1SenderHandle};
use crate::zkstack_config::ZkstackConfig;
use anvil_zksync_core::node::blockchain::ReadBlockchain;
use zksync_types::{L1BatchNumber, H256};

mod anvil;
mod commitment_generator;
mod contracts;
mod l1_sender;
mod zkstack_config;

#[derive(Debug, Clone)]
pub struct L1Sidecar {
    inner: Option<L1SidecarInner>,
}

#[derive(Debug, Clone)]
struct L1SidecarInner {
    commitment_generator: CommitmentGenerator,
    l1_sender_handle: L1SenderHandle,
}

impl L1Sidecar {
    pub fn none() -> Self {
        Self { inner: None }
    }

    pub async fn builtin(
        port: u16,
        blockchain: Box<dyn ReadBlockchain>,
    ) -> anyhow::Result<(Self, L1SidecarRunner)> {
        let zkstack_config = ZkstackConfig::builtin();
        let (anvil_handle, anvil_provider) = anvil::spawn_builtin(port, &zkstack_config).await?;
        let commitment_generator = CommitmentGenerator::new(&zkstack_config, blockchain);
        let genesis_with_metadata = commitment_generator
            .get_or_generate_metadata(L1BatchNumber(0))
            .await
            .ok_or(anyhow::anyhow!(
                "genesis is missing from local storage, can't start L1 sidecar"
            ))?;
        let (l1_sender, l1_sender_handle) =
            L1Sender::new(&zkstack_config, genesis_with_metadata, anvil_provider);
        let this = Self {
            inner: Some(L1SidecarInner {
                commitment_generator,
                l1_sender_handle,
            }),
        };
        let runner = L1SidecarRunner {
            anvil_handle,
            l1_sender,
        };
        Ok((this, runner))
    }
}

pub struct L1SidecarRunner {
    anvil_handle: AnvilHandle,
    l1_sender: L1Sender,
}

impl L1SidecarRunner {
    pub async fn run(self) -> anyhow::Result<()> {
        tokio::select! {
            _ = self.l1_sender.run() => {
                tracing::trace!("L1 sender was stopped");
            },
            _ = self.anvil_handle.run() => {
                tracing::trace!("L1 anvil exited unexpectedly");
            },
        }
        Ok(())
    }
}

impl L1Sidecar {
    pub async fn commit_batch(&self, batch_number: L1BatchNumber) -> anyhow::Result<H256> {
        let Some(inner) = self.inner.as_ref() else {
            return Err(anyhow::anyhow!(
                "cannot commit a batch as there is no L1 configured"
            ));
        };
        let batch_with_metadata = inner
            .commitment_generator
            .get_or_generate_metadata(batch_number)
            .await
            .ok_or_else(|| anyhow::anyhow!("batch #{batch_number} does not exist"))?;
        inner
            .l1_sender_handle
            .commit_sync(batch_with_metadata)
            .await
    }

    pub async fn prove_batch(&self, batch_number: L1BatchNumber) -> anyhow::Result<H256> {
        let Some(inner) = self.inner.as_ref() else {
            return Err(anyhow::anyhow!(
                "cannot prove a batch as there is no L1 configured"
            ));
        };
        let batch_with_metadata = inner
            .commitment_generator
            .get_or_generate_metadata(batch_number)
            .await
            .ok_or_else(|| anyhow::anyhow!("batch #{batch_number} does not exist"))?;
        inner.l1_sender_handle.prove_sync(batch_with_metadata).await
    }

    pub async fn execute_batch(&self, batch_number: L1BatchNumber) -> anyhow::Result<H256> {
        let Some(inner) = self.inner.as_ref() else {
            return Err(anyhow::anyhow!(
                "cannot execute a batch as there is no L1 configured"
            ));
        };
        let batch_with_metadata = inner
            .commitment_generator
            .get_or_generate_metadata(batch_number)
            .await
            .ok_or_else(|| anyhow::anyhow!("batch #{batch_number} does not exist"))?;
        inner
            .l1_sender_handle
            .execute_sync(batch_with_metadata)
            .await
    }
}
