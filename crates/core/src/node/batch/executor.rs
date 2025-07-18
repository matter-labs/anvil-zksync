use std::{error::Error as StdError, sync::Arc};

use anyhow::Context as _;
use async_trait::async_trait;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use zksync_multivm::interface::{
    BatchTransactionExecutionResult, FinishedL1Batch, L2BlockEnv, VmExecutionResultAndLogs,
    executor::BatchExecutor,
    storage::{ReadStorage, StorageView},
};
use zksync_types::Transaction;

#[derive(Debug)]
enum HandleOrError<S> {
    Handle(JoinHandle<anyhow::Result<StorageView<S>>>),
    Err(Arc<dyn StdError + Send + Sync>),
}

impl<S> HandleOrError<S> {
    async fn wait_for_error(&mut self) -> anyhow::Error {
        let err_arc = match self {
            Self::Handle(handle) => {
                let err = match handle.await {
                    Ok(Ok(_)) => anyhow::anyhow!("batch executor unexpectedly stopped"),
                    Ok(Err(err)) => err,
                    Err(err) => anyhow::Error::new(err).context("batch executor panicked"),
                };
                let err: Box<dyn StdError + Send + Sync> = err.into();
                let err: Arc<dyn StdError + Send + Sync> = err.into();
                *self = Self::Err(err.clone());
                err
            }
            Self::Err(err) => err.clone(),
        };
        anyhow::Error::new(err_arc)
    }

    async fn wait(self) -> anyhow::Result<StorageView<S>> {
        match self {
            Self::Handle(handle) => handle.await.context("batch executor panicked")?,
            Self::Err(err_arc) => Err(anyhow::Error::new(err_arc)),
        }
    }
}

/// "Main" [`BatchExecutor`] implementation instantiating a VM in a blocking Tokio thread.
#[derive(Debug)]
pub struct MainBatchExecutor<S> {
    handle: HandleOrError<S>,
    commands: mpsc::Sender<Command>,
}

impl<S: ReadStorage> MainBatchExecutor<S> {
    pub(super) fn new(
        handle: JoinHandle<anyhow::Result<StorageView<S>>>,
        commands: mpsc::Sender<Command>,
    ) -> Self {
        Self {
            handle: HandleOrError::Handle(handle),
            commands,
        }
    }

    /// Custom method (not present in zksync-era) that runs bootloader once thus applying the bare
    /// minimum of changes to the state on batch sealing. Not as time-consuming as [`Self::finish_batch`].
    ///
    /// To be deleted once we stop sealing batches on every block.
    pub(crate) async fn bootloader(
        mut self,
    ) -> anyhow::Result<(VmExecutionResultAndLogs, StorageView<S>)> {
        let (response_sender, response_receiver) = oneshot::channel();
        let send_failed = self
            .commands
            .send(Command::Bootloader(response_sender))
            .await
            .is_err();
        if send_failed {
            return Err(self.handle.wait_for_error().await);
        }

        let bootloader_result = match response_receiver.await {
            Ok(batch) => batch,
            Err(_) => return Err(self.handle.wait_for_error().await),
        };
        let storage_view = self.handle.wait().await?;

        Ok((bootloader_result, storage_view))
    }
}

#[async_trait]
impl<S> BatchExecutor<S> for MainBatchExecutor<S>
where
    S: ReadStorage + Send + 'static,
{
    #[tracing::instrument(skip_all)]
    async fn execute_tx(
        &mut self,
        tx: Transaction,
    ) -> anyhow::Result<BatchTransactionExecutionResult> {
        let (response_sender, response_receiver) = oneshot::channel();
        let send_failed = self
            .commands
            .send(Command::ExecuteTx(Box::new(tx), response_sender))
            .await
            .is_err();
        if send_failed {
            return Err(self.handle.wait_for_error().await);
        }

        let res = match response_receiver.await {
            Ok(res) => res,
            Err(_) => return Err(self.handle.wait_for_error().await),
        };

        Ok(res)
    }

    #[tracing::instrument(skip_all)]
    async fn rollback_last_tx(&mut self) -> anyhow::Result<()> {
        // While we don't get anything from the channel, it's useful to have it as a confirmation that the operation
        // indeed has been processed.
        let (response_sender, response_receiver) = oneshot::channel();
        let send_failed = self
            .commands
            .send(Command::RollbackLastTx(response_sender))
            .await
            .is_err();
        if send_failed {
            return Err(self.handle.wait_for_error().await);
        }

        if response_receiver.await.is_err() {
            return Err(self.handle.wait_for_error().await);
        }
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn start_next_l2_block(&mut self, env: L2BlockEnv) -> anyhow::Result<()> {
        // While we don't get anything from the channel, it's useful to have it as a confirmation that the operation
        // indeed has been processed.
        let (response_sender, response_receiver) = oneshot::channel();
        let send_failed = self
            .commands
            .send(Command::StartNextL2Block(env, response_sender))
            .await
            .is_err();
        if send_failed {
            return Err(self.handle.wait_for_error().await);
        }

        if response_receiver.await.is_err() {
            return Err(self.handle.wait_for_error().await);
        }
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn finish_batch(
        mut self: Box<Self>,
    ) -> anyhow::Result<(FinishedL1Batch, StorageView<S>)> {
        let (response_sender, response_receiver) = oneshot::channel();
        let send_failed = self
            .commands
            .send(Command::FinishBatch(response_sender))
            .await
            .is_err();
        if send_failed {
            return Err(self.handle.wait_for_error().await);
        }

        let finished_batch = match response_receiver.await {
            Ok(batch) => batch,
            Err(_) => return Err(self.handle.wait_for_error().await),
        };
        let storage_view = self.handle.wait().await?;
        Ok((finished_batch, storage_view))
    }
}

#[derive(Debug)]
pub(super) enum Command {
    ExecuteTx(
        Box<Transaction>,
        oneshot::Sender<BatchTransactionExecutionResult>,
    ),
    StartNextL2Block(L2BlockEnv, oneshot::Sender<()>),
    RollbackLastTx(oneshot::Sender<()>),
    FinishBatch(oneshot::Sender<FinishedL1Batch>),
    Bootloader(oneshot::Sender<VmExecutionResultAndLogs>),
}
