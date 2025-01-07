use crate::node::block_producer::{BlockProducerHandle, Command};
use crate::node::pool::{TxBatch, TxPool};
use futures::channel::mpsc::Receiver;
use futures::future::LocalBoxFuture;
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
//       that certain tx batch needs to be sealed. The actual sealing is handled in `BlockProducer`.
//       Consider renaming.
#[pin_project::pin_project]
pub struct BlockSealer {
    /// Block sealer state (externally mutable).
    state: BlockSealerState,
    /// Pool where block sealer is sourcing transactions from.
    pool: TxPool,
    /// Block producer to be used when a block needs to be sealed.
    block_producer_handle: BlockProducerHandle,
    /// Future that is sending the next seal command to [`super::BlockProducer`]
    #[pin]
    future: Option<LocalBoxFuture<'static, Result<(), mpsc::error::SendError<Command>>>>,
}

impl BlockSealer {
    pub fn new(
        mode: BlockSealerMode,
        pool: TxPool,
        block_producer_handle: BlockProducerHandle,
    ) -> (Self, BlockSealerState) {
        let state = BlockSealerState {
            mode: Arc::new(RwLock::new(mode)),
            waker: Arc::new(AtomicWaker::new()),
        };
        (
            Self {
                state: state.clone(),
                pool,
                block_producer_handle,
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
            tracing::debug!("no pending messages to block producer, polling for a new tx batch");
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
            let handle = this.block_producer_handle.clone();
            *this.future = Some(Box::pin(async move { handle.seal_block(tx_batch).await }));
        }

        if let Some(future) = this.future.as_mut().as_pin_mut() {
            match futures::ready!(future.poll(cx)) {
                Ok(()) => {
                    // Clear pending future if it completed successfully
                    *this.future = None;
                    Poll::Pending
                }
                Err(_) => {
                    tracing::error!(
                        "failed to seal a block as block producer is dropped; shutting down"
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
    use crate::node::pool::TxBatch;
    use crate::node::sealer::BlockSealerMode;
    use crate::node::{BlockSealer, ImpersonationManager, TxPool};
    use anvil_zksync_types::TransactionOrder;
    use std::ptr;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::time::Duration;

    const NOOP: RawWaker = {
        const VTABLE: RawWakerVTable = RawWakerVTable::new(
            // Cloning just returns a new no-op raw waker
            |_| NOOP,
            // `wake` does nothing
            |_| {},
            // `wake_by_ref` does nothing
            |_| {},
            // Dropping does nothing as we don't allocate anything
            |_| {},
        );
        RawWaker::new(ptr::null(), &VTABLE)
    };
    const WAKER_NOOP: Waker = unsafe { Waker::from_raw(NOOP) };

    #[test]
    fn immediate_empty() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer =
            BlockSealer::new(BlockSealerMode::immediate(1000, pool.add_tx_listener()));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);
    }

    #[test]
    fn immediate_one_tx() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer =
            BlockSealer::new(BlockSealerMode::immediate(1000, pool.add_tx_listener()));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        let [tx] = pool.populate::<1>();

        assert_eq!(
            block_sealer.poll(&pool, &mut cx),
            Poll::Ready(TxBatch {
                impersonating: false,
                txs: vec![tx]
            })
        );
        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);
    }

    #[test]
    fn immediate_several_txs() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer =
            BlockSealer::new(BlockSealerMode::immediate(1000, pool.add_tx_listener()));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        let txs = pool.populate::<10>();

        assert_eq!(
            block_sealer.poll(&pool, &mut cx),
            Poll::Ready(TxBatch {
                impersonating: false,
                txs: txs.to_vec()
            })
        );
        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);
    }

    #[test]
    fn immediate_respect_max_txs() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer =
            BlockSealer::new(BlockSealerMode::immediate(3, pool.add_tx_listener()));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        let txs = pool.populate::<10>();

        for txs in txs.chunks(3) {
            assert_eq!(
                block_sealer.poll(&pool, &mut cx),
                Poll::Ready(TxBatch {
                    impersonating: false,
                    txs: txs.to_vec()
                })
            );
        }
    }

    #[test]
    fn immediate_gradual_txs() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer =
            BlockSealer::new(BlockSealerMode::immediate(1000, pool.add_tx_listener()));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        // Txs are added to the pool in small chunks
        let txs0 = pool.populate::<3>();
        let txs1 = pool.populate::<4>();
        let txs2 = pool.populate::<5>();

        let mut txs = txs0.to_vec();
        txs.extend(txs1);
        txs.extend(txs2);

        assert_eq!(
            block_sealer.poll(&pool, &mut cx),
            Poll::Ready(TxBatch {
                impersonating: false,
                txs,
            })
        );
        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);

        // Txs added after the first poll should be available for sealing
        let txs = pool.populate::<10>().to_vec();
        assert_eq!(
            block_sealer.poll(&pool, &mut cx),
            Poll::Ready(TxBatch {
                impersonating: false,
                txs,
            })
        );
        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);
    }

    #[tokio::test]
    async fn fixed_time_very_long() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer = BlockSealer::new(BlockSealerMode::fixed_time(
            1000,
            Duration::from_secs(10000),
        ));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);
    }

    #[tokio::test]
    async fn fixed_time_seal_empty() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer = BlockSealer::new(BlockSealerMode::fixed_time(
            1000,
            Duration::from_millis(100),
        ));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        // Sleep enough time to (theoretically) produce at least 2 blocks
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Sealer should seal one empty block when polled and then refuse to seal another one
        // shortly after as it ensures enough time passes in-between of blocks.
        assert_eq!(
            block_sealer.poll(&pool, &mut cx),
            Poll::Ready(TxBatch {
                impersonating: false,
                txs: vec![]
            })
        );
        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);

        // Sleep enough time to produce one block
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Next block should be sealable
        assert_eq!(
            block_sealer.poll(&pool, &mut cx),
            Poll::Ready(TxBatch {
                impersonating: false,
                txs: vec![]
            })
        );
    }

    #[tokio::test]
    async fn fixed_time_seal_with_txs() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer = BlockSealer::new(BlockSealerMode::fixed_time(
            1000,
            Duration::from_millis(100),
        ));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        let txs = pool.populate::<3>();

        // Sleep enough time to produce one block
        tokio::time::sleep(Duration::from_millis(150)).await;

        assert_eq!(
            block_sealer.poll(&pool, &mut cx),
            Poll::Ready(TxBatch {
                impersonating: false,
                txs: txs.to_vec()
            })
        );
    }

    #[tokio::test]
    async fn fixed_time_respect_max_txs() {
        let pool = TxPool::new(ImpersonationManager::default(), TransactionOrder::Fifo);
        let mut block_sealer =
            BlockSealer::new(BlockSealerMode::fixed_time(3, Duration::from_millis(100)));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        let txs = pool.populate::<10>();

        for txs in txs.chunks(3) {
            // Sleep enough time to produce one block
            tokio::time::sleep(Duration::from_millis(150)).await;

            assert_eq!(
                block_sealer.poll(&pool, &mut cx),
                Poll::Ready(TxBatch {
                    impersonating: false,
                    txs: txs.to_vec()
                })
            );
        }
    }
}
