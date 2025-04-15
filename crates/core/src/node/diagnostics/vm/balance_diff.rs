use std::collections::BTreeMap;
use std::collections::HashMap;

use anvil_zksync_common::address_map;
use anvil_zksync_common::address_map::KNOWN_ADDRESSES;
use anvil_zksync_types::traces::LabeledAddress;
use zksync_types::utils::storage_key_for_eth_balance;
use zksync_types::Address;
use zksync_types::StorageKey;
use zksync_types::Transaction;
use zksync_types::U256;

///
/// Holds a part of account state before and after transaction.
///
pub struct BalanceDiff {
    pub address: LabeledAddress,
    pub balance_before: U256,
    pub balance_after: U256,
}

///
/// Extract information about balances of accounts before and after given
/// storage logs.
///
pub fn extract_balance_diffs(
    addresses: &HashMap<Address, Option<String>>,
    log: &Vec<zksync_types::StorageLogWithPreviousValue>,
) -> Vec<BalanceDiff> {
    let mut diffs: BTreeMap<StorageKey, internal::BalanceDiffStaging> = addresses
        .iter()
        .map(|(a, label)| {
            (
                storage_key_for_eth_balance(a),
                internal::BalanceDiffStaging::new(a, label.clone()),
            )
        })
        .collect();

    for entry in log {
        if entry.log.is_write() {
            if let Some(d) = diffs.get_mut(&entry.log.key) {
                d.balance_before = d.balance_before.or(Some(entry.previous_value));
                d.balance_after = Some(entry.log.value);
            }
        }
    }

    diffs
        .into_values()
        .filter(|d| d.balance_after != d.balance_before)
        .filter(|d| !address_map::is_system(&d.address.address))
        .map(Into::into)
        .collect()
}

mod internal {
    use anvil_zksync_types::traces::LabeledAddress;
    use zksync_types::{h256_to_u256, Address, H256};

    use super::BalanceDiff;

    ///
    /// A semi-uninitialized struct to hold a part of account state before and after transaction.
    ///
    #[derive(Default)]
    pub(super) struct BalanceDiffStaging {
        pub address: LabeledAddress,
        pub balance_before: Option<H256>,
        pub balance_after: Option<H256>,
    }

    impl BalanceDiffStaging {
        pub fn new(address: &Address, label: Option<String>) -> Self {
            Self {
                address: LabeledAddress {
                    label,
                    address: *address,
                },
                ..Self::default()
            }
        }
    }

    impl From<BalanceDiffStaging> for BalanceDiff {
        fn from(val: BalanceDiffStaging) -> Self {
            let BalanceDiffStaging {
                address,
                balance_before,
                balance_after,
            } = val;
            BalanceDiff {
                address,
                balance_before: h256_to_u256(balance_before.unwrap_or_default()),
                balance_after: h256_to_u256(balance_after.unwrap_or_default()),
            }
        }
    }
}

pub fn known_addresses_after_transaction(tx: &Transaction) -> HashMap<Address, Option<String>> {
    let mut known_addresses: HashMap<Address, Option<String>> = KNOWN_ADDRESSES
        .iter()
        .map(|(address, known_address)| (*address, Some(known_address.name.clone())))
        .collect();
    for address in [
        Some(tx.payer()),
        Some(tx.initiator_account()),
        tx.recipient_account(),
    ]
    .into_iter()
    .flatten()
    {
        known_addresses.entry(address).or_insert(None);
    }
    known_addresses
}
