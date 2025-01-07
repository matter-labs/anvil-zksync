use super::inner::InMemoryNodeInner;
use crate::node::pool::TxBatch;
use crate::system_contracts::SystemContracts;
use futures::future::LocalBoxFuture;
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
pub struct BlockProducer {
    node_inner: Arc<RwLock<InMemoryNodeInner>>,
    system_contracts: SystemContracts,
    command_receiver: mpsc::Receiver<Command>,
    /// Future that is processing the next command
    #[pin]
    future: Option<LocalBoxFuture<'static, ()>>,
}

impl BlockProducer {
    pub fn new(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        system_contracts: SystemContracts,
    ) -> (Self, BlockProducerHandle) {
        let (command_sender, command_receiver) = mpsc::channel(128);
        let this = Self {
            node_inner,
            system_contracts,
            command_receiver,
            future: None,
        };
        let handle = BlockProducerHandle { command_sender };
        (this, handle)
    }
}

impl BlockProducer {
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
        let old_interval = node_inner.get_timestamp_interval();
        let result = (|| async {
            let mut block_numbers = Vec::with_capacity(tx_batches.len());
            // Processing the entire vector is essentially atomic here because `BlockProducer` is
            // the only component that seals blocks.
            for (i, TxBatch { txs, impersonating }) in tx_batches.into_iter().enumerate() {
                // Enforce provided interval starting from the second block (i.e. first block should
                // use the existing interval).
                if i == 1 {
                    node_inner.set_timestamp_interval(Some(interval));
                }
                let base_system_contracts = system_contracts
                    .contracts(TxExecutionMode::VerifyExecute, impersonating)
                    .clone();
                let number = node_inner.seal_block(txs, base_system_contracts).await?;
                block_numbers.push(number);
            }
            anyhow::Ok(block_numbers)
        })()
        .await;
        // Restore old interval
        node_inner.set_timestamp_interval(old_interval);

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

    async fn increase_time(node_inner: Arc<RwLock<InMemoryNodeInner>>, delta: u64) {
        node_inner.write().await.increase_time(delta);
    }

    async fn enforce_next_timestamp(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        timestamp: u64,
        reply: oneshot::Sender<anyhow::Result<()>>,
    ) {
        let result = node_inner.write().await.enforce_next_timestamp(timestamp);
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
        let result = node_inner.write().await.set_current_timestamp(timestamp);
        // Reply to sender if we can
        if let Err(_) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn set_timestamp_interval(node_inner: Arc<RwLock<InMemoryNodeInner>>, delta: u64) {
        node_inner.write().await.set_timestamp_interval(Some(delta));
    }

    async fn remove_timestamp_interval(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        reply: oneshot::Sender<bool>,
    ) {
        let result = node_inner.write().await.remove_timestamp_interval();
        // Reply to sender if we can
        if let Err(_) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }
}

impl Future for BlockProducer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if this.future.is_none() {
            let command_opt = futures::ready!(this.command_receiver.poll_recv(cx));
            let Some(command) = command_opt else {
                tracing::trace!("channel has been closed; stopping block production");
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
                Command::IncreaseTime(delta) => {
                    let node_inner = this.node_inner.clone();
                    *this.future = Some(Box::pin(Self::increase_time(node_inner, delta)));
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
pub struct BlockProducerHandle {
    command_sender: mpsc::Sender<Command>,
}

impl BlockProducerHandle {
    /// Request [`BlockProducer`] to seal a new block from the provided transaction batch. Does not
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

    /// Request [`BlockProducer`] to seal a new block from the provided transaction batch. Waits for
    /// the block to be produced and returns its number.
    ///
    /// It is sender's responsibility to make sure [`TxBatch`] is constructed correctly (see its
    /// docs).
    pub async fn seal_block_sync(&self, tx_batch: TxBatch) -> anyhow::Result<L2BlockNumber> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SealBlock(tx_batch, Some(response_sender)))
            .await
            .map_err(|_| anyhow::anyhow!("failed to seal a block as block producer is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to seal a block as block producer is dropped"),
        }
    }

    pub async fn seal_blocks_sync(
        &self,
        tx_batches: Vec<TxBatch>,
        interval: u64,
    ) -> anyhow::Result<Vec<L2BlockNumber>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SealBlocks(tx_batches, interval, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to seal a block as block producer is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to seal a block as block producer is dropped"),
        }
    }

    pub async fn increase_time(&self, delta: u64) -> Result<(), mpsc::error::SendError<Command>> {
        self.command_sender.send(Command::IncreaseTime(delta)).await
    }

    pub async fn enforce_next_timestamp_sync(&self, timestamp: u64) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::EnforceNextTimestamp(timestamp, response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to enforce next timestamp as block producer is dropped")
            })?;
        match response_receiver.await {
            Ok(result) => result,
            Err(_) => {
                anyhow::bail!("failed to enforce next timestamp as block producer is dropped")
            }
        }
    }

    pub async fn set_current_timestamp_sync(&self, timestamp: u64) -> anyhow::Result<i128> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SetCurrentTimestamp(timestamp, response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to set current timestamp as block producer is dropped")
            })?;

        match response_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => anyhow::bail!("failed to set current timestamp as block producer is dropped"),
        }
    }

    pub async fn set_block_timestamp_interval(
        &self,
        seconds: u64,
    ) -> Result<(), mpsc::error::SendError<Command>> {
        self.command_sender
            .send(Command::SetTimestampInterval(seconds))
            .await
    }

    pub async fn remove_block_timestamp_interval_sync(&self) -> anyhow::Result<bool> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::RemoveTimestampInterval(response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to remove block interval as block producer is dropped")
            })?;

        match response_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => anyhow::bail!("failed to remove block interval as block producer is dropped"),
        }
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
    IncreaseTime(u64),
    EnforceNextTimestamp(u64, oneshot::Sender<anyhow::Result<()>>),
    SetCurrentTimestamp(u64, oneshot::Sender<i128>),
    SetTimestampInterval(u64),
    RemoveTimestampInterval(oneshot::Sender<bool>),
}
