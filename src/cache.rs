use rustc_hash::FxHashMap;
use serde::Serialize;
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::result::Result;
use std::str::FromStr;
use zksync_basic_types::H256;
use zksync_types::api::{Block, BridgeAddresses, Transaction, TransactionVariant};
use zksync_types::Transaction as RawTransaction;

/// Caches full blocks by their hashes
const CACHE_TYPE_BLOCKS_FULL: &str = "blocks_full";
/// Caches minimal blocks by their hashes
const CACHE_TYPE_BLOCKS_MIN: &str = "blocks_min";
/// Caches raw transactions by their hashes
const CACHE_TYPE_BLOCK_RAW_TRANSACTIONS: &str = "block_raw_transactions";
/// Caches transactions by their hashes
const CACHE_TYPE_TRANSACTIONS: &str = "transactions";
/// Caches arbitrary values by their keys
const CACHE_TYPE_KEY_VALUE: &str = "key_value";

/// Caching key for bridge addresses
const CACHE_KEY_BRIDGE_ADDRESSES: &str = "bridge_addresses";

/// Cache configuration. Can be one of:
///
/// None    : Caching is disabled
/// Memory  : Caching is provided in-memory and not persisted across runs
/// Disk    : Caching is persisted on disk in the provided directory and can be reset
#[derive(Default, Debug, Clone)]
pub enum CacheConfig {
    #[default]
    None,
    Memory,
    Disk {
        dir: String,
        reset: bool,
    },
}

/// A general purpose cache.
#[derive(Default, Debug, Clone)]
pub(crate) struct Cache {
    config: CacheConfig,
    block_hashes: FxHashMap<u64, H256>,
    blocks_full: FxHashMap<H256, Block<TransactionVariant>>,
    blocks_min: FxHashMap<H256, Block<TransactionVariant>>,
    block_raw_transactions: FxHashMap<u64, Vec<RawTransaction>>,
    transactions: FxHashMap<H256, Transaction>,
    bridge_addresses: Option<BridgeAddresses>,
    confirmed_tokens: FxHashMap<(u32, u8), Vec<zksync_web3_decl::types::Token>>,
}

impl Cache {
    /// Creates a new cache with the provided config.
    pub(crate) fn new(config: CacheConfig) -> Self {
        let mut cache = Cache {
            config: config.clone(),
            ..Default::default()
        };

        if let CacheConfig::Disk { dir, reset } = &config {
            if *reset {
                for cache_type in [
                    CACHE_TYPE_BLOCKS_FULL,
                    CACHE_TYPE_BLOCKS_MIN,
                    CACHE_TYPE_BLOCK_RAW_TRANSACTIONS,
                    CACHE_TYPE_TRANSACTIONS,
                    CACHE_TYPE_KEY_VALUE,
                ] {
                    fs::remove_dir_all(Path::new(dir).join(cache_type)).unwrap_or_else(|err| {
                        tracing::warn!(
                            "failed removing directory {:?}: {:?}",
                            Path::new(dir).join(cache_type),
                            err
                        )
                    });
                }

                fs::remove_dir(Path::new(dir)).unwrap_or_else(|err| {
                    tracing::warn!("failed removing cache directory: {:?}", err)
                });
            }

            for cache_type in [
                CACHE_TYPE_BLOCKS_FULL,
                CACHE_TYPE_BLOCKS_MIN,
                CACHE_TYPE_BLOCK_RAW_TRANSACTIONS,
                CACHE_TYPE_TRANSACTIONS,
                CACHE_TYPE_KEY_VALUE,
            ] {
                fs::create_dir_all(Path::new(dir).join(cache_type)).unwrap_or_else(|err| {
                    panic!("failed creating directory {}: {:?}", cache_type, err)
                });
            }
            cache
                .read_all_from_disk(dir)
                .unwrap_or_else(|err| tracing::error!("failed reading cache from disk: {:?}", err));
        }

        cache
    }

