use super::inner::node_executor::{Command, NodeExecutorHandle};
use super::pool::{TxBatch, TxPool};
use futures::channel::mpsc::Receiver;
use futures::future::BoxFuture;
use futures::stream::{Fuse, StreamExt};
use futures::task::AtomicWaker;
use futures::Stream;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{Interval, MissedTickBehavior};
use zksync_types::H256;

// TODO: `BlockSealer` is probably a bad name as this doesn't actually seal blocks, just decides
//       that certain tx batch needs to be sealed. The actual sealing is handled in `NodeExecutor`.
//       Consider renaming.
#[pin_project::pin_project]
pub struct BlockSealer {
    /// Block sealer state (externally mutable).
    state: BlockSealerState,
    /// Pool where block sealer is sourcing transactions from.
    pool: TxPool,
    /// Node handle to be used when a block needs to be sealed.
    node_handle: NodeExecutorHandle,
    /// Future that is sending the next seal command to [`super::NodeExecutor`]
    #[pin]
    future: Option<BoxFuture<'static, Result<(), mpsc::error::SendError<Command>>>>,
}

impl BlockSealer {
    pub fn new(
        mode: BlockSealerMode,
        pool: TxPool,
        node_handle: NodeExecutorHandle,
    ) -> (Self, BlockSealerState) {
        let state = BlockSealerState {
            mode: Arc::new(RwLock::new(mode)),
            waker: Arc::new(AtomicWaker::new()),
        };
        (
            Self {
                state: state.clone(),
                pool,
                node_handle,
                future: None,
            },
            state,
        )
    }
}

