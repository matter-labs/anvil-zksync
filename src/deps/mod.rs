use std::collections::HashMap;
pub mod system_contracts;
use zksync_types::{
    get_code_key, get_known_code_key, get_system_context_init_logs,
    system_contracts::get_system_smart_contracts, L2ChainId, StorageKey, StorageLog,
    StorageLogKind, StorageValue, H256,
};

use std::fmt;

use self::system_contracts::COMPILED_IN_SYSTEM_CONTRACTS;

/// In-memory storage.
#[derive(Debug, Default, Clone)]
pub struct InMemoryStorage {
    pub(crate) state: HashMap<StorageKey, StorageValue>,
    pub(crate) factory_deps: HashMap<H256, Vec<u8>>,
}

impl InMemoryStorage {
    /// Constructs a storage that contains system smart contracts (with a given chain id).
    pub fn with_system_contracts_and_chain_id(
        chain_id: L2ChainId,
        bytecode_hasher: impl Fn(&[u8]) -> H256,
        load_contracts_from_files: bool,
    ) -> Self {
        let contracts = if load_contracts_from_files {
            get_system_smart_contracts()
        } else {
            COMPILED_IN_SYSTEM_CONTRACTS.clone()
        };
        let system_context_init_log = get_system_context_init_logs(chain_id);

        let state = contracts
            .iter()
            .map(|contract| {
                let deployer_code_key = get_code_key(contract.account_id.address());
                StorageLog::new_write_log(deployer_code_key, bytecode_hasher(&contract.bytecode))
            })
            .chain(system_context_init_log)
            .filter_map(|log| (log.kind == StorageLogKind::Write).then_some((log.key, log.value)))
            .collect();

        let factory_deps = contracts
            .into_iter()
            .map(|contract| (bytecode_hasher(&contract.bytecode), contract.bytecode))
            .collect();
        Self {
            state,
            factory_deps,
        }
    }

    /// Sets the storage `value` at the specified `key`.
    pub fn set_value(&mut self, key: StorageKey, value: StorageValue) {
        self.state.insert(key, value);
    }

    /// Stores a factory dependency with the specified `hash` and `bytecode`.
    pub fn store_factory_dep(&mut self, hash: H256, bytecode: Vec<u8>) {
        self.factory_deps.insert(hash, bytecode);
    }
}

impl ReadStorage for &InMemoryStorage {
    fn read_value(&mut self, key: &StorageKey) -> StorageValue {
        self.state.get(key).copied().unwrap_or_default()
    }

    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        !self.state.contains_key(key)
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        self.factory_deps.get(&hash).cloned()
    }
}

impl ReadStorage for InMemoryStorage {
    fn read_value(&mut self, key: &StorageKey) -> StorageValue {
        (&*self).read_value(key)
    }

    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        (&*self).is_write_initial(key)
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        (&*self).load_factory_dep(hash)
    }
}

/// Functionality to read from the VM storage.
pub trait ReadStorage: fmt::Debug {
    /// Read value of the key.
    fn read_value(&mut self, key: &StorageKey) -> StorageValue;

    /// Checks whether a write to this storage at the specified `key` would be an initial write.
    /// Roughly speaking, this is the case when the storage doesn't contain `key`, although
    /// in case of mutable storages, the caveats apply (a write to a key that is present
    /// in the storage but was not committed is still an initial write).
    fn is_write_initial(&mut self, key: &StorageKey) -> bool;

    /// Load the factory dependency code by its hash.
    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>>;

    /// Returns whether a bytecode hash is "known" to the system.
    fn is_bytecode_known(&mut self, bytecode_hash: &H256) -> bool {
        let code_key = get_known_code_key(bytecode_hash);
        self.read_value(&code_key) != H256::zero()
    }
}

/// Functionality to write to the VM storage in a batch.
///
/// So far, this trait is implemented only for [`StorageView`].
pub trait WriteStorage: ReadStorage {
    /// Sets the new value under a given key and returns the previous value.
    fn set_value(&mut self, key: StorageKey, value: StorageValue) -> StorageValue;

    /// Returns a map with the key–value pairs updated by this batch.
    fn modified_storage_keys(&self) -> &HashMap<StorageKey, StorageValue>;

    /// Returns the number of read / write ops for which the value was read from the underlying
    /// storage.
    fn missed_storage_invocations(&self) -> usize;
}
