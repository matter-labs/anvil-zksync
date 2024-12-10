use crate::namespaces::DetailedTransaction;
use crate::node::pool::TxBatch;
use crate::node::sealer::BlockSealerMode;
use crate::utils::Numeric;
use crate::{
    fork::{ForkDetails, ForkSource},
    namespaces::ResetRequest,
    node::InMemoryNode,
    utils::bytecode_to_factory_dep,
};
use anyhow::{anyhow, Context};
use std::convert::TryInto;
use std::time::Duration;
use zksync_multivm::interface::TxExecutionMode;
use zksync_types::api::{Block, TransactionVariant};
use zksync_types::{
    get_code_key, get_nonce_key,
    utils::{nonces_to_full_nonce, storage_key_for_eth_balance},
    L2BlockNumber, StorageKey,
};
use zksync_types::{AccountTreeId, Address, H256, U256, U64};
use zksync_utils::u256_to_h256;

type Result<T> = anyhow::Result<T>;

/// The maximum number of [Snapshot]s to store. Each snapshot represents the node state
/// and can be used to revert the node to an earlier point in time.
const MAX_SNAPSHOTS: u8 = 100;

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> InMemoryNode<S> {
    /// Increase the current timestamp for the node
    ///
    /// # Parameters
    /// - `time_delta`: The number of seconds to increase time by
    ///
    /// # Returns
    /// The applied time delta to `current_timestamp` value for the InMemoryNodeInner.
    pub fn increase_time(&self, time_delta_seconds: Numeric) -> Result<u64> {
        let time_delta_seconds = time_delta_seconds
            .try_into()
            .context("The time delta is too big")?;
        self.time.increase_time(time_delta_seconds);
        Ok(time_delta_seconds)
    }

    /// Set the current timestamp for the node. The timestamp must be in future.
    ///
    /// # Parameters
    /// - `timestamp`: The timestamp to set the time to
    ///
    /// # Returns
    /// The new timestamp value for the InMemoryNodeInner.
    pub fn set_next_block_timestamp(&self, timestamp: Numeric) -> Result<()> {
        let timestamp: u64 = timestamp.try_into().context("The timestamp is too big")?;
        self.time.enforce_next_timestamp(timestamp)
    }

    /// Set the current timestamp for the node.
    /// Warning: This will allow you to move backwards in time, which may cause new blocks to appear to be
    /// mined before old blocks. This will result in an invalid state.
    ///
    /// # Parameters
    /// - `time`: The timestamp to set the time to
    ///
    /// # Returns
    /// The difference between the `current_timestamp` and the new timestamp for the InMemoryNodeInner.
    pub fn set_time(&self, timestamp: Numeric) -> Result<i128> {
        Ok(self.time.set_current_timestamp_unchecked(
            timestamp.try_into().context("The timestamp is too big")?,
        ))
    }

    /// Force a single block to be mined.
    ///
    /// Will mine an empty block (containing zero transactions)
    ///
    /// # Returns
    /// The string "0x0".
    pub fn mine_block(&self) -> Result<L2BlockNumber> {
        // TODO: Remove locking once `TestNodeConfig` is refactored into mutable/immutable components
        let max_transactions = self.read_inner()?.config.max_transactions;
        let TxBatch { impersonating, txs } =
            self.pool.take_uniform(max_transactions).unwrap_or(TxBatch {
                impersonating: false,
                txs: Vec::new(),
            });
        let base_system_contracts = self
            .system_contracts
            .contracts(TxExecutionMode::VerifyExecute, impersonating)
            .clone();

        let block_number = self.seal_block(&mut self.time.lock(), txs, base_system_contracts)?;
        tracing::info!("👷 Mined block #{}", block_number);
        Ok(block_number)
    }

    pub fn mine_detailed(&self) -> Result<Block<DetailedTransaction>> {
        let block_number = self.mine_block()?;
        let inner = self.read_inner()?;
        let mut block = inner
            .block_hashes
            .get(&(block_number.0 as u64))
            .and_then(|hash| inner.blocks.get(hash))
            .expect("freshly mined block is missing from storage")
            .clone();
        let detailed_txs = std::mem::take(&mut block.transactions)
            .into_iter()
            .map(|tx| match tx {
                TransactionVariant::Full(tx) => {
                    let tx_result = inner
                        .tx_results
                        .get(&tx.hash)
                        .expect("freshly executed tx is missing from storage");
                    let output = Some(tx_result.debug.output.clone());
                    let revert_reason = tx_result.debug.revert_reason.clone();
                    DetailedTransaction {
                        inner: tx,
                        output,
                        revert_reason,
                    }
                }
                TransactionVariant::Hash(_) => {
                    unreachable!()
                }
            })
            .collect();
        Ok(block.with_transactions(detailed_txs))
    }

    /// Snapshot the state of the blockchain at the current block. Takes no parameters. Returns the id of the snapshot
    /// that was created. A snapshot can only be reverted once. After a successful evm_revert, the same snapshot id cannot
    /// be used again. Consider creating a new snapshot after each evm_revert if you need to revert to the same
    /// point multiple times.
    ///
    /// # Returns
    /// The `U64` identifier for this snapshot.
    pub fn snapshot(&self) -> Result<U64> {
        let snapshots = self.snapshots.clone();
        self.read_inner().and_then(|writer| {
            // validate max snapshots
            snapshots
                .read()
                .map_err(|err| anyhow!("failed acquiring read lock for snapshot: {:?}", err))
                .and_then(|snapshots| {
                    if snapshots.len() >= MAX_SNAPSHOTS as usize {
                        return Err(anyhow!(
                            "maximum number of '{}' snapshots exceeded",
                            MAX_SNAPSHOTS
                        ));
                    }

                    Ok(())
                })?;

            // snapshot the node
            let snapshot = writer.snapshot().map_err(|err| anyhow!("{}", err))?;
            snapshots
                .write()
                .map(|mut snapshots| {
                    snapshots.push(snapshot);
                    tracing::info!("Created snapshot '{}'", snapshots.len());
                    snapshots.len()
                })
                .map_err(|err| anyhow!("failed storing snapshot: {:?}", err))
                .map(U64::from)
        })
    }

    /// Revert the state of the blockchain to a previous snapshot. Takes a single parameter,
    /// which is the snapshot id to revert to. This deletes the given snapshot, as well as any snapshots
    /// taken after (e.g.: reverting to id 0x1 will delete snapshots with ids 0x1, 0x2, etc.)
    ///
    /// # Parameters
    /// - `snapshot_id`: The snapshot id to revert.
    ///
    /// # Returns
    /// `true` if a snapshot was reverted, otherwise `false`.
    pub fn revert_snapshot(&self, snapshot_id: U64) -> Result<bool> {
        let snapshots = self.snapshots.clone();
        self.write_inner().and_then(|mut writer| {
            let mut snapshots = snapshots
                .write()
                .map_err(|err| anyhow!("failed acquiring read lock for snapshots: {:?}", err))?;
            let snapshot_id_index = snapshot_id.as_usize().saturating_sub(1);
            if snapshot_id_index >= snapshots.len() {
                return Err(anyhow!("no snapshot exists for the id '{}'", snapshot_id));
            }

            // remove all snapshots following the index and use the first snapshot for restore
            let selected_snapshot = snapshots
                .drain(snapshot_id_index..)
                .next()
                .expect("unexpected failure, value must exist");

            tracing::info!("Reverting node to snapshot '{snapshot_id:?}'");
            writer
                .restore_snapshot(selected_snapshot)
                .map(|_| {
                    tracing::info!("Reverting node to snapshot '{snapshot_id:?}'");
                    true
                })
                .map_err(|err| anyhow!("{}", err))
        })
    }

    pub fn set_balance(&self, address: Address, balance: U256) -> Result<bool> {
        self.write_inner().map(|mut writer| {
            let balance_key = storage_key_for_eth_balance(&address);
            writer
                .fork_storage
                .set_value(balance_key, u256_to_h256(balance));
            tracing::info!(
                "👷 Balance for address {:?} has been manually set to {} Wei",
                address,
                balance
            );
            true
        })
    }

    pub fn set_nonce(&self, address: Address, nonce: U256) -> Result<bool> {
        self.write_inner().map(|mut writer| {
            let nonce_key = get_nonce_key(&address);
            let enforced_full_nonce = nonces_to_full_nonce(nonce, nonce);
            tracing::info!(
                "👷 Nonces for address {:?} have been set to {}",
                address,
                nonce
            );
            writer
                .fork_storage
                .set_value(nonce_key, u256_to_h256(enforced_full_nonce));
            true
        })
    }

    pub fn mine_blocks(&self, num_blocks: Option<U64>, interval: Option<U64>) -> Result<()> {
        let num_blocks = num_blocks.map_or(1, |x| x.as_u64());
        let interval_sec = interval.map_or(1, |x| x.as_u64());

        if num_blocks == 0 {
            return Ok(());
        }
        if num_blocks > 1 && interval_sec == 0 {
            anyhow::bail!("Provided interval is `0`; unable to produce {num_blocks} blocks with the same timestamp");
        }

        // TODO: Remove locking once `TestNodeConfig` is refactored into mutable/immutable components
        let max_transactions = self.read_inner()?.config.max_transactions;
        let mut time = self
            .time
            .lock_with_offsets((0..num_blocks).map(|i| i * interval_sec));
        for _ in 0..num_blocks {
            let TxBatch { impersonating, txs } =
                self.pool.take_uniform(max_transactions).unwrap_or(TxBatch {
                    impersonating: false,
                    txs: Vec::new(),
                });
            let base_system_contracts = self
                .system_contracts
                .contracts(TxExecutionMode::VerifyExecute, impersonating)
                .clone();
            self.seal_block(&mut time, txs, base_system_contracts)?;
        }
        tracing::info!("👷 Mined {} blocks", num_blocks);

        Ok(())
    }

    // @dev This function is necessary for Hardhat Ignite compatibility with `evm_emulator`.
    // It always returns `true`, as each new transaction automatically mines a new block by default.
    // Disabling auto mining would require adding functionality to mine blocks with pending transactions.
    // This feature is not yet implemented and should be deferred until `run_l2_tx` and `run_l2_tx_raw` are
    // refactored to handle pending transactions and modularized into smaller functions for maintainability.
    pub fn get_automine(&self) -> Result<bool> {
        Ok(true)
    }

    pub fn reset_network(&self, reset_spec: Option<ResetRequest>) -> Result<bool> {
        let (opt_url, block_number) = if let Some(spec) = reset_spec {
            if let Some(to) = spec.to {
                if spec.forking.is_some() {
                    return Err(anyhow!(
                        "Only one of 'to' and 'forking' attributes can be specified"
                    ));
                }
                let url = match self.get_fork_url() {
                    Ok(url) => url,
                    Err(error) => {
                        tracing::error!("For returning to past local state, mark it with `evm_snapshot`, then revert to it with `evm_revert`.");
                        return Err(anyhow!(error.to_string()));
                    }
                };
                (Some(url), Some(to.as_u64()))
            } else if let Some(forking) = spec.forking {
                let block_number = forking.block_number.map(|n| n.as_u64());
                (Some(forking.json_rpc_url), block_number)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let fork_details = if let Some(url) = opt_url {
            let cache_config = self.get_cache_config().map_err(|err| anyhow!(err))?;
            match ForkDetails::from_url(url, block_number, cache_config) {
                Ok(fd) => Some(fd),
                Err(error) => {
                    return Err(anyhow!(error.to_string()));
                }
            }
        } else {
            None
        };

        match self.reset(fork_details) {
            Ok(()) => {
                tracing::info!("👷 Network reset");
                Ok(true)
            }
            Err(error) => Err(anyhow!(error.to_string())),
        }
    }

    pub fn auto_impersonate_account(&self, enabled: bool) {
        self.impersonation.set_auto_impersonation(enabled);
    }

    pub fn impersonate_account(&self, address: Address) -> Result<bool> {
        if self.impersonation.impersonate(address) {
            tracing::info!("🕵️ Account {:?} has been impersonated", address);
            Ok(true)
        } else {
            tracing::info!("🕵️ Account {:?} was already impersonated", address);
            Ok(false)
        }
    }

    pub fn stop_impersonating_account(&self, address: Address) -> Result<bool> {
        if self.impersonation.stop_impersonating(&address) {
            tracing::info!("🕵️ Stopped impersonating account {:?}", address);
            Ok(true)
        } else {
            tracing::info!(
                "🕵️ Account {:?} was not impersonated, nothing to stop",
                address
            );
            Ok(false)
        }
    }

    pub fn set_code(&self, address: Address, code: String) -> Result<()> {
        self.write_inner().and_then(|mut writer| {
            let code_key = get_code_key(&address);
            tracing::info!("set code for address {address:#x}");
            let code_slice = code
                .strip_prefix("0x")
                .ok_or_else(|| anyhow!("code must be 0x-prefixed"))?;
            let code_bytes = hex::decode(code_slice)?;
            let hashcode = bytecode_to_factory_dep(code_bytes)?;
            let hash = u256_to_h256(hashcode.0);
            let code = hashcode
                .1
                .iter()
                .flat_map(|entry| {
                    let mut bytes = vec![0u8; 32];
                    entry.to_big_endian(&mut bytes);
                    bytes.to_vec()
                })
                .collect();
            writer.fork_storage.store_factory_dep(hash, code);
            writer.fork_storage.set_value(code_key, hash);
            Ok(())
        })
    }

    pub fn set_storage_at(&self, address: Address, slot: U256, value: U256) -> Result<bool> {
        self.write_inner().map(|mut writer| {
            let key = StorageKey::new(AccountTreeId::new(address), u256_to_h256(slot));
            writer.fork_storage.set_value(key, u256_to_h256(value));
            true
        })
    }

    pub fn set_logging_enabled(&self, enable: bool) -> Result<()> {
        let Some(observability) = &self.observability else {
            anyhow::bail!("Node's logging is not set up");
        };
        if enable {
            observability.enable_logging()
        } else {
            observability.disable_logging()
        }
    }

    pub fn get_immediate_sealing(&self) -> Result<bool> {
        Ok(self.sealer.is_immediate())
    }

    pub fn set_immediate_sealing(&self, enable: bool) -> Result<()> {
        if enable {
            self.sealer.set_mode(BlockSealerMode::immediate(
                self.inner
                    .read()
                    .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))?
                    .config
                    .max_transactions,
            ))
        } else {
            self.sealer.set_mode(BlockSealerMode::Noop)
        }
        Ok(())
    }

    pub fn set_interval_sealing(&self, seconds: u64) -> Result<()> {
        let sealing_mode = if seconds == 0 {
            BlockSealerMode::noop()
        } else {
            let block_time = Duration::from_secs(seconds);

            BlockSealerMode::fixed_time(
                self.inner
                    .read()
                    .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))?
                    .config
                    .max_transactions,
                block_time,
            )
        };
        self.sealer.set_mode(sealing_mode);
        Ok(())
    }

    pub fn drop_transaction(&self, hash: H256) -> Result<Option<H256>> {
        Ok(self.pool.drop_transaction(hash).map(|tx| tx.hash()))
    }

    pub fn drop_all_transactions(&self) -> Result<()> {
        self.pool.clear();
        Ok(())
    }

    pub fn remove_pool_transactions(&self, address: Address) -> Result<()> {
        self.pool.drop_transactions_by_sender(address);
        Ok(())
    }

    pub fn set_next_block_base_fee_per_gas(&self, base_fee: U256) -> Result<()> {
        self.inner
            .write()
            .expect("")
            .fee_input_provider
            .set_base_fee(base_fee.as_u64());
        Ok(())
    }

    pub fn set_rpc_url(&self, url: String) -> Result<()> {
        let inner = self
            .inner
            .read()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))?;
        let mut fork_storage = inner
            .fork_storage
            .inner
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))?;
        if let Some(fork) = &mut fork_storage.fork {
            let old_url = fork.fork_source.get_fork_url().map_err(|e| {
                anyhow::anyhow!(
                    "failed to resolve current fork's RPC URL: {}",
                    e.to_string()
                )
            })?;
            fork.set_rpc_url(url.clone());
            tracing::info!("Updated fork rpc from \"{}\" to \"{}\"", old_url, url);
        } else {
            tracing::info!("Non-forking node tried to switch RPC URL to '{url}'. Call `anvil_reset` instead if you wish to switch to forking mode");
        }
        Ok(())
    }

    pub fn set_chain_id(&self, id: u32) -> Result<()> {
        let mut inner =  self.inner
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;

        inner.config.update_chain_id(Some(id));
        inner.fork_storage.set_chain_id(id.into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::ForkStorage;
    use crate::namespaces::EthNamespaceT;
    use crate::node::time::{ReadTime, TimestampManager};
    use crate::node::{BlockSealer, ImpersonationManager, InMemoryNodeInner, Snapshot, TxPool};
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};
    use std::str::FromStr;
    use std::sync::{Arc, RwLock};
    use zksync_multivm::interface::storage::ReadStorage;
    use zksync_types::{api::BlockNumber, fee::Fee, l2::L2Tx, PackedEthSignature};
    use zksync_types::{Nonce, H256};
    use zksync_utils::h256_to_u256;

    #[tokio::test]
    async fn test_set_balance() {
        let address = Address::from_str("0x36615Cf349d7F6344891B1e7CA7C72883F5dc049").unwrap();
        let node = InMemoryNode::<HttpForkSource>::default();

        let balance_before = node.get_balance(address, None).await.unwrap();

        let result = node.set_balance(address, U256::from(1337)).unwrap();
        assert!(result);

        let balance_after = node.get_balance(address, None).await.unwrap();
        assert_eq!(balance_after, U256::from(1337));
        assert_ne!(balance_before, balance_after);
    }

    #[tokio::test]
    async fn test_set_nonce() {
        let address = Address::from_str("0x36615Cf349d7F6344891B1e7CA7C72883F5dc049").unwrap();
        let node = InMemoryNode::<HttpForkSource>::default();

        let nonce_before = node.get_transaction_count(address, None).await.unwrap();

        let result = node.set_nonce(address, U256::from(1337)).unwrap();
        assert!(result);

        let nonce_after = node.get_transaction_count(address, None).await.unwrap();
        assert_eq!(nonce_after, U256::from(1337));
        assert_ne!(nonce_before, nonce_after);

        let result = node.set_nonce(address, U256::from(1336)).unwrap();
        assert!(result);

        let nonce_after = node.get_transaction_count(address, None).await.unwrap();
        assert_eq!(nonce_after, U256::from(1336));
    }

    #[tokio::test]
    async fn test_mine_blocks_default() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        // test with defaults
        node.mine_blocks(None, None).expect("mine_blocks");

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 1, current_block.number);
        assert_eq!(start_block.timestamp + 1, current_block.timestamp);
        node.mine_blocks(None, None).expect("mine_blocks");

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 2, current_block.number);
        assert_eq!(start_block.timestamp + 2, current_block.timestamp);
    }

    #[tokio::test]
    async fn test_mine_blocks() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        let num_blocks = 5;
        let interval = 3;
        let start_timestamp = start_block.timestamp + 1;

        node.mine_blocks(Some(U64::from(num_blocks)), Some(U64::from(interval)))
            .expect("mine blocks");

        for i in 0..num_blocks {
            let current_block = node
                .get_block_by_number(BlockNumber::Number(start_block.number + i + 1), false)
                .await
                .unwrap()
                .expect("block exists");
            assert_eq!(start_block.number + i + 1, current_block.number);
            assert_eq!(start_timestamp + i * interval, current_block.timestamp);
        }
    }

    #[tokio::test]
    async fn test_reset() {
        let old_snapshots = Arc::new(RwLock::new(vec![Snapshot::default()]));
        let old_system_contracts_options = Default::default();
        let time = TimestampManager::new(123);
        let impersonation = ImpersonationManager::default();
        let old_inner = InMemoryNodeInner::<HttpForkSource> {
            current_batch: 100,
            current_miniblock: 300,
            current_miniblock_hash: H256::random(),
            fee_input_provider: Default::default(),
            tx_results: Default::default(),
            blocks: Default::default(),
            block_hashes: Default::default(),
            filters: Default::default(),
            fork_storage: ForkStorage::new(None, &old_system_contracts_options, false, None),
            config: Default::default(),
            console_log_handler: Default::default(),
            system_contracts: Default::default(),
            impersonation: impersonation.clone(),
            rich_accounts: Default::default(),
            previous_states: Default::default(),
        };
        let pool = TxPool::new(impersonation.clone());

        let node = InMemoryNode::<HttpForkSource> {
            inner: Arc::new(RwLock::new(old_inner)),
            snapshots: old_snapshots,
            system_contracts_options: old_system_contracts_options,
            time,
            impersonation,
            observability: None,
            pool,
            sealer: BlockSealer::default(),
            system_contracts: Default::default(),
        };

        let address = Address::from_str("0x36615Cf349d7F6344891B1e7CA7C72883F5dc049").unwrap();
        let nonce_before = node.get_transaction_count(address, None).await.unwrap();

        let set_result = node.set_nonce(address, U256::from(1337)).unwrap();
        assert!(set_result);

        let reset_result = node.reset_network(None).unwrap();
        assert!(reset_result);

        let nonce_after = node.get_transaction_count(address, None).await.unwrap();
        assert_eq!(nonce_before, nonce_after);

        assert_eq!(node.snapshots.read().unwrap().len(), 0);

        let inner = node.inner.read().unwrap();
        assert_eq!(node.time.current_timestamp(), 1000);
        assert_eq!(inner.current_batch, 0);
        assert_eq!(inner.current_miniblock, 0);
        assert_ne!(inner.current_miniblock_hash, H256::random());
    }

    #[tokio::test]
    async fn test_impersonate_account() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let to_impersonate =
            Address::from_str("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

        // give impersonated account some balance
        let result = node.set_balance(to_impersonate, U256::exp10(18)).unwrap();
        assert!(result);

        // construct a tx
        let mut tx = L2Tx::new(
            Some(Address::random()),
            vec![],
            Nonce(0),
            Fee {
                gas_limit: U256::from(100_000_000),
                max_fee_per_gas: U256::from(50_000_000),
                max_priority_fee_per_gas: U256::from(50_000_000),
                gas_per_pubdata_limit: U256::from(50000),
            },
            to_impersonate,
            U256::one(),
            vec![],
            Default::default(),
        );
        tx.set_input(vec![], H256::random());
        if tx.common_data.signature.is_empty() {
            tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        // try to execute the tx- should fail without signature
        assert!(node.apply_txs(vec![tx.clone()], 1).is_err());

        // impersonate the account
        let result = node
            .impersonate_account(to_impersonate)
            .expect("impersonate_account");

        // result should be true
        assert!(result);

        // impersonating the same account again should return false
        let result = node
            .impersonate_account(to_impersonate)
            .expect("impersonate_account");
        assert!(!result);

        // execution should now succeed
        assert!(node.apply_txs(vec![tx.clone()], 1).is_ok());

        // stop impersonating the account
        let result = node
            .stop_impersonating_account(to_impersonate)
            .expect("stop_impersonating_account");

        // result should be true
        assert!(result);

        // stop impersonating the same account again should return false
        let result = node
            .stop_impersonating_account(to_impersonate)
            .expect("stop_impersonating_account");
        assert!(!result);

        // execution should now fail again
        assert!(node.apply_txs(vec![tx], 1).is_err());
    }

    #[tokio::test]
    async fn test_set_code() {
        let address = Address::repeat_byte(0x1);
        let node = InMemoryNode::<HttpForkSource>::default();
        let new_code = vec![0x1u8; 32];

        let code_before = node
            .get_code(address, None)
            .await
            .expect("failed getting code")
            .0;
        assert_eq!(Vec::<u8>::default(), code_before);

        node.set_code(address, format!("0x{}", hex::encode(new_code.clone())))
            .expect("failed setting code");

        let code_after = node
            .get_code(address, None)
            .await
            .expect("failed getting code")
            .0;
        assert_eq!(new_code, code_after);
    }

    #[tokio::test]
    async fn test_set_storage_at() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let address = Address::repeat_byte(0x1);
        let slot = U256::from(37);
        let value = U256::from(42);

        let key = StorageKey::new(AccountTreeId::new(address), u256_to_h256(slot));
        let value_before = node.write_inner().unwrap().fork_storage.read_value(&key);
        assert_eq!(H256::default(), value_before);

        let result = node
            .set_storage_at(address, slot, value)
            .expect("failed setting value");
        assert!(result);

        let value_after = node.write_inner().unwrap().fork_storage.read_value(&key);
        assert_eq!(value, h256_to_u256(value_after));
    }

    #[tokio::test]
    async fn test_increase_time_zero_value() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let increase_value_seconds = 0u64;
        let timestamp_before = node.time.current_timestamp();
        let expected_response = increase_value_seconds;

        let actual_response = node
            .increase_time(increase_value_seconds.into())
            .expect("failed increasing timestamp");
        let timestamp_after = node.time.current_timestamp();

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            increase_value_seconds.saturating_mul(1000u64),
            timestamp_after.saturating_sub(timestamp_before),
            "timestamp did not increase by the specified amount",
        );
    }

    #[tokio::test]
    async fn test_increase_time_max_value() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let increase_value_seconds = u64::MAX;
        let timestamp_before = node.time.current_timestamp();
        assert_ne!(0, timestamp_before, "initial timestamp must be non zero",);
        let expected_response = increase_value_seconds;

        let actual_response = node
            .increase_time(increase_value_seconds.into())
            .expect("failed increasing timestamp");
        let timestamp_after = node.time.current_timestamp();

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            u64::MAX,
            timestamp_after,
            "timestamp did not saturate upon increase",
        );
    }

    #[tokio::test]
    async fn test_increase_time() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let increase_value_seconds = 100u64;
        let timestamp_before = node.time.current_timestamp();
        let expected_response = increase_value_seconds;

        let actual_response = node
            .increase_time(increase_value_seconds.into())
            .expect("failed increasing timestamp");
        let timestamp_after = node.time.current_timestamp();

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            increase_value_seconds,
            timestamp_after.saturating_sub(timestamp_before),
            "timestamp did not increase by the specified amount",
        );
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_future() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_timestamp = 10_000u64;
        let timestamp_before = node.time.current_timestamp();
        assert_ne!(
            timestamp_before, new_timestamp,
            "timestamps must be different"
        );

        node.set_next_block_timestamp(new_timestamp.into())
            .expect("failed setting timestamp");
        node.mine_block().expect("failed to mine a block");
        let timestamp_after = node.time.current_timestamp();

        assert_eq!(
            new_timestamp, timestamp_after,
            "timestamp was not set correctly",
        );
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_past_fails() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let timestamp_before = node.time.current_timestamp();

        let new_timestamp = timestamp_before + 500;
        node.set_next_block_timestamp(new_timestamp.into())
            .expect("failed setting timestamp");

        node.mine_block().expect("failed to mine a block");

        let result = node.set_next_block_timestamp(timestamp_before.into());

        assert!(result.is_err(), "expected an error for timestamp in past");
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_same_value() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_timestamp = 1000u64;
        let timestamp_before = node.time.current_timestamp();
        assert_eq!(timestamp_before, new_timestamp, "timestamps must be same");

        let response = node.set_next_block_timestamp(new_timestamp.into());
        assert!(response.is_err());

        let timestamp_after = node.time.current_timestamp();
        assert_eq!(
            timestamp_before, timestamp_after,
            "timestamp must not change",
        );
    }

    #[tokio::test]
    async fn test_set_time_future() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_time = 10_000u64;
        let timestamp_before = node.time.current_timestamp();
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = 9000;

        let actual_response = node
            .set_time(new_time.into())
            .expect("failed setting timestamp");
        let timestamp_after = node.time.current_timestamp();

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(new_time, timestamp_after, "timestamp was not set correctly",);
    }

    #[tokio::test]
    async fn test_set_time_past() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_time = 10u64;
        let timestamp_before = node.time.current_timestamp();
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = -990;

        let actual_response = node
            .set_time(new_time.into())
            .expect("failed setting timestamp");
        let timestamp_after = node.time.current_timestamp();

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(new_time, timestamp_after, "timestamp was not set correctly",);
    }

    #[tokio::test]
    async fn test_set_time_same_value() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_time = 1000u64;
        let timestamp_before = node.time.current_timestamp();
        assert_eq!(timestamp_before, new_time, "timestamps must be same");
        let expected_response = 0;

        let actual_response = node
            .set_time(new_time.into())
            .expect("failed setting timestamp");
        let timestamp_after = node.time.current_timestamp();

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            timestamp_before, timestamp_after,
            "timestamp must not change",
        );
    }

    #[tokio::test]
    async fn test_set_time_edges() {
        let node = InMemoryNode::<HttpForkSource>::default();

        for new_time in [0, u64::MAX] {
            let timestamp_before = node.time.current_timestamp();
            assert_ne!(
                timestamp_before, new_time,
                "case {new_time}: timestamps must be different"
            );
            let expected_response = (new_time as i128).saturating_sub(timestamp_before as i128);

            let actual_response = node
                .set_time(new_time.into())
                .expect("failed setting timestamp");
            let timestamp_after = node.time.current_timestamp();

            assert_eq!(
                expected_response, actual_response,
                "case {new_time}: erroneous response"
            );
            assert_eq!(
                new_time, timestamp_after,
                "case {new_time}: timestamp was not set correctly",
            );
        }
    }

    #[tokio::test]
    async fn test_mine_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");
        let result = node.mine_block().expect("mine_block");
        assert_eq!(result, L2BlockNumber(1));

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 1, current_block.number);
        assert_eq!(start_block.timestamp + 1, current_block.timestamp);

        let result = node.mine_block().expect("mine_block");
        assert_eq!(result, L2BlockNumber(start_block.number.as_u32() + 2));

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 2, current_block.number);
        assert_eq!(start_block.timestamp + 2, current_block.timestamp);
    }

    #[tokio::test]
    async fn test_evm_snapshot_creates_incrementing_ids() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let snapshot_id_1 = node.snapshot().expect("failed creating snapshot 1");
        let snapshot_id_2 = node.snapshot().expect("failed creating snapshot 2");

        assert_eq!(snapshot_id_1, U64::from(1));
        assert_eq!(snapshot_id_2, U64::from(2));
    }

    #[tokio::test]
    async fn test_evm_revert_snapshot_restores_state() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let initial_block = node
            .get_block_number()
            .await
            .expect("failed fetching block number");
        let snapshot_id = node.snapshot().expect("failed creating snapshot");
        node.mine_block().expect("mine_block");
        let current_block = node
            .get_block_number()
            .await
            .expect("failed fetching block number");
        assert_eq!(current_block, initial_block + 1);

        let reverted = node
            .revert_snapshot(snapshot_id)
            .expect("failed reverting snapshot");
        assert!(reverted);

        let restored_block = node
            .get_block_number()
            .await
            .expect("failed fetching block number");
        assert_eq!(restored_block, initial_block);
    }

    #[tokio::test]
    async fn test_evm_revert_snapshot_removes_all_snapshots_following_the_reverted_one() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let _snapshot_id_1 = node.snapshot().expect("failed creating snapshot");
        let snapshot_id_2 = node.snapshot().expect("failed creating snapshot");
        let _snapshot_id_3 = node.snapshot().expect("failed creating snapshot");
        assert_eq!(3, node.snapshots.read().unwrap().len());

        let reverted = node
            .revert_snapshot(snapshot_id_2)
            .expect("failed reverting snapshot");
        assert!(reverted);

        assert_eq!(1, node.snapshots.read().unwrap().len());
    }

    #[tokio::test]
    async fn test_evm_revert_snapshot_fails_for_invalid_snapshot_id() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let result = node.revert_snapshot(U64::from(100));
        assert!(result.is_err());
    }
}
