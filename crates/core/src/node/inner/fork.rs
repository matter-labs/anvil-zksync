//! This file hold tools used for test-forking other networks.
//!
//! There is ForkStorage (that is a wrapper over InMemoryStorage)
//! And ForkDetails - that parses network address and fork height from arguments.

use crate::utils::block_on;
use crate::{deps::InMemoryStorage, http_fork_source::HttpForkSource};
use anvil_zksync_config::constants::{
    DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    DEFAULT_FAIR_PUBDATA_PRICE, TEST_NODE_NETWORK_ID,
};
use anvil_zksync_config::types::{CacheConfig, SystemContractsOptions};
use eyre::eyre;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::iter::FromIterator;
use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    fmt,
    str::FromStr,
    sync::{Arc, RwLock},
};
use zksync_multivm::interface::storage::ReadStorage;
use zksync_types::api::BlockId;
use zksync_types::web3::Bytes;
use zksync_types::{
    api,
    api::{
        Block, BlockDetails, BlockIdVariant, BlockNumber, BridgeAddresses, Transaction,
        TransactionDetails, TransactionVariant,
    },
    fee_model::FeeParams,
    get_system_context_key,
    l2::L2Tx,
    url::SensitiveUrl,
    ProtocolVersionId, StorageKey, SYSTEM_CONTEXT_CHAIN_ID_POSITION,
};
use zksync_types::{bytecode::BytecodeHash, h256_to_u256};
use zksync_types::{
    Address, L1BatchNumber, L2BlockNumber, L2ChainId, StorageValue, H256, U256, U64,
};
use zksync_web3_decl::{
    client::{Client, L2},
    namespaces::ZksNamespaceClient,
};
use zksync_web3_decl::{namespaces::EthNamespaceClient, types::Index};

/// The possible networks to fork from.
#[derive(Debug, Clone)]
pub enum ForkNetwork {
    Mainnet,
    SepoliaTestnet,
    GoerliTestnet,
    Other(String),
}

impl ForkNetwork {
    /// Return the URL for the underlying fork source.
    pub fn to_url(&self) -> &str {
        match self {
            ForkNetwork::Mainnet => "https://mainnet.era.zksync.io:443",
            ForkNetwork::SepoliaTestnet => "https://sepolia.era.zksync.dev:443",
            ForkNetwork::GoerliTestnet => "https://testnet.era.zksync.dev:443",
            ForkNetwork::Other(url) => url,
        }
    }
    // TODO: This needs to be dynamic based on the network.
    /// Returns the local gas scale factors currently in use by the upstream network.
    pub fn local_gas_scale_factors(&self) -> (f64, f32) {
        match self {
            ForkNetwork::Mainnet => (1.5, 1.4),
            ForkNetwork::SepoliaTestnet => (2.0, 1.3),
            ForkNetwork::GoerliTestnet => (1.2, 1.2),
            ForkNetwork::Other(_) => (
                DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
                DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
            ),
        }
    }
}

/// In memory storage, that allows 'forking' from other network.
/// If forking is enabled, it reads missing data from remote location.
/// S - is a struct that is used for source of the fork.
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
    pub fork: Option<Box<ForkDetails>>,
}

impl ForkStorage {
    pub(super) fn new(
        fork: Option<ForkDetails>,
        system_contracts_options: &SystemContractsOptions,
        use_evm_emulator: bool,
        override_chain_id: Option<u32>,
    ) -> Self {
        let chain_id = if let Some(override_id) = override_chain_id {
            L2ChainId::from(override_id)
        } else {
            fork.as_ref()
                .and_then(|d| d.overwrite_chain_id)
                .unwrap_or(L2ChainId::from(TEST_NODE_NETWORK_ID))
        };

        ForkStorage {
            inner: Arc::new(RwLock::new(ForkStorageInner {
                raw_storage: InMemoryStorage::with_system_contracts_and_chain_id(
                    chain_id,
                    |b| BytecodeHash::for_bytecode(b).value(),
                    system_contracts_options,
                    use_evm_emulator,
                ),
                value_read_cache: Default::default(),
                fork: fork.map(Box::new),
                factory_dep_cache: Default::default(),
            })),
            chain_id,
        }
    }

