use std::fmt;
use std::str::FromStr;
use zksync_types::{Transaction, U256};

/// Metric value for the priority of a transaction.
///
/// The `TransactionPriority` determines the ordering of two transactions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionPriority(pub U256);

/// Modes that determine the transaction ordering of the mempool
///
/// This type controls the transaction order via the priority metric of a transaction
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransactionOrder {
    /// Keep the pool transactions sorted in the order they arrive.
    ///
    /// This will essentially assign every transaction the exact priority so the order is
    /// determined by their internal submission number
    Fifo,
    /// This means that it prioritizes transactions based on the fees paid to the miner.
    #[default]
    Fees,
}

impl TransactionOrder {
    /// Returns the priority of the transactions
    pub fn priority(&self, tx: &Transaction) -> TransactionPriority {
        match self {
            Self::Fifo => TransactionPriority::default(),
            Self::Fees => TransactionPriority(tx.max_fee_per_gas()),
        }
    }
}

impl FromStr for TransactionOrder {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_lowercase();
        let order = match s.as_str() {
            "fees" => Self::Fees,
            "fifo" => Self::Fifo,
            _ => return Err(format!("Unknown TransactionOrder: `{s}`")),
        };
        Ok(order)
    }
}

impl fmt::Display for TransactionOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TransactionOrder::Fifo => f.write_str("fifo"),
            TransactionOrder::Fees => f.write_str("fees"),
        }
    }
}
