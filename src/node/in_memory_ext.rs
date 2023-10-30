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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};
    use std::str::FromStr;
    use zksync_basic_types::{Nonce, H256};
    use zksync_core::api_server::web3::backend_jsonrpc::namespaces::eth::EthNamespaceT;
    use zksync_types::{api::BlockNumber, fee::Fee, l2::L2Tx, PackedEthSignature};

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

        // setting nonce lower than the current one should fail
        let result = node.set_nonce(address, U256::from(1336));
        assert!(result.is_err());
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
        let result = node.mine_blocks(None, None).expect("mine_blocks");
        assert!(result);

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 1, current_block.number);
        assert_eq!(start_block.timestamp + 1, current_block.timestamp);
        let result = node.mine_blocks(None, None).expect("mine_blocks");
        assert!(result);

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

        let result = node
            .mine_blocks(Some(U64::from(num_blocks)), Some(U64::from(interval)))
            .expect("mine blocks");
        assert!(result);

        for i in 0..num_blocks {
            let current_block = node
                .get_block_by_number(BlockNumber::Number(start_block.number + i + 1), false)
                .await
                .unwrap()
                .expect("block exists");
            assert_eq!(start_block.number + i + 1, current_block.number);
            assert_eq!(
                start_timestamp + i * interval * 1_000,
                current_block.timestamp
            );
        }
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
            Address::random(),
            vec![],
            Nonce(0),
            Fee {
                gas_limit: U256::from(1_000_000),
                max_fee_per_gas: U256::from(250_000_000),
                max_priority_fee_per_gas: U256::from(250_000_000),
                gas_per_pubdata_limit: U256::from(20000),
            },
            to_impersonate,
            U256::one(),
            None,
            Default::default(),
        );
        tx.set_input(vec![], H256::random());
        if tx.common_data.signature.is_empty() {
            tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        // try to execute the tx- should fail without signature
        assert!(node.apply_txs(vec![tx.clone()]).is_err());

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
        assert!(node.apply_txs(vec![tx.clone()]).is_ok());

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
        assert!(node.apply_txs(vec![tx]).is_err());
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

        node.set_code(address, new_code.clone())
            .expect("failed setting code");

        let code_after = node
            .get_code(address, None)
            .await
            .expect("failed getting code")
            .0;
        assert_eq!(new_code, code_after);
    }

    #[tokio::test]
    async fn test_increase_time_zero_value() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let increase_value_seconds = 0u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        let expected_response = increase_value_seconds;

        let actual_response = node
            .increase_time(increase_value_seconds)
            .expect("failed increasing timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

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
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(0, timestamp_before, "initial timestamp must be non zero",);
        let expected_response = increase_value_seconds;

        let actual_response = node
            .increase_time(increase_value_seconds)
            .expect("failed increasing timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

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
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        let expected_response = increase_value_seconds;

        let actual_response = node
            .increase_time(increase_value_seconds)
            .expect("failed increasing timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            increase_value_seconds.saturating_mul(1000u64),
            timestamp_after.saturating_sub(timestamp_before),
            "timestamp did not increase by the specified amount",
        );
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_future() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_timestamp = 10_000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(
            timestamp_before, new_timestamp,
            "timestamps must be different"
        );
        let expected_response = new_timestamp;

        let actual_response = node
            .set_next_block_timestamp(new_timestamp)
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            new_timestamp, timestamp_after,
            "timestamp was not set correctly",
        );
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_past_fails() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        let new_timestamp = timestamp_before + 500;
        node.set_next_block_timestamp(new_timestamp)
            .expect("failed setting timestamp");

        let result = node.set_next_block_timestamp(timestamp_before);

        assert!(result.is_err(), "expected an error for timestamp in past");
    }

    #[tokio::test]
    async fn test_set_next_block_timestamp_same_value() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_timestamp = 1000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_eq!(timestamp_before, new_timestamp, "timestamps must be same");
        let expected_response = new_timestamp;

        let actual_response = node
            .set_next_block_timestamp(new_timestamp)
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            timestamp_before, timestamp_after,
            "timestamp must not change",
        );
    }

    #[tokio::test]
    async fn test_set_time_future() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_time = 10_000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = 9000;

        let actual_response = node.set_time(new_time).expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(new_time, timestamp_after, "timestamp was not set correctly",);
    }

    #[tokio::test]
    async fn test_set_time_past() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_time = 10u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = -990;

        let actual_response = node.set_time(new_time).expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(new_time, timestamp_after, "timestamp was not set correctly",);
    }

    #[tokio::test]
    async fn test_set_time_same_value() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let new_time = 1000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_eq!(timestamp_before, new_time, "timestamps must be same");
        let expected_response = 0;

        let actual_response = node.set_time(new_time).expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

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
            let timestamp_before = node
                .get_inner()
                .read()
                .map(|inner| inner.current_timestamp)
                .unwrap_or_else(|_| panic!("case {}: failed reading timestamp", new_time));
            assert_ne!(
                timestamp_before, new_time,
                "case {new_time}: timestamps must be different"
            );
            let expected_response = (new_time as i128).saturating_sub(timestamp_before as i128);

            let actual_response = node.set_time(new_time).expect("failed setting timestamp");
            let timestamp_after = node
                .get_inner()
                .read()
                .map(|inner| inner.current_timestamp)
                .unwrap_or_else(|_| panic!("case {}: failed reading timestamp", new_time));

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
        assert_eq!(&result, "0x0");

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 1, current_block.number);
        assert_eq!(start_block.timestamp + 1, current_block.timestamp);

        let result = node.mine_block().expect("mine_block");
        assert_eq!(&result, "0x0");

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
