use zksync_types::{Address, StorageKey};

#[derive(Copy, Clone)]
pub enum StorageKeyLayout {
    ZkEra,
    ZKsyncOS,
}

impl StorageKeyLayout {
    pub fn get_nonce_key(&self, account: &Address) -> StorageKey {
        match self {
            StorageKeyLayout::ZkEra => zksync_types::get_nonce_key(account),
            StorageKeyLayout::ZKsyncOS => crate::node::zksync_os::zksync_os_get_nonce_key(account),
        }
    }

    pub fn get_storage_key_for_base_token(&self, address: &Address) -> StorageKey {
        match self {
            StorageKeyLayout::ZkEra => zksync_types::utils::storage_key_for_eth_balance(address),
            StorageKeyLayout::ZKsyncOS => {
                crate::node::zksync_os::zksync_os_storage_key_for_eth_balance(address)
            }
        }
    }
}
