use anvil_zksync_common::{
    cache::{Cache, CacheConfig},
    sh_err,
};
use anvil_zksync_config::constants::{
    DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    DEFAULT_FAIR_PUBDATA_PRICE,
};
use anyhow::Context;
use async_trait::async_trait;
use futures::TryFutureExt;
use itertools::Itertools;
use std::fmt;
use std::future::Future;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::Instrument;
use url::Url;
use zksync_types::fee_model::FeeParams;
use zksync_types::url::SensitiveUrl;
use zksync_types::web3::Index;
use zksync_types::{
    api, Address, L1BatchNumber, L2BlockNumber, L2ChainId, ProtocolVersionId, Transaction, H256,
    U256,
};
use zksync_web3_decl::client::{DynClient, L2};
use zksync_web3_decl::error::Web3Error;
use zksync_web3_decl::namespaces::{EthNamespaceClient, ZksNamespaceClient};

/// Trait that provides necessary data when forking a remote chain.
///
/// Most methods' signatures are similar to corresponding methods from [`EthNamespaceClient`] and [`ZksNamespaceClient`]
/// but with domain-specific types where that makes sense. Additionally, return types are wrapped
/// into [`anyhow::Result`] to avoid leaking RPC-specific implementation details.
#[async_trait]
pub trait ForkSource: fmt::Debug + Send + Sync {
    /// Alternative for [`Clone::clone`] that is object safe.
    fn dyn_cloned(&self) -> Box<dyn ForkSource>;

    /// Human-readable description on the fork's origin. None if there is no fork.
    fn url(&self) -> Option<Url>;

    /// Details on the fork's state at the moment when we forked from it. None if there is no fork.
    fn details(&self) -> Option<ForkDetails>;

    /// Fetches fork's storage value at a given index for given address for the forked blocked.
    async fn get_storage_at(
        &self,
        address: Address,
        idx: U256,
        block: Option<api::BlockIdVariant>,
    ) -> anyhow::Result<H256>;

    /// Fetches fork's storage value at a given index for given address for the forked blocked.
    async fn get_storage_at_forked(&self, address: Address, idx: U256) -> anyhow::Result<H256>;

    /// Returns the bytecode stored under this hash (if available).
    async fn get_bytecode_by_hash(&self, hash: H256) -> anyhow::Result<Option<Vec<u8>>>;

    /// Fetches fork's transaction for a given hash.
    async fn get_transaction_by_hash(&self, hash: H256)
        -> anyhow::Result<Option<api::Transaction>>;

