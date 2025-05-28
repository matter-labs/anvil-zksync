use std::cmp::Ordering;

use anvil_zksync_common::utils::cost::{format_eth, format_gwei};
use anvil_zksync_types::traces::LabeledAddress;
use zksync_types::U256;

///
/// Holds a fragment of account state before and after transaction.
///
pub struct BalanceDiff {
    pub address: LabeledAddress,
    pub balance_before: U256,
    pub balance_after: U256,
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