    pub fn get_cache_config(&self) -> Result<CacheConfig, String> {
        let reader = self
            .inner
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let cache_config = if let Some(ref fork_details) = reader.fork {
            fork_details.cache_config.clone()
        } else {
            CacheConfig::default()
        };
        Ok(cache_config)
    }

    pub fn get_fork_url(&self) -> Result<String, String> {
        let reader = self
            .inner
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        if let Some(ref fork_details) = reader.fork {
            fork_details
                .fork_source
                .get_fork_url()
                .map_err(|e| e.to_string())
        } else {
            Err("not forked".to_string())
        }
    }

    pub fn read_value_internal(
        &self,
        key: &StorageKey,
    ) -> eyre::Result<zksync_types::StorageValue> {
        let mut mutator = self.inner.write().unwrap();
        let local_storage = mutator.raw_storage.read_value(key);

        if let Some(fork) = &mutator.fork {
            if !H256::is_zero(&local_storage) {
                return Ok(local_storage);
            }

            if let Some(value) = mutator.value_read_cache.get(key) {
                return Ok(*value);
            }
            let l2_miniblock = fork.l2_miniblock;
            let key_ = *key;

            let result = fork.fork_source.get_storage_at(
                *key_.account().address(),
                h256_to_u256(*key_.key()),
                Some(BlockIdVariant::BlockNumber(BlockNumber::Number(U64::from(
                    l2_miniblock,
                )))),
            )?;

            mutator.value_read_cache.insert(*key, result);
            Ok(result)
        } else {
            Ok(local_storage)
        }
    }

    pub fn load_factory_dep_internal(&self, hash: H256) -> eyre::Result<Option<Vec<u8>>> {
        let mut mutator = self.inner.write().unwrap();
        let local_storage = mutator.raw_storage.load_factory_dep(hash);
        if let Some(fork) = &mutator.fork {
            if local_storage.is_some() {
                return Ok(local_storage);
            }
            if let Some(value) = mutator.factory_dep_cache.get(&hash) {
                return Ok(value.clone());
            }

            let result = fork.fork_source.get_bytecode_by_hash(hash)?;
            mutator.factory_dep_cache.insert(hash, result.clone());
            Ok(result)
        } else {
            Ok(local_storage)
        }
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
        if let Some(fork) = &mut mutator.fork {
            fork.set_chain_id(id)
        }
        mutator.raw_storage.set_value(
            get_system_context_key(SYSTEM_CONTEXT_CHAIN_ID_POSITION),
            H256::from_low_u64_be(id.as_u64()),
        );
    }
}

/// Trait that provides necessary data when
/// forking a remote chain.
/// The method signatures are similar to methods from ETHNamespace and ZKNamespace.
pub trait ForkSource {
    /// Returns the forked URL.
    fn get_fork_url(&self) -> eyre::Result<String>;

    /// Returns the Storage value at a given index for given address.
    fn get_storage_at(
        &self,
        address: Address,
        idx: U256,
        block: Option<BlockIdVariant>,
    ) -> eyre::Result<H256>;

    /// Returns the bytecode stored under this hash (if available).
    fn get_bytecode_by_hash(&self, hash: H256) -> eyre::Result<Option<Vec<u8>>>;

    /// Returns the transaction for a given hash.
    fn get_transaction_by_hash(&self, hash: H256) -> eyre::Result<Option<Transaction>>;

    /// Returns the transaction details for a given hash.
    fn get_transaction_details(&self, hash: H256) -> eyre::Result<Option<TransactionDetails>>;

    /// Gets all transactions that belong to a given miniblock.
    fn get_raw_block_transactions(
        &self,
        block_number: L2BlockNumber,
    ) -> eyre::Result<Vec<zksync_types::Transaction>>;

    /// Returns the block for a given hash.
    fn get_block_by_hash(
        &self,
        hash: H256,
        full_transactions: bool,
    ) -> eyre::Result<Option<Block<TransactionVariant>>>;

