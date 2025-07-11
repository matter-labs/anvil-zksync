//! This file hold tools used for test-forking other networks.
//!
//! There is ForkStorage (that is a wrapper over InMemoryStorage)
//! And ForkDetails - that parses network address and fork height from arguments.

use crate::deps::InMemoryStorage;
use crate::node::inner::fork::{Fork, ForkSource};
use crate::node::inner::storage::ReadStorageDyn;
use crate::utils;
use anvil_zksync_config::constants::TEST_NODE_NETWORK_ID;
use anvil_zksync_config::types::SystemContractsOptions;
use async_trait::async_trait;
use eyre::eyre;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::iter::FromIterator;
use std::path::Path;
use std::sync::{Arc, RwLock};
use zksync_multivm::interface::storage::ReadStorage;
use zksync_types::bytecode::BytecodeHash;
use zksync_types::web3::Bytes;
use zksync_types::{
    H256, L2ChainId, ProtocolVersionId, SYSTEM_CONTEXT_CHAIN_ID_POSITION, StorageKey, StorageValue,
    get_system_context_key, h256_to_u256,
};

/// In memory storage, that allows 'forking' from other network.
/// If forking is enabled, it reads missing data from remote location.
#[derive(Debug, Clone)]
pub struct ForkStorage {
    pub inner: Arc<RwLock<ForkStorageInner>>,
    pub chain_id: L2ChainId,
}

// TODO: Hide mutable state and mark everything with `pub(super)`
#[derive(Debug)]
pub struct ForkStorageInner {
    // Underlying local storage
    pub raw_storage: InMemoryStorage,
    // Cache of data that was read from remote location.
    pub(super) value_read_cache: HashMap<StorageKey, H256>,
    // Cache of factory deps that were read from remote location.
    pub(super) factory_dep_cache: HashMap<H256, Option<Vec<u8>>>,
    // If set - it hold the necessary information on where to fetch the data.
    // If not set - it will simply read from underlying storage.
    fork: Fork,
}

impl ForkStorage {
    pub(super) fn new(
        fork: Fork,
        system_contracts_options: SystemContractsOptions,
        protocol_version: ProtocolVersionId,
        override_chain_id: Option<u32>,
        system_contracts_path: Option<&Path>,
    ) -> Self {
        let chain_id = if let Some(override_id) = override_chain_id {
            L2ChainId::from(override_id)
        } else {
            fork.details()
                .map(|fd| fd.chain_id)
                .unwrap_or(L2ChainId::from(TEST_NODE_NETWORK_ID))
        };

        ForkStorage {
            inner: Arc::new(RwLock::new(ForkStorageInner {
                raw_storage: InMemoryStorage::with_system_contracts_and_chain_id(
                    chain_id,
                    |b| BytecodeHash::for_bytecode(b).value(),
                    system_contracts_options,
                    protocol_version,
                    system_contracts_path,
                ),
                value_read_cache: Default::default(),
                fork,
                factory_dep_cache: Default::default(),
            })),
            chain_id,
        }
    }

    pub fn read_value_internal(&self, key: &StorageKey) -> eyre::Result<StorageValue> {
        let inner = self.inner.read().unwrap();
        if let Some(local_value) = inner.raw_storage.read_value_opt(key) {
            return Ok(local_value);
        }
        if let Some(cached_value) = inner.value_read_cache.get(key) {
            return Ok(*cached_value);
        }
        let fork = inner.fork.clone();
        drop(inner);
        let address = *key.account().address();
        let idx = h256_to_u256(*key.key());
        let value =
            utils::block_on(async move { fork.get_storage_at_forked(address, idx).await }).unwrap();

        let mut writer = self.inner.write().unwrap();
        writer.value_read_cache.insert(*key, value);
        Ok(value)
    }

    pub fn load_factory_dep_internal(&self, hash: H256) -> eyre::Result<Option<Vec<u8>>> {
        let fork = {
            let mut writer = self.inner.write().unwrap();
            let local_storage = writer.raw_storage.load_factory_dep(hash);
            if local_storage.is_some() {
                return Ok(local_storage);
            }
            if let Some(value) = writer.factory_dep_cache.get(&hash) {
                return Ok(value.clone());
            }
            writer.fork.clone()
        };

        let result = utils::block_on(async move { fork.get_bytecode_by_hash(hash).await }).unwrap();

        let mut writer = self.inner.write().unwrap();
        writer.factory_dep_cache.insert(hash, result.clone());
        Ok(result)
    }

