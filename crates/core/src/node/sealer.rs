use super::inner::node_executor::NodeExecutorHandle;
use super::pool::{TxBatch, TxPool};
use futures::channel::mpsc::Receiver;
use futures::stream::{Fuse, StreamExt};
use futures::task::AtomicWaker;
use futures::Stream;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Interval, MissedTickBehavior};
use zksync_types::H256;

// TODO: `BlockSealer` is probably a bad name as this doesn't actually seal blocks, just decides
//       that certain tx batch needs to be sealed. The actual sealing is handled in `NodeExecutor`.
//       Consider renaming.
pub struct BlockSealer {
    /// Block sealer state (externally mutable).
    state: BlockSealerState,
    /// Pool where block sealer is sourcing transactions from.
    pool: TxPool,
    /// Node handle to be used when a block needs to be sealed.
    node_handle: NodeExecutorHandle,
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
            },
            state,
        )
    }

    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            tracing::debug!("polling for a new tx batch");
            let tx_batch = futures::future::poll_fn(|cx| {
                // Register to be woken up when sealer mode changes
                self.state.waker.register(cx.waker());
                let mut mode = self
                    .state
                    .mode
                    .write()
                    .expect("BlockSealer lock is poisoned");
                match &mut *mode {
                    BlockSealerMode::Noop => Poll::Pending,
                    BlockSealerMode::Immediate(immediate) => immediate.poll(&self.pool, cx),
                    BlockSealerMode::FixedTime(fixed) => fixed.poll(&self.pool, cx),
                }
            })
            .await;
            tracing::debug!(
                impersonating = tx_batch.impersonating,
                txs = tx_batch.txs.len(),
                "new tx batch found"
            );
            self.node_handle.seal_block(tx_batch).await?;
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
        tracing::debug!("polling immediate block sealer");
        match pool.take_uniform(self.max_transactions) {
            Some(tx_batch) => Poll::Ready(tx_batch),
            None => {
                tracing::debug!("checking for new txs");
                let mut has_new_txs = false;
                // Yield until new transactions are available in the pool
                while let Poll::Ready(Some(_hash)) = Pin::new(&mut self.rx).poll_next(cx) {
                    tracing::debug!("found new tx");
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
    use crate::node::node_executor::testing::NodeExecutorTester;
    use crate::node::pool::TxBatch;
    use crate::node::sealer::BlockSealerMode;
    use crate::node::{BlockSealer, ImpersonationManager, TxPool};
    use anvil_zksync_types::TransactionOrder;
    use std::time::Duration;
    use tokio::task::JoinHandle;

    struct BlockSealerTester {
        _handle: JoinHandle<anyhow::Result<()>>,
        node_executor_tester: NodeExecutorTester,
    }

    impl BlockSealerTester {
        fn new(sealer_mode_fn: impl FnOnce(&TxPool) -> BlockSealerMode) -> (Self, TxPool) {
            let (node_executor_tester, node_handle) = NodeExecutorTester::new();
            let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
            let (block_sealer, _) =
                BlockSealer::new(sealer_mode_fn(&pool), pool.clone(), node_handle);
            let _handle = tokio::spawn(block_sealer.run());

            (
                Self {
                    _handle,
                    node_executor_tester,
                },
                pool,
            )
        }
    }

    #[tokio::test]
    async fn immediate_empty() -> anyhow::Result<()> {
        let (tester, _pool) =
            BlockSealerTester::new(|pool| BlockSealerMode::immediate(1000, pool.add_tx_listener()));

        tester.node_executor_tester.expect_empty().await
    }

    #[tokio::test]
    async fn immediate_one_tx() -> anyhow::Result<()> {
        let (tester, pool) =
            BlockSealerTester::new(|pool| BlockSealerMode::immediate(1000, pool.add_tx_listener()));

        let [tx] = pool.populate::<1>();
        tester
            .node_executor_tester
            .expect_seal_block(TxBatch {
                impersonating: false,
                txs: vec![tx],
            })
            .await
    }

    #[tokio::test]
    async fn immediate_several_txs() -> anyhow::Result<()> {
        let (tester, pool) =
            BlockSealerTester::new(|pool| BlockSealerMode::immediate(1000, pool.add_tx_listener()));

        let txs = pool.populate::<10>();
        tester
            .node_executor_tester
            .expect_seal_block(TxBatch {
                impersonating: false,
                txs: txs.to_vec(),
            })
            .await
    }

    #[tokio::test]
    async fn immediate_respect_max_txs() -> anyhow::Result<()> {
        let (tester, pool) =
            BlockSealerTester::new(|pool| BlockSealerMode::immediate(3, pool.add_tx_listener()));

        let txs = pool.populate::<10>();
        for txs in txs.chunks(3) {
            tester
                .node_executor_tester
                .expect_seal_block(TxBatch {
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
            BlockSealerTester::new(|pool| BlockSealerMode::immediate(1000, pool.add_tx_listener()));

        // Txs are added to the pool in small chunks
        let txs0 = pool.populate::<3>();
        let txs1 = pool.populate::<4>();
        let txs2 = pool.populate::<5>();

        let mut txs = txs0.to_vec();
        txs.extend(txs1);
        txs.extend(txs2);

        tester
            .node_executor_tester
            .expect_seal_block(TxBatch {
                impersonating: false,
                txs,
            })
            .await?;

        // Txs added after the first poll should be available for sealing
        let txs = pool.populate::<10>().to_vec();
        tester
            .node_executor_tester
            .expect_seal_block(TxBatch {
                impersonating: false,
                txs,
            })
            .await
    }

    #[tokio::test]
    async fn fixed_time_very_long() -> anyhow::Result<()> {
        let (tester, _pool) = BlockSealerTester::new(|_| {
            BlockSealerMode::fixed_time(1000, Duration::from_secs(10000))
        });

        tester.node_executor_tester.expect_empty().await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fixed_time_seal_empty() -> anyhow::Result<()> {
        let (tester, _pool) = BlockSealerTester::new(|_| {
            BlockSealerMode::fixed_time(1000, Duration::from_millis(100))
        });

        // Sleep enough time to produce exactly 1 block
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Sealer should have sealed exactly one empty block by now
        tester
            .node_executor_tester
            .expect_seal_block_immediate(TxBatch {
                impersonating: false,
                txs: vec![],
            })
            .await?;
        tester.node_executor_tester.expect_empty_immediate().await?;

        // Sleep enough time to produce one more block
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Next block should be sealable
        tester
            .node_executor_tester
            .expect_seal_block_immediate(TxBatch {
                impersonating: false,
                txs: vec![],
            })
            .await
    }

    #[tokio::test]
    async fn fixed_time_seal_with_txs() -> anyhow::Result<()> {
        let (tester, pool) = BlockSealerTester::new(|_| {
            BlockSealerMode::fixed_time(1000, Duration::from_millis(100))
        });

        let txs = pool.populate::<3>();

        // Sleep enough time to produce one block
        tokio::time::sleep(Duration::from_millis(150)).await;

        tester
            .node_executor_tester
            .expect_seal_block_immediate(TxBatch {
                impersonating: false,
                txs: txs.to_vec(),
            })
            .await
    }

    #[tokio::test]
    async fn fixed_time_respect_max_txs() -> anyhow::Result<()> {
        let (tester, pool) =
            BlockSealerTester::new(|_| BlockSealerMode::fixed_time(3, Duration::from_millis(100)));

        let txs = pool.populate::<10>();

        for txs in txs.chunks(3) {
            // Sleep enough time to produce one block
            tokio::time::sleep(Duration::from_millis(150)).await;

            tester
                .node_executor_tester
                .expect_seal_block_immediate(TxBatch {
                    impersonating: false,
                    txs: txs.to_vec(),
                })
                .await?;
        }

        Ok(())
    }
}
