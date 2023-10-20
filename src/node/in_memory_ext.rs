use anyhow::anyhow;
use zksync_basic_types::{Address, U256, U64};
use zksync_state::ReadStorage;
use zksync_types::{
    get_code_key, get_nonce_key,
    utils::{decompose_full_nonce, nonces_to_full_nonce, storage_key_for_eth_balance},
};
use zksync_utils::{h256_to_u256, u256_to_h256};

use crate::{
    fork::ForkSource,
    node::InMemoryNode,
    utils::{self, bytecode_to_factory_dep},
};

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
    pub fn increase_time(&self, time_delta_seconds: u64) -> Result<u64> {
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .map(|mut writer| {
                if time_delta_seconds == 0 {
                    return time_delta_seconds;
                }

                let time_delta = time_delta_seconds.saturating_mul(1000);
                writer.current_timestamp = writer.current_timestamp.saturating_add(time_delta);
                time_delta_seconds
            })
    }

    /// Set the current timestamp for the node. The timestamp must be in future.
    ///
    /// # Parameters
    /// - `timestamp`: The timestamp to set the time to
    ///
    /// # Returns
    /// The new timestamp value for the InMemoryNodeInner.
    pub fn set_next_block_timestamp(&self, timestamp: u64) -> Result<u64> {
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .and_then(|mut writer| {
                if timestamp < writer.current_timestamp {
                    Err(anyhow!(
                        "timestamp ({}) must be greater than current timestamp ({})",
                        timestamp,
                        writer.current_timestamp
                    ))
                } else {
                    writer.current_timestamp = timestamp;
                    Ok(timestamp)
                }
            })
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
    pub fn set_time(&self, time: u64) -> Result<i128> {
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .map(|mut writer| {
                let time_diff = (time as i128).saturating_sub(writer.current_timestamp as i128);
                writer.current_timestamp = time;
                time_diff
            })
    }

    /// Force a single block to be mined.
    ///
    /// Will mine an empty block (containing zero transactions)
    ///
    /// # Returns
    /// The string "0x0".
    pub fn mine_block(&self) -> Result<String> {
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .map(|mut writer| {
                utils::mine_empty_blocks(&mut writer, 1, 1000);
                tracing::info!("👷 Mined block #{}", writer.current_miniblock);
                "0x0".to_string()
            })
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
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .and_then(|writer| {
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
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .and_then(|mut writer| {
                let mut snapshots = snapshots.write().map_err(|err| {
                    anyhow!("failed acquiring read lock for snapshots: {:?}", err)
                })?;
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
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .map(|mut writer| {
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
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .and_then(|mut writer| {
                let nonce_key = get_nonce_key(&address);
                let full_nonce = writer.fork_storage.read_value(&nonce_key);
                let (mut account_nonce, mut deployment_nonce) =
                    decompose_full_nonce(h256_to_u256(full_nonce));
                if account_nonce >= nonce {
                    return Err(anyhow!(
                        "Account Nonce is already set to a higher value ({}, requested {})",
                        account_nonce,
                        nonce
                    ));
                }
                account_nonce = nonce;
                if deployment_nonce >= nonce {
                    return Err(anyhow!(
                        "Deployment Nonce is already set to a higher value ({}, requested {})",
                        deployment_nonce,
                        nonce
                    ));
                }
                deployment_nonce = nonce;
                let enforced_full_nonce = nonces_to_full_nonce(account_nonce, deployment_nonce);
                tracing::info!(
                    "👷 Nonces for address {:?} have been set to {}",
                    address,
                    nonce
                );
                writer
                    .fork_storage
                    .set_value(nonce_key, u256_to_h256(enforced_full_nonce));
                Ok(true)
            })
    }

    pub fn mine_blocks(&self, num_blocks: Option<U64>, interval: Option<U64>) -> Result<bool> {
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .and_then(|mut writer| {
                let num_blocks = num_blocks.unwrap_or_else(|| U64::from(1));
                let interval_ms = interval
                    .unwrap_or_else(|| U64::from(1))
                    .saturating_mul(1_000.into());
                if num_blocks.is_zero() {
                    return Err(anyhow!(
                        "Number of blocks must be greater than 0".to_string(),
                    ));
                }
                utils::mine_empty_blocks(&mut writer, num_blocks.as_u64(), interval_ms.as_u64());
                tracing::info!("👷 Mined {} blocks", num_blocks);

                Ok(true)
            })
    }

    pub fn impersonate_account(&self, address: Address) -> Result<bool> {
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .map(|mut writer| {
                if writer.set_impersonated_account(address) {
                    tracing::info!("🕵️ Account {:?} has been impersonated", address);
                    true
                } else {
                    tracing::info!("🕵️ Account {:?} was already impersonated", address);
                    false
                }
            })
    }

    pub fn stop_impersonating_account(&self, address: Address) -> Result<bool> {
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .map(|mut writer| {
                if writer.stop_impersonating_account(address) {
                    tracing::info!("🕵️ Stopped impersonating account {:?}", address);
                    true
                } else {
                    tracing::info!(
                        "🕵️ Account {:?} was not impersonated, nothing to stop",
                        address
                    );
                    false
                }
            })
    }

    pub fn set_code(&self, address: Address, code: Vec<u8>) -> Result<()> {
        self.get_inner()
            .write()
            .map_err(|err| anyhow!("failed acquiring lock: {:?}", err))
            .map(|mut writer| {
                let code_key = get_code_key(&address);
                tracing::info!("set code for address {address:#x}");
                let (hash, code) = bytecode_to_factory_dep(code);
                let hash = u256_to_h256(hash);
                writer.fork_storage.store_factory_dep(
                    hash,
                    code.iter()
                        .flat_map(|entry| {
                            let mut bytes = vec![0u8; 32];
                            entry.to_big_endian(&mut bytes);
                            bytes.to_vec()
                        })
                        .collect(),
                );
                writer.fork_storage.set_value(code_key, hash);
            })
    }
}