    /// Returns the block for a given number.
    fn get_block_by_number(
        &self,
        block_number: zksync_types::api::BlockNumber,
        full_transactions: bool,
    ) -> eyre::Result<Option<Block<TransactionVariant>>>;

    fn get_block_by_id(
        &self,
        block_id: zksync_types::api::BlockId,
        full_transactions: bool,
    ) -> eyre::Result<Option<Block<TransactionVariant>>> {
        match block_id {
            BlockId::Hash(hash) => self.get_block_by_hash(hash, full_transactions),
            BlockId::Number(number) => self.get_block_by_number(number, full_transactions),
        }
    }

    /// Returns the block details for a given miniblock number.
    fn get_block_details(&self, miniblock: L2BlockNumber) -> eyre::Result<Option<BlockDetails>>;

    /// Returns fee parameters for the give source.
    fn get_fee_params(&self) -> eyre::Result<FeeParams>;

    /// Returns the  transaction count for a given block hash.
    fn get_block_transaction_count_by_hash(&self, block_hash: H256) -> eyre::Result<Option<U256>>;

    /// Returns the transaction count for a given block number.
    fn get_block_transaction_count_by_number(
        &self,
        block_number: zksync_types::api::BlockNumber,
    ) -> eyre::Result<Option<U256>>;

    fn get_block_transaction_count_by_id(
        &self,
        block_id: api::BlockId,
    ) -> eyre::Result<Option<U256>> {
        match block_id {
            BlockId::Hash(hash) => self.get_block_transaction_count_by_hash(hash),
            BlockId::Number(number) => self.get_block_transaction_count_by_number(number),
        }
    }

    /// Returns information about a transaction by block hash and transaction index position.
    fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> eyre::Result<Option<Transaction>>;

    /// Returns information about a transaction by block number and transaction index position.
    fn get_transaction_by_block_number_and_index(
        &self,
        block_number: BlockNumber,
        index: Index,
    ) -> eyre::Result<Option<Transaction>>;

    fn get_transaction_by_block_id_and_index(
        &self,
        block_id: api::BlockId,
        index: Index,
    ) -> eyre::Result<Option<Transaction>> {
        match block_id {
            BlockId::Hash(hash) => self.get_transaction_by_block_hash_and_index(hash, index),
            BlockId::Number(number) => {
                self.get_transaction_by_block_number_and_index(number, index)
            }
        }
    }

    /// Returns addresses of the default bridge contracts.
    fn get_bridge_contracts(&self) -> eyre::Result<BridgeAddresses>;

    /// Returns confirmed tokens
    fn get_confirmed_tokens(
        &self,
        from: u32,
        limit: u8,
    ) -> eyre::Result<Vec<zksync_web3_decl::types::Token>>;
}

/// Holds the information about the original chain.
pub struct ForkDetails {
    // Source of the fork data (for example HttpForkSource)
    pub fork_source: Box<dyn ForkSource + Send + Sync>,
    // Chain ID of fork
    pub chain_id: L2ChainId,
    // Block number at which we forked (the next block to create is l1_block + 1)
    pub l1_block: L1BatchNumber,
    // The actual L2 block
    pub l2_block: zksync_types::api::Block<zksync_types::api::TransactionVariant>,
    pub l2_miniblock: u64,
    pub l2_miniblock_hash: H256,
    pub block_timestamp: u64,
    pub overwrite_chain_id: Option<L2ChainId>,
    pub l1_gas_price: u64,
    pub l2_fair_gas_price: u64,
    // Cost of publishing one byte.
    pub fair_pubdata_price: u64,
    /// L1 Gas Price Scale Factor for gas estimation.
    pub estimate_gas_price_scale_factor: f64,
    /// The factor by which to scale the gasLimit.
    pub estimate_gas_scale_factor: f32,
    pub fee_params: Option<FeeParams>,
    pub cache_config: CacheConfig,
}

