use crate::node::pool::TxBatch;
use crate::node::InMemoryNode;
use crate::system_contracts::SystemContracts;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::{mpsc, oneshot};
use zksync_multivm::interface::TxExecutionMode;
use zksync_types::L2BlockNumber;

pub struct BlockProducer {
    node: InMemoryNode,
    system_contracts: SystemContracts,
    command_receiver: mpsc::Receiver<Command>,
}

impl BlockProducer {
    pub fn new(
        node: InMemoryNode,
        system_contracts: SystemContracts,
    ) -> (Self, BlockProducerHandle) {
        // Since we process `BlockProducer` commands one-by-one (the next command is never enqueued
        // until a previous command is processed), capacity 1 is enough for the commands channel.
        let (command_sender, command_receiver) = mpsc::channel(1);
        let this = Self {
            node,
            system_contracts,
            command_receiver,
        };
        let handle = BlockProducerHandle { command_sender };
        (this, handle)
    }
}

impl Future for BlockProducer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();

        while let Poll::Ready(command_opt) = pin.command_receiver.poll_recv(cx) {
            let Some(command) = command_opt else {
                tracing::trace!("channel has been closed; stopping block production");
                return Poll::Ready(());
            };
            match command {
                Command::SealBlock(tx_batch, reply) => {
                    let TxBatch { impersonating, txs } = tx_batch;

                    let base_system_contracts = pin
                        .system_contracts
                        .contracts(TxExecutionMode::VerifyExecute, impersonating)
                        .clone();
                    let result =
                        pin.node
                            .seal_block(&mut pin.node.time.lock(), txs, base_system_contracts);
                    // Reply to sender if we can, otherwise hold result for further processing
                    let result = if let Some(reply) = reply {
                        if let Err(result) = reply.send(result) {
                            tracing::info!("failed to reply as receiver has been dropped");
                            result
                        } else {
                            continue;
                        }
                    } else {
                        result
                    };
                    // Not much we can do with an error at this level so we just print it
                    if let Err(err) = result {
                        tracing::error!("failed to seal a block: {:#?}", err);
                    }
                }
            }
        }

        Poll::Pending
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
}

#[derive(Debug)]
pub enum Command {
    SealBlock(
        TxBatch,
        Option<oneshot::Sender<anyhow::Result<L2BlockNumber>>>,
    ),
}