    /// Returns the cached full/minimal block for the provided hash.
    pub(crate) fn get_block(
        &self,
        hash: &H256,
        full_transactions: bool,
    ) -> Option<&Block<TransactionVariant>> {
        if matches!(self.config, CacheConfig::None) {
            return None;
        }

        if full_transactions {
            self.blocks_full.get(hash)
        } else {
            self.blocks_min.get(hash)
        }
    }

    /// Cache a full/minimal block for the provided hash.
    pub(crate) fn insert_block(
        &mut self,
        hash: H256,
        full_transactions: bool,
        block: Block<TransactionVariant>,
    ) {
        if matches!(self.config, CacheConfig::None) {
            return;
        }

        self.block_hashes.insert(block.number.as_u64(), block.hash);
        if full_transactions {
            self.write_to_disk(CACHE_TYPE_BLOCKS_FULL, format!("{:#x}", hash), &block);
            self.blocks_full.insert(hash, block);
        } else {
            self.write_to_disk(CACHE_TYPE_BLOCKS_MIN, format!("{:#x}", hash), &block);
            self.blocks_min.insert(hash, block);
        }
    }

    /// Returns the cached full/minimal block for the provided hash.
    pub(crate) fn get_block_hash(&self, number: &u64) -> Option<&H256> {
        if matches!(self.config, CacheConfig::None) {
            return None;
        }

        self.block_hashes.get(number)
    }

    /// Returns the cached raw transactions for the provided block number.
    pub(crate) fn get_block_raw_transactions(&self, number: &u64) -> Option<&Vec<RawTransaction>> {
        if matches!(self.config, CacheConfig::None) {
            return None;
        }

        self.block_raw_transactions.get(number)
    }

    /// Returns the cached confirmed tokens.
    pub(crate) fn get_confirmed_tokens(
        &self,
        from: u32,
        limit: u8,
    ) -> Option<&Vec<zksync_web3_decl::types::Token>> {
        if matches!(self.config, CacheConfig::None) {
            return None;
        }
        self.confirmed_tokens.get(&(from, limit))
    }

    /// Cache confirmed tokens
    pub(crate) fn set_confirmed_tokens(
        &mut self,
        from: u32,
        limit: u8,
        confirmed_tokens: Vec<zksync_web3_decl::types::Token>,
    ) {
        if matches!(self.config, CacheConfig::None) {
            return;
        }
        self.confirmed_tokens
            .insert((from, limit), confirmed_tokens);
    }

    /// Cache the raw transactions for the provided block number.
    pub(crate) fn insert_block_raw_transactions(
        &mut self,
        number: u64,
        transactions: Vec<RawTransaction>,
    ) {
        if matches!(self.config, CacheConfig::None) {
            return;
        }

        self.write_to_disk(
            CACHE_TYPE_BLOCK_RAW_TRANSACTIONS,
            format!("{}", number),
            &transactions,
        );
        self.block_raw_transactions.insert(number, transactions);
    }

    /// Returns the cached transaction for the provided hash.
    pub(crate) fn get_transaction(&self, hash: &H256) -> Option<&Transaction> {
        if matches!(self.config, CacheConfig::None) {
            return None;
        }

        self.transactions.get(hash)
    }

    /// Cache a transaction for the provided hash.
    pub(crate) fn insert_transaction(&mut self, hash: H256, transaction: Transaction) {
        if matches!(self.config, CacheConfig::None) {
            return;
        }

        self.write_to_disk(
            CACHE_TYPE_TRANSACTIONS,
            format!("{:#x}", hash),
            &transaction,
        );
        self.transactions.insert(hash, transaction);
    }

    /// Returns the cached bridge addresses for the provided hash.
    pub(crate) fn get_bridge_addresses(&self) -> Option<&BridgeAddresses> {
        if matches!(self.config, CacheConfig::None) {
            return None;
        }

        self.bridge_addresses.as_ref()
    }

