use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;
use zksync_types::{Address, Execute, H256, ProtocolVersionId};

static BUILTIN_UPGRADE_TXS: Lazy<HashMap<ProtocolVersionId, UpgradeTx>> = Lazy::new(|| {
    HashMap::from_iter([
        (
            ProtocolVersionId::Version26,
            serde_json::from_slice::<UpgradeTx>(include_bytes!(
                "../../../l1-setup/state/v26-l2-upgrade-tx.json"
            ))
            .unwrap(),
        ),
        (
            ProtocolVersionId::Version27,
            serde_json::from_slice::<UpgradeTx>(include_bytes!(
                "../../../l1-setup/state/v27-l2-upgrade-tx.json"
            ))
            .unwrap(),
        ),
        (
            ProtocolVersionId::Version28,
            serde_json::from_slice::<UpgradeTx>(include_bytes!(
                "../../../l1-setup/state/v28-l2-upgrade-tx.json"
            ))
            .unwrap(),
        ),
    ])
});

#[derive(Clone, Deserialize)]
pub struct UpgradeTx {
    pub data: Execute,
    pub hash: H256,
    pub gas_limit: u64,
    pub l1_tx_mint: u64,
    pub l1_block_number: u64,
    pub max_fee_per_gas: u64,
    pub initiator_address: Address,
    pub gas_per_pubdata_limit: u64,
    pub l1_tx_refund_recipient: Address,
}

impl UpgradeTx {
    pub fn builtin(protocol_version: ProtocolVersionId) -> Self {
        BUILTIN_UPGRADE_TXS
            .get(&protocol_version)
            .expect("unsupported protocol version")
            .clone()
    }
}
