use super::InMemoryNodeInner;
use crate::node::pool::TxBatch;
use crate::system_contracts::SystemContracts;
use futures::future::BoxFuture;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::{mpsc, oneshot, RwLock};
use zksync_contracts::BaseSystemContracts;
use zksync_multivm::interface::TxExecutionMode;
use zksync_types::l2::L2Tx;
use zksync_types::L2BlockNumber;

#[pin_project::pin_project]
pub struct NodeExecutor {
    node_inner: Arc<RwLock<InMemoryNodeInner>>,
    system_contracts: SystemContracts,
    command_receiver: mpsc::Receiver<Command>,
    /// Future that is processing the next command
    #[pin]
    future: Option<BoxFuture<'static, ()>>,
}

impl NodeExecutor {
    pub fn new(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        system_contracts: SystemContracts,
    ) -> (Self, NodeExecutorHandle) {
        let (command_sender, command_receiver) = mpsc::channel(128);
        let this = Self {
            node_inner,
            system_contracts,
            command_receiver,
            future: None,
        };
        let handle = NodeExecutorHandle { command_sender };
        (this, handle)
    }
}

impl NodeExecutor {
    async fn seal_block(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        txs: Vec<L2Tx>,
        base_system_contracts: BaseSystemContracts,
        reply: Option<oneshot::Sender<anyhow::Result<L2BlockNumber>>>,
    ) {
        let result = node_inner
            .write()
            .await
            .seal_block(txs, base_system_contracts)
            .await;
        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Some(reply) = reply {
            if let Err(result) = reply.send(result) {
                tracing::info!("failed to reply as receiver has been dropped");
                result
            } else {
                return;
            }
        } else {
            result
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to seal a block: {:#?}", err);
        }
    }

    async fn seal_blocks(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        tx_batches: Vec<TxBatch>,
        interval: u64,
        system_contracts: SystemContracts,
        reply: oneshot::Sender<anyhow::Result<Vec<L2BlockNumber>>>,
    ) {
        let mut node_inner = node_inner.write().await;

        // Save old interval to restore later: it might get replaced with `interval` below
        let old_interval = node_inner.time_writer.get_block_timestamp_interval();
        let result = async {
            let mut block_numbers = Vec::with_capacity(tx_batches.len());
            // Processing the entire vector is essentially atomic here because `NodeExecutor` is
            // the only component that seals blocks.
            for (i, TxBatch { txs, impersonating }) in tx_batches.into_iter().enumerate() {
                // Enforce provided interval starting from the second block (i.e. first block should
                // use the existing interval).
                if i == 1 {
                    node_inner
                        .time_writer
                        .set_block_timestamp_interval(Some(interval));
                }
                let base_system_contracts = system_contracts
                    .contracts(TxExecutionMode::VerifyExecute, impersonating)
                    .clone();
                let number = node_inner.seal_block(txs, base_system_contracts).await?;
                block_numbers.push(number);
            }
            anyhow::Ok(block_numbers)
        }
        .await;
        // Restore old interval
        node_inner
            .time_writer
            .set_block_timestamp_interval(old_interval);

        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Err(result) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
            result
        } else {
            return;
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to seal blocks: {:#?}", err);
        }
    }

    async fn increase_time(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        delta: u64,
        reply: oneshot::Sender<()>,
    ) {
        node_inner.write().await.time_writer.increase_time(delta);
        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn enforce_next_timestamp(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        timestamp: u64,
        reply: oneshot::Sender<anyhow::Result<()>>,
    ) {
        let result = node_inner
            .write()
            .await
            .time_writer
            .enforce_next_timestamp(timestamp);
        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Err(result) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
            result
        } else {
            return;
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to enforce next timestamp: {:#?}", err);
        }
    }

    async fn set_current_timestamp(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        timestamp: u64,
        reply: oneshot::Sender<i128>,
    ) {
        let result = node_inner
            .write()
            .await
            .time_writer
            .set_current_timestamp_unchecked(timestamp);
        // Reply to sender if we can
        if reply.send(result).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn set_timestamp_interval(node_inner: Arc<RwLock<InMemoryNodeInner>>, delta: u64) {
        node_inner
            .write()
            .await
            .time_writer
            .set_block_timestamp_interval(Some(delta));
    }

    async fn remove_timestamp_interval(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        reply: oneshot::Sender<bool>,
    ) {
        let result = node_inner
            .write()
            .await
            .time_writer
            .remove_block_timestamp_interval();
        // Reply to sender if we can
        if reply.send(result).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }
}

impl Future for NodeExecutor {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if this.future.is_none() {
            let command_opt = futures::ready!(this.command_receiver.poll_recv(cx));
            let Some(command) = command_opt else {
                tracing::trace!("channel has been closed; stopping node executor");
                return Poll::Ready(());
            };
            match command {
                Command::SealBlock(tx_batch, reply) => {
                    let TxBatch { impersonating, txs } = tx_batch;

                    let base_system_contracts = this
                        .system_contracts
                        .contracts(TxExecutionMode::VerifyExecute, impersonating)
                        .clone();
                    let node_inner = this.node_inner.clone();
                    *this.future = Some(Box::pin(Self::seal_block(
                        node_inner,
                        txs,
                        base_system_contracts,
                        reply,
                    )));
                }
                Command::SealBlocks(tx_batches, interval, reply) => {
                    let node_inner = this.node_inner.clone();
                    let system_contracts = this.system_contracts.clone();
                    *this.future = Some(Box::pin(Self::seal_blocks(
                        node_inner,
                        tx_batches,
                        interval,
                        system_contracts,
                        reply,
                    )));
                }
                Command::IncreaseTime(delta, reply) => {
                    let node_inner = this.node_inner.clone();
                    *this.future = Some(Box::pin(Self::increase_time(node_inner, delta, reply)));
                }
                Command::EnforceNextTimestamp(timestamp, reply) => {
                    let node_inner = this.node_inner.clone();
                    *this.future = Some(Box::pin(Self::enforce_next_timestamp(
                        node_inner, timestamp, reply,
                    )));
                }
                Command::SetCurrentTimestamp(timestamp, reply) => {
                    let node_inner = this.node_inner.clone();
                    *this.future = Some(Box::pin(Self::set_current_timestamp(
                        node_inner, timestamp, reply,
                    )));
                }
                Command::SetTimestampInterval(seconds) => {
                    let node_inner = this.node_inner.clone();
                    *this.future =
                        Some(Box::pin(Self::set_timestamp_interval(node_inner, seconds)));
                }
                Command::RemoveTimestampInterval(reply) => {
                    let node_inner = this.node_inner.clone();
                    *this.future =
                        Some(Box::pin(Self::remove_timestamp_interval(node_inner, reply)));
                }
            }
        }

        if let Some(future) = this.future.as_mut().as_pin_mut() {
            // Clear pending future if it completed
            if let Poll::Ready(()) = future.poll(cx) {
                *this.future = None;
                // Wake yourself up as we might have some unprocessed commands left
                cx.waker().wake_by_ref();
            }
            Poll::Pending
        } else {
            Poll::Pending
        }
    }
}

#[derive(Clone, Debug)]
pub struct NodeExecutorHandle {
    command_sender: mpsc::Sender<Command>,
}

impl NodeExecutorHandle {
    /// Request [`NodeExecutor`] to seal a new block from the provided transaction batch. Does not
    /// wait for the block to actually be produced.
    ///
    /// It is sender's responsibility to make sure [`TxBatch`] is constructed correctly (see its
    /// docs).
    pub async fn seal_block(
        &self,
        tx_batch: TxBatch,
    ) -> Result<(), mpsc::error::SendError<Command>> {
        self.command_sender
            .send(Command::SealBlock(tx_batch, None))
            .await
    }

    /// Request [`NodeExecutor`] to seal a new block from the provided transaction batch. Waits for
    /// the block to be produced and returns its number.
    ///
    /// It is sender's responsibility to make sure [`TxBatch`] is constructed correctly (see its
    /// docs).
    pub async fn seal_block_sync(&self, tx_batch: TxBatch) -> anyhow::Result<L2BlockNumber> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SealBlock(tx_batch, Some(response_sender)))
            .await
            .map_err(|_| anyhow::anyhow!("failed to seal a block as node executor is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to seal a block as node executor is dropped"),
        }
    }

