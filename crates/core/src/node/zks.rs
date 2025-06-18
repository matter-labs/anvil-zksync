use crate::node::InMemoryNode;
use anyhow::Context;
use std::collections::HashMap;
use zksync_error::anvil_zksync::node::AnvilNodeResult;
use zksync_mini_merkle_tree::MiniMerkleTree;
use zksync_types::api;
use zksync_types::fee::Fee;
use zksync_types::hasher::keccak::KeccakHasher;
use zksync_types::hasher::Hasher;
use zksync_types::l2_to_l1_log::{
    l2_to_l1_logs_tree_size, L2ToL1Log, LOG_PROOF_SUPPORTED_METADATA_VERSION,
};
use zksync_types::transaction_request::CallRequest;
use zksync_types::utils::storage_key_for_standard_token_balance;
use zksync_types::{h256_to_u256, L1BatchNumber};
use zksync_types::{
    AccountTreeId, Address, L2BlockNumber, Transaction, H160, H256, L2_BASE_TOKEN_ADDRESS, U256,
};
use zksync_web3_decl::error::Web3Error;

impl InMemoryNode {
    pub async fn estimate_fee_impl(&self, req: CallRequest) -> AnvilNodeResult<Fee> {
        self.inner.read().await.estimate_gas_impl(req).await
    }

    pub async fn estimate_gas_l1_to_l2(&self, req: CallRequest) -> AnvilNodeResult<U256> {
        // self.inner
        //     .read()
        //     .await
        //     .estimate_l1_to_l2_gas_impl(req)
        //     .await
        Ok(U256::from(500_000))
    }

    pub async fn get_raw_block_transactions_impl(
        &self,
        block_number: L2BlockNumber,
    ) -> Result<Vec<Transaction>, Web3Error> {
        let tx_hashes = self
            .blockchain
            .get_block_tx_hashes_by_number(block_number)
            .await;
        if let Some(tx_hashes) = tx_hashes {
            let mut transactions = Vec::with_capacity(tx_hashes.len());
            for tx_hash in tx_hashes {
                let transaction = self
                    .blockchain
                    .get_zksync_tx(&tx_hash)
                    .await
                    .with_context(|| anyhow::anyhow!("Unexpectedly transaction (hash={tx_hash}) belongs to a block but could not be found"))?;
                transactions.push(transaction);
            }
            Ok(transactions)
        } else {
            Ok(self.fork.get_raw_block_transactions(block_number).await?)
        }
    }

    pub async fn get_bridge_contracts_impl(&self) -> Result<api::BridgeAddresses, Web3Error> {
        Ok(self
            .fork
            .get_bridge_contracts()
            .await?
            .unwrap_or(api::BridgeAddresses {
                l1_shared_default_bridge: Default::default(),
                l2_shared_default_bridge: Default::default(),
                l1_erc20_default_bridge: Default::default(),
                l2_erc20_default_bridge: Default::default(),
                l1_weth_bridge: Default::default(),
                l2_weth_bridge: Default::default(),
                l2_legacy_shared_bridge: Default::default(),
            }))
    }

    pub async fn get_confirmed_tokens_impl(
        &self,
        from: u32,
        limit: u8,
    ) -> anyhow::Result<Vec<zksync_web3_decl::types::Token>> {
        Ok(self
            .fork
            .get_confirmed_tokens(from, limit)
            .await?
            .unwrap_or(vec![zksync_web3_decl::types::Token {
                l1_address: Address::zero(),
                l2_address: L2_BASE_TOKEN_ADDRESS,
                name: "Ether".to_string(),
                symbol: "ETH".to_string(),
                decimals: 18,
            }]))
    }

    pub async fn get_all_account_balances_impl(
        &self,
        address: Address,
    ) -> Result<HashMap<Address, U256>, Web3Error> {
        let tokens = self.get_confirmed_tokens_impl(0, 100).await?;

        let balances = {
            let mut balances = HashMap::new();
            for token in tokens {
                // TODO: Use StorageKeyLayout once boojumos can lookup other tokens
                let balance_key = storage_key_for_standard_token_balance(
                    AccountTreeId::new(token.l2_address),
                    &address,
                );
                let balance = self.storage.read_value_alt(&balance_key).await?;
                if !balance.is_zero() {
                    balances.insert(token.l2_address, h256_to_u256(balance));
                }
            }
            balances
        };

        Ok(balances)
    }