    /// Fetches fork's transaction details for a given hash.
    async fn get_transaction_details(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<api::TransactionDetails>>;

    /// Fetches fork's transactions that belong to a block with the given number.
    async fn get_raw_block_transactions(
        &self,
        block_number: L2BlockNumber,
    ) -> anyhow::Result<Vec<zksync_types::Transaction>>;

    /// Fetches fork's block for a given hash.
    async fn get_block_by_hash(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<api::Block<api::TransactionVariant>>>;

    /// Fetches fork's block for a given number.
    async fn get_block_by_number(
        &self,
        block_number: api::BlockNumber,
    ) -> anyhow::Result<Option<api::Block<api::TransactionVariant>>>;

    /// Fetches fork's block for a given id.
    async fn get_block_by_id(
        &self,
        block_id: api::BlockId,
    ) -> anyhow::Result<Option<api::Block<api::TransactionVariant>>> {
        match block_id {
            api::BlockId::Hash(hash) => self.get_block_by_hash(hash).await,
            api::BlockId::Number(number) => self.get_block_by_number(number).await,
        }
    }

    /// Fetches fork's block details for a given block number.
    async fn get_block_details(
        &self,
        block_number: L2BlockNumber,
    ) -> anyhow::Result<Option<api::BlockDetails>>;

    /// Fetches fork's transaction count for a given block hash.
    async fn get_block_transaction_count_by_hash(
        &self,
        block_hash: H256,
    ) -> anyhow::Result<Option<U256>>;

    /// Fetches fork's transaction count for a given block number.
    async fn get_block_transaction_count_by_number(
        &self,
        block_number: api::BlockNumber,
    ) -> anyhow::Result<Option<U256>>;

    /// Fetches fork's transaction count for a given block id.
    async fn get_block_transaction_count_by_id(
        &self,
        block_id: api::BlockId,
    ) -> anyhow::Result<Option<U256>> {
        match block_id {
            api::BlockId::Hash(hash) => self.get_block_transaction_count_by_hash(hash).await,
            api::BlockId::Number(number) => {
                self.get_block_transaction_count_by_number(number).await
            }
        }
    }

    /// Fetches fork's transaction by block hash and transaction index position.
    async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> anyhow::Result<Option<api::Transaction>>;

    /// Fetches fork's transaction by block number and transaction index position.
    async fn get_transaction_by_block_number_and_index(
        &self,
        block_number: api::BlockNumber,
        index: Index,
    ) -> anyhow::Result<Option<api::Transaction>>;

    /// Fetches fork's transaction by block id and transaction index position.
    async fn get_transaction_by_block_id_and_index(
        &self,
        block_id: api::BlockId,
        index: Index,
    ) -> anyhow::Result<Option<api::Transaction>> {
        match block_id {
            api::BlockId::Hash(hash) => {
                self.get_transaction_by_block_hash_and_index(hash, index)
                    .await
            }
            api::BlockId::Number(number) => {
                self.get_transaction_by_block_number_and_index(number, index)
                    .await
            }
        }
    }

    /// Fetches fork's addresses of the default bridge contracts.
    async fn get_bridge_contracts(&self) -> anyhow::Result<Option<api::BridgeAddresses>>;
}

impl Clone for Box<dyn ForkSource> {
    fn clone(&self) -> Self {
        self.dyn_cloned()
    }
}

#[derive(Debug, Clone)]
pub struct ForkDetails {
    /// Chain ID of the fork.
    pub chain_id: L2ChainId,
    /// Protocol version of the block at which we forked (to be used for all new blocks).
    pub protocol_version: ProtocolVersionId,
    /// Batch number at which we forked (the next batch to seal locally is `batch_number + 1`).
    pub batch_number: L1BatchNumber,
    /// Block number at which we forked (the next block to seal locally is `block_number + 1`).
    pub block_number: L2BlockNumber,
    /// Block hash at which we forked (corresponds to the hash of block #`block_number`).
    pub block_hash: H256,
    /// Block timestamp at which we forked (corresponds to the timestamp of block #`block_number`).
    pub block_timestamp: u64,
    /// API block at which we forked (corresponds to the hash of block #`block_number`).
    pub api_block: api::Block<api::TransactionVariant>,
    pub l1_gas_price: u64,
    pub l2_fair_gas_price: u64,
    // Cost of publishing one byte.
    pub fair_pubdata_price: u64,
    /// L1 Gas Price Scale Factor for gas estimation.
    pub estimate_gas_price_scale_factor: f64,
    /// The factor by which to scale the gasLimit.
    pub estimate_gas_scale_factor: f32,
    pub fee_params: FeeParams,
}

pub struct ForkConfig {
    pub url: Url,
    pub estimate_gas_price_scale_factor: f64,
    pub estimate_gas_scale_factor: f32,
}

impl ForkConfig {
    /// Default configuration for an unknown chain.
    pub fn unknown(url: Url) -> Self {
        // TODO: Unfortunately there is no endpoint that exposes this information and there is no
        //       easy way to derive these values either. Recent releases of zksync-era report unscaled
        //       open batch's fee input and we should mimic something similar.
        let (estimate_gas_price_scale_factor, estimate_gas_scale_factor) = (
            DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
            DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
        );
        Self {
            url,
            estimate_gas_price_scale_factor,
            estimate_gas_scale_factor,
        }
    }
}

/// Simple wrapper over `eth`/`zks`-capable client that propagates all [`ForkSource`] RPC requests to it.
#[derive(Debug, Clone)]
pub struct ForkClient {
    pub url: Url,
    pub details: ForkDetails,
    l2_client: Box<DynClient<L2>>,
}

impl ForkClient {
    async fn new(
        config: ForkConfig,
        l2_client: Box<DynClient<L2>>,
        block_number: L2BlockNumber,
    ) -> anyhow::Result<Self> {
        let ForkConfig {
            url,
            estimate_gas_price_scale_factor,
            estimate_gas_scale_factor,
        } = config;
        let chain_id = l2_client
            .chain_id()
            .await
            .with_context(|| format!("failed to get chain id from fork={url}"))?;
        let chain_id = L2ChainId::try_from(chain_id.as_u64())
            .map_err(|e| anyhow::anyhow!("fork has malformed chain id: {e}"))?;
        let block_details = l2_client
            .get_block_details(block_number)
            .await?
            .ok_or_else(
                || anyhow::anyhow!("could not find block #{block_number} at fork={url}",),
            )?;
        let root_hash = block_details
            .base
            .root_hash
            .ok_or_else(|| anyhow::anyhow!("fork block #{block_number} missing root hash"))?;
        let mut block = l2_client
            .get_block_by_hash(root_hash, true)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "could not find API block #{block_number} at fork={url} despite finding its details \
                    through `zks_getBlockDetails` previously; this is likely a bug, please report this",
                )
            })?;
        let batch_number = block_details.l1_batch_number;
        // TODO: This is a bit weird, we should just grab last block from the latest sealed L1 batch
        //       instead to ensure `l1BatchNumber` is always present.
        block.l1_batch_number = Some(batch_number.0.into());