const SUPPORTED_VERSIONS: &[ProtocolVersionId] = &[
    ProtocolVersionId::Version9,
    ProtocolVersionId::Version10,
    ProtocolVersionId::Version11,
    ProtocolVersionId::Version12,
    ProtocolVersionId::Version13,
    ProtocolVersionId::Version14,
    ProtocolVersionId::Version15,
    ProtocolVersionId::Version16,
    ProtocolVersionId::Version17,
    ProtocolVersionId::Version18,
    ProtocolVersionId::Version19,
    ProtocolVersionId::Version20,
    ProtocolVersionId::Version21,
    ProtocolVersionId::Version22,
    ProtocolVersionId::Version23,
    ProtocolVersionId::Version24,
    ProtocolVersionId::Version25,
];

pub fn supported_protocol_versions(version: ProtocolVersionId) -> bool {
    SUPPORTED_VERSIONS.contains(&version)
}

pub fn supported_versions_to_string() -> String {
    let versions: Vec<String> = SUPPORTED_VERSIONS
        .iter()
        .map(|v| format!("{:?}", v))
        .collect();
    versions.join(", ")
}

impl fmt::Debug for ForkDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForkDetails")
            .field("chain_id", &self.chain_id)
            .field("l1_block", &self.l1_block)
            .field("l2_block", &self.l2_block)
            .field("l2_miniblock", &self.l2_miniblock)
            .field("l2_miniblock_hash", &self.l2_miniblock_hash)
            .field("block_timestamp", &self.block_timestamp)
            .field("overwrite_chain_id", &self.overwrite_chain_id)
            .field("l1_gas_price", &self.l1_gas_price)
            .field("l2_fair_gas_price", &self.l2_fair_gas_price)
            .finish()
    }
}

impl ForkDetails {
    pub async fn from_network_and_miniblock_and_chain(
        network: ForkNetwork,
        client: Client<L2>,
        miniblock: u64,
        chain_id: Option<L2ChainId>,
        cache_config: &CacheConfig,
    ) -> eyre::Result<Self> {
        let url = network.to_url();
        let opt_block_details = client
            .get_block_details(L2BlockNumber(miniblock as u32))
            .await
            .map_err(|error| eyre!(error))?;
        let block_details = opt_block_details
            .ok_or_else(|| eyre!("Could not find block {:?} in {:?}", miniblock, url))?;
        let root_hash = block_details
            .base
            .root_hash
            .ok_or_else(|| eyre!("fork block #{} missing root hash", miniblock))?;
        let opt_block = client
            .get_block_by_hash(root_hash, true)
            .await
            .map_err(|error| eyre!(error))?;
        let mut block = opt_block.ok_or_else(|| {
            eyre!(
                "Could not find block #{:?} ({:#x}) in {:?}",
                miniblock,
                root_hash,
                url
            )
        })?;
        let l1_batch_number = block_details.l1_batch_number;
        block.l1_batch_number = Some(l1_batch_number.0.into());

        if !block_details
            .protocol_version
            .is_some_and(supported_protocol_versions)
        {
            return Err(eyre!("This block is using the unsupported protocol version: {:?}. This binary supports versions {}.",
                             block_details.protocol_version,
                             supported_versions_to_string()));
        }

        let (estimate_gas_price_scale_factor, estimate_gas_scale_factor) =
            network.local_gas_scale_factors();
        let fee_params = match client.get_fee_params().await {
            Ok(fp) => Some(fp),
            Err(error) => {
                tracing::warn!("Cannot get fee params: {:?}", error);
                None
            }
        };

        Ok(ForkDetails {
            fork_source: Box::new(HttpForkSource::new(url.to_owned(), cache_config.clone())),
            chain_id: chain_id.unwrap_or_else(|| L2ChainId::from(TEST_NODE_NETWORK_ID)),
            l1_block: l1_batch_number,
            l2_block: block,
            block_timestamp: block_details.base.timestamp,
            l2_miniblock: miniblock,
            l2_miniblock_hash: root_hash,
            overwrite_chain_id: chain_id,
            l1_gas_price: block_details.base.l1_gas_price,
            l2_fair_gas_price: block_details.base.l2_fair_gas_price,
            fair_pubdata_price: block_details
                .base
                .fair_pubdata_price
                .unwrap_or(DEFAULT_FAIR_PUBDATA_PRICE),
            estimate_gas_price_scale_factor,
            estimate_gas_scale_factor,
            fee_params,
            cache_config: cache_config.clone(), // TODO: This is a temporary solution, we should avoid cloning the cache config here. We should look to refactor how cache is being configured / used as it currently feels a bit too rigid. See: https://github.com/matter-labs/anvil-zksync/issues/387
        })
    }
    /// Create a fork from a given network at a given height.
    pub async fn from_network(
        fork: &str,
        fork_block_number: Option<u64>,
        cache_config: &CacheConfig,
    ) -> eyre::Result<Self> {
        let (network, client) = Self::fork_network_and_client(fork)?;
        let chain_id_u64 = client.chain_id().await?;
        let chain_id = L2ChainId::from(chain_id_u64.as_u32());

        let l2_miniblock = if let Some(fork_block_number) = fork_block_number {
            fork_block_number
        } else {
            match client.get_block_number().await {
                Ok(bn) => bn.as_u64(),
                Err(error) => {
                    return Err(eyre!(error));
                }
            }
        };

        Self::from_network_and_miniblock_and_chain(
            network,
            client,
            l2_miniblock,
            chain_id.into(),
            cache_config,
        )
        .await
    }