impl Future for BlockSealer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        this.state.waker.register(cx.waker());
        if this.future.is_none() {
            tracing::debug!("no pending messages to node executor, polling for a new tx batch");
            let mut mode = this
                .state
                .mode
                .write()
                .expect("BlockSealer lock is poisoned");
            let tx_batch = futures::ready!(match &mut *mode {
                BlockSealerMode::Noop => Poll::Pending,
                BlockSealerMode::Immediate(immediate) => immediate.poll(&this.pool, cx),
                BlockSealerMode::FixedTime(fixed) => fixed.poll(&this.pool, cx),
            });
            tracing::debug!(
                impersonating = tx_batch.impersonating,
                txs = tx_batch.txs.len(),
                "new tx batch found"
            );
            let handle = this.node_handle.clone();
            *this.future = Some(Box::pin(async move { handle.seal_block(tx_batch).await }));
        }

        if let Some(future) = this.future.as_mut().as_pin_mut() {
            match futures::ready!(future.poll(cx)) {
                Ok(()) => {
                    // Clear pending future if it completed successfully
                    *this.future = None;
                    // Wake yourself up as we might have some unprocessed txs in the pool left
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Err(_) => {
                    tracing::error!(
                        "failed to seal a block as node executor is dropped; shutting down"
                    );
                    Poll::Ready(())
                }
            }
        } else {
            Poll::Pending
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlockSealerState {
    /// The mode this sealer currently operates in
    mode: Arc<RwLock<BlockSealerMode>>,
    /// Used for task wake up when the sealing mode was forcefully changed
    waker: Arc<AtomicWaker>,
}

impl BlockSealerState {
    pub fn is_immediate(&self) -> bool {
        matches!(
            *self.mode.read().expect("BlockSealer lock is poisoned"),
            BlockSealerMode::Immediate(_)
        )
    }

    pub fn set_mode(&self, mode: BlockSealerMode) {
        *self.mode.write().expect("BlockSealer lock is poisoned") = mode;
        // Notify last used waker that the mode might have changed
        self.waker.wake();
    }
}

/// Represents different modes of block sealing available on the node
#[derive(Debug)]
pub enum BlockSealerMode {
    /// Never seals blocks.
    Noop,
    /// Seals a block as soon as there is at least one transaction.
    Immediate(ImmediateBlockSealer),
    /// Seals a new block every `interval` tick
    FixedTime(FixedTimeBlockSealer),
}

impl BlockSealerMode {
    pub fn noop() -> Self {
        Self::Noop
    }

    pub fn immediate(max_transactions: usize, listener: Receiver<H256>) -> Self {
        Self::Immediate(ImmediateBlockSealer {
            max_transactions,
            rx: listener.fuse(),
        })
    }

    pub fn fixed_time(max_transactions: usize, block_time: Duration) -> Self {
        Self::FixedTime(FixedTimeBlockSealer::new(max_transactions, block_time))
    }

    pub fn poll(&mut self, pool: &TxPool, cx: &mut Context<'_>) -> Poll<TxBatch> {
        match self {
            BlockSealerMode::Noop => Poll::Pending,
            BlockSealerMode::Immediate(immediate) => immediate.poll(pool, cx),
            BlockSealerMode::FixedTime(fixed) => fixed.poll(pool, cx),
        }
    }
}

#[derive(Debug)]
pub struct ImmediateBlockSealer {
    /// Maximum number of transactions to include in a block.
    max_transactions: usize,
    /// Receives hashes of new transactions.
    rx: Fuse<Receiver<H256>>,
}

impl ImmediateBlockSealer {
    pub fn poll(&mut self, pool: &TxPool, cx: &mut Context<'_>) -> Poll<TxBatch> {
        match pool.take_uniform(self.max_transactions) {
            Some(tx_batch) => Poll::Ready(tx_batch),
            None => {
                let mut has_new_txs = false;
                // Yield until new transactions are available in the pool
                while let Poll::Ready(Some(_hash)) = Pin::new(&mut self.rx).poll_next(cx) {
                    has_new_txs = true;
                }

                if has_new_txs {
                    self.poll(pool, cx)
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct FixedTimeBlockSealer {
    /// Maximum number of transactions to include in a block.
    max_transactions: usize,
    /// The interval when a block should be sealed.
    interval: Interval,
}

impl FixedTimeBlockSealer {
    pub fn new(max_transactions: usize, block_time: Duration) -> Self {
        let start = tokio::time::Instant::now() + block_time;
        let mut interval = tokio::time::interval_at(start, block_time);
        // Avoid shortening interval if a tick was missed
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        Self {
            max_transactions,
            interval,
        }
    }

    pub fn poll(&mut self, pool: &TxPool, cx: &mut Context<'_>) -> Poll<TxBatch> {
        if self.interval.poll_tick(cx).is_ready() {
            // Return a batch even if the pool is empty, i.e. we produce empty blocks by design in
            // fixed time mode.
            let tx_batch = pool.take_uniform(self.max_transactions).unwrap_or(TxBatch {
                impersonating: false,
                txs: vec![],
            });
            return Poll::Ready(tx_batch);
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use crate::node::node_executor::{Command, NodeExecutorHandle};
    use crate::node::pool::TxBatch;
    use crate::node::sealer::BlockSealerMode;
    use crate::node::{BlockSealer, ImpersonationManager, TxPool};
    use anvil_zksync_types::TransactionOrder;
    use backon::Retryable;
    use backon::{ConstantBuilder, ExponentialBuilder};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc::error::TryRecvError;
    use tokio::sync::{mpsc, RwLock};
    use tokio::task::JoinHandle;

    struct Tester {
        _handle: JoinHandle<()>,
        receiver: Arc<RwLock<mpsc::Receiver<Command>>>,
    }

    impl Tester {
        fn new(sealer_mode_fn: impl FnOnce(&TxPool) -> BlockSealerMode) -> (Self, TxPool) {
            let (node_handle, receiver) = NodeExecutorHandle::test();
            let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
            let (block_sealer, _) =
                BlockSealer::new(sealer_mode_fn(&pool), pool.clone(), node_handle);
            let _handle = tokio::spawn(block_sealer);
            let receiver = Arc::new(RwLock::new(receiver));

            (Self { _handle, receiver }, pool)
        }

        async fn recv(&self) -> anyhow::Result<Command> {
            let mut receiver = self.receiver.write().await;
            tokio::time::timeout(Duration::from_millis(100), receiver.recv())
                .await
                .map_err(|_| anyhow::anyhow!("no command received"))
                .and_then(|res| res.ok_or(anyhow::anyhow!("disconnected")))
        }

        async fn expect_tx_batch(&self, expected_tx_batch: TxBatch) -> anyhow::Result<()> {
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

        /// Assert that the next command is sealing provided tx batch. Unlike `expect_tx_batch`
        /// this method does not retry.
        async fn expect_immediate_tx_batch(
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

        async fn expect_no_tx_batch(&self) -> anyhow::Result<()> {
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

        /// Assert that there are no command currently in receiver queue. Unlike `expect_no_tx_batch`
        /// this method does not retry.
        async fn expect_no_immediate_tx_batch(&self) -> anyhow::Result<()> {
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

    #[tokio::test]
    async fn immediate_empty() -> anyhow::Result<()> {
        let (tester, _pool) =
            Tester::new(|pool| BlockSealerMode::immediate(1000, pool.add_tx_listener()));

        tester.expect_no_tx_batch().await
    }

    #[tokio::test]
    async fn immediate_one_tx() -> anyhow::Result<()> {
        let (tester, pool) =
            Tester::new(|pool| BlockSealerMode::immediate(1000, pool.add_tx_listener()));

        let [tx] = pool.populate::<1>();
        tester
            .expect_tx_batch(TxBatch {
                impersonating: false,
                txs: vec![tx],
            })
            .await
    }

    #[tokio::test]
    async fn immediate_several_txs() -> anyhow::Result<()> {
        let (tester, pool) =
            Tester::new(|pool| BlockSealerMode::immediate(1000, pool.add_tx_listener()));

        let txs = pool.populate::<10>();
        tester
            .expect_tx_batch(TxBatch {
                impersonating: false,
                txs: txs.to_vec(),
            })
            .await
    }

    #[tokio::test]
    async fn immediate_respect_max_txs() -> anyhow::Result<()> {
        let (tester, pool) =
            Tester::new(|pool| BlockSealerMode::immediate(3, pool.add_tx_listener()));

        let txs = pool.populate::<10>();
        for txs in txs.chunks(3) {
            tester
                .expect_tx_batch(TxBatch {
                    impersonating: false,
                    txs: txs.to_vec(),
                })
                .await?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn immediate_gradual_txs() -> anyhow::Result<()> {
        let (tester, pool) =
            Tester::new(|pool| BlockSealerMode::immediate(1000, pool.add_tx_listener()));

        // Txs are added to the pool in small chunks
        let txs0 = pool.populate::<3>();
        let txs1 = pool.populate::<4>();
        let txs2 = pool.populate::<5>();

        let mut txs = txs0.to_vec();
        txs.extend(txs1);
        txs.extend(txs2);

        tester
            .expect_tx_batch(TxBatch {
                impersonating: false,
                txs,
            })
            .await?;

        // Txs added after the first poll should be available for sealing
        let txs = pool.populate::<10>().to_vec();
        tester
            .expect_tx_batch(TxBatch {
                impersonating: false,
                txs,
            })
            .await
    }

    #[tokio::test]
    async fn fixed_time_very_long() -> anyhow::Result<()> {
        let (tester, _pool) =
            Tester::new(|_| BlockSealerMode::fixed_time(1000, Duration::from_secs(10000)));

        tester.expect_no_tx_batch().await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fixed_time_seal_empty() -> anyhow::Result<()> {
        let (tester, _pool) =
            Tester::new(|_| BlockSealerMode::fixed_time(1000, Duration::from_millis(100)));

        // Sleep enough time to produce exactly 1 block
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Sealer should have sealed exactly one empty block by now
        tester
            .expect_immediate_tx_batch(TxBatch {
                impersonating: false,
                txs: vec![],
            })
            .await?;
        tester.expect_no_immediate_tx_batch().await?;

        // Sleep enough time to produce one more block
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Next block should be sealable
        tester
            .expect_immediate_tx_batch(TxBatch {
                impersonating: false,
                txs: vec![],
            })
            .await
    }

    #[tokio::test]
    async fn fixed_time_seal_with_txs() -> anyhow::Result<()> {
        let (tester, pool) =
            Tester::new(|_| BlockSealerMode::fixed_time(1000, Duration::from_millis(100)));

        let txs = pool.populate::<3>();

        // Sleep enough time to produce one block
        tokio::time::sleep(Duration::from_millis(150)).await;

        tester
            .expect_immediate_tx_batch(TxBatch {
                impersonating: false,
                txs: txs.to_vec(),
            })
            .await
    }

    #[tokio::test]
    async fn fixed_time_respect_max_txs() -> anyhow::Result<()> {
        let (tester, pool) =
            Tester::new(|_| BlockSealerMode::fixed_time(3, Duration::from_millis(100)));

        let txs = pool.populate::<10>();

        for txs in txs.chunks(3) {
            // Sleep enough time to produce one block
            tokio::time::sleep(Duration::from_millis(150)).await;

            tester
                .expect_immediate_tx_batch(TxBatch {
                    impersonating: false,
                    txs: txs.to_vec(),
                })
                .await?;
        }

        Ok(())
    }
}
