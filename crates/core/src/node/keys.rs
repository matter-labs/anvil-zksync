use zksync_types::{Address, StorageKey};

pub struct StorageKeyLayout {}

impl StorageKeyLayout {
    pub fn get_nonce_key(account: &Address) -> StorageKey {
        #[cfg(not(feature = "zkos"))]
        return zksync_types::get_nonce_key(account);
        #[cfg(feature = "zkos")]
        return crate::node::zkos::zkos_get_nonce_key(account);
    }

    pub fn get_storage_key_for_base_token(address: &Address) -> StorageKey {
        #[cfg(not(feature = "zkos"))]
        return zksync_types::utils::storage_key_for_standard_token_balance(
            zksync_types::AccountTreeId::new(zksync_types::L2_BASE_TOKEN_ADDRESS),
            address,
        );
        #[cfg(feature = "zkos")]
        return crate::node::zkos::zkos_storage_key_for_eth_balance(address);
    }
}