    /// Create a fork from a given network, at a height BEFORE a transaction.
    /// This will allow us to apply this transaction locally on top of this fork.
    pub async fn from_network_tx(
        fork: &str,
        tx: H256,
        cache_config: &CacheConfig,
    ) -> eyre::Result<Self> {
        let (network, client) = Self::fork_network_and_client(fork)?;
        let opt_tx_details = client
            .get_transaction_by_hash(tx)
            .await
            .map_err(|error| eyre!(error))?;
        let tx_details = opt_tx_details.ok_or_else(|| eyre!("could not find {:?}", tx))?;
        let overwrite_chain_id = L2ChainId::try_from(tx_details.chain_id.as_u64())
            .map_err(|error| eyre!("erroneous chain id {}: {:?}", tx_details.chain_id, error))?;
        let block_number = tx_details
            .block_number
            .ok_or_else(|| eyre!("tx {:?} has no block number", tx))?;
        let miniblock_number = L2BlockNumber(block_number.as_u32());
        // We have to sync to the one-miniblock before the one where transaction is.
        let l2_miniblock = miniblock_number.saturating_sub(1) as u64;

        Self::from_network_and_miniblock_and_chain(
            network,
            client,
            l2_miniblock,
            Some(overwrite_chain_id),
            cache_config,
        )
        .await
    }

    /// Return URL and HTTP client for `hardhat_reset`.
    pub fn from_url(
        url: String,
        fork_block_number: Option<u64>,
        cache_config: CacheConfig,
    ) -> eyre::Result<Self> {
        let parsed_url = SensitiveUrl::from_str(&url)?;
        let builder = Client::http(parsed_url).map_err(|error| eyre!(error))?;
        let client = builder.build();

        block_on(async move {
            let chain_id_u64 = client.chain_id().await?;
            let chain_id = L2ChainId::from(chain_id_u64.as_u32());
            let l2_miniblock = if let Some(fork_block_number) = fork_block_number {
                fork_block_number
            } else {
                client.get_block_number().await?.as_u64()
            };

            Self::from_network_and_miniblock_and_chain(
                ForkNetwork::Other(url),
                client,
                l2_miniblock,
                chain_id.into(),
                &cache_config,
            )
            .await
        })
    }

    /// Return [`ForkNetwork`] and HTTP client for a given fork name.
    pub fn fork_network_and_client(fork: &str) -> eyre::Result<(ForkNetwork, Client<L2>)> {
        let network = match fork {
            "mainnet" => ForkNetwork::Mainnet,
            "sepolia-testnet" => ForkNetwork::SepoliaTestnet,
            "goerli-testnet" => ForkNetwork::GoerliTestnet,
            _ => ForkNetwork::Other(fork.to_string()),
        };

        let url = network.to_url();
        let parsed_url = SensitiveUrl::from_str(url)
            .map_err(|_| eyre!("Unable to parse client URL: {}", &url))?;
        let builder = Client::http(parsed_url)
            .map_err(|_| eyre!("Unable to create a client for fork: {}", &url))?;
        Ok((network, builder.build()))
    }

