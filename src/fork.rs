//! This file hold tools used for test-forking other networks.
//!
//! There is ForkStorage (that is a wrapper over InMemoryStorage)
//! And ForkDetails - that parses network address and fork height from arguments.

use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    future::Future,
    sync::{Arc, RwLock},
};

use tokio::runtime::Builder;
use zksync_basic_types::{Address, L1BatchNumber, L2ChainId, MiniblockNumber, H256, U256, U64};

use zksync_types::{
    api::{
        Block, BlockDetails, BlockIdVariant, BlockNumber, BridgeAddresses, Transaction,
        TransactionDetails, TransactionVariant,
    },
    l2::L2Tx,
    ProtocolVersionId, StorageKey,
};

use zksync_state::ReadStorage;
use zksync_utils::{bytecode::hash_bytecode, h256_to_u256};

use zksync_web3_decl::{
    jsonrpsee::http_client::HttpClient, namespaces::EthNamespaceClient, types::Index,
};
use zksync_web3_decl::{jsonrpsee::http_client::HttpClientBuilder, namespaces::ZksNamespaceClient};

use crate::system_contracts;
use crate::{cache::CacheConfig, node::TEST_NODE_NETWORK_ID};
use crate::{deps::InMemoryStorage, http_fork_source::HttpForkSource};

pub fn block_on<F: Future + Send + 'static>(future: F) -> F::Output
where
    F::Output: Send,
{
    std::thread::spawn(move || {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime creation failed");
        runtime.block_on(future)
    })
    .join()
    .unwrap()
}

/// In memory storage, that allows 'forking' from other network.
/// If forking is enabled, it reads missing data from remote location.
/// S - is a struct that is used for source of the fork.
#[derive(Debug, Clone)]
pub struct ForkStorage<S> {
    pub inner: Arc<RwLock<ForkStorageInner<S>>>,
    pub chain_id: L2ChainId,
}

#[derive(Debug)]
pub struct ForkStorageInner<S> {
    // Underlying local storage
    pub raw_storage: InMemoryStorage,
    // Cache of data that was read from remote location.
    pub value_read_cache: HashMap<StorageKey, H256>,
    // Cache of factory deps that were read from remote location.
    pub factory_dep_cache: HashMap<H256, Option<Vec<u8>>>,
    // If set - it hold the necessary information on where to fetch the data.
    // If not set - it will simply read from underlying storage.
    pub fork: Option<ForkDetails<S>>,
}

impl<S: ForkSource> ForkStorage<S> {
    pub fn new(
        fork: Option<ForkDetails<S>>,
        system_contracts_options: &system_contracts::Options,
    ) -> Self {
        let chain_id = fork
            .as_ref()
            .and_then(|d| d.overwrite_chain_id)
            .unwrap_or(L2ChainId::from(TEST_NODE_NETWORK_ID));
        tracing::info!("Starting network with chain id: {:?}", chain_id);

        ForkStorage {
            inner: Arc::new(RwLock::new(ForkStorageInner {
                raw_storage: InMemoryStorage::with_system_contracts_and_chain_id(
                    chain_id,
                    hash_bytecode,
                    system_contracts_options,
                ),
                value_read_cache: Default::default(),
                fork,
                factory_dep_cache: Default::default(),
            })),
            chain_id,
        }
    }

    fn read_value_internal(&self, key: &StorageKey) -> zksync_types::StorageValue {
        let mut mutator = self.inner.write().unwrap();
        let local_storage = mutator.raw_storage.read_value(key);

        if let Some(fork) = &mutator.fork {
            if !H256::is_zero(&local_storage) {
                return local_storage;
            }

            if let Some(value) = mutator.value_read_cache.get(key) {
                return *value;
            }
            let l2_miniblock = fork.l2_miniblock;
            let key_ = *key;

            let result = fork
                .fork_source
                .get_storage_at(
                    *key_.account().address(),
                    h256_to_u256(*key_.key()),
                    Some(BlockIdVariant::BlockNumber(BlockNumber::Number(U64::from(
                        l2_miniblock,
                    )))),
                )
                .unwrap();

            mutator.value_read_cache.insert(*key, result);
            result
        } else {
            local_storage
        }
    }

