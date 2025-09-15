use zksync_types::{Address, StorageKey};

#[derive(Copy, Clone)]
pub enum StorageKeyLayout {
    Era,
    ZKsyncOs,
}

impl StorageKeyLayout {
    pub fn get_nonce_key(&self, account: &Address) -> StorageKey {
        match self {
            StorageKeyLayout::Era => zksync_types::get_nonce_key(account),
            StorageKeyLayout::ZKsyncOs => {
                crate::node::zksync_os::ZkSyncOSHelpers::get_nonce_key(account)
            }
        }
    }

    pub fn get_storage_key_for_base_token(&self, address: &Address) -> StorageKey {
        match self {
            StorageKeyLayout::Era => zksync_types::utils::storage_key_for_eth_balance(address),
            StorageKeyLayout::ZKsyncOs => {
                crate::node::zksync_os::ZkSyncOSHelpers::storage_key_for_eth_balance(address)
            }
        }
    }
}