    /// Returns transactions that are in the same L2 miniblock as replay_tx, but were executed before it.
    pub fn get_earlier_transactions_in_same_block(
        &self,
        replay_tx: H256,
    ) -> eyre::Result<Vec<L2Tx>> {
        let opt_tx_details = self
            .fork_source
            .get_transaction_by_hash(replay_tx)
            .map_err(|err| {
                eyre!(
                    "Cannot get transaction to replay by hash from fork source: {:?}",
                    err
                )
            })?;
        let tx_details =
            opt_tx_details.ok_or_else(|| eyre!("Cannot find transaction {:?}", replay_tx))?;
        let block_number = tx_details
            .block_number
            .ok_or_else(|| eyre!("Block has no number"))?;
        let miniblock = L2BlockNumber(block_number.as_u32());

        // And we're fetching all the transactions from this miniblock.
        let block_transactions = self.fork_source.get_raw_block_transactions(miniblock)?;

        let mut tx_to_apply = Vec::new();
        for tx in block_transactions {
            let h = tx.hash();
            let l2_tx: L2Tx = tx.try_into().unwrap();
            tx_to_apply.push(l2_tx);

            if h == replay_tx {
                return Ok(tx_to_apply);
            }
        }
        Err(eyre!(
            "Cound not find tx {:?} in miniblock: {:?}",
            replay_tx,
            miniblock
        ))
    }

    /// Returns
    ///
    /// - `l1_gas_price`
    /// - `l2_fair_gas_price`
    /// - `fair_pubdata_price`
    ///
    /// for the given l2 block.
    pub fn get_block_gas_details(&self, miniblock: u32) -> Option<(u64, u64, u64)> {
        let res_opt_block_details = self.fork_source.get_block_details(L2BlockNumber(miniblock));
        match res_opt_block_details {
            Ok(opt_block_details) => {
                if let Some(block_details) = opt_block_details {
                    if let Some(fair_pubdata_price) = block_details.base.fair_pubdata_price {
                        Some((
                            block_details.base.l1_gas_price,
                            block_details.base.l2_fair_gas_price,
                            fair_pubdata_price,
                        ))
                    } else {
                        tracing::warn!(
                            "Fair pubdata price is not present in {} l2 block details",
                            miniblock
                        );
                        None
                    }
                } else {
                    tracing::warn!("No block details for {}", miniblock);
                    None
                }
            }
            Err(e) => {
                tracing::warn!("Error getting block details: {:?}", e);
                None
            }
        }
    }

    /// Sets fork's internal URL. Assumes the underlying chain is the same as before.
    pub fn set_rpc_url(&mut self, url: String) {
        self.fork_source = Box::new(HttpForkSource::new(url, self.cache_config.clone()));
    }