    fn load_factory_dep_internal(&self, hash: H256) -> Option<Vec<u8>> {
        let mut mutator = self.inner.write().unwrap();
        let local_storage = mutator.raw_storage.load_factory_dep(hash);
        if let Some(fork) = &mutator.fork {
            if local_storage.is_some() {
                return local_storage;
            }
            if let Some(value) = mutator.factory_dep_cache.get(&hash) {
                return value.clone();
            }

            let result = fork.fork_source.get_bytecode_by_hash(hash).unwrap();
            mutator.factory_dep_cache.insert(hash, result.clone());
            result
        } else {
            local_storage
        }
    }

    /// Check if this is the first time when we're ever writing to this key.
    /// This has impact on amount of pubdata that we have to spend for the write.
    fn is_write_initial_internal(&self, key: &StorageKey) -> bool {
        // Currently we don't have the zks API to return us the information on whether a given
        // key was written to before a given block.
        // This means, we have to depend on the following heuristic: we'll read the value of the slot.
        //  - if value != 0 -> this means that the slot was written to in the past (so we can return intitial_write = false)
        //  - but if the value = 0 - there is a chance, that slot was written to in the past - and later was reset.
        //                            but unfortunately we cannot detect that with the current zks api, so we'll attempt to do it
        //                           only on local storage.
        let value = self.read_value_internal(key);
        if value != H256::zero() {
            return false;
        }

        // If value was 0, there is still a chance, that the slot was written to in the past - and only now set to 0.
        // We unfortunately don't have the API to check it on the fork, but we can at least try to check it on local storage.
        let mut mutator = self.inner.write().unwrap();
        mutator.raw_storage.is_write_initial(key)
    }

    /// Retrieves the enumeration index for a given `key`.
    fn get_enumeration_index_internal(&self, _key: &StorageKey) -> Option<u64> {
        // TODO: Update this file to use proper enumeration index value once it's exposed for forks via API
        Some(0_u64)
    }
}

impl<S: std::fmt::Debug + ForkSource> ReadStorage for ForkStorage<S> {
    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        self.is_write_initial_internal(key)
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        self.load_factory_dep_internal(hash)
    }

    fn read_value(&mut self, key: &StorageKey) -> zksync_types::StorageValue {
        self.read_value_internal(key)
    }

    fn get_enumeration_index(&mut self, key: &StorageKey) -> Option<u64> {
        self.get_enumeration_index_internal(key)
    }
}

impl<S: std::fmt::Debug + ForkSource> ReadStorage for &ForkStorage<S> {
    fn read_value(&mut self, key: &StorageKey) -> zksync_types::StorageValue {
        self.read_value_internal(key)
    }

    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        self.is_write_initial_internal(key)
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        self.load_factory_dep_internal(hash)
    }

    fn get_enumeration_index(&mut self, key: &StorageKey) -> Option<u64> {
        self.get_enumeration_index_internal(key)
    }
}

impl<S> ForkStorage<S> {
    pub fn set_value(&mut self, key: StorageKey, value: zksync_types::StorageValue) {
        let mut mutator = self.inner.write().unwrap();
        mutator.raw_storage.set_value(key, value)
    }
    pub fn store_factory_dep(&mut self, hash: H256, bytecode: Vec<u8>) {
        let mut mutator = self.inner.write().unwrap();
        mutator.raw_storage.store_factory_dep(hash, bytecode)
    }
}

/// Trait that provides necessary data when
/// forking a remote chain.
/// The method signatures are similar to methods from ETHNamespace and ZKNamespace.
pub trait ForkSource {
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
        block_number: MiniblockNumber,
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