        let Some(protocol_version) = block_details.protocol_version else {
            // It is possible that some external nodes do not store protocol versions for versions below 9.
            // That's why we assume that whenever a protocol version is not present, it is unsupported by anvil-zksync.
            anyhow::bail!(
                "Block #{block_number} from fork={url} does not have protocol version set. \
                Likely you are using an external node with a block for protocol version below 9 which are unsupported in anvil-zksync. \
                Please report this as a bug if that's not the case."
            )
        };
        if !SupportedProtocolVersions::is_supported(protocol_version) {
            anyhow::bail!(
                "Block #{block_number} from fork={url} is using unsupported protocol version `{protocol_version}`. \
                    anvil-zksync only supports the following versions: {SupportedProtocolVersions}."
            )
        }

        let fee_params = l2_client.get_fee_params().await?;
        let details = ForkDetails {
            chain_id,
            protocol_version,
            batch_number,
            block_number,
            block_hash: root_hash,
            block_timestamp: block_details.base.timestamp,
            api_block: block,
            l1_gas_price: block_details.base.l1_gas_price,
            l2_fair_gas_price: block_details.base.l2_fair_gas_price,
            fair_pubdata_price: block_details
                .base
                .fair_pubdata_price
                .unwrap_or(DEFAULT_FAIR_PUBDATA_PRICE),
            estimate_gas_price_scale_factor,
            estimate_gas_scale_factor,
            fee_params,
        };
        let fork = ForkClient {
            url,
            details,
            l2_client,
        };
        Ok(fork)
    }

    /// Initializes a fork based on config at a given block number.
    pub async fn at_block_number(
        config: ForkConfig,
        block_number: Option<L2BlockNumber>,
    ) -> anyhow::Result<Self> {
        let l2_client =
            zksync_web3_decl::client::Client::http(SensitiveUrl::from(config.url.clone()))?.build();
        let block_number = if let Some(block_number) = block_number {
            block_number
        } else {
            let block_number = l2_client
                .get_block_number()
                .await
                .with_context(|| format!("failed to get block number from fork={}", config.url))?;
            L2BlockNumber(block_number.as_u32())
        };

        Self::new(config, Box::new(l2_client), block_number).await
    }

    /// Initializes a fork based on config at a block BEFORE given transaction.
    /// This will allow us to apply this transaction locally on top of this fork.
    pub async fn at_before_tx(
        config: ForkConfig,
        tx_hash: H256,
    ) -> anyhow::Result<(Self, Vec<Transaction>)> {
        let l2_client =
            zksync_web3_decl::client::Client::http(SensitiveUrl::from(config.url.clone()))?.build();
        let tx_details = l2_client
            .get_transaction_by_hash(tx_hash)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "could not find tx with hash={tx_hash:?} at fork={}",
                    config.url
                )
            })?;
        let block_number = tx_details.block_number.ok_or_else(|| {
            anyhow::anyhow!(
                "could not initialize fork from tx with hash={tx_hash:?} as it is still pending"
            )
        })?;
        let block_number = L2BlockNumber(block_number.as_u32());
        if block_number == L2BlockNumber(0) {
            anyhow::bail!(
                "could not initialize fork from tx with hash={tx_hash:?} as it belongs to genesis"
            );
        }
        let mut earlier_txs = Vec::new();
        for tx in l2_client.get_raw_block_transactions(block_number).await? {
            let hash = tx.hash();
            earlier_txs.push(tx);

            if hash == tx_hash {
                break;
            }
        }

        // We initialize fork from the parent of the block containing transaction.
        Ok((
            Self::new(config, Box::new(l2_client), block_number - 1).await?,
            earlier_txs,
        ))
    }
}