    // Sets fork's chain id.
    pub fn set_chain_id(&mut self, id: L2ChainId) {
        self.chain_id = id;
        self.overwrite_chain_id = Some(id);
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
    use zksync_types::{AccountTreeId, Address, StorageKey, StorageValue, H256};

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
    use super::{ForkDetails, ForkStorage};
    use crate::{deps::InMemoryStorage, testing};
    use anvil_zksync_config::constants::{
        DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
        DEFAULT_FAIR_PUBDATA_PRICE, DEFAULT_L2_GAS_PRICE, TEST_NODE_NETWORK_ID,
    };
    use anvil_zksync_config::types::{CacheConfig, SystemContractsOptions};
    use zksync_multivm::interface::storage::ReadStorage;
    use zksync_types::{api::TransactionVariant, StorageKey};
    use zksync_types::{
        get_system_context_key, AccountTreeId, L1BatchNumber, L2ChainId, H256,
        SYSTEM_CONTEXT_CHAIN_ID_POSITION,
    };

    #[test]
    fn test_initial_writes() {
        let account = AccountTreeId::default();
        let never_written_key = StorageKey::new(account, H256::from_low_u64_be(1));
        let key_with_some_value = StorageKey::new(account, H256::from_low_u64_be(2));
        let key_with_value_0 = StorageKey::new(account, H256::from_low_u64_be(3));
        let mut in_memory_storage = InMemoryStorage::default();
        in_memory_storage.set_value(key_with_some_value, H256::from_low_u64_be(13));
        in_memory_storage.set_value(key_with_value_0, H256::from_low_u64_be(0));

        let external_storage = testing::ExternalStorage {
            raw_storage: in_memory_storage,
        };

        let options = SystemContractsOptions::default();

        let fork_details = ForkDetails {
            fork_source: Box::new(external_storage),
            chain_id: TEST_NODE_NETWORK_ID.into(),
            l1_block: L1BatchNumber(1),
            l2_block: zksync_types::api::Block::<TransactionVariant>::default(),
            l2_miniblock: 1,
            l2_miniblock_hash: H256::zero(),
            block_timestamp: 0,
            overwrite_chain_id: None,
            l1_gas_price: 100,
            l2_fair_gas_price: DEFAULT_L2_GAS_PRICE,
            fair_pubdata_price: DEFAULT_FAIR_PUBDATA_PRICE,
            estimate_gas_price_scale_factor: DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
            estimate_gas_scale_factor: DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
            fee_params: None,
            cache_config: CacheConfig::None,
        };

        let mut fork_storage: ForkStorage =
            ForkStorage::new(Some(fork_details), &options, false, None);

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
    fn test_get_block_gas_details() {
        let fork_details = ForkDetails {
            fork_source: Box::new(testing::ExternalStorage {
                raw_storage: InMemoryStorage::default(),
            }),
            chain_id: TEST_NODE_NETWORK_ID.into(),
            l1_block: L1BatchNumber(0),
            l2_block: zksync_types::api::Block::<TransactionVariant>::default(),
            l2_miniblock: 0,
            l2_miniblock_hash: H256::zero(),
            block_timestamp: 0,
            overwrite_chain_id: None,
            l1_gas_price: 0,
            l2_fair_gas_price: 0,
            fair_pubdata_price: 0,
            estimate_gas_price_scale_factor: 0.0,
            estimate_gas_scale_factor: 0.0,
            fee_params: None,
            cache_config: CacheConfig::None,
        };

        let actual_result = fork_details.get_block_gas_details(1);
        let expected_result = Some((123, 234, 345));

        assert_eq!(actual_result, expected_result);
    }

    #[test]
    fn test_fork_storage_set_chain_id() {
        let fork_details = ForkDetails {
            fork_source: Box::new(testing::ExternalStorage {
                raw_storage: InMemoryStorage::default(),
            }),
            chain_id: TEST_NODE_NETWORK_ID.into(),
            l1_block: L1BatchNumber(0),
            l2_block: zksync_types::api::Block::<TransactionVariant>::default(),
            l2_miniblock: 0,
            l2_miniblock_hash: H256::zero(),
            block_timestamp: 0,
            overwrite_chain_id: None,
            l1_gas_price: 0,
            l2_fair_gas_price: 0,
            fair_pubdata_price: 0,
            estimate_gas_price_scale_factor: 0.0,
            estimate_gas_scale_factor: 0.0,
            fee_params: None,
            cache_config: CacheConfig::None,
        };
        let mut fork_storage: ForkStorage = ForkStorage::new(
            Some(fork_details),
            &SystemContractsOptions::default(),
            false,
            None,
        );
        let new_chain_id = L2ChainId::from(261);
        fork_storage.set_chain_id(new_chain_id);

        let inner = fork_storage.inner.read().unwrap();

        assert_eq!(new_chain_id, fork_storage.chain_id);
        assert_eq!(
            new_chain_id,
            inner.fork.as_ref().map(|f| f.chain_id).unwrap()
        );
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