    /// Returns the block details for a given miniblock number.
    fn get_block_details(&self, miniblock: MiniblockNumber) -> eyre::Result<Option<BlockDetails>>;

    /// Returns the  transaction count for a given block hash.
    fn get_block_transaction_count_by_hash(&self, block_hash: H256) -> eyre::Result<Option<U256>>;

    /// Returns the transaction count for a given block number.
    fn get_block_transaction_count_by_number(
        &self,
        block_number: zksync_types::api::BlockNumber,
    ) -> eyre::Result<Option<U256>>;

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
/// "S" is the implementation of the ForkSource.
#[derive(Debug, Clone)]
pub struct ForkDetails<S> {
    // Source of the fork data (for example HTTPForkSoruce)
    pub fork_source: S,
    // Block number at which we forked (the next block to create is l1_block + 1)
    pub l1_block: L1BatchNumber,
    // The actual L2 block
    pub l2_block: zksync_types::api::Block<zksync_types::api::TransactionVariant>,
    pub l2_miniblock: u64,
    pub l2_miniblock_hash: H256,
    pub block_timestamp: u64,
    pub overwrite_chain_id: Option<L2ChainId>,
    pub l1_gas_price: u64,
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

impl ForkDetails<HttpForkSource> {
    pub async fn from_url_and_miniblock_and_chain(
        url: &str,
        client: HttpClient,
        miniblock: u64,
        chain_id: Option<L2ChainId>,
        cache_config: CacheConfig,
    ) -> Self {
        let block_details = client
            .get_block_details(MiniblockNumber(miniblock as u32))
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("Could not find block {:?} in {:?}", miniblock, url));

        let root_hash = block_details
            .base
            .root_hash
            .unwrap_or_else(|| panic!("fork block #{} missing root hash", miniblock));
        let block = client
            .get_block_by_hash(root_hash, true)
            .await
            .expect("failed retrieving block")
            .unwrap_or_else(|| {
                panic!(
                    "Could not find block #{:?} ({:#x}) in {:?}",
                    miniblock, root_hash, url
                )
            });
        let l1_batch_number = block_details.l1_batch_number;

        tracing::info!(
            "Creating fork from {:?} L1 block: {:?} L2 block: {:?} with timestamp {:?}, L1 gas price {:?} and protocol version: {:?}" ,
            url, l1_batch_number, miniblock, block_details.base.timestamp, block_details.base.l1_gas_price, block_details.protocol_version
        );

        if !block_details
            .protocol_version
            .map(supported_protocol_versions)
            .unwrap_or(false)
        {
            panic!(
                "This block is using the unsupported protocol version: {:?}. This binary supports versions {}.",
                block_details.protocol_version,
                supported_versions_to_string()
            );
        }

        ForkDetails {
            fork_source: HttpForkSource::new(url.to_owned(), cache_config),
            l1_block: l1_batch_number,
            l2_block: block,
            block_timestamp: block_details.base.timestamp,
            l2_miniblock: miniblock,
            l2_miniblock_hash: root_hash,
            overwrite_chain_id: chain_id,
            l1_gas_price: block_details.base.l1_gas_price,
        }
    }
    /// Create a fork from a given network at a given height.
    pub async fn from_network(fork: &str, fork_at: Option<u64>, cache_config: CacheConfig) -> Self {
        let (url, client) = Self::fork_to_url_and_client(fork);
        let l2_miniblock = if let Some(fork_at) = fork_at {
            fork_at
        } else {
            client.get_block_number().await.unwrap().as_u64()
        };
        Self::from_url_and_miniblock_and_chain(url, client, l2_miniblock, None, cache_config).await
    }

