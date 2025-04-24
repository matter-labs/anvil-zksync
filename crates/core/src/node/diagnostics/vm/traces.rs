use std::collections::HashMap;

use anvil_zksync_types::traces::{CallTraceArena, DecodedValue::Address, LabeledAddress};
use zksync_types::H160;

pub fn extract_addresses(
    arena: &CallTraceArena,
    known_addresses: &mut HashMap<H160, Option<String>>,
) {
    for node in arena.nodes().iter() {
        if let Some(ref call_data) = node.trace.decoded.call_data {
            for arg in &call_data.args {
                if let Address(LabeledAddress { label, address }) = arg {
                    if !known_addresses.contains_key(address) {
                        known_addresses.entry(*address).or_insert(label.clone());
                    }
                }
            }
        }
    }
}