    pub async fn get_block_details_impl(
        &self,
        block_number: L2BlockNumber,
    ) -> anyhow::Result<Option<api::BlockDetails>> {
        let base_system_contracts_hashes = self.system_contracts.base_system_contracts_hashes();
        let reader = self.inner.read().await;
        let l2_fair_gas_price = reader.fee_input_provider.gas_price();
        let fair_pubdata_price = Some(reader.fee_input_provider.fair_pubdata_price());
        drop(reader);

        let block_details = self
            .blockchain
            .get_block_details_by_number(
                block_number,
                l2_fair_gas_price,
                fair_pubdata_price,
                base_system_contracts_hashes,
            )
            .await;

        match block_details {
            Some(block_details) => Ok(Some(block_details)),
            None => self.fork.get_block_details(block_number).await,
        }
    }

    pub async fn get_transaction_details_impl(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<api::TransactionDetails>> {
        match self.blockchain.get_tx_details(&hash).await {
            Some(tx_details) => Ok(Some(tx_details)),
            None => self.fork.get_transaction_details(hash).await,
        }
    }

    pub async fn get_bytecode_by_hash_impl(&self, hash: H256) -> anyhow::Result<Option<Vec<u8>>> {
        if let Some(bytecode) = self.storage.load_factory_dep_alt(hash).await? {
            return Ok(Some(bytecode));
        }

        self.fork.get_bytecode_by_hash(hash).await
    }

    pub async fn get_base_token_l1_address_impl(&self) -> anyhow::Result<Address> {
        Ok(H160::from_low_u64_be(1))
    }

    pub async fn get_l2_to_l1_log_proof_impl(
        &self,
        tx_hash: H256,
        index: Option<usize>,
    ) -> anyhow::Result<Option<api::L2ToL1LogProof>> {
        let Some(tx_receipt) = self.blockchain.get_tx_receipt(&tx_hash).await else {
            return Ok(None);
        };
        let l1_batch_number = L1BatchNumber(tx_receipt.l1_batch_number.expect("").as_u32());
        let Some(l1_batch) = self.blockchain.get_batch_header(l1_batch_number).await else {
            return Ok(None);
        };
        let all_l1_logs_in_batch = l1_batch
            .l2_to_l1_logs
            .into_iter()
            .map(|log| log.0)
            .collect::<Vec<_>>();
        let l1_batch_tx_index = tx_receipt.l1_batch_tx_index.expect("").as_u32() as u16;
        let log_filter = |log: &L2ToL1Log| log.tx_number_in_block == l1_batch_tx_index;
        let index_in_filtered_logs = index.unwrap_or(0);

        // Copied from zksync-era
        let Some((l1_log_index, _)) = all_l1_logs_in_batch
            .iter()
            .enumerate()
            .filter(|(_, log)| log_filter(log))
            .nth(index_in_filtered_logs)
        else {
            return Ok(None);
        };
        let merkle_tree_leaves = all_l1_logs_in_batch.iter().map(L2ToL1Log::to_bytes);
        let tree_size = l2_to_l1_logs_tree_size(self.blockchain.protocol_version());

        let (local_root, proof) = MiniMerkleTree::new(merkle_tree_leaves, Some(tree_size))
            .merkle_root_and_path(l1_log_index);
        let Some(aggregated_root) = self
            .blockchain
            .get_batch_aggregation_root(l1_batch_number)
            .await
        else {
            return Ok(None);
        };
        let root = KeccakHasher.compress(&local_root, &aggregated_root);

        let mut log_leaf_proof = proof;
        log_leaf_proof.push(aggregated_root);

        let (batch_proof_len, batch_chain_proof, is_final_node) = (0, Vec::<H256>::new(), true);

        let proof = {
            let mut metadata = [0u8; 32];
            metadata[0] = LOG_PROOF_SUPPORTED_METADATA_VERSION;
            metadata[1] = log_leaf_proof.len() as u8;
            metadata[2] = batch_proof_len as u8;
            metadata[3] = if is_final_node { 1 } else { 0 };

            let mut result = vec![H256(metadata)];

            result.extend(log_leaf_proof);
            result.extend(batch_chain_proof);

            result
        };

        Ok(Some(api::L2ToL1LogProof {
            proof,
            root,
            id: l1_log_index as u32,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use zksync_types::{
        api, transaction_request::CallRequest, Address, ProtocolVersionId, H160, H256,
    };
    use zksync_types::{u256_to_h256, L1BatchNumber};

    use super::*;
    use crate::node::fork::{ForkClient, ForkConfig};
    use crate::node::TransactionResult;
    use crate::{
        node::InMemoryNode,
        testing,
        testing::{ForkBlockConfig, MockServer},
    };

    #[tokio::test]
    async fn test_estimate_fee() {
        let node = InMemoryNode::test(None);

        let mock_request = CallRequest {
            from: Some(
                "0xa61464658afeaf65cccaafd3a512b69a83b77618"
                    .parse()
                    .unwrap(),
            ),
            to: Some(
                "0x36615cf349d7f6344891b1e7ca7c72883f5dc049"
                    .parse()
                    .unwrap(),
            ),
            gas: Some(U256::from(0)),
            gas_price: Some(U256::from(0)),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            value: Some(U256::from(0)),
            data: Some(vec![0, 0].into()),
            nonce: Some(U256::from(0)),
            transaction_type: None,
            access_list: None,
            eip712_meta: None,
            input: None,
        };

        let result = node.estimate_fee_impl(mock_request).await.unwrap();

        assert_eq!(result.gas_limit, U256::from(413091));
        assert_eq!(result.max_fee_per_gas, U256::from(45250000));
        assert_eq!(result.max_priority_fee_per_gas, U256::from(0));
        assert_eq!(result.gas_per_pubdata_limit, U256::from(3143));
    }

    #[tokio::test]
    async fn test_get_transaction_details_local() {
        // Arrange
        let node = InMemoryNode::test(None);
        {
            let mut writer = node.inner.write().await;
            writer
                .insert_tx_result(
                    H256::repeat_byte(0x1),
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        new_bytecodes: vec![],
                        receipt: api::TransactionReceipt {
                            logs: vec![],
                            gas_used: Some(U256::from(10_000)),
                            effective_gas_price: Some(U256::from(1_000_000_000)),
                            ..Default::default()
                        },
                        debug: testing::default_tx_debug_info(),
                    },
                )
                .await;
        }
        let result = node
            .get_transaction_details_impl(H256::repeat_byte(0x1))
            .await
            .expect("get transaction details")
            .expect("transaction details");

        // Assert
        assert!(matches!(result.status, api::TransactionStatus::Included));
        assert_eq!(result.fee, U256::from(10_000_000_000_000u64));
    }

    #[tokio::test]
    async fn test_get_transaction_details_fork() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_tx_hash = H256::repeat_byte(0x02);
        mock_server.expect(
            "zks_getTransactionDetails",
            Some(serde_json::json!([format!("{:#x}", input_tx_hash),])),
            serde_json::json!({
                "isL1Originated": false,
                "status": "included",
                "fee": "0x74293f087500",
                "gasPerPubdata": "0x4e20",
                "initiatorAddress": "0x63ab285cd87a189f345fed7dd4e33780393e01f0",
                "receivedAt": "2023-10-12T15:45:53.094Z",
                "ethCommitTxHash": null,
                "ethProveTxHash": null,
                "ethExecuteTxHash": null
            }),
        );

        let node = InMemoryNode::test(Some(
            ForkClient::at_block_number(ForkConfig::unknown(mock_server.url()), None)
                .await
                .unwrap(),
        ));

        let result = node
            .get_transaction_details_impl(input_tx_hash)
            .await
            .expect("get transaction details")
            .expect("transaction details");

        assert!(matches!(result.status, api::TransactionStatus::Included));
        assert_eq!(result.fee, U256::from(127_720_500_000_000u64));
    }

    #[tokio::test]
    async fn test_get_block_details_local() {
        // Arrange
        let node = InMemoryNode::test(None);
        {
            let mut writer = node.inner.write().await;
            let block = api::Block::<api::TransactionVariant>::default();
            writer.insert_block(H256::repeat_byte(0x1), block).await;
            writer
                .insert_block_hash(L2BlockNumber(0), H256::repeat_byte(0x1))
                .await;
        }
        let result = node
            .get_block_details_impl(L2BlockNumber(0))
            .await
            .expect("get block details")
            .expect("block details");

        // Assert
        assert!(matches!(result.number, L2BlockNumber(0)));
        assert_eq!(result.l1_batch_number, L1BatchNumber(0));
        assert_eq!(result.base.timestamp, 0);
    }

    #[tokio::test]
    async fn test_get_block_details_fork() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let miniblock = L2BlockNumber::from(16474138);
        mock_server.expect(
            "zks_getBlockDetails",
            Some(serde_json::json!([miniblock.0])),
            serde_json::json!({
                  "number": 16474138,
                  "l1BatchNumber": 270435,
                  "timestamp": 1697405098,
                  "l1TxCount": 0,
                  "l2TxCount": 1,
                  "rootHash": "0xd9e60f9a684fd7fc16e87ae923341a6e4af24f286e76612efdfc2d55f3f4d064",
                  "status": "sealed",
                  "commitTxHash": null,
                  "committedAt": null,
                  "proveTxHash": null,
                  "provenAt": null,
                  "executeTxHash": null,
                  "executedAt": null,
                  "l1GasPrice": 6156252068u64,
                  "l2FairGasPrice": 50000000u64,
                  "fairPubdataPrice": 100u64,
                  "baseSystemContractsHashes": {
                    "bootloader": "0x0100089b8a2f2e6a20ba28f02c9e0ed0c13d702932364561a0ea61621f65f0a8",
                    "default_aa": "0x0100067d16a5485875b4249040bf421f53e869337fe118ec747cf40a4c777e5f"
                  },
                  "operatorAddress": "0xa9232040bf0e0aea2578a5b2243f2916dbfc0a69",
                  "protocolVersion": ProtocolVersionId::Version26,
              }),
        );

        let node = InMemoryNode::test(Some(
            ForkClient::at_block_number(ForkConfig::unknown(mock_server.url()), None)
                .await
                .unwrap(),
        ));

        let result = node
            .get_block_details_impl(miniblock)
            .await
            .expect("get block details")
            .expect("block details");

        assert!(matches!(result.number, L2BlockNumber(16474138)));
        assert_eq!(result.l1_batch_number, L1BatchNumber(270435));
        assert_eq!(result.base.timestamp, 1697405098);
        assert_eq!(result.base.fair_pubdata_price, Some(100));
    }

    #[tokio::test]
    async fn test_get_bridge_contracts_uses_default_values_if_local() {
        // Arrange
        let node = InMemoryNode::test(None);
        let expected_bridge_addresses = api::BridgeAddresses {
            l1_shared_default_bridge: Default::default(),
            l2_shared_default_bridge: Default::default(),
            l1_erc20_default_bridge: Default::default(),
            l2_erc20_default_bridge: Default::default(),
            l1_weth_bridge: Default::default(),
            l2_weth_bridge: Default::default(),
            l2_legacy_shared_bridge: Default::default(),
        };

        let actual_bridge_addresses = node
            .get_bridge_contracts_impl()
            .await
            .expect("get bridge addresses");

        // Assert
        testing::assert_bridge_addresses_eq(&expected_bridge_addresses, &actual_bridge_addresses)
    }

    #[tokio::test]
    async fn test_get_bridge_contracts_uses_fork() {
        // Arrange
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_bridge_addresses = api::BridgeAddresses {
            l1_shared_default_bridge: Some(H160::repeat_byte(0x1)),
            l2_shared_default_bridge: Some(H160::repeat_byte(0x2)),
            l1_erc20_default_bridge: Some(H160::repeat_byte(0x1)),
            l2_erc20_default_bridge: Some(H160::repeat_byte(0x2)),
            l1_weth_bridge: Some(H160::repeat_byte(0x3)),
            l2_weth_bridge: Some(H160::repeat_byte(0x4)),
            l2_legacy_shared_bridge: Some(H160::repeat_byte(0x6)),
        };
        mock_server.expect(
            "zks_getBridgeContracts",
            None,
            serde_json::json!({
                "l1Erc20SharedBridge": format!("{:#x}", input_bridge_addresses.l1_shared_default_bridge.unwrap()),
                "l2Erc20SharedBridge": format!("{:#x}", input_bridge_addresses.l2_shared_default_bridge.unwrap()),
                "l1Erc20DefaultBridge": format!("{:#x}", input_bridge_addresses.l1_erc20_default_bridge.unwrap()),
                "l2Erc20DefaultBridge": format!("{:#x}", input_bridge_addresses.l2_erc20_default_bridge.unwrap()),
                "l1WethBridge": format!("{:#x}", input_bridge_addresses.l1_weth_bridge.unwrap()),
                "l2WethBridge": format!("{:#x}", input_bridge_addresses.l2_weth_bridge.unwrap())
            }),
        );

        let node = InMemoryNode::test(Some(
            ForkClient::at_block_number(ForkConfig::unknown(mock_server.url()), None)
                .await
                .unwrap(),
        ));

        let actual_bridge_addresses = node
            .get_bridge_contracts_impl()
            .await
            .expect("get bridge addresses");

        // Assert
        testing::assert_bridge_addresses_eq(&input_bridge_addresses, &actual_bridge_addresses)
    }

    #[tokio::test]
    async fn test_get_bytecode_by_hash_returns_local_value_if_available() {
        // Arrange
        let node = InMemoryNode::test(None);
        let input_hash = H256::repeat_byte(0x1);
        let input_bytecode = vec![0x1];
        node.inner
            .write()
            .await
            .fork_storage
            .store_factory_dep(input_hash, input_bytecode.clone());

        let actual = node
            .get_bytecode_by_hash_impl(input_hash)
            .await
            .expect("failed fetching bytecode")
            .expect("no bytecode was found");

        // Assert
        assert_eq!(input_bytecode, actual);
    }

    // FIXME: Multi-threaded flavor is needed because of the `block_on` mess inside `ForkStorage`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_get_bytecode_by_hash_uses_fork_if_value_unavailable() {
        // Arrange
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_hash = H256::repeat_byte(0x1);
        let input_bytecode = vec![0x1];
        mock_server.expect(
            "zks_getBytecodeByHash",
            Some(serde_json::json!([format!("{:#x}", input_hash)])),
            serde_json::json!(input_bytecode),
        );

        let node = InMemoryNode::test(Some(
            ForkClient::at_block_number(ForkConfig::unknown(mock_server.url()), None)
                .await
                .unwrap(),
        ));

        let actual = node
            .get_bytecode_by_hash_impl(input_hash)
            .await
            .expect("failed fetching bytecode")
            .expect("no bytecode was found");

        // Assert
        assert_eq!(input_bytecode, actual);
    }

    #[tokio::test]
    async fn test_get_raw_block_transactions_local() {
        // Arrange
        let node = InMemoryNode::test(None);
        {
            let mut writer = node.inner.write().await;
            let mut block = api::Block::<api::TransactionVariant>::default();
            let txn = api::Transaction::default();
            writer
                .insert_tx_result(
                    txn.hash,
                    TransactionResult {
                        info: testing::default_tx_execution_info(),
                        new_bytecodes: vec![],
                        receipt: api::TransactionReceipt {
                            logs: vec![],
                            gas_used: Some(U256::from(10_000)),
                            effective_gas_price: Some(U256::from(1_000_000_000)),
                            ..Default::default()
                        },
                        debug: testing::default_tx_debug_info(),
                    },
                )
                .await;
            block.transactions.push(api::TransactionVariant::Full(txn));
            writer.insert_block(H256::repeat_byte(0x1), block).await;
            writer
                .insert_block_hash(L2BlockNumber(0), H256::repeat_byte(0x1))
                .await;
        }

        let txns = node
            .get_raw_block_transactions_impl(L2BlockNumber(0))
            .await
            .expect("get transaction details");

        // Assert
        assert_eq!(txns.len(), 1);
    }

    #[tokio::test]
    async fn test_get_raw_block_transactions_fork() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let miniblock = L2BlockNumber::from(16474138);
        mock_server.expect(
            "zks_getRawBlockTransactions",
            Some(serde_json::json!([miniblock.0])),
            serde_json::json!([
              {
                "common_data": {
                  "L2": {
                    "nonce": 86,
                    "fee": {
                      "gas_limit": "0xcc626",
                      "max_fee_per_gas": "0x141dd760",
                      "max_priority_fee_per_gas": "0x0",
                      "gas_per_pubdata_limit": "0x4e20"
                    },
                    "initiatorAddress": "0x840bd73f903ba7dbb501be8326fe521dadcae1a5",
                    "signature": [
                      135,
                      163,
                      2,
                      78,
                      118,
                      14,
                      209
                    ],
                    "transactionType": "EIP1559Transaction",
                    "input": {
                      "hash": "0xc1f625f55d186ad0b439054adfe3317ae703c5f588f4fa1896215e8810a141e0",
                      "data": [
                        2,
                        249,
                        1,
                        110,
                        130
                      ]
                    },
                    "paymasterParams": {
                      "paymaster": "0x0000000000000000000000000000000000000000",
                      "paymasterInput": []
                    }
                  }
                },
                "execute": {
                  "contractAddress": "0xbe7d1fd1f6748bbdefc4fbacafbb11c6fc506d1d",
                  "calldata": "0x38ed173900000000000000000000000000000000000000000000000000000000002c34cc00000000000000000000000000000000000000000000000000000000002c9a2500000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000840bd73f903ba7dbb501be8326fe521dadcae1a500000000000000000000000000000000000000000000000000000000652c5d1900000000000000000000000000000000000000000000000000000000000000020000000000000000000000008e86e46278518efc1c5ced245cba2c7e3ef115570000000000000000000000003355df6d4c9c3035724fd0e3914de96a5a83aaf4",
                  "value": "0x0",
                  "factoryDeps": null
                },
                "received_timestamp_ms": 1697405097873u64,
                "raw_bytes": "0x02f9016e820144568084141dd760830cc62694be7d1fd1f6748bbdefc4fbacafbb11c6fc506d1d80b9010438ed173900000000000000000000000000000000000000000000000000000000002c34cc00000000000000000000000000000000000000000000000000000000002c9a2500000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000840bd73f903ba7dbb501be8326fe521dadcae1a500000000000000000000000000000000000000000000000000000000652c5d1900000000000000000000000000000000000000000000000000000000000000020000000000000000000000008e86e46278518efc1c5ced245cba2c7e3ef115570000000000000000000000003355df6d4c9c3035724fd0e3914de96a5a83aaf4c080a087a3024e760ed14134ef541608bf308e083c899a89dba3c02bf3040f07c8b91b9fc3a7eeb6b3b8b36bb03ea4352415e7815dda4954f4898d255bd7660736285e"
              }
            ]),
        );

        let node = InMemoryNode::test(Some(
            ForkClient::at_block_number(ForkConfig::unknown(mock_server.url()), None)
                .await
                .unwrap(),
        ));

        let txns = node
            .get_raw_block_transactions_impl(miniblock)
            .await
            .expect("get transaction details");
        assert_eq!(txns.len(), 1);
    }

