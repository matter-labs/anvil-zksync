use crate::node::impersonate::ImpersonationManager;
use anvil_zksync_types::{TransactionOrder, TransactionPriority};
use futures::channel::mpsc::{channel, Receiver, Sender};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use zksync_types::{Transaction, H256};

#[derive(Debug, Clone)]
pub struct TxPool {
    inner: Arc<RwLock<BTreeSet<PoolTransaction>>>,
    /// Transaction ordering in the mempool.
    transaction_order: Arc<RwLock<TransactionOrder>>,
    /// Used to preserve transactions submission order in the pool
    submission_number: Arc<Mutex<u64>>,
    /// Listeners for new transactions' hashes
    tx_listeners: Arc<Mutex<Vec<Sender<H256>>>>,
    pub(crate) impersonation: ImpersonationManager,
}

impl TxPool {
    pub fn new(impersonation: ImpersonationManager, transaction_order: TransactionOrder) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BTreeSet::new())),
            submission_number: Arc::new(Mutex::new(0)),
            tx_listeners: Arc::new(Mutex::new(Vec::new())),
            impersonation,
            transaction_order: Arc::new(RwLock::new(transaction_order)),
        }
    }

    fn lock_submission_number(&self) -> MutexGuard<'_, u64> {
        self.submission_number
            .lock()
            .expect("submission_number lock is poisoned")
    }

    fn read_transaction_order(&self) -> RwLockReadGuard<'_, TransactionOrder> {
        self.transaction_order
            .read()
            .expect("transaction_order lock is poisoned")
    }

    pub fn add_tx(&self, tx: Transaction) {
        let hash = tx.hash();
        let priority = self.read_transaction_order().priority(&tx);
        let mut submission_number = self.lock_submission_number();
        *submission_number = submission_number.wrapping_add(1);

        let mut guard = self.inner.write().expect("TxPool lock is poisoned");
        guard.insert(PoolTransaction {
            transaction: tx,
            submission_number: *submission_number,
            priority,
        });
        self.notify_listeners(hash);
    }

    pub fn add_txs(&self, txs: Vec<Transaction>) {
        let transaction_order = self.read_transaction_order();
        let mut submission_number = self.lock_submission_number();

        let mut guard = self.inner.write().expect("TxPool lock is poisoned");
        for tx in txs {
            let hash = tx.hash();
            let priority = transaction_order.priority(&tx);
            *submission_number = submission_number.wrapping_add(1);
            guard.insert(PoolTransaction {
                transaction: tx,
                submission_number: *submission_number,
                priority,
            });
            self.notify_listeners(hash);
        }
    }

    /// Removes a single transaction from the pool
    pub fn drop_transaction(&self, hash: H256) -> Option<Transaction> {
        let dropped = self.drop_transactions(|tx| tx.transaction.hash() == hash);
        dropped.first().cloned()
    }

    /// Remove transactions matching the specified condition
    pub fn drop_transactions<F>(&self, f: F) -> Vec<Transaction>
    where
        F: Fn(&PoolTransaction) -> bool,
    {
        let mut guard = self.inner.write().expect("TxPool lock is poisoned");
        let txs = std::mem::take(&mut *guard);
        let (matching_txs, other_txs) = txs.into_iter().partition(f);
        *guard = other_txs;
        matching_txs.into_iter().map(|tx| tx.transaction).collect()
    }

    /// Removes all transactions from the pool
    pub fn clear(&self) {
        let mut guard = self.inner.write().expect("TxPool lock is poisoned");
        guard.clear();
    }

    /// Take up to `n` continuous transactions from the pool that are all uniform in impersonation
    /// type (either all are impersonating or all non-impersonating).
    // TODO: We should distinguish ready transactions from non-ready ones. Only ready txs should be takeable.
    pub fn take_uniform(&self, n: usize) -> Option<TxBatch> {
        if n == 0 {
            return None;
        }
        let mut guard = self.inner.write().expect("TxPool lock is poisoned");
        let Some(head_tx) = guard.pop_last() else {
            // Pool is empty
            return None;
        };
        let mut taken_txs = vec![];
        let impersonating = self.impersonation.inspect(|state| {
            // First tx's impersonation status decides what all other txs' impersonation status is
            // expected to be.
            let impersonating = state.is_impersonating(&head_tx.transaction.initiator_account());
            taken_txs.insert(0, head_tx.transaction);
            let mut taken_txs_number = 1;

            while taken_txs_number < n {
                let Some(next_tx) = guard.last() else {
                    break;
                };
                if impersonating != state.is_impersonating(&next_tx.transaction.initiator_account())
                {
                    break;
                }
                taken_txs.insert(taken_txs_number, guard.pop_last().unwrap().transaction);
                taken_txs_number += 1;
            }
            impersonating
        });

        Some(TxBatch {
            impersonating,
            txs: taken_txs,
        })
    }

    /// Adds a new transaction listener to the pool that gets notified about every new transaction.
    pub fn add_tx_listener(&self) -> Receiver<H256> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.tx_listeners
            .lock()
            .expect("TxPool lock is poisoned")
            .push(tx);
        rx
    }

    /// Notifies all listeners about the transaction.
    fn notify_listeners(&self, tx_hash: H256) {
        let mut tx_listeners = self.tx_listeners.lock().expect("TxPool lock is poisoned");
        tx_listeners.retain_mut(|listener| match listener.try_send(tx_hash) {
            Ok(()) => true,
            Err(e) => {
                if e.is_full() {
                    tracing::warn!(
                        %tx_hash,
                        "Failed to send transaction notification because channel is full",
                    );
                    true
                } else {
                    false
                }
            }
        });
    }
}

