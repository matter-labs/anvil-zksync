use std::collections::HashMap;

use anvil_zksync_common::address_map::KNOWN_ADDRESSES;
use zksync_types::{Address, Transaction};


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
