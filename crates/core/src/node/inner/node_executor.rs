use super::InMemoryNodeInner;
use crate::node::pool::TxBatch;
use crate::system_contracts::SystemContracts;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use zksync_multivm::interface::TxExecutionMode;
use zksync_types::L2BlockNumber;

pub struct NodeExecutor {
    node_inner: Arc<RwLock<InMemoryNodeInner>>,
    system_contracts: SystemContracts,
    command_receiver: mpsc::Receiver<Command>,
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
        };
        let handle = NodeExecutorHandle { command_sender };
        (this, handle)
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        while let Some(command) = self.command_receiver.recv().await {
            match command {
                Command::SealBlock(tx_batch, reply) => {
                    self.seal_block(tx_batch, reply).await;
                }
                Command::SealBlocks(tx_batches, interval, reply) => {
                    self.seal_blocks(tx_batches, interval, reply).await;
                }
                Command::IncreaseTime(delta, reply) => {
                    self.increase_time(delta, reply).await;
                }
                Command::EnforceNextTimestamp(timestamp, reply) => {
                    self.enforce_next_timestamp(timestamp, reply).await;
                }
                Command::SetCurrentTimestamp(timestamp, reply) => {
                    self.set_current_timestamp(timestamp, reply).await;
                }
                Command::SetTimestampInterval(seconds) => {
                    self.set_timestamp_interval(seconds).await;
                }
                Command::RemoveTimestampInterval(reply) => {
                    self.remove_timestamp_interval(reply).await;
                }
            }
        }

        tracing::trace!("channel has been closed; stopping node executor");
        Ok(())
    }
}

impl NodeExecutor {
    async fn seal_block(
        &self,
        TxBatch { impersonating, txs }: TxBatch,
        reply: Option<oneshot::Sender<anyhow::Result<L2BlockNumber>>>,
    ) {
        let base_system_contracts = self
            .system_contracts
            .contracts(TxExecutionMode::VerifyExecute, impersonating)
            .clone();
        let result = self
            .node_inner
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
        &self,
        tx_batches: Vec<TxBatch>,
        interval: u64,
        reply: oneshot::Sender<anyhow::Result<Vec<L2BlockNumber>>>,
    ) {
        let mut node_inner = self.node_inner.write().await;

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
                let base_system_contracts = self
                    .system_contracts
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

    async fn increase_time(&self, delta: u64, reply: oneshot::Sender<()>) {
        self.node_inner
            .write()
            .await
            .time_writer
            .increase_time(delta);
        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn enforce_next_timestamp(
        &self,
        timestamp: u64,
        reply: oneshot::Sender<anyhow::Result<()>>,
    ) {
        let result = self
            .node_inner
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

    async fn set_current_timestamp(&self, timestamp: u64, reply: oneshot::Sender<i128>) {
        let result = self
            .node_inner
            .write()
            .await
            .time_writer
            .set_current_timestamp_unchecked(timestamp);
        // Reply to sender if we can
        if reply.send(result).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn set_timestamp_interval(&self, delta: u64) {
        self.node_inner
            .write()
            .await
            .time_writer
            .set_block_timestamp_interval(Some(delta));
    }

    async fn remove_timestamp_interval(&self, reply: oneshot::Sender<bool>) {
        let result = self
            .node_inner
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
    pub async fn seal_block(&self, tx_batch: TxBatch) -> anyhow::Result<()> {
        Ok(self
            .command_sender
            .send(Command::SealBlock(tx_batch, None))
            .await?)
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
    pub async fn set_block_timestamp_interval(&self, seconds: u64) -> anyhow::Result<()> {
        Ok(self
            .command_sender
            .send(Command::SetTimestampInterval(seconds))
            .await?)
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

#[derive(Debug)]
enum Command {
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

#[cfg(test)]
pub mod testing {
    use super::*;
    use backon::{ConstantBuilder, ExponentialBuilder, Retryable};
    use std::time::Duration;
    use tokio::sync::mpsc::error::TryRecvError;

    pub struct NodeExecutorTester {
        receiver: Arc<RwLock<mpsc::Receiver<Command>>>,
    }

    impl NodeExecutorTester {
        pub fn new() -> (Self, NodeExecutorHandle) {
            let (command_sender, command_receiver) = mpsc::channel(128);
            (
                Self {
                    receiver: Arc::new(RwLock::new(command_receiver)),
                },
                NodeExecutorHandle { command_sender },
            )
        }

        async fn recv(&self) -> anyhow::Result<Command> {
            let mut receiver = self.receiver.write().await;
            tokio::time::timeout(Duration::from_millis(100), receiver.recv())
                .await
                .map_err(|_| anyhow::anyhow!("no command received"))
                .and_then(|res| res.ok_or(anyhow::anyhow!("disconnected")))
        }

        /// Assert that the next command is sealing provided tx batch. Waits with a timeout for the
        /// next command to arrive if the command queue is empty.
        pub async fn expect_seal_block(&self, expected_tx_batch: TxBatch) -> anyhow::Result<()> {
            let command = (|| self.recv())
                .retry(ExponentialBuilder::default())
                .await?;
            match command {
                Command::SealBlock(actual_tx_batch, _) if actual_tx_batch == expected_tx_batch => {
                    Ok(())
                }
                _ => anyhow::bail!("unexpected command: {:?}", command),
            }
        }

        /// Assert that the next command is sealing provided tx batch. Unlike `expect_seal_block`
        /// this method does not retry.
        pub async fn expect_seal_block_immediate(
            &self,
            expected_tx_batch: TxBatch,
        ) -> anyhow::Result<()> {
            let result = self.receiver.write().await.try_recv();
            match result {
                Ok(Command::SealBlock(actual_tx_batch, _))
                    if actual_tx_batch == expected_tx_batch =>
                {
                    Ok(())
                }
                Ok(command) => anyhow::bail!("unexpected command: {:?}", command),
                Err(TryRecvError::Empty) => anyhow::bail!("no command received"),
                Err(TryRecvError::Disconnected) => anyhow::bail!("disconnected"),
            }
        }

        /// Assert that there are no command currently in receiver queue. Waits up to a timeout for
        /// the next command to potentially arrive.
        pub async fn expect_empty(&self) -> anyhow::Result<()> {
            let result = (|| self.recv())
                .retry(
                    ConstantBuilder::default()
                        .with_delay(Duration::from_millis(100))
                        .with_max_times(3),
                )
                .await;
            match result {
                Ok(command) => {
                    anyhow::bail!("unexpected command: {:?}", command)
                }
                Err(err) if err.to_string().contains("no command received") => Ok(()),
                Err(err) => anyhow::bail!("unexpected error: {:?}", err),
            }
        }

        /// Assert that there are no command currently in receiver queue. Unlike `expect_empty`
        /// this method does not retry.
        pub async fn expect_empty_immediate(&self) -> anyhow::Result<()> {
            let result = self.receiver.write().await.try_recv();
            match result {
                Ok(command) => {
                    anyhow::bail!("unexpected command: {:?}", command)
                }
                Err(TryRecvError::Empty) => Ok(()),
                Err(TryRecvError::Disconnected) => anyhow::bail!("disconnected"),
            }
        }
    }
}
