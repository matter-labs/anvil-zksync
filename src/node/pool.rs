use crate::node::impersonate::ImpersonationManager;
use std::sync::{Arc, RwLock};
use zksync_types::l2::L2Tx;

#[derive(Clone)]
pub struct TxPool {
    inner: Arc<RwLock<Vec<L2Tx>>>,
    impersonation: ImpersonationManager,
}

impl TxPool {
    pub fn new(impersonation: ImpersonationManager) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Vec::new())),
            impersonation,
        }
    }

    pub fn add_tx(&self, tx: L2Tx) {
        let mut guard = self.inner.write().expect("TxPool lock is poisoned");
        guard.push(tx);
    }

    /// Take up to `n` continuous transactions from the pool that are all uniform in impersonation
    /// type (either all are impersonating or all non-impersonating).
    // TODO: We should distinguish ready transactions from non-ready ones. Only ready txs should be takeable.
    pub fn take_uniform(&self, n: usize) -> Option<TxBatch> {
        if n == 0 {
            return None;
        }
        let mut guard = self.inner.write().expect("TxPool lock is poisoned");
        let mut iter = guard.iter();
        let Some(head_tx) = iter.next() else {
            // Pool is empty
            return None;
        };
        let (impersonating, tx_count) = self.impersonation.inspect(|impersonated_accounts| {
            // First tx's impersonation status decides what all other txs' impersonation status is
            // expected to be.
            let impersonating =
                impersonated_accounts.contains(&head_tx.common_data.initiator_address);
            let tail_txs = iter
                // Guaranteed to be non-zero
                .take(n - 1)
                .take_while(|tx| {
                    impersonating
                        == impersonated_accounts.contains(&tx.common_data.initiator_address)
                });
            // The amount of transactions that can be taken from the pool; `+1` accounts for `head_tx`.
            (impersonating, tail_txs.count() + 1)
        });

        let txs = guard.drain(0..tx_count).collect();
        Some(TxBatch { impersonating, txs })
    }
}

/// A batch of transactions sharing the same impersonation status.
pub struct TxBatch {
    pub impersonating: bool,
    pub txs: Vec<L2Tx>,
}