    /// Create a fork from a given network, at a height BEFORE a transaction.
    /// This will allow us to apply this transaction locally on top of this fork.
    pub async fn from_network_tx(fork: &str, tx: H256, cache_config: CacheConfig) -> Self {
        let (url, client) = Self::fork_to_url_and_client(fork);
        let tx_details = client.get_transaction_by_hash(tx).await.unwrap().unwrap();
        let overwrite_chain_id = Some(
            L2ChainId::try_from(tx_details.chain_id.as_u64()).unwrap_or_else(|err| {
                panic!("erroneous chain id {}: {:?}", tx_details.chain_id, err,)
            }),
        );
        let miniblock_number = MiniblockNumber(tx_details.block_number.unwrap().as_u32());
        // We have to sync to the one-miniblock before the one where transaction is.
        let l2_miniblock = miniblock_number.saturating_sub(1) as u64;

        Self::from_url_and_miniblock_and_chain(
            url,
            client,
            l2_miniblock,
            overwrite_chain_id,
            cache_config,
        )
        .await
    }
}

impl<S: ForkSource> ForkDetails<S> {
    /// Return URL and HTTP client for a given fork name.
    pub fn fork_to_url_and_client(fork: &str) -> (&str, HttpClient) {
        let url = match fork {
            "mainnet" => "https://mainnet.era.zksync.io:443",
            "sepolia-testnet" => "https://sepolia.era.zksync.dev:443",
            "goerli-testnet" => "https://testnet.era.zksync.dev:443",
            _ => fork,
        };

        let client = HttpClientBuilder::default()
            .build(url)
            .expect("Unable to create a client for fork");

        (url, client)
    }

    /// Returns transactions that are in the same L2 miniblock as replay_tx, but were executed before it.
    pub async fn get_earlier_transactions_in_same_block(&self, replay_tx: H256) -> Vec<L2Tx> {
        let tx_details = self
            .fork_source
            .get_transaction_by_hash(replay_tx)
            .unwrap()
            .unwrap();
        let miniblock = MiniblockNumber(tx_details.block_number.unwrap().as_u32());

        // And we're fetching all the transactions from this miniblock.
        let block_transactions = self
            .fork_source
            .get_raw_block_transactions(miniblock)
            .unwrap();

        let mut tx_to_apply = Vec::new();

        for tx in block_transactions {
            let h = tx.hash();
            let l2_tx: L2Tx = tx.try_into().unwrap();
            tx_to_apply.push(l2_tx);

            if h == replay_tx {
                return tx_to_apply;
            }
        }
        panic!(
            "Cound not find tx {:?} in miniblock: {:?}",
            replay_tx, miniblock
        );
    }
}

#[cfg(test)]
mod tests {
    use zksync_basic_types::{AccountTreeId, L1BatchNumber, H256};
    use zksync_state::ReadStorage;
    use zksync_types::{api::TransactionVariant, StorageKey};

    use crate::{deps::InMemoryStorage, system_contracts, testing};

    use super::{ForkDetails, ForkStorage};

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

        let options = system_contracts::Options::default();

        let fork_details = ForkDetails {
            fork_source: &external_storage,
            l1_block: L1BatchNumber(1),
            l2_block: zksync_types::api::Block::<TransactionVariant>::default(),
            l2_miniblock: 1,
            l2_miniblock_hash: H256::zero(),
            block_timestamp: 0,
            overwrite_chain_id: None,
            l1_gas_price: 100,
        };

        let mut fork_storage = ForkStorage::new(Some(fork_details), &options);

        assert_eq!(fork_storage.is_write_initial(&never_written_key), true);
        assert_eq!(fork_storage.is_write_initial(&key_with_some_value), false);
        // This is the current limitation of the sytem. In theory, this should return false - as the value was written, but we don't have the API to the
        // backend to get this information.
        assert_eq!(fork_storage.is_write_initial(&key_with_value_0), true);

        // But writing any value there in the local storage (even 0) - should make it non-initial write immediately.
        fork_storage.set_value(key_with_value_0, H256::zero());
        assert_eq!(fork_storage.is_write_initial(&key_with_value_0), false);
    }
}