    /// Check if this is the first time when we're ever writing to this key.
    /// This has impact on amount of pubdata that we have to spend for the write.
    pub fn is_write_initial_internal(&self, key: &StorageKey) -> eyre::Result<bool> {
        // Currently we don't have the zks API to return us the information on whether a given
        // key was written to before a given block.
        // This means, we have to depend on the following heuristic: we'll read the value of the slot.
        //  - if value != 0 -> this means that the slot was written to in the past (so we can return intitial_write = false)
        //  - but if the value = 0 - there is a chance, that slot was written to in the past - and later was reset.
        //                            but unfortunately we cannot detect that with the current zks api, so we'll attempt to do it
        //                           only on local storage.
        let value = self.read_value_internal(key)?;
        if value != H256::zero() {
            return Ok(false);
        }

        // If value was 0, there is still a chance, that the slot was written to in the past - and only now set to 0.
        // We unfortunately don't have the API to check it on the fork, but we can at least try to check it on local storage.
        let mut mutator = self
            .inner
            .write()
            .map_err(|err| eyre!("failed acquiring write lock on fork storage: {:?}", err))?;
        Ok(mutator.raw_storage.is_write_initial(key))
    }

    /// Retrieves the enumeration index for a given `key`.
    fn get_enumeration_index_internal(&self, _key: &StorageKey) -> Option<u64> {
        // TODO: Update this file to use proper enumeration index value once it's exposed for forks via API
        Some(0_u64)
    }

    /// Creates a serializable representation of current storage state. It will contain both locally
    /// stored data and cached data read from the fork.
    pub fn dump_state(&self) -> SerializableForkStorage {
        let inner = self.inner.read().unwrap();
        let mut state = BTreeMap::from_iter(inner.value_read_cache.clone());
        state.extend(inner.raw_storage.state.clone());
        let mut factory_deps = BTreeMap::from_iter(
            inner
                .factory_dep_cache
                .iter()
                // Ignore cache misses
                .filter_map(|(k, v)| v.as_ref().map(|v| (k, v)))
                .map(|(k, v)| (*k, Bytes::from(v.clone()))),
        );
        factory_deps.extend(
            inner
                .raw_storage
                .factory_deps
                .iter()
                .map(|(k, v)| (*k, Bytes::from(v.clone()))),
        );

        SerializableForkStorage {
            storage: SerializableStorage(state),
            factory_deps,
        }
    }

    pub fn load_state(&self, state: SerializableForkStorage) {
        tracing::trace!(
            slots = state.storage.0.len(),
            factory_deps = state.factory_deps.len(),
            "loading fork storage from supplied state"
        );
        let mut inner = self.inner.write().unwrap();
        inner.raw_storage.state.extend(state.storage.0);
        inner
            .raw_storage
            .factory_deps
            .extend(state.factory_deps.into_iter().map(|(k, v)| (k, v.0)));
    }
}

impl ReadStorage for ForkStorage {
    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        self.is_write_initial_internal(key).unwrap()
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        self.load_factory_dep_internal(hash).unwrap()
    }

    fn read_value(&mut self, key: &StorageKey) -> zksync_types::StorageValue {
        self.read_value_internal(key).unwrap()
    }

    fn get_enumeration_index(&mut self, key: &StorageKey) -> Option<u64> {
        self.get_enumeration_index_internal(key)
    }
}

impl ReadStorage for &ForkStorage {
    fn read_value(&mut self, key: &StorageKey) -> zksync_types::StorageValue {
        self.read_value_internal(key).unwrap()
    }

    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        self.is_write_initial_internal(key).unwrap()
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        self.load_factory_dep_internal(hash).unwrap()
    }

    fn get_enumeration_index(&mut self, key: &StorageKey) -> Option<u64> {
        self.get_enumeration_index_internal(key)
    }
}

#[async_trait]
impl ReadStorageDyn for ForkStorage {
    fn dyn_cloned(&self) -> Box<dyn ReadStorageDyn> {
        Box::new(self.clone())
    }

