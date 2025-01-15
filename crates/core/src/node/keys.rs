use zksync_types::{Address, StorageKey};

pub struct StorageKeyLayout {}

impl StorageKeyLayout {
    pub fn get_nonce_key(is_zkos: bool, account: &Address) -> StorageKey {
        if is_zkos {
            crate::node::zkos::zkos_get_nonce_key(account)
        } else {
            zksync_types::get_nonce_key(account)
        }
    }

    pub fn get_storage_key_for_base_token(is_zkos: bool, address: &Address) -> StorageKey {
        if is_zkos {
            crate::node::zkos::zkos_storage_key_for_eth_balance(address)
        } else {
            zksync_types::utils::storage_key_for_standard_token_balance(
                zksync_types::AccountTreeId::new(zksync_types::L2_BASE_TOKEN_ADDRESS),
                address,
            )
        }
    }
}