// Test utilities
#[cfg(test)]
impl TxPool {
    /// Populates pool with `N` randomly generated transactions without impersonation.
    pub fn populate<const N: usize>(&self) -> [Transaction; N] {
        let to_impersonate = [false; N];
        self.populate_impersonate(to_impersonate)
    }

    /// Populates pool with `N` randomly generated transactions where `i`-th transaction is using an
    /// impersonated account if `to_impersonate[i]` is `true`.
    pub fn populate_impersonate<const N: usize>(
        &self,
        to_impersonate: [bool; N],
    ) -> [Transaction; N] {
        to_impersonate.map(|to_impersonate| {
            let tx: Transaction = crate::testing::TransactionBuilder::new().build().into();

            if to_impersonate {
                assert!(self.impersonation.impersonate(tx.initiator_account()));
            }

            self.add_tx(tx.clone());
            tx
        })
    }
}

/// A batch of transactions meant to be sealed as a block. All transactions in the batch share the
/// same impersonation status on the moment of the batch's creation.
///
/// A block produced from this batch is guaranteed to:
/// * Not contain any transactions outside of this transaction batch
/// * Use contracts matching `impersonating` mode of this transaction batch.
///
/// Potential caveats:
/// * The impersonation status of transactions' initiators (as defined by [`ImpersonationManager`])
///   is not guaranteed to be the same by the time the batch gets executed
/// * The resulting block is not guaranteed to contain all transactions as some of them could be
///   non-executable.
#[derive(PartialEq, Debug)]
pub struct TxBatch {
    pub impersonating: bool,
    pub txs: Vec<Transaction>,
}

/// A reference to a transaction in the pool
#[derive(Clone, Debug)]
pub struct PoolTransaction {
    /// actual transaction
    pub transaction: Transaction,
    /// Used to internally compare the transaction in the pool
    pub submission_number: u64,
    /// priority of the transaction
    pub priority: TransactionPriority,
}

impl Eq for PoolTransaction {}

impl PartialEq<Self> for PoolTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd<Self> for PoolTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PoolTransaction {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.submission_number.cmp(&self.submission_number))
    }
}

#[cfg(test)]
mod tests {
    use crate::node::impersonate::ImpersonationState;
    use crate::node::pool::TxBatch;
    use crate::node::{ImpersonationManager, TxPool};
    use crate::testing;
    use anvil_zksync_types::TransactionOrder;
    use test_case::test_case;
    use zksync_types::{Transaction, U256};

    #[test]
    fn take_from_empty() {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);
        assert_eq!(pool.take_uniform(1), None);
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_zero(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

        pool.populate_impersonate([imp]);
        assert_eq!(pool.take_uniform(0), None);
    }

    #[test_case(false ; "not impersonated")]
    #[test_case(true  ; "is impersonated")]
    fn take_exactly_one(imp: bool) {
        let impersonation = ImpersonationManager::default();
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
        let pool = TxPool::new(impersonation, TransactionOrder::Fifo);

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
                .stop_impersonating(&tx2.initiator_account());
        } else {
            pool.impersonation.impersonate(tx2.initiator_account());
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
        let pool = TxPool::new(impersonation.clone(), TransactionOrder::Fifo);

        for _ in 0..4096 {
            let tx: Transaction = testing::TransactionBuilder::new().build().into();

            assert!(pool.impersonation.impersonate(tx.initiator_account()));

            pool.add_tx(tx.clone());
        }

        let take_handle = tokio::spawn(async move { pool.take_uniform(4096) });
        let clear_impersonation_handle =
            tokio::spawn(async move { impersonation.set_state(ImpersonationState::default()) });

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

    #[tokio::test]
    async fn take_uses_transaction_order() {
        let impersonation = ImpersonationManager::default();
        let pool_fifo = TxPool::new(impersonation.clone(), TransactionOrder::Fifo);
        let pool_fees = TxPool::new(impersonation.clone(), TransactionOrder::Fees);

        let txs: Vec<Transaction> = [1, 2, 3]
            .iter()
            .map(|index| {
                let tx: Transaction = testing::TransactionBuilder::new()
                    .set_max_fee_per_gas(U256::from(50_000_000 + index))
                    .build()
                    .into();
                pool_fifo.add_tx(tx.clone());
                pool_fees.add_tx(tx.clone());
                tx
            })
            .collect();

        assert_eq!(
            pool_fifo.take_uniform(3),
            Some(TxBatch {
                impersonating: false,
                txs: vec![txs[0].clone(), txs[1].clone(), txs[2].clone()]
            })
        );

        assert_eq!(
            pool_fees.take_uniform(3),
            Some(TxBatch {
                impersonating: false,
                txs: vec![txs[2].clone(), txs[1].clone(), txs[0].clone()]
            })
        );
    }
}
