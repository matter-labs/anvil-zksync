use crate::commitment_generator::CommitmentGenerator;
use crate::l1_sender::L1SenderHandle;
use std::time::Duration;
use tokio::sync::watch;
use zksync_types::L1BatchNumber;

#[derive(Debug, Clone)]
pub struct L1Executor {
    mode: L1ExecutorMode,
}

impl L1Executor {
    /// Batches will not be executed on L1 unless requested manually through JSON RPC API.
    pub fn manual() -> Self {
        Self {
            mode: L1ExecutorMode::Manual,
        }
    }

    /// Batches will be executed on L1 shortly after they get sealed.
    pub fn auto(
        commitment_generator: CommitmentGenerator,
        l1_sender_handle: L1SenderHandle,
    ) -> Self {
        Self {
            mode: L1ExecutorMode::Auto(L1ExecutorModeAuto {
                last_executed_batch: L1BatchNumber(0),
                commitment_generator,
                l1_sender_handle,
            }),
        }
    }

    pub async fn run(self, stop_receiver: &mut watch::Receiver<bool>) -> anyhow::Result<()> {
        match self.mode {
            L1ExecutorMode::Manual => {
                stop_receiver.changed().await?;
                Ok(())
            }
            L1ExecutorMode::Auto(executor) => executor.run(stop_receiver).await,
        }
    }
}

#[derive(Debug, Clone)]
enum L1ExecutorMode {
    Manual,
    Auto(L1ExecutorModeAuto),
}

#[derive(Debug, Clone)]
struct L1ExecutorModeAuto {
    last_executed_batch: L1BatchNumber,
    commitment_generator: CommitmentGenerator,
    l1_sender_handle: L1SenderHandle,
}

impl L1ExecutorModeAuto {
    async fn run(mut self, stop_receiver: &mut watch::Receiver<bool>) -> anyhow::Result<()> {
        const POLL_INTERVAL: Duration = Duration::from_millis(100);

        loop {
            if *stop_receiver.borrow() {
                tracing::info!("automatic L1 executor was interrupted");
                return Ok(());
            }
            let next_batch = self.last_executed_batch + 1;
            let Some(batch_with_metadata) = self
                .commitment_generator
                .get_or_generate_metadata(next_batch)
                .await
            else {
                tracing::trace!(batch_number=%next_batch, "batch is not ready to be executed yet");
                tokio::time::timeout(POLL_INTERVAL, stop_receiver.changed())
                    .await
                    .ok();
                continue;
            };
            self.l1_sender_handle
                .commit_sync(batch_with_metadata.clone())
                .await?;
            self.l1_sender_handle
                .prove_sync(batch_with_metadata.clone())
                .await?;
            self.l1_sender_handle
                .execute_sync(batch_with_metadata)
                .await?;
            tracing::debug!(batch_number=%next_batch, "batch has been automatically executed on L1");
            self.last_executed_batch = next_batch;
        }
    }
}