impl ForkClient {
    pub async fn get_fee_params(&self) -> anyhow::Result<FeeParams> {
        self.l2_client
            .get_fee_params()
            .await
            .with_context(|| format!("failed to get fee parameters from fork={}", self.url))
    }
}

#[cfg(test)]
impl ForkClient {
    pub fn mock(details: ForkDetails, storage: crate::deps::InMemoryStorage) -> Self {
        use zksync_types::{u256_to_h256, AccountTreeId, StorageKey, H160};

        let storage = Arc::new(RwLock::new(storage));
        let storage_clone = storage.clone();
        let l2_client = Box::new(
            zksync_web3_decl::client::MockClient::builder(L2::default())
                .method(
                    "eth_getStorageAt",
                    move |address: Address, idx: U256, _block: Option<api::BlockIdVariant>| {
                        let key = StorageKey::new(AccountTreeId::new(address), u256_to_h256(idx));
                        Ok(storage
                            .read()
                            .unwrap()
                            .state
                            .get(&key)
                            .cloned()
                            .unwrap_or_default())
                    },
                )
                .method("zks_getBytecodeByHash", move |hash: H256| {
                    Ok(storage_clone
                        .read()
                        .unwrap()
                        .factory_deps
                        .get(&hash)
                        .cloned())
                })
                .method("zks_getBlockDetails", move |block_number: L2BlockNumber| {
                    Ok(Some(api::BlockDetails {
                        number: block_number,
                        l1_batch_number: L1BatchNumber(123),
                        base: api::BlockDetailsBase {
                            timestamp: 0,
                            l1_tx_count: 0,
                            l2_tx_count: 0,
                            root_hash: None,
                            status: api::BlockStatus::Sealed,
                            commit_tx_hash: None,
                            committed_at: None,
                            commit_chain_id: None,
                            prove_tx_hash: None,
                            proven_at: None,
                            prove_chain_id: None,
                            execute_tx_hash: None,
                            executed_at: None,
                            execute_chain_id: None,
                            precommit_tx_hash: None,
                            precommit_tx_finality: None,
                            precommitted_at: None,
                            precommit_chain_id: None,
                            l1_gas_price: 123,
                            l2_fair_gas_price: 234,
                            fair_pubdata_price: Some(345),
                            base_system_contracts_hashes: Default::default(),
                            commit_tx_finality: None,
                            prove_tx_finality: None,
                            execute_tx_finality: None,
                        },
                        operator_address: H160::zero(),
                        protocol_version: None,
                    }))
                })
                .build(),
        );
        ForkClient {
            url: Url::parse("http://test-fork-in-memory-storage.local").unwrap(),
            details,
            l2_client,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Fork {
    state: Arc<RwLock<ForkState>>,
}

#[derive(Debug)]
struct ForkState {
    client: Option<ForkClient>,
    cache: Cache,
}

impl Fork {
    pub(super) fn new(client: Option<ForkClient>, cache_config: CacheConfig) -> Self {
        let cache = Cache::new(cache_config);
        Self {
            state: Arc::new(RwLock::new(ForkState { client, cache })),
        }
    }

    pub(super) fn reset_fork_client(&self, client: Option<ForkClient>) {
        // TODO: We don't clean cache here so it might interfere with the new fork. Consider
        //       parametrizing cache by fork URL to avoid this.
        self.write().client = client;
    }

    pub(super) fn set_fork_url(&self, new_url: Url) -> Option<Url> {
        // We are assuming that the new url is pointing to the same logical data source so we do not
        // invalidate cache
        let mut writer = self.write();
        if let Some(client) = writer.client.as_mut() {
            Some(std::mem::replace(&mut client.url, new_url))
        } else {
            None
        }
    }

    fn read(&self) -> RwLockReadGuard<ForkState> {
        self.state.read().expect("Fork lock is poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<ForkState> {
        self.state.write().expect("Fork lock is poisoned")
    }

    async fn make_call<T, F: Future<Output = anyhow::Result<T>>>(
        &self,
        method: &str,
        call_body: impl FnOnce(Box<DynClient<L2>>) -> F,
    ) -> Option<anyhow::Result<T>> {
        let (client, span) = if let Some(client) = self.read().client.as_ref() {
            let span = tracing::info_span!("fork_rpc_call", method, url = %client.url);
            (client.l2_client.clone(), span)
        } else {
            return None;
        };
        Some(
            call_body(client)
                .map_err(|error| {
                    sh_err!("call failed: {}", error);
                    error
                })
                .instrument(span)
                .await,
        )
    }
}

#[async_trait]
impl ForkSource for Fork {
    fn dyn_cloned(&self) -> Box<dyn ForkSource> {
        Box::new(self.clone())
    }

    fn url(&self) -> Option<Url> {
        self.read().client.as_ref().map(|client| client.url.clone())
    }

    fn details(&self) -> Option<ForkDetails> {
        self.read()
            .client
            .as_ref()
            .map(|client| client.details.clone())
    }

    async fn get_storage_at(
        &self,
        address: Address,
        idx: U256,
        block: Option<api::BlockIdVariant>,
    ) -> anyhow::Result<H256> {
        // TODO: This is currently cached at the `ForkStorage` level but I am unsure if this is a
        //       good thing. Intuitively it feels like cache should be centralized in a single place.
        self.make_call("get_storage_at", |client| async move {
            client
                .get_storage_at(address, idx, block)
                .await
                .with_context(|| format!("(address={address:?}, idx={idx:?})"))
        })
        .await
        .unwrap_or(Ok(H256::zero()))
    }

    async fn get_storage_at_forked(&self, address: Address, idx: U256) -> anyhow::Result<H256> {
        let Some(block_number) = self
            .read()
            .client
            .as_ref()
            .map(|client| client.details.block_number)
        else {
            return Ok(H256::zero());
        };
        self.get_storage_at(
            address,
            idx,
            Some(api::BlockIdVariant::BlockNumber(api::BlockNumber::Number(
                block_number.0.into(),
            ))),
        )
        .await
    }

    async fn get_bytecode_by_hash(&self, hash: H256) -> anyhow::Result<Option<Vec<u8>>> {
        // TODO: This is currently cached at the `ForkStorage` level but I am unsure if this is a
        //       good thing. Intuitively it feels like cache should be centralized in a single place.
        self.make_call("get_bytecode_by_hash", |client| async move {
            client
                .get_bytecode_by_hash(hash)
                .await
                .with_context(|| format!("(hash={hash:?})"))
        })
        .await
        .unwrap_or(Ok(None))
    }

    async fn get_transaction_by_hash(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<api::Transaction>> {
        if let Some(tx) = self.read().cache.get_transaction(&hash).cloned() {
            tracing::debug!(?hash, "using cached transaction");
            return Ok(Some(tx));
        }

        let tx = self
            .make_call("get_transaction_by_hash", |client| async move {
                client
                    .get_transaction_by_hash(hash)
                    .await
                    .with_context(|| format!("(hash={hash:?})"))
            })
            .await
            .unwrap_or(Ok(None))?;

        if let Some(tx) = tx {
            self.write().cache.insert_transaction(hash, tx.clone());
            Ok(Some(tx))
        } else {
            Ok(None)
        }
    }

    async fn get_transaction_details(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<api::TransactionDetails>> {
        // N.B. We don't cache these responses as they will change through the lifecycle of the transaction
        // and caching could be error-prone. In theory, we could cache responses once the txn status
        // is `final` or `failed` but currently this does not warrant the additional complexity.
        self.make_call("get_transaction_details", |client| async move {
            client
                .get_transaction_details(hash)
                .await
                .with_context(|| format!("(hash={hash:?})"))
        })
        .await
        .unwrap_or(Ok(None))
    }

    async fn get_raw_block_transactions(
        &self,
        block_number: L2BlockNumber,
    ) -> anyhow::Result<Vec<zksync_types::Transaction>> {
        if let Some(txs) = self
            .read()
            .cache
            .get_block_raw_transactions(&(block_number.0 as u64))
            .cloned()
        {
            tracing::debug!(%block_number, "using cached block raw transactions");
            return Ok(txs);
        }

        let txs = self
            .make_call("get_raw_block_transactions", |client| async move {
                client
                    .get_raw_block_transactions(block_number)
                    .await
                    .with_context(|| format!("(block_number={block_number})"))
            })
            .await
            .unwrap_or(Err(Web3Error::NoBlock.into()))?;

        self.write()
            .cache
            .insert_block_raw_transactions(block_number.0 as u64, txs.clone());
        Ok(txs)
    }

    async fn get_block_by_hash(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<api::Block<api::TransactionVariant>>> {
        if let Some(block) = self.read().cache.get_block(&hash, true).cloned() {
            tracing::debug!(?hash, "using cached block");
            return Ok(Some(block));
        }

        let block = self
            .make_call("get_block_by_hash", |client| async move {
                client
                    .get_block_by_hash(hash, true)
                    .await
                    .with_context(|| format!("(hash={hash:?}, full_transactions=true)"))
            })
            .await
            .unwrap_or(Ok(None))?;

        if let Some(block) = block {
            self.write().cache.insert_block(hash, true, block.clone());
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    async fn get_block_by_number(
        &self,
        block_number: api::BlockNumber,
    ) -> anyhow::Result<Option<api::Block<api::TransactionVariant>>> {
        match block_number {
            api::BlockNumber::Number(block_number) => {
                {
                    let guard = self.read();
                    let cache = &guard.cache;
                    if let Some(block) = cache
                        .get_block_hash(&block_number.as_u64())
                        .and_then(|hash| cache.get_block(hash, true))
                        .cloned()
                    {
                        tracing::debug!(%block_number, "using cached block");
                        return Ok(Some(block));
                    }
                }

                let block = self
                    .make_call("get_block_by_number", |client| async move {
                        client
                            .get_block_by_number(api::BlockNumber::Number(block_number), true)
                            .await
                            .with_context(|| {
                                format!("(block_number={block_number}, full_transactions=true)")
                            })
                    })
                    .await
                    .unwrap_or(Ok(None))?;

                if let Some(block) = block {
                    self.write()
                        .cache
                        .insert_block(block.hash, true, block.clone());
                    Ok(Some(block))
                } else {
                    Ok(None)
                }
            }
            _ => self
                .make_call("get_block_by_number", |client| async move {
                    client
                        .get_block_by_number(block_number, true)
                        .await
                        .with_context(|| {
                            format!("(block_number={block_number}, full_transactions=true)")
                        })
                })
                .await
                .unwrap_or(Ok(None)),
        }
    }

    async fn get_block_details(
        &self,
        block_number: L2BlockNumber,
    ) -> anyhow::Result<Option<api::BlockDetails>> {
        // N.B. We don't cache these responses as they will change through the lifecycle of the block
        // and caching could be error-prone. In theory, we could cache responses once the block
        // is finalized but currently this does not warrant the additional complexity.
        self.make_call("get_block_details", |client| async move {
            client
                .get_block_details(block_number)
                .await
                .with_context(|| format!("(block_number={block_number})"))
        })
        .await
        .unwrap_or(Ok(None))
    }

    async fn get_block_transaction_count_by_hash(
        &self,
        block_hash: H256,
    ) -> anyhow::Result<Option<U256>> {
        // TODO: Cache?
        self.make_call("get_block_transaction_count_by_hash", |client| async move {
            client
                .get_block_transaction_count_by_hash(block_hash)
                .await
                .with_context(|| format!("(block_hash={block_hash:?})"))
        })
        .await
        .unwrap_or(Ok(None))
    }

    async fn get_block_transaction_count_by_number(
        &self,
        block_number: api::BlockNumber,
    ) -> anyhow::Result<Option<U256>> {
        // TODO: Cache?
        self.make_call(
            "get_block_transaction_count_by_number",
            |client| async move {
                client
                    .get_block_transaction_count_by_number(block_number)
                    .await
                    .with_context(|| format!("(block_number={block_number})"))
            },
        )
        .await
        .unwrap_or(Ok(None))
    }

    async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> anyhow::Result<Option<api::Transaction>> {
        // TODO: Cache?
        self.make_call(
            "get_transaction_by_block_hash_and_index",
            |client| async move {
                client
                    .get_transaction_by_block_hash_and_index(block_hash, index)
                    .await
                    .with_context(|| format!("(block_hash={block_hash:?}, index={index})"))
            },
        )
        .await
        .unwrap_or(Ok(None))
    }

    async fn get_transaction_by_block_number_and_index(
        &self,
        block_number: api::BlockNumber,
        index: Index,
    ) -> anyhow::Result<Option<api::Transaction>> {
        // TODO: Cache?
        self.make_call(
            "get_transaction_by_block_number_and_index",
            |client| async move {
                client
                    .get_transaction_by_block_number_and_index(block_number, index)
                    .await
                    .with_context(|| format!("(block_number={block_number}, index={index})"))
            },
        )
        .await
        .unwrap_or(Ok(None))
    }

    async fn get_bridge_contracts(&self) -> anyhow::Result<Option<api::BridgeAddresses>> {
        if let Some(bridge_contracts) = self.read().cache.get_bridge_addresses().cloned() {
            tracing::debug!("using cached bridge contracts");
            return Ok(Some(bridge_contracts));
        }

        let bridge_contracts = self
            .make_call("get_bridge_contracts", |client| async move {
                Ok(Some(client.get_bridge_contracts().await?))
            })
            .await
            .unwrap_or(Ok(None))?;

        if let Some(bridge_contracts) = bridge_contracts {
            self.write()
                .cache
                .set_bridge_addresses(bridge_contracts.clone());
            Ok(Some(bridge_contracts))
        } else {
            Ok(None)
        }
    }
}

struct SupportedProtocolVersions;

impl SupportedProtocolVersions {
    const SUPPORTED_VERSIONS: [ProtocolVersionId; 5] = [
        ProtocolVersionId::Version26,
        ProtocolVersionId::Version27,
        ProtocolVersionId::Version28,
        ProtocolVersionId::Version29,
        ProtocolVersionId::Version30,
    ];

    fn is_supported(version: ProtocolVersionId) -> bool {
        Self::SUPPORTED_VERSIONS.contains(&version)
    }
}

impl fmt::Display for SupportedProtocolVersions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            &Self::SUPPORTED_VERSIONS
                .iter()
                .map(|v| v.to_string())
                .join(", "),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::deps::InMemoryStorage;
    use maplit::hashmap;
    use zksync_types::block::{pack_block_info, unpack_block_info};
    use zksync_types::fee_model::{BaseTokenConversionRatio, FeeModelConfigV2, FeeParamsV2};
    use zksync_types::{h256_to_u256, u256_to_h256, AccountTreeId, StorageKey};

    impl Default for ForkDetails {
        fn default() -> Self {
            let config = FeeModelConfigV2 {
                minimal_l2_gas_price: 10_000_000_000,
                compute_overhead_part: 0.0,
                pubdata_overhead_part: 1.0,
                batch_overhead_l1_gas: 800_000,
                max_gas_per_batch: 200_000_000,
                max_pubdata_per_batch: 500_000,
            };
            Self {
                chain_id: Default::default(),
                protocol_version: ProtocolVersionId::latest(),
                batch_number: Default::default(),
                block_number: Default::default(),
                block_hash: Default::default(),
                block_timestamp: 0,
                api_block: Default::default(),
                l1_gas_price: 0,
                l2_fair_gas_price: 0,
                fair_pubdata_price: 0,
                estimate_gas_price_scale_factor: 0.0,
                estimate_gas_scale_factor: 0.0,
                fee_params: FeeParams::V2(FeeParamsV2::new(
                    config,
                    10_000_000_000,
                    5_000_000_000,
                    BaseTokenConversionRatio::default(),
                )),
            }
        }
    }

    #[tokio::test]
    async fn test_mock_client() {
        let input_batch = 1;
        let input_l2_block = 2;
        let input_timestamp = 3;
        let input_bytecode = vec![0x4];
        let batch_key = StorageKey::new(
            AccountTreeId::new(zksync_types::SYSTEM_CONTEXT_ADDRESS),
            zksync_types::SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
        );
        let l2_block_key = StorageKey::new(
            AccountTreeId::new(zksync_types::SYSTEM_CONTEXT_ADDRESS),
            zksync_types::SYSTEM_CONTEXT_CURRENT_L2_BLOCK_INFO_POSITION,
        );

        let client = ForkClient::mock(
            ForkDetails::default(),
            InMemoryStorage {
                state: hashmap! {
                    batch_key => u256_to_h256(U256::from(input_batch)),
                    l2_block_key => u256_to_h256(pack_block_info(
                        input_l2_block,
                        input_timestamp,
                    ))
                },
                factory_deps: hashmap! {
                    H256::repeat_byte(0x1) => input_bytecode.clone(),
                },
            },
        );
        let fork = Fork::new(Some(client), CacheConfig::None);

        let actual_batch = fork
            .get_storage_at(
                zksync_types::SYSTEM_CONTEXT_ADDRESS,
                h256_to_u256(zksync_types::SYSTEM_CONTEXT_BLOCK_INFO_POSITION),
                None,
            )
            .await
            .map(|value| h256_to_u256(value).as_u64())
            .expect("failed getting batch number");
        assert_eq!(input_batch, actual_batch);

        let (actual_l2_block, actual_timestamp) = fork
            .get_storage_at(
                zksync_types::SYSTEM_CONTEXT_ADDRESS,
                h256_to_u256(zksync_types::SYSTEM_CONTEXT_CURRENT_L2_BLOCK_INFO_POSITION),
                None,
            )
            .await
            .map(|value| unpack_block_info(h256_to_u256(value)))
            .expect("failed getting l2 block info");
        assert_eq!(input_l2_block, actual_l2_block);
        assert_eq!(input_timestamp, actual_timestamp);

        let zero_missing_value = fork
            .get_storage_at(
                zksync_types::SYSTEM_CONTEXT_ADDRESS,
                h256_to_u256(H256::repeat_byte(0x1e)),
                None,
            )
            .await
            .map(|value| h256_to_u256(value).as_u64())
            .expect("failed missing value");
        assert_eq!(0, zero_missing_value);

        let actual_bytecode = fork
            .get_bytecode_by_hash(H256::repeat_byte(0x1))
            .await
            .expect("failed getting bytecode")
            .expect("missing bytecode");
        assert_eq!(input_bytecode, actual_bytecode);
    }
}