    /// Cache default bridge addresses.
    pub(crate) fn set_bridge_addresses(&mut self, bridge_addresses: BridgeAddresses) {
        if matches!(self.config, CacheConfig::None) {
            return;
        }

        self.write_to_disk(
            CACHE_TYPE_KEY_VALUE,
            String::from(CACHE_KEY_BRIDGE_ADDRESSES),
            &bridge_addresses,
        );
        self.bridge_addresses = Some(bridge_addresses);
    }

    /// Reads the cache contents from the disk, if available.
    fn read_all_from_disk(&mut self, dir: &str) -> Result<(), String> {
        for cache_type in [
            CACHE_TYPE_BLOCKS_FULL,
            CACHE_TYPE_BLOCKS_MIN,
            CACHE_TYPE_BLOCK_RAW_TRANSACTIONS,
            CACHE_TYPE_TRANSACTIONS,
            CACHE_TYPE_KEY_VALUE,
        ] {
            let cache_dir = Path::new(dir).join(cache_type);
            let dir_listing = fs::read_dir(cache_dir.clone())
                .map_err(|err| format!("failed reading dir '{:?}': {:?}", cache_dir, err))?
                .flatten();
            for file in dir_listing {
                let key = file
                    .file_name()
                    .to_str()
                    .ok_or_else(|| String::from("failed converting filename to string"))?
                    .to_string();

                let cache_file = File::open(file.path()).map_err(|err| {
                    format!("failed reading file: '{:?}': {:?}", file.path(), err)
                })?;
                let reader = BufReader::new(cache_file);
                match cache_type {
                    CACHE_TYPE_BLOCKS_FULL => {
                        let key = H256::from_str(&key).map_err(|err| {
                            format!("invalid key for cache file '{:?}': {:?}", key, err)
                        })?;
                        let block: Block<TransactionVariant> = serde_json::from_reader(reader)
                            .map_err(|err| {
                                format!("failed parsing json for cache file '{:?}': {:?}", key, err)
                            })?;
                        self.block_hashes.insert(block.number.as_u64(), block.hash);
                        self.blocks_full.insert(key, block);
                    }
                    CACHE_TYPE_BLOCKS_MIN => {
                        let key = H256::from_str(&key).map_err(|err| {
                            format!("invalid key for cache file '{:?}': {:?}", key, err)
                        })?;
                        let block: Block<TransactionVariant> = serde_json::from_reader(reader)
                            .map_err(|err| {
                                format!("failed parsing json for cache file '{:?}': {:?}", key, err)
                            })?;
                        self.block_hashes.insert(block.number.as_u64(), block.hash);
                        self.blocks_min.insert(key, block);
                    }
                    CACHE_TYPE_BLOCK_RAW_TRANSACTIONS => {
                        let key = key.parse::<u64>().map_err(|err| {
                            format!("invalid key for cache file '{:?}': {:?}", key, err)
                        })?;
                        let transactions: Vec<RawTransaction> = serde_json::from_reader(reader)
                            .map_err(|err| {
                                format!("failed parsing json for cache file '{:?}': {:?}", key, err)
                            })?;
                        self.block_raw_transactions.insert(key, transactions);
                    }
                    CACHE_TYPE_TRANSACTIONS => {
                        let key = H256::from_str(&key).map_err(|err| {
                            format!("invalid key for cache file '{:?}': {:?}", key, err)
                        })?;
                        let transaction: Transaction =
                            serde_json::from_reader(reader).map_err(|err| {
                                format!("failed parsing json for cache file '{:?}': {:?}", key, err)
                            })?;
                        self.transactions.insert(key, transaction);
                    }
                    CACHE_TYPE_KEY_VALUE => match key.as_str() {
                        CACHE_KEY_BRIDGE_ADDRESSES => {
                            self.bridge_addresses =
                                Some(serde_json::from_reader(reader).map_err(|err| {
                                    format!(
                                        "failed parsing json for cache file '{:?}': {:?}",
                                        key, err
                                    )
                                })?);
                        }
                        _ => return Err(format!("invalid cache_type_value key {}", cache_type)),
                    },
                    _ => return Err(format!("invalid cache_type {}", cache_type)),
                }
            }
        }

        Ok(())
    }