    #[tokio::test]
    async fn test_get_all_account_balances_empty() {
        let node = InMemoryNode::test(None);
        let balances = node
            .get_all_account_balances_impl(Address::zero())
            .await
            .expect("get balances");
        assert!(balances.is_empty());
    }

    #[tokio::test]
    async fn test_get_confirmed_tokens_eth() {
        let node = InMemoryNode::test(None);
        let balances = node
            .get_confirmed_tokens_impl(0, 100)
            .await
            .expect("get balances");
        assert_eq!(balances.len(), 1);
        assert_eq!(&balances[0].name, "Ether");
    }

    #[tokio::test]
    async fn test_get_all_account_balances_forked() {
        let cbeth_address = Address::from_str("0x75af292c1c9a37b3ea2e6041168b4e48875b9ed5")
            .expect("failed to parse address");
        let mock_server = testing::MockServer::run();
        mock_server.expect("eth_chainId", None, serde_json::json!("0x104"));

        mock_server.expect(
            "zks_getBlockDetails",
            Some(serde_json::json!([1])),
            serde_json::json!({
                "baseSystemContractsHashes": {
                    "bootloader": "0x010008a5c30072f79f8e04f90b31f34e554279957e7e2bf85d3e9c7c1e0f834d",
                    "default_aa": "0x01000663d7941c097ba2631096508cf9ec7769ddd40e081fd81b0d04dc07ea0e"
                },
                "commitTxHash": null,
                "committedAt": null,
                "executeTxHash": null,
                "executedAt": null,
                "l1BatchNumber": 0,
                "l1GasPrice": 0,
                "l1TxCount": 1,
                "l2FairGasPrice": 50000000,
                "l2TxCount": 0,
                "number": 0,
                "operatorAddress": "0x0000000000000000000000000000000000000000",
                "protocolVersion": ProtocolVersionId::Version26,
                "proveTxHash": null,
                "provenAt": null,
                "rootHash": "0xdaa77426c30c02a43d9fba4e841a6556c524d47030762eb14dc4af897e605d9b",
                "status": "verified",
                "timestamp": 1000
            }),
        );
        mock_server.expect(
            "eth_getBlockByHash",
            Some(serde_json::json!(["0xdaa77426c30c02a43d9fba4e841a6556c524d47030762eb14dc4af897e605d9b", true])),
            serde_json::json!({
                "baseFeePerGas": "0x0",
                "difficulty": "0x0",
                "extraData": "0x",
                "gasLimit": "0xffffffff",
                "gasUsed": "0x0",
                "hash": "0xdaa77426c30c02a43d9fba4e841a6556c524d47030762eb14dc4af897e605d9b",
                "l1BatchNumber": "0x0",
                "l1BatchTimestamp": null,
                "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                "miner": "0x0000000000000000000000000000000000000000",
                "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "nonce": "0x0000000000000000",
                "number": "0x0",
                "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "receiptsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "sealFields": [],
                "sha3Uncles": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "size": "0x0",
                "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "timestamp": "0x3e8",
                "totalDifficulty": "0x0",
                "transactions": [],
                "transactionsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "uncles": []
            }),
        );
        mock_server.expect(
            "zks_getConfirmedTokens",
            Some(serde_json::json!([0, 100])),
            serde_json::json!([
              {
                "decimals": 18,
                "l1Address": "0xbe9895146f7af43049ca1c1ae358b0541ea49704",
                "l2Address": "0x75af292c1c9a37b3ea2e6041168b4e48875b9ed5",
                "name": "Coinbase Wrapped Staked ETH",
                "symbol": "cbETH"
              }
            ]),
        );
        mock_server.expect(
            "zks_getFeeParams",
            None,
            serde_json::json!({
              "V2": {
                "config": {
                  "minimal_l2_gas_price": 25000000,
                  "compute_overhead_part": 0,
                  "pubdata_overhead_part": 1,
                  "batch_overhead_l1_gas": 800000,
                  "max_gas_per_batch": 200000000,
                  "max_pubdata_per_batch": 240000
                },
                "l1_gas_price": 46226388803u64,
                "l1_pubdata_price": 100780475095u64,
                "conversion_ratio": {
                  "numerator": 1,
                  "denominator": 1
                }
              }
            }),
        );

        let node = InMemoryNode::test(Some(
            ForkClient::at_block_number(ForkConfig::unknown(mock_server.url()), Some(1.into()))
                .await
                .unwrap(),
        ));

        {
            let writer = node.inner.write().await;
            let mut fork = writer.fork_storage.inner.write().unwrap();
            fork.raw_storage.set_value(
                storage_key_for_standard_token_balance(
                    AccountTreeId::new(cbeth_address),
                    &Address::repeat_byte(0x1),
                ),
                u256_to_h256(U256::from(1337)),
            );
        }

        let balances = node
            .get_all_account_balances_impl(Address::repeat_byte(0x1))
            .await
            .expect("get balances");
        assert_eq!(balances.get(&cbeth_address).unwrap(), &U256::from(1337));
    }

    #[tokio::test]
    async fn test_get_base_token_l1_address() {
        let node = InMemoryNode::test(None);
        let token_address = node
            .get_base_token_l1_address_impl()
            .await
            .expect("get base token l1 address");
        assert_eq!(
            "0x0000000000000000000000000000000000000001",
            format!("{:?}", token_address)
        );
    }
}