    async fn read_value_alt(&self, key: &StorageKey) -> anyhow::Result<StorageValue> {
        // TODO: Get rid of `block_on` inside to propagate asynchronous execution up to this level
        self.read_value_internal(key)
            .map_err(|e| anyhow::anyhow!("failed reading value: {:?}", e))
    }

    async fn load_factory_dep_alt(&self, hash: H256) -> anyhow::Result<Option<Vec<u8>>> {
        // TODO: Get rid of `block_on` inside to propagate asynchronous execution up to this level
        self.load_factory_dep_internal(hash)
            .map_err(|e| anyhow::anyhow!("failed to load factory dep: {:?}", e))
    }
}

impl ForkStorage {
    pub fn set_value(&self, key: StorageKey, value: zksync_types::StorageValue) {
        let mut mutator = self.inner.write().unwrap();
        mutator.raw_storage.set_value(key, value)
    }
    pub fn store_factory_dep(&self, hash: H256, bytecode: Vec<u8>) {
        let mut mutator = self.inner.write().unwrap();
        mutator.raw_storage.store_factory_dep(hash, bytecode)
    }
    pub fn set_chain_id(&mut self, id: L2ChainId) {
        self.chain_id = id;
        let mut mutator = self.inner.write().unwrap();
        mutator.raw_storage.set_value(
            get_system_context_key(SYSTEM_CONTEXT_CHAIN_ID_POSITION),
            H256::from_low_u64_be(id.as_u64()),
        );
    }
}

/// Serializable representation of [`ForkStorage`]'s state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableForkStorage {
    /// Node's current key-value storage state (contains both local and cached fork data if applicable).
    pub storage: SerializableStorage,
    /// Factory dependencies by their hash.
    pub factory_deps: BTreeMap<H256, Bytes>,
}

/// Wrapper for [`BTreeMap<StorageKey, StorageValue>`] to avoid serializing [`StorageKey`] as a struct.
/// JSON does not support non-string keys so we use conversion to [`Bytes`] via [`crate::node::state::SerializableStorageKey`]
/// instead.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    into = "BTreeMap<serde_from::SerializableStorageKey, StorageValue>",
    from = "BTreeMap<serde_from::SerializableStorageKey, StorageValue>"
)]
pub struct SerializableStorage(pub BTreeMap<StorageKey, StorageValue>);

mod serde_from {
    use super::SerializableStorage;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use std::convert::TryFrom;
    use zksync_types::web3::Bytes;
    use zksync_types::{AccountTreeId, Address, H256, StorageKey, StorageValue};

    impl From<BTreeMap<SerializableStorageKey, StorageValue>> for SerializableStorage {
        fn from(value: BTreeMap<SerializableStorageKey, StorageValue>) -> Self {
            SerializableStorage(value.into_iter().map(|(k, v)| (k.into(), v)).collect())
        }
    }

    impl From<SerializableStorage> for BTreeMap<SerializableStorageKey, StorageValue> {
        fn from(value: SerializableStorage) -> Self {
            value.0.into_iter().map(|(k, v)| (k.into(), v)).collect()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(into = "Bytes", try_from = "Bytes")]
    pub struct SerializableStorageKey(StorageKey);

    impl From<StorageKey> for SerializableStorageKey {
        fn from(value: StorageKey) -> Self {
            SerializableStorageKey(value)
        }
    }

    impl From<SerializableStorageKey> for StorageKey {
        fn from(value: SerializableStorageKey) -> Self {
            value.0
        }
    }

    impl TryFrom<Bytes> for SerializableStorageKey {
        type Error = anyhow::Error;

        fn try_from(bytes: Bytes) -> anyhow::Result<Self> {
            if bytes.0.len() != 52 {
                anyhow::bail!("invalid bytes length (expected 52, got {})", bytes.0.len())
            }
            let address = Address::from_slice(&bytes.0[0..20]);
            let key = H256::from_slice(&bytes.0[20..52]);
            Ok(SerializableStorageKey(StorageKey::new(
                AccountTreeId::new(address),
                key,
            )))
        }
    }

