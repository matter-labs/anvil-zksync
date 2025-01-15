use async_trait::async_trait;
use zksync_multivm::interface::storage::ReadStorage;
use zksync_types::{StorageKey, StorageValue, H256};

#[async_trait]
pub trait ReadStorageDyn: ReadStorage + Send + Sync {
    /// Alternative for [`Clone::clone`] that is object safe.
    fn dyn_cloned(&self) -> Box<dyn ReadStorageDyn>;

    /// Alternative version of [`ReadStorage::read_value`] that is fallible, async and does not
    /// require `&mut self`.
    async fn read_value_alt(&self, key: &StorageKey) -> anyhow::Result<StorageValue>;

    /// Alternative version of [`ReadStorage::load_factory_dep`] that is fallible, async and does not
    /// require `&mut self`.
    async fn load_factory_dep_alt(&self, hash: H256) -> anyhow::Result<Option<Vec<u8>>>;
}

impl Clone for Box<dyn ReadStorageDyn> {
    fn clone(&self) -> Self {
        self.dyn_cloned()
    }
}
