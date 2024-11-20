use crate::node::pool::{TxBatch, TxPool};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Interval, MissedTickBehavior};

/// Mode of operations for the `BlockSealer`
#[derive(Debug)]
pub enum BlockSealer {
    /// Seals a block as soon as there is at least one transaction.
    Immediate(ImmediateBlockSealer),
    /// Seals a new block every `interval` tick
    FixedTime(FixedTimeBlockSealer),
}

impl BlockSealer {
    pub fn immediate(max_transactions: usize) -> Self {
        Self::Immediate(ImmediateBlockSealer { max_transactions })
    }

    pub fn fixed_time(max_transactions: usize, block_time: Duration) -> Self {
        Self::FixedTime(FixedTimeBlockSealer::new(max_transactions, block_time))
    }

    pub fn poll(&mut self, pool: &TxPool, cx: &mut Context<'_>) -> Poll<TxBatch> {
        match self {
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
