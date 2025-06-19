//!
//! Summarizes transaction effects and pretty prints them.
//!
//! This module provides utilities for creating detailed summaries of transaction
//! execution, including gas usage, costs, storage changes, and execution status.
//!

use std::fmt::Display;

use crate::utils::to_human_size;
use anvil_zksync_common::utils::cost::{format_eth, format_gwei};
use anvil_zksync_types::traces::LabeledAddress;
use zksync_multivm::interface::{ExecutionResult, VmExecutionResultAndLogs};
use zksync_types::{Address, Transaction, H256, U256};

///
/// Kind of outcomes of transaction execution.
///
pub enum TransactionStatus {
    Success,
    Failure,
    Halt,
}

///
/// Part of the transaction summary describing the chain-level context.
/// Contains information about the environment where the transaction was executed.
///
pub struct TransactionContext {
    /// Gas price on L2 in wei
    l2_gas_price: u64,
}

///
/// Part of the transaction summary describing the gas consumption details.
///
///
/// Details of gas usage for transaction execution.
///
pub struct GasDetails {
    limit: U256,
    used: U256,
    refunded: u64,
}

///
/// Holds a fragment of account state before and after transaction.
///
pub struct BalanceDiff {
    pub address: LabeledAddress,
    pub balance_before: U256,
    pub balance_after: U256,
}

///
/// A comprehensive summary of transaction execution results.
/// Contains all details about transaction status, participants,
/// resources consumed, and costs.
///
pub struct TransactionSummary {
    /// Execution outcome
    status: TransactionStatus,
    /// Transaction hash
    tx_hash: H256,
    /// Address that initiated the transaction
    initiator: Address,
    /// Address that paid for the transaction
    payer: Address,
    /// Execution context information
    context: TransactionContext,
    /// Gas consumption details
    gas: GasDetails,
    /// Changes in balances.
    balance_diffs: Option<Vec<BalanceDiff>>,
}

impl TransactionSummary {
    /// Creates a new transaction summary from execution results.
    ///
    /// # Arguments
    ///
    /// * `l2_gas_price` - The gas price on L2 in wei
    /// * `tx` - The executed transaction
    /// * `tx_result` - The execution results and logs
    pub fn new(
        l2_gas_price: u64,
        tx: &Transaction,
        tx_result: &VmExecutionResultAndLogs,
        balance_diffs: Option<Vec<BalanceDiff>>,
    ) -> Self {
        let status: TransactionStatus = (&tx_result.result).into();
        let tx_hash = tx.hash();
        let initiator = tx.initiator_account();
        let payer = tx.payer();

        let used = tx.gas_limit() - tx_result.refunds.gas_refunded;
        let limit = tx.gas_limit();
        let refunded = tx_result.refunds.gas_refunded;

        Self {
            status,
            tx_hash,
            initiator,
            payer,
            context: TransactionContext { l2_gas_price },
            gas: GasDetails {
                limit,
                used,
                refunded,
            },
            balance_diffs,
        }
    }
}

impl Display for TransactionSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            status,
            tx_hash,
            initiator,
            payer,
            context: TransactionContext { l2_gas_price },
            gas,
            balance_diffs,
        } = self;

        // Calculate gas costs in ETH
        let paid = U256::from(*l2_gas_price) * gas.used;
        let refunded = U256::from(*l2_gas_price) * gas.refunded;

        // Format human-readable values
        let gas_used = gas.used;
        let gas_limit_human = to_human_size(gas.limit);
        let gas_used_human = to_human_size(gas.used);
        let gas_refunded_human = to_human_size(gas.refunded.into());
        let emoji = self.status.emoji();
        let l2_gas_price_human = format_gwei(self.context.l2_gas_price.into());

        // Basic transaction information
        write!(
            f,
            r#"
{emoji} [{status}] Hash: {tx_hash:?}
Initiator: {initiator:?}
Payer: {payer:?}
Gas Limit: {gas_limit_human} | Used: {gas_used_human} | Refunded: {gas_refunded_human}
Paid: {paid_in_eth} ({gas_used} gas * {l2_gas_price_human})
Refunded: {refunded_in_eth}
"#,
            paid_in_eth = format_eth(paid),
            refunded_in_eth = format_eth(refunded),
        )?;

        if let Some(balance_diffs) = balance_diffs {
            if !balance_diffs.is_empty() {
                let mut balance_diffs_formatted_table = tabled::Table::new(
                    balance_diffs
                        .iter()
                        .map(Into::<internal::BalanceDiffRepr>::into)
                        .collect::<Vec<_>>(),
                );
                balance_diffs_formatted_table.with(tabled::settings::Style::modern());

                write!(f, "\n{balance_diffs_formatted_table}")?;
            }
        }
        Ok(())
    }
}

impl TransactionStatus {
    fn emoji(&self) -> &str {
        match self {
            Self::Success => "✅",
            Self::Failure => "❌",
            Self::Halt => "⏸️",
        }
    }
}
impl From<&ExecutionResult> for TransactionStatus {
    fn from(value: &ExecutionResult) -> Self {
        match value {
            ExecutionResult::Success { .. } => Self::Success,
            ExecutionResult::Revert { .. } => Self::Failure,
            ExecutionResult::Halt { .. } => Self::Halt,
        }
    }
}
impl Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TransactionStatus::Success => "SUCCESS",
            TransactionStatus::Failure => "FAILED",
            TransactionStatus::Halt => "HALTED",
        })
    }
}

impl From<crate::node::diagnostics::vm::balance_diff::BalanceDiff> for BalanceDiff {
    fn from(value: crate::node::diagnostics::vm::balance_diff::BalanceDiff) -> Self {
        let crate::node::diagnostics::vm::balance_diff::BalanceDiff {
            address,
            balance_before,
            balance_after,
        } = value;
        Self {
            address,
            balance_before,
            balance_after,
        }
    }
}
mod internal {
    use std::cmp::Ordering;

    use anvil_zksync_common::utils::cost::{format_eth, format_gwei};
    use zksync_types::U256;

    use super::BalanceDiff;

    ///
    /// Representation of `[BalanceDiff]`, prepared for formatting using `Tabled`
    ///
    #[derive(tabled::Tabled)]
    pub(super) struct BalanceDiffRepr {
        pub address: String,
        pub before: String,
        pub after: String,
        pub delta: String,
    }

    fn compute_delta(before: &U256, after: &U256) -> String {
        match before.cmp(after) {
            Ordering::Less => format!("+{}", format_gwei(after - before)),
            Ordering::Equal => "0".to_string(),
            Ordering::Greater => format!("-{}", format_gwei(before - after)),
        }
    }

    impl From<&BalanceDiff> for BalanceDiffRepr {
        fn from(val: &BalanceDiff) -> Self {
            let BalanceDiff {
                address,
                balance_before,
                balance_after,
            } = val;
            BalanceDiffRepr {
                address: address.to_string(),
                before: format_eth(*balance_before),
                after: format_eth(*balance_after),
                delta: compute_delta(balance_before, balance_after),
            }
        }
    }
}