    impl From<SerializableStorageKey> for Bytes {
        fn from(value: SerializableStorageKey) -> Self {
            let bytes = [value.0.address().as_bytes(), value.0.key().as_bytes()].concat();
            bytes.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ForkStorage;
    use crate::deps::InMemoryStorage;
    use crate::node::fork::{Fork, ForkClient, ForkDetails};
    use anvil_zksync_common::cache::CacheConfig;
    use anvil_zksync_config::constants::{
        DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
        DEFAULT_FAIR_PUBDATA_PRICE, DEFAULT_L2_GAS_PRICE, TEST_NODE_NETWORK_ID,
    };
    use anvil_zksync_config::types::SystemContractsOptions;
    use zksync_multivm::interface::storage::ReadStorage;
    use zksync_types::{
        AccountTreeId, H256, L1BatchNumber, L2ChainId, SYSTEM_CONTEXT_CHAIN_ID_POSITION,
        get_system_context_key,
    };
    use zksync_types::{L2BlockNumber, ProtocolVersionId, StorageKey, api::TransactionVariant};

    #[test]
    fn test_initial_writes() {
        let account = AccountTreeId::default();
        let never_written_key = StorageKey::new(account, H256::from_low_u64_be(1));
        let key_with_some_value = StorageKey::new(account, H256::from_low_u64_be(2));
        let key_with_value_0 = StorageKey::new(account, H256::from_low_u64_be(3));
        let mut in_memory_storage = InMemoryStorage::default();
        in_memory_storage.set_value(key_with_some_value, H256::from_low_u64_be(13));
        in_memory_storage.set_value(key_with_value_0, H256::from_low_u64_be(0));

        let fork_details = ForkDetails {
            chain_id: TEST_NODE_NETWORK_ID.into(),
            batch_number: L1BatchNumber(1),
            block_number: L2BlockNumber(1),
            block_hash: H256::zero(),
            block_timestamp: 0,
            api_block: zksync_types::api::Block::<TransactionVariant>::default(),
            l1_gas_price: 100,
            l2_fair_gas_price: DEFAULT_L2_GAS_PRICE,
            fair_pubdata_price: DEFAULT_FAIR_PUBDATA_PRICE,
            estimate_gas_price_scale_factor: DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
            estimate_gas_scale_factor: DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
            ..Default::default()
        };
        let client = ForkClient::mock(fork_details, in_memory_storage);
        let fork = Fork::new(Some(client), CacheConfig::None);

        let options = SystemContractsOptions::default();
        let mut fork_storage: ForkStorage =
            ForkStorage::new(fork, options, ProtocolVersionId::latest(), None, None);

        assert!(fork_storage.is_write_initial(&never_written_key));
        assert!(!fork_storage.is_write_initial(&key_with_some_value));
        // This is the current limitation of the system. In theory, this should return false - as the value was written, but we don't have the API to the
        // backend to get this information.
        assert!(fork_storage.is_write_initial(&key_with_value_0));

        // But writing any value there in the local storage (even 0) - should make it non-initial write immediately.
        fork_storage.set_value(key_with_value_0, H256::zero());
        assert!(!fork_storage.is_write_initial(&key_with_value_0));
    }

    #[test]
    fn test_fork_storage_set_chain_id() {
        let fork_details = ForkDetails {
            chain_id: TEST_NODE_NETWORK_ID.into(),
            batch_number: L1BatchNumber(0),
            block_number: L2BlockNumber(0),
            block_hash: H256::zero(),
            block_timestamp: 0,
            api_block: zksync_types::api::Block::<TransactionVariant>::default(),
            l1_gas_price: 0,
            l2_fair_gas_price: 0,
            fair_pubdata_price: 0,
            estimate_gas_price_scale_factor: 0.0,
            estimate_gas_scale_factor: 0.0,
            ..Default::default()
        };
        let client = ForkClient::mock(fork_details, InMemoryStorage::default());
        let fork = Fork::new(Some(client), CacheConfig::None);
        let mut fork_storage: ForkStorage = ForkStorage::new(
            fork,
            SystemContractsOptions::default(),
            ProtocolVersionId::latest(),
            None,
            None,
        );
        let new_chain_id = L2ChainId::from(261);
        fork_storage.set_chain_id(new_chain_id);

        let inner = fork_storage.inner.read().unwrap();

        assert_eq!(new_chain_id, fork_storage.chain_id);
        assert_eq!(
            H256::from_low_u64_be(new_chain_id.as_u64()),
            *inner
                .raw_storage
                .state
                .get(&get_system_context_key(SYSTEM_CONTEXT_CHAIN_ID_POSITION))
                .unwrap()
        );
    }
}
