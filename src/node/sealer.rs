use crate::node::pool::{TxBatch, TxPool};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Interval, MissedTickBehavior};

/// Represents different modes of block sealing available on the node
#[derive(Debug)]
pub enum BlockSealer {
    /// Never seals blocks.
    Noop,
    /// Seals a block as soon as there is at least one transaction.
    Immediate(ImmediateBlockSealer),
    /// Seals a new block every `interval` tick
    FixedTime(FixedTimeBlockSealer),
}

impl BlockSealer {
    pub fn noop() -> Self {
        Self::Noop
    }

    pub fn immediate(max_transactions: usize) -> Self {
        Self::Immediate(ImmediateBlockSealer { max_transactions })
    }

    pub fn fixed_time(max_transactions: usize, block_time: Duration) -> Self {
        Self::FixedTime(FixedTimeBlockSealer::new(max_transactions, block_time))
    }

    pub fn poll(&mut self, pool: &TxPool, cx: &mut Context<'_>) -> Poll<TxBatch> {
        match self {
            BlockSealer::Noop => Poll::Pending,
            BlockSealer::Immediate(immediate) => immediate.poll(pool),
            BlockSealer::FixedTime(fixed) => fixed.poll(pool, cx),
        }
    }
}

#[derive(Debug)]
pub struct ImmediateBlockSealer {
    /// Maximum number of transactions to include in a block.
    max_transactions: usize,
}

impl ImmediateBlockSealer {
    pub fn poll(&mut self, pool: &TxPool) -> Poll<TxBatch> {
        let Some(tx_batch) = pool.take_uniform(self.max_transactions) else {
            return Poll::Pending;
        };

        Poll::Ready(tx_batch)
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
    use crate::node::{BlockSealer, ImpersonationManager, TxPool};
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
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::immediate(1000);
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);
    }

    #[test]
    fn immediate_one_tx() {
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::immediate(1000);
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
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::immediate(1000);
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
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::immediate(3);
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
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::immediate(1000);
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
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::fixed_time(1000, Duration::from_secs(10000));
        let waker = &WAKER_NOOP;
        let mut cx = Context::from_waker(waker);

        assert_eq!(block_sealer.poll(&pool, &mut cx), Poll::Pending);
    }

    #[tokio::test]
    async fn fixed_time_seal_empty() {
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::fixed_time(1000, Duration::from_millis(100));
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
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::fixed_time(1000, Duration::from_millis(100));
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
        let pool = TxPool::new(ImpersonationManager::default());
        let mut block_sealer = BlockSealer::fixed_time(3, Duration::from_millis(100));
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