    /// Request [`NodeExecutor`] to seal multiple blocks from the provided transaction batches with
    /// `interval` seconds in-between of two consecutive blocks.
    /// Waits for the blocks to be produced and returns their numbers.
    ///
    /// Guarantees that the resulting block numbers will be sequential (i.e. no other blocks can
    /// be produced in-between).
    ///
    /// It is sender's responsibility to make sure [`TxBatch`]es are constructed correctly (see
    /// docs).
    pub async fn seal_blocks_sync(
        &self,
        tx_batches: Vec<TxBatch>,
        interval: u64,
    ) -> anyhow::Result<Vec<L2BlockNumber>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SealBlocks(tx_batches, interval, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to seal a block as node executor is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to seal a block as node executor is dropped"),
        }
    }

    /// Request [`NodeExecutor`] to increase time by the given delta (in seconds). Waits for the
    /// change to take place.
    pub async fn increase_time_sync(&self, delta: u64) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::IncreaseTime(delta, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to increase time as node executor is dropped"))?;
        match response_receiver.await {
            Ok(()) => Ok(()),
            Err(_) => {
                anyhow::bail!("failed to increase time as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to enforce next block's timestamp (in seconds). Waits for the
    /// timestamp validity to be confirmed. Block might still not be produced by then.
    pub async fn enforce_next_timestamp_sync(&self, timestamp: u64) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::EnforceNextTimestamp(timestamp, response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to enforce next timestamp as node executor is dropped")
            })?;
        match response_receiver.await {
            Ok(result) => result,
            Err(_) => {
                anyhow::bail!("failed to enforce next timestamp as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to set current timestamp (in seconds). Waits for the
    /// change to take place.
    pub async fn set_current_timestamp_sync(&self, timestamp: u64) -> anyhow::Result<i128> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SetCurrentTimestamp(timestamp, response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to set current timestamp as node executor is dropped")
            })?;

        match response_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => anyhow::bail!("failed to set current timestamp as node executor is dropped"),
        }
    }

    /// Request [`NodeExecutor`] to set block timestamp interval (in seconds). Does not wait for the
    /// change to take place.
    pub async fn set_block_timestamp_interval(
        &self,
        seconds: u64,
    ) -> Result<(), mpsc::error::SendError<Command>> {
        self.command_sender
            .send(Command::SetTimestampInterval(seconds))
            .await
    }

    /// Request [`NodeExecutor`] to remove block timestamp interval. Waits for the change to take
    /// place. Returns `true` if an existing interval was removed, `false` otherwise.
    pub async fn remove_block_timestamp_interval_sync(&self) -> anyhow::Result<bool> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::RemoveTimestampInterval(response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to remove block interval as node executor is dropped")
            })?;

        match response_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => anyhow::bail!("failed to remove block interval as node executor is dropped"),
        }
    }
}

#[cfg(test)]
impl NodeExecutorHandle {
    pub fn test() -> (Self, mpsc::Receiver<Command>) {
        let (command_sender, command_receiver) = mpsc::channel(128);
        (NodeExecutorHandle { command_sender }, command_receiver)
    }
}

#[derive(Debug)]
pub enum Command {
    // Block sealing commands
    SealBlock(
        TxBatch,
        Option<oneshot::Sender<anyhow::Result<L2BlockNumber>>>,
    ),
    SealBlocks(
        Vec<TxBatch>,
        u64,
        oneshot::Sender<anyhow::Result<Vec<L2BlockNumber>>>,
    ),
    // Time manipulation commands. Caveat: reply-able commands can hold user connections alive for
    // a long time (until the command is processed).
    IncreaseTime(u64, oneshot::Sender<()>),
    EnforceNextTimestamp(u64, oneshot::Sender<anyhow::Result<()>>),
    SetCurrentTimestamp(u64, oneshot::Sender<i128>),
    SetTimestampInterval(u64),
    RemoveTimestampInterval(oneshot::Sender<bool>),
}