    /// Writes the cache contents to disk, if supported.
    fn write_to_disk<T: Serialize>(&self, cache_type: &'static str, key: String, data: &T) {
        if let CacheConfig::Disk { dir, .. } = &self.config {
            let file = Path::new(&dir).join(cache_type).join(key);

            tracing::debug!("writing cache {:?}", file);
            match File::create(file.clone()) {
                Ok(cache_file) => {
                    let writer = BufWriter::new(cache_file);
                    if let Err(err) = serde_json::to_writer(writer, data) {
                        tracing::error!("failed writing to cache '{:?}': {:?}", file, err);
                    }
                }
                Err(err) => tracing::error!("failed creating file: '{:?}': {:?}", file, err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use zksync_basic_types::{H160, U64};
    use zksync_types::{Execute, ExecuteTransactionCommon};

    use crate::testing;

    use super::*;

    #[test]
    fn test_cache_config_none_disables_cache() {
        let mut cache = Cache::new(CacheConfig::None);

        cache.insert_block(H256::zero(), true, Default::default());
        assert_eq!(None, cache.get_block(&H256::zero(), true));
        assert_eq!(None, cache.get_block_hash(&0));

        cache.insert_block(H256::zero(), false, Default::default());
        assert_eq!(None, cache.get_block(&H256::zero(), false));
        assert_eq!(None, cache.get_block_hash(&0));

        cache.insert_block_raw_transactions(0, Default::default());
        assert_eq!(None, cache.get_block_raw_transactions(&0));

        cache.insert_transaction(H256::zero(), Default::default());
        assert_eq!(None, cache.get_transaction(&H256::zero()));
    }

    #[test]
    fn test_cache_config_memory_enables_cache() {
        let block_full = Block::<TransactionVariant> {
            hash: H256::repeat_byte(0x1),
            number: U64::from(1),
            ..Default::default()
        };
        let block_min = Block::<TransactionVariant> {
            hash: H256::repeat_byte(0x2),
            number: U64::from(2),
            ..Default::default()
        };
        let transaction = Transaction::default();
        let raw_transactions = vec![RawTransaction {
            common_data: ExecuteTransactionCommon::L1(Default::default()),
            execute: Execute {
                calldata: Default::default(),
                contract_address: Default::default(),
                factory_deps: None,
                value: Default::default(),
            },
            received_timestamp_ms: 0,
            raw_bytes: None,
        }];
        let bridge_addresses = BridgeAddresses {
            l1_erc20_default_bridge: H160::repeat_byte(0x1),
            l2_erc20_default_bridge: H160::repeat_byte(0x2),
            l1_weth_bridge: Some(H160::repeat_byte(0x3)),
            l2_weth_bridge: Some(H160::repeat_byte(0x4)),
        };

        let mut cache = Cache::new(CacheConfig::Memory);

        cache.insert_block(block_full.hash, true, block_full.clone());
        assert_eq!(
            Some(&block_full),
            cache.get_block(&H256::repeat_byte(0x1), true)
        );
        assert_eq!(Some(&H256::repeat_byte(0x1)), cache.get_block_hash(&1));

        cache.insert_block(block_min.hash, false, block_min.clone());
        assert_eq!(
            Some(&block_min),
            cache.get_block(&H256::repeat_byte(0x2), false)
        );
        assert_eq!(Some(&H256::repeat_byte(0x2)), cache.get_block_hash(&2));

        cache.insert_block_raw_transactions(0, raw_transactions.clone());
        assert_eq!(
            Some(&raw_transactions),
            cache.get_block_raw_transactions(&0)
        );

        cache.insert_transaction(H256::zero(), transaction.clone());
        assert_eq!(Some(&transaction), cache.get_transaction(&H256::zero()));

        cache.set_bridge_addresses(bridge_addresses.clone());
        testing::assert_bridge_addresses_eq(
            &bridge_addresses,
            cache.get_bridge_addresses().expect("expected addresses"),
        );
    }

    #[test]
    fn test_cache_config_disk_enables_cache_and_preserves_it_to_disk() {
        let block_full = Block::<TransactionVariant> {
            hash: H256::repeat_byte(0x1),
            number: U64::from(1),
            ..Default::default()
        };
        let block_min = Block::<TransactionVariant> {
            hash: H256::repeat_byte(0x2),
            number: U64::from(2),
            ..Default::default()
        };
        let transaction = Transaction::default();
        let raw_transactions = vec![RawTransaction {
            common_data: ExecuteTransactionCommon::L1(Default::default()),
            execute: Execute {
                calldata: Default::default(),
                contract_address: Default::default(),
                factory_deps: None,
                value: Default::default(),
            },
            received_timestamp_ms: 0,
            raw_bytes: None,
        }];
        let bridge_addresses = BridgeAddresses {
            l1_erc20_default_bridge: H160::repeat_byte(0x1),
            l2_erc20_default_bridge: H160::repeat_byte(0x2),
            l1_weth_bridge: Some(H160::repeat_byte(0x3)),
            l2_weth_bridge: Some(H160::repeat_byte(0x4)),
        };

        let cache_dir = TempDir::new("cache-test").expect("failed creating temporary dir");
        let cache_dir_path = cache_dir
            .path()
            .to_str()
            .expect("invalid dir name")
            .to_string();
        let mut cache = Cache::new(CacheConfig::Disk {
            dir: cache_dir_path.clone(),
            reset: true,
        });

        cache.insert_block(block_full.hash, true, block_full.clone());
        assert_eq!(
            Some(&block_full),
            cache.get_block(&H256::repeat_byte(0x1), true)
        );
        assert_eq!(Some(&H256::repeat_byte(0x1)), cache.get_block_hash(&1));

        cache.insert_block(block_min.hash, false, block_min.clone());
        assert_eq!(
            Some(&block_min),
            cache.get_block(&H256::repeat_byte(0x2), false)
        );
        assert_eq!(Some(&H256::repeat_byte(0x2)), cache.get_block_hash(&2));

        cache.insert_block_raw_transactions(0, raw_transactions.clone());
        assert_eq!(
            Some(&raw_transactions),
            cache.get_block_raw_transactions(&0)
        );

        cache.insert_transaction(H256::zero(), transaction.clone());
        assert_eq!(Some(&transaction), cache.get_transaction(&H256::zero()));

        cache.set_bridge_addresses(bridge_addresses.clone());
        testing::assert_bridge_addresses_eq(
            &bridge_addresses,
            cache.get_bridge_addresses().expect("expected addresses"),
        );

        let new_cache = Cache::new(CacheConfig::Disk {
            dir: cache_dir_path,
            reset: false,
        });
        assert_eq!(
            Some(&block_full),
            new_cache.get_block(&H256::repeat_byte(0x1), true)
        );
        assert_eq!(Some(&H256::repeat_byte(0x1)), new_cache.get_block_hash(&1));
        assert_eq!(
            Some(&block_min),
            new_cache.get_block(&H256::repeat_byte(0x2), false)
        );
        assert_eq!(Some(&H256::repeat_byte(0x2)), new_cache.get_block_hash(&2));
        assert_eq!(
            Some(&raw_transactions),
            new_cache.get_block_raw_transactions(&0)
        );
        assert_eq!(Some(&transaction), new_cache.get_transaction(&H256::zero()));
        testing::assert_bridge_addresses_eq(
            &bridge_addresses,
            new_cache
                .get_bridge_addresses()
                .expect("expected addresses"),
        );
    }

    #[test]
    fn test_cache_config_disk_enables_cache_and_can_reset_data_on_disk() {
        let block_full = Block::<TransactionVariant> {
            hash: H256::repeat_byte(0x1),
            number: U64::from(1),
            ..Default::default()
        };
        let block_min = Block::<TransactionVariant> {
            hash: H256::repeat_byte(0x2),
            number: U64::from(2),
            ..Default::default()
        };
        let transaction = Transaction::default();
        let raw_transactions = vec![RawTransaction {
            common_data: ExecuteTransactionCommon::L1(Default::default()),
            execute: Execute {
                calldata: Default::default(),
                contract_address: Default::default(),
                factory_deps: None,
                value: Default::default(),
            },
            received_timestamp_ms: 0,
            raw_bytes: None,
        }];
        let bridge_addresses = BridgeAddresses {
            l1_erc20_default_bridge: H160::repeat_byte(0x1),
            l2_erc20_default_bridge: H160::repeat_byte(0x2),
            l1_weth_bridge: Some(H160::repeat_byte(0x3)),
            l2_weth_bridge: Some(H160::repeat_byte(0x4)),
        };

        let cache_dir = TempDir::new("cache-test").expect("failed creating temporary dir");
        let cache_dir_path = cache_dir
            .path()
            .to_str()
            .expect("invalid dir name")
            .to_string();
        let mut cache = Cache::new(CacheConfig::Disk {
            dir: cache_dir_path.clone(),
            reset: true,
        });

        cache.insert_block(block_full.hash, true, block_full.clone());
        assert_eq!(
            Some(&block_full),
            cache.get_block(&H256::repeat_byte(0x1), true)
        );
        assert_eq!(Some(&H256::repeat_byte(0x1)), cache.get_block_hash(&1));

        cache.insert_block(block_min.hash, false, block_min.clone());
        assert_eq!(
            Some(&block_min),
            cache.get_block(&H256::repeat_byte(0x2), false)
        );
        assert_eq!(Some(&H256::repeat_byte(0x2)), cache.get_block_hash(&2));

        cache.insert_block_raw_transactions(0, raw_transactions.clone());
        assert_eq!(
            Some(&raw_transactions),
            cache.get_block_raw_transactions(&0)
        );

        cache.insert_transaction(H256::zero(), transaction.clone());
        assert_eq!(Some(&transaction), cache.get_transaction(&H256::zero()));

        cache.set_bridge_addresses(bridge_addresses.clone());
        testing::assert_bridge_addresses_eq(
            &bridge_addresses,
            cache.get_bridge_addresses().expect("expected addresses"),
        );

        let new_cache = Cache::new(CacheConfig::Disk {
            dir: cache_dir_path,
            reset: true,
        });
        assert_eq!(None, new_cache.get_block(&H256::zero(), true));
        assert_eq!(None, new_cache.get_block_hash(&1));
        assert_eq!(None, new_cache.get_block(&H256::zero(), false));
        assert_eq!(None, new_cache.get_block_hash(&2));
        assert_eq!(None, new_cache.get_block_raw_transactions(&0));
        assert_eq!(None, new_cache.get_transaction(&H256::zero()));
        assert!(new_cache.get_bridge_addresses().is_none());
    }

    #[test]
    fn test_cache_config_disk_only_resets_created_data_on_disk() {
        let cache_dir = TempDir::new("cache-test").expect("failed creating temporary dir");
        let cache_dir_path = cache_dir
            .path()
            .to_str()
            .expect("invalid dir name")
            .to_string();
        let mut cache = Cache::new(CacheConfig::Disk {
            dir: cache_dir_path.clone(),
            reset: true,
        });

        cache.insert_transaction(H256::zero(), Default::default());
        let cached_tx_file = cache_dir
            .path()
            .join(CACHE_TYPE_TRANSACTIONS)
            .join(format!("{:#x}", H256::zero()));
        assert!(
            cached_tx_file.exists(),
            "cached transaction did not exist on disk"
        );

        let random_file_path = cache_dir.path().join("foobar.txt");
        _ = File::create(&random_file_path).expect("failed creating random file");

        Cache::new(CacheConfig::Disk {
            dir: cache_dir_path,
            reset: true,
        });

        assert!(
            !cached_tx_file.exists(),
            "cached transaction was not reset on disk"
        );
        assert!(random_file_path.exists(), "random file was reset from disk");
    }
}
