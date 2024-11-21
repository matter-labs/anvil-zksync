use crate::node::impersonate::ImpersonationManager;
use std::sync::{Arc, RwLock};
use zksync_types::l2::L2Tx;

#[derive(Clone)]
pub struct TxPool {
    inner: Arc<RwLock<Vec<L2Tx>>>,
    pub(crate) impersonation: ImpersonationManager,
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

// Test utilities
#[cfg(test)]
impl TxPool {
    /// Populates pool with `N` randomly generated transactions without impersonation.
    pub fn populate<const N: usize>(&self) -> [L2Tx; N] {
        let to_impersonate = [false; N];
        self.populate_impersonate(to_impersonate)
    }

    /// Populates pool with `N` randomly generated transactions where `i`-th transaction is using an
    /// impersonated account if `to_impersonate[i]` is `true`.
    pub fn populate_impersonate<const N: usize>(&self, to_impersonate: [bool; N]) -> [L2Tx; N] {
        to_impersonate.map(|to_impersonate| {
            let tx = crate::testing::TransactionBuilder::new().build();

            if to_impersonate {
                assert!(self
                    .impersonation
                    .impersonate(tx.common_data.initiator_address));
            }

            self.add_tx(tx.clone());
            tx
        })
    }
}

/// A batch of transactions sharing the same impersonation status.
#[derive(PartialEq, Debug)]
pub struct TxBatch {
    pub impersonating: bool,
    pub txs: Vec<L2Tx>,
}

#[cfg(test)]
mod tests {
    use crate::node::pool::TxBatch;
    use crate::node::{ImpersonationManager, TxPool};
    use crate::testing;
    use std::collections::HashSet;
    use test_case::test_case;

    #[test]
    fn take_from_empty() {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);
        assert_eq!(pool.take_uniform(1), None);
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_zero(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        pool.populate_impersonate([imp]);
        assert_eq!(pool.take_uniform(0), None);
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_exactly_one(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let [tx0, ..] = pool.populate_impersonate([imp, false]);
        assert_eq!(
            pool.take_uniform(1),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx0]
            })
        );
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_exactly_two(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let [tx0, tx1, ..] = pool.populate_impersonate([imp, imp, false]);
        assert_eq!(
            pool.take_uniform(2),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx0, tx1]
            })
        );
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_one_eligible(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let [tx0, ..] = pool.populate_impersonate([imp, !imp, !imp, !imp]);
        assert_eq!(
            pool.take_uniform(4),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx0]
            })
        );
    }

    // 3 transactions in total: 1 and 2 share impersonation status, 3 does not.
    // `TxPool` should only take [1, 2] when 3 txs are requested.
    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_two_when_third_is_not_uniform(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let [tx0, tx1, ..] = pool.populate_impersonate([imp, imp, !imp]);
        assert_eq!(
            pool.take_uniform(3),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx0, tx1]
            })
        );
    }

    // 4 transactions in total: 1, 2 and 4 share impersonation status, 3 does not.
    // `TxPool` should only take [1, 2] when 4 txs are requested.
    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_interrupted_by_non_uniformness(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let [tx0, tx1, ..] = pool.populate_impersonate([imp, imp, !imp, imp]);
        assert_eq!(
            pool.take_uniform(4),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx0, tx1]
            })
        );
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_multiple(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let [tx0, tx1, tx2, tx3] = pool.populate_impersonate([imp, !imp, !imp, imp]);
        assert_eq!(
            pool.take_uniform(100),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx0]
            })
        );
        assert_eq!(
            pool.take_uniform(100),
            Some(TxBatch {
                impersonating: !imp,
                txs: vec![tx1, tx2]
            })
        );
        assert_eq!(
            pool.take_uniform(100),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx3]
            })
        );
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn pool_clones_share_state(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let txs = {
            let pool_clone = pool.clone();
            pool_clone.populate_impersonate([imp, imp, imp])
        };
        assert_eq!(
            pool.take_uniform(3),
            Some(TxBatch {
                impersonating: imp,
                txs: txs.to_vec()
            })
        );
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_multiple_from_clones(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let [tx0, tx1, tx2, tx3] = {
            let pool_clone = pool.clone();
            pool_clone.populate_impersonate([imp, !imp, !imp, imp])
        };
        let pool0 = pool.clone();
        assert_eq!(
            pool0.take_uniform(100),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx0]
            })
        );
        let pool1 = pool.clone();
        assert_eq!(
            pool1.take_uniform(100),
            Some(TxBatch {
                impersonating: !imp,
                txs: vec![tx1, tx2]
            })
        );
        let pool2 = pool.clone();
        assert_eq!(
            pool2.take_uniform(100),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx3]
            })
        );
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_respects_impersonation_change(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation);

        let [tx0, tx1, tx2, tx3] = pool.populate_impersonate([imp, imp, !imp, imp]);
        assert_eq!(
            pool.take_uniform(4),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx0, tx1]
            })
        );

        // Change tx2's impersonation status to opposite
        if !imp {
            pool.impersonation
                .stop_impersonating(&tx2.common_data.initiator_address);
        } else {
            pool.impersonation
                .impersonate(tx2.common_data.initiator_address);
        }

        assert_eq!(
            pool.take_uniform(4),
            Some(TxBatch {
                impersonating: imp,
                txs: vec![tx2, tx3]
            })
        );
    }

    #[tokio::test]
    async fn take_uses_consistent_impersonation() {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation.clone());

        for _ in 0..4096 {
            let tx = testing::TransactionBuilder::new().build();

            assert!(pool
                .impersonation
                .impersonate(tx.common_data.initiator_address));

            pool.add_tx(tx.clone());
        }

        let take_handle = tokio::spawn(async move { pool.take_uniform(4096) });
        let clear_impersonation_handle =
            tokio::spawn(async move { impersonation.set_impersonated_accounts(HashSet::new()) });

        clear_impersonation_handle.await.unwrap();
        let tx_batch = take_handle
            .await
            .unwrap()
            .expect("failed to take a tx batch");
        // Note that we do not assert impersonation status as both `true` and `false` are valid
        // results here depending on the race between the two tasks above. But the returned
        // transactions should always be a complete set - in other words, `TxPool` should not see
        // a change in impersonation state partway through iterating the transactions.
        assert_eq!(tx_batch.txs.len(), 4096);
    }
}
