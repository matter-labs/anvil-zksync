#![cfg_attr(not(feature = "zksync-os"), allow(unused_variables))]

#[cfg(not(feature = "zksync-os"))]
mod mock;

// NEVER expose anything from this module directly.
// Use `ZkSyncOsHelpers` instead and make sure to mimic functionality for the mock impl.
// ZKsync OS requires nightly, but foundry-zksync uses anvil-zksync as a dependency and is built with stable.
// Exposing ZKsync OS without a feature flag will cause build errors in foundry-zksync.
#[cfg(feature = "zksync-os")]
mod real;

use zksync_types::{Address, StorageKey};

#[cfg(not(feature = "zksync-os"))]
pub use self::mock::MockZKsyncOsVM as ZKsyncOsVM;

#[cfg(feature = "zksync-os")]
pub use self::real::ZKsyncOsVM;

pub struct ZkSyncOSHelpers;

impl ZkSyncOSHelpers {
    pub const fn call_gas_limit() -> u64 {
        #[cfg(feature = "zksync-os")]
        {
            self::real::ZKSYNC_OS_CALL_GAS_LIMIT
        }

        #[cfg(not(feature = "zksync-os"))]
        {
            panic!("Cannot use ZkSyncOSHelpers without 'zksync-os' feature enabled");
        }
    }

    pub fn get_nonce_key(account: &Address) -> StorageKey {
        #[cfg(feature = "zksync-os")]
        {
            self::real::zksync_os_get_nonce_key(account)
        }
        #[cfg(not(feature = "zksync-os"))]
        {
            panic!("Cannot use ZkSyncOSHelpers without 'zksync-os' feature enabled");
        }
    }

    pub fn get_batch_witness(key: &u32) -> Option<Vec<u8>> {
        #[cfg(feature = "zksync-os")]
        {
            self::real::zksync_os_get_batch_witness(key)
        }
        #[cfg(not(feature = "zksync-os"))]
        {
            panic!("Cannot use ZkSyncOSHelpers without 'zksync-os' feature enabled");
        }
    }

    pub fn storage_key_for_eth_balance(address: &Address) -> StorageKey {
        #[cfg(feature = "zksync-os")]
        {
            self::real::zksync_os_storage_key_for_eth_balance(address)
        }
        #[cfg(not(feature = "zksync-os"))]
        {
            panic!("Cannot use ZkSyncOSHelpers without 'zksync-os' feature enabled");
        }
    }
}
