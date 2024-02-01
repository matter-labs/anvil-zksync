use std::collections::HashMap;

use bigdecimal::BigDecimal;
use colored::Colorize;
use futures::FutureExt;
use zksync_basic_types::{AccountTreeId, Address, L1BatchNumber, MiniblockNumber, H256, U256};
use zksync_core::api_server::web3::backend_jsonrpc::error::{internal_error, into_jsrpc_error};
use zksync_state::ReadStorage;
use zksync_types::{
    api::{
        BlockDetails, BlockDetailsBase, BlockStatus, BridgeAddresses, Proof, ProtocolVersion,
        TransactionDetails, TransactionStatus, TransactionVariant,
    },
    fee::Fee,
    utils::storage_key_for_standard_token_balance,
    ExecuteTransactionCommon, ProtocolVersionId, Transaction, L2_ETH_TOKEN_ADDRESS,
};
use zksync_utils::h256_to_u256;
use zksync_web3_decl::error::Web3Error;

use crate::{
    fork::ForkSource,
    namespaces::{RpcResult, ZksNamespaceT},
    node::{InMemoryNode, TransactionResult, L2_GAS_PRICE},
    utils::{not_implemented, utc_datetime_from_epoch_ms, IntoBoxedFuture},
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> ZksNamespaceT
    for InMemoryNode<S>
{
    /// Estimates the gas fee data required for a given call request.
    ///
    /// # Arguments
    ///
    /// * `req` - A `CallRequest` struct representing the call request to estimate gas for.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `Fee` representing the estimated gas data required.
    fn estimate_fee(&self, req: zksync_types::transaction_request::CallRequest) -> RpcResult<Fee> {
        self.get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .and_then(|reader| reader.estimate_gas_impl(req))
            .into_boxed_future()
    }

    /// Returns data of transactions in a block.
    ///
    /// # Arguments
    ///
    /// * `block` - Block number
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `Vec` of `Transaction`s representing the transactions in the block.
    fn get_raw_block_transactions(
        &self,
        block_number: MiniblockNumber,
    ) -> RpcResult<Vec<zksync_types::Transaction>> {
        let inner = self.get_inner().clone();
        Box::pin(async move {
            let reader = inner
                .read()
                .map_err(|_err| into_jsrpc_error(Web3Error::InternalError))?;

            let maybe_transactions = reader
                .block_hashes
                .get(&(block_number.0 as u64))
                .and_then(|hash| reader.blocks.get(hash))
                .map(|block| {
                    block
                        .transactions
                        .iter()
                        .map(|tx| match tx {
                            TransactionVariant::Full(tx) => &tx.hash,
                            TransactionVariant::Hash(hash) => hash,
                        })
                        .flat_map(|tx_hash| {
                            reader.tx_results.get(tx_hash).map(
                                |TransactionResult { info, .. }| Transaction {
                                    common_data: ExecuteTransactionCommon::L2(
                                        info.tx.common_data.clone(),
                                    ),
                                    execute: info.tx.execute.clone(),
                                    received_timestamp_ms: info.tx.received_timestamp_ms,
                                    raw_bytes: info.tx.raw_bytes.clone(),
                                },
                            )
                        })
                        .collect()
                });

            let transactions = match maybe_transactions {
                Some(txns) => Ok(txns),
                None => {
                    let fork_storage_read = reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage");

                    match fork_storage_read.fork.as_ref() {
                        Some(fork) => fork
                            .fork_source
                            .get_raw_block_transactions(block_number)
                            .map_err(|e| internal_error("get_raw_block_transactions", e)),
                        None => Ok(vec![]),
                    }
                }
            }
            .map_err(into_jsrpc_error)?;

            Ok(transactions)
        })
    }

    fn get_proof(
        &self,
        _address: Address,
        _keys: Vec<H256>,
        _l1_batch_number: L1BatchNumber,
    ) -> RpcResult<Proof> {
        not_implemented("zks_getProof")
    }

    fn estimate_gas_l1_to_l2(
        &self,
        _req: zksync_types::transaction_request::CallRequest,
    ) -> RpcResult<U256> {
        not_implemented("zks_estimateGasL1ToL2")
    }

    fn get_main_contract(&self) -> RpcResult<zksync_basic_types::Address> {
        not_implemented("zks_getMainContract")
    }

    fn get_testnet_paymaster(&self) -> RpcResult<Option<zksync_basic_types::Address>> {
        not_implemented("zks_getTestnetPaymaster")
    }

    fn get_bridge_contracts(&self) -> RpcResult<BridgeAddresses> {
        let inner = self.get_inner().clone();
        Box::pin(async move {
            let reader = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;

            let result = match reader
                .fork_storage
                .inner
                .read()
                .expect("failed reading fork storage")
                .fork
                .as_ref()
            {
                Some(fork) => fork.fork_source.get_bridge_contracts().map_err(|err| {
                    tracing::error!("failed fetching bridge contracts from the fork: {:?}", err);
                    into_jsrpc_error(Web3Error::InternalError)
                })?,
                None => BridgeAddresses {
                    l1_erc20_default_bridge: Default::default(),
                    l2_erc20_default_bridge: Default::default(),
                    l1_weth_bridge: Default::default(),
                    l2_weth_bridge: Default::default(),
                },
            };

            Ok(result)
        })
    }

    fn l1_chain_id(&self) -> RpcResult<zksync_basic_types::U64> {
        not_implemented("zks_L1ChainId")
    }

    fn get_confirmed_tokens(
        &self,
        from: u32,
        limit: u8,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<zksync_web3_decl::types::Token>>> {
        let inner = self.get_inner().clone();
        Box::pin(async move {
            let reader = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;

            let fork_storage_read = reader
                .fork_storage
                .inner
                .read()
                .expect("failed reading fork storage");

            match fork_storage_read.fork.as_ref() {
                Some(fork) => Ok(fork
                    .fork_source
                    .get_confirmed_tokens(from, limit)
                    .map_err(|_e| into_jsrpc_error(Web3Error::InternalError))?),
                None => Ok(vec![zksync_web3_decl::types::Token {
                    l1_address: Address::zero(),
                    l2_address: L2_ETH_TOKEN_ADDRESS,
                    name: "Ether".to_string(),
                    symbol: "ETH".to_string(),
                    decimals: 18,
                }]),
            }
        })
    }

    fn get_token_price(&self, token_address: zksync_basic_types::Address) -> RpcResult<BigDecimal> {
        match format!("{:?}", token_address).to_lowercase().as_str() {
            "0x0000000000000000000000000000000000000000" => {
                // ETH
                Ok(1_500.into()).into_boxed_future()
            }
            "0x40609141db628beee3bfab8034fc2d8278d0cc78" => {
                // LINK
                Ok(1.into()).into_boxed_future()
            }
            "0x0bfce1d53451b4a8175dd94e6e029f7d8a701e9c" => {
                // wBTC
                Ok(1.into()).into_boxed_future()
            }
            "0x0faf6df7054946141266420b43783387a78d82a9" => {
                // USDC
                Ok(1.into()).into_boxed_future()
            }
            "0x3e7676937a7e96cfb7616f255b9ad9ff47363d4b" => {
                // DAI
                Ok(1.into()).into_boxed_future()
            }
            address => {
                tracing::error!(
                    "{}",
                    format!("Token price requested for unknown address {:?}", address).red()
                );
                futures::future::err(into_jsrpc_error(Web3Error::InternalError)).boxed()
            }
        }
    }

    /// Get all known balances for a given account.
    ///
    /// # Arguments
    ///
    /// * `address` - The user address with balances to check.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a (Token, Balance) map where account has non-zero value.
    fn get_all_account_balances(
        &self,
        address: zksync_basic_types::Address,
    ) -> jsonrpc_core::BoxFuture<
        jsonrpc_core::Result<std::collections::HashMap<zksync_basic_types::Address, U256>>,
    > {
        let inner = self.get_inner().clone();
        Box::pin({
            self.get_confirmed_tokens(0, 100)
                .then(move |tokens| async move {
                    let tokens =
                        tokens.map_err(|_err| into_jsrpc_error(Web3Error::InternalError))?;

                    let mut writer = inner
                        .write()
                        .map_err(|_err| into_jsrpc_error(Web3Error::InternalError))?;

                    let mut balances = HashMap::new();
                    for token in tokens {
                        let balance_key = storage_key_for_standard_token_balance(
                            AccountTreeId::new(token.l2_address),
                            &address,
                        );

                        let balance = writer.fork_storage.read_value(&balance_key);
                        if !balance.is_zero() {
                            balances.insert(token.l2_address, h256_to_u256(balance));
                        }
                    }

                    Ok(balances)
                })
        })
    }

    fn get_l2_to_l1_msg_proof(
        &self,
        _block: zksync_basic_types::MiniblockNumber,
        _sender: zksync_basic_types::Address,
        _msg: zksync_basic_types::H256,
        _l2_log_position: Option<usize>,
    ) -> RpcResult<Option<zksync_types::api::L2ToL1LogProof>> {
        not_implemented("zks_getL2ToL1MsgProof")
    }

    fn get_l2_to_l1_log_proof(
        &self,
        _tx_hash: zksync_basic_types::H256,
        _index: Option<usize>,
    ) -> RpcResult<Option<zksync_types::api::L2ToL1LogProof>> {
        not_implemented("zks_getL2ToL1LogProof")
    }

    fn get_l1_batch_number(&self) -> RpcResult<zksync_basic_types::U64> {
        not_implemented("zks_L1BatchNumber")
    }

    /// Get block details.
    ///
    /// # Arguments
    ///
    /// * `blockNumber` - `u32` miniblock number
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with an `Option<BlockDetails>` representing details of the block (if found).
    fn get_block_details(
        &self,
        block_number: zksync_basic_types::MiniblockNumber,
    ) -> RpcResult<Option<zksync_types::api::BlockDetails>> {
        let inner = self.get_inner().clone();
        Box::pin(async move {
            let reader = inner
                .read()
                .map_err(|_err| into_jsrpc_error(Web3Error::InternalError))?;

            let maybe_block = reader
                .block_hashes
                .get(&(block_number.0 as u64))
                .and_then(|hash| reader.blocks.get(hash))
                .map(|block| BlockDetails {
                    number: MiniblockNumber(block.number.as_u32()),
                    l1_batch_number: L1BatchNumber(
                        block.l1_batch_number.unwrap_or_default().as_u32(),
                    ),
                    base: BlockDetailsBase {
                        timestamp: block.timestamp.as_u64(),
                        l1_tx_count: 1,
                        l2_tx_count: block.transactions.len(),
                        root_hash: Some(block.hash),
                        status: BlockStatus::Verified,
                        commit_tx_hash: None,
                        committed_at: None,
                        prove_tx_hash: None,
                        proven_at: None,
                        execute_tx_hash: None,
                        executed_at: None,
                        l1_gas_price: 0,
                        l2_fair_gas_price: L2_GAS_PRICE,
                        base_system_contracts_hashes: reader
                            .system_contracts
                            .baseline_contracts
                            .hashes(),
                    },
                    operator_address: Address::zero(),
                    protocol_version: Some(ProtocolVersionId::latest()),
                })
                .or_else(|| {
                    reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage")
                        .fork
                        .as_ref()
                        .and_then(|fork| {
                            fork.fork_source
                                .get_block_details(block_number)
                                .ok()
                                .flatten()
                        })
                });

            Ok(maybe_block)
        })
    }

    fn get_miniblock_range(
        &self,
        _batch: zksync_basic_types::L1BatchNumber,
    ) -> jsonrpc_core::BoxFuture<
        jsonrpc_core::Result<Option<(zksync_basic_types::U64, zksync_basic_types::U64)>>,
    > {
        not_implemented("zks_getL1BatchBlockRange")
    }

    /// Get transaction details.
    ///
    /// # Arguments
    ///
    /// * `transactionHash` - `H256` hash of the transaction
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with an `Option<TransactionDetails>` representing details of the transaction (if found).
    fn get_transaction_details(
        &self,
        hash: zksync_basic_types::H256,
    ) -> RpcResult<Option<zksync_types::api::TransactionDetails>> {
        let inner = self.get_inner().clone();
        Box::pin(async move {
            let reader = inner
                .read()
                .map_err(|_err| into_jsrpc_error(Web3Error::InternalError))?;

            let maybe_result = {
                reader
                    .tx_results
                    .get(&hash)
                    .map(|TransactionResult { info, receipt, .. }| {
                        TransactionDetails {
                            is_l1_originated: false,
                            status: TransactionStatus::Included,
                            // if these are not set, fee is effectively 0
                            fee: receipt.effective_gas_price.unwrap_or_default()
                                * receipt.gas_used.unwrap_or_default(),
                            gas_per_pubdata: info.tx.common_data.fee.gas_per_pubdata_limit,
                            initiator_address: info.tx.initiator_account(),
                            received_at: utc_datetime_from_epoch_ms(info.tx.received_timestamp_ms),
                            eth_commit_tx_hash: None,
                            eth_prove_tx_hash: None,
                            eth_execute_tx_hash: None,
                        }
                    })
                    .or_else(|| {
                        reader
                            .fork_storage
                            .inner
                            .read()
                            .expect("failed reading fork storage")
                            .fork
                            .as_ref()
                            .and_then(|fork| {
                                fork.fork_source
                                    .get_transaction_details(hash)
                                    .ok()
                                    .flatten()
                            })
                    })
            };

            Ok(maybe_result)
        })
    }

    /// Retrieves details for a given L1 batch.
    ///
    /// This method is intended to handle queries related to L1 batch details. However, as of the current implementation,
    /// L1 communication is not supported. Instead of an error or no method found, this method intentionally returns
    /// `{"jsonrpc":"2.0","result":null,"id":1}` to ensure compatibility with block explorer integration.
    ///
    /// # Parameters
    ///
    /// * `_batch`: The batch number of type `zksync_basic_types::L1BatchNumber` for which the details are to be fetched.
    ///
    /// # Returns
    ///
    /// A boxed future resolving to a `jsonrpc_core::Result` containing an `Option` of `zksync_types::api::L1BatchDetails`.
    /// Given the current implementation, this will always be `None`.
    fn get_l1_batch_details(
        &self,
        _batch: zksync_basic_types::L1BatchNumber,
    ) -> RpcResult<Option<zksync_types::api::L1BatchDetails>> {
        Box::pin(async { Ok(None) })
    }

    /// Returns bytecode of a transaction given by its hash.
    ///
    /// # Parameters
    ///
    /// * `hash`: Hash address.
    ///
    /// # Returns
    ///
    /// A boxed future resolving to a `jsonrpc_core::Result` containing an `Option` of bytes.
    fn get_bytecode_by_hash(&self, hash: zksync_basic_types::H256) -> RpcResult<Option<Vec<u8>>> {
        let inner = self.get_inner().clone();
        Box::pin(async move {
            let mut writer = inner
                .write()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;

            let maybe_bytecode = writer.fork_storage.load_factory_dep(hash).or_else(|| {
                writer
                    .fork_storage
                    .inner
                    .read()
                    .expect("failed reading fork storage")
                    .fork
                    .as_ref()
                    .and_then(|fork| fork.fork_source.get_bytecode_by_hash(hash).ok().flatten())
            });

            Ok(maybe_bytecode)
        })
    }

    fn get_l1_gas_price(&self) -> RpcResult<zksync_basic_types::U64> {
        not_implemented("zks_getL1GasPrice")
    }

    fn get_protocol_version(&self, _version_id: Option<u16>) -> RpcResult<Option<ProtocolVersion>> {
        not_implemented("zks_getProtocolVersion")
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::cache::CacheConfig;
    use crate::fork::ForkDetails;
    use crate::testing;
    use crate::testing::{ForkBlockConfig, MockServer};
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};

    use super::*;
    use zksync_basic_types::{Address, H160, H256};
    use zksync_types::api::{self, Block, TransactionReceipt, TransactionVariant};
    use zksync_types::transaction_request::CallRequest;
    use zksync_utils::u256_to_h256;

    #[tokio::test]
    async fn test_estimate_fee() {
        let node = InMemoryNode::<HttpForkSource>::default();

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
        };

        let result = node.estimate_fee(mock_request).await.unwrap();

        assert_eq!(result.gas_limit, U256::from(746532));
        assert_eq!(result.max_fee_per_gas, U256::from(250000000));
        assert_eq!(result.max_priority_fee_per_gas, U256::from(0));
        assert_eq!(result.gas_per_pubdata_limit, U256::from(4080));
    }

    #[tokio::test]
    async fn test_get_token_price_given_eth_should_return_price() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();

        let mock_address = Address::from_str("0x0000000000000000000000000000000000000000")
            .expect("Failed to parse address");

        // Act
        let result = node.get_token_price(mock_address).await.unwrap();

        // Assert
        assert_eq!(result, BigDecimal::from(1_500));
    }

    #[tokio::test]
    async fn test_get_token_price_given_capitalized_link_address_should_return_price() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();

        let mock_address = Address::from_str("0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78")
            .expect("Failed to parse address");

        // Act
        let result = node.get_token_price(mock_address).await.unwrap();

        // Assert
        assert_eq!(result, BigDecimal::from(1));
    }

    #[tokio::test]
    async fn test_get_token_price_given_unknown_address_should_return_error() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();

        let mock_address = Address::from_str("0x0000000000000000000000000000000000000042")
            .expect("Failed to parse address");

        // Act
        let result = node.get_token_price(mock_address).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_transaction_details_local() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            writer.tx_results.insert(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![],
                        gas_used: Some(U256::from(10_000)),
                        effective_gas_price: Some(U256::from(1_000_000_000)),
                        ..Default::default()
                    },
                    debug: testing::default_tx_debug_info(),
                },
            );
        }
        let result = node
            .get_transaction_details(H256::repeat_byte(0x1))
            .await
            .expect("get transaction details")
            .expect("transaction details");

        // Assert
        assert!(matches!(result.status, TransactionStatus::Included));
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
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getTransactionDetails",
                "params": [
                    format!("{:#x}", input_tx_hash),
                ],
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
                    "isL1Originated": false,
                    "status": "included",
                    "fee": "0x74293f087500",
                    "gasPerPubdata": "0x4e20",
                    "initiatorAddress": "0x63ab285cd87a189f345fed7dd4e33780393e01f0",
                    "receivedAt": "2023-10-12T15:45:53.094Z",
                    "ethCommitTxHash": null,
                    "ethProveTxHash": null,
                    "ethExecuteTxHash": null
                },
                "id": 0
            }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
        );

        let result = node
            .get_transaction_details(input_tx_hash)
            .await
            .expect("get transaction details")
            .expect("transaction details");

        assert!(matches!(result.status, TransactionStatus::Included));
        assert_eq!(result.fee, U256::from(127_720_500_000_000u64));
    }

    #[tokio::test]
    async fn test_get_block_details_local() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            let block = Block::<TransactionVariant>::default();
            writer.blocks.insert(H256::repeat_byte(0x1), block);
            writer.block_hashes.insert(0, H256::repeat_byte(0x1));
        }
        let result = node
            .get_block_details(MiniblockNumber(0))
            .await
            .expect("get block details")
            .expect("block details");

        // Assert
        assert!(matches!(result.number, MiniblockNumber(0)));
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
        let miniblock = MiniblockNumber::from(16474138);
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getBlockDetails",
                "params": [
                    miniblock.0,
                ],
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
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
                  "l2FairGasPrice": 250000000u64,
                  "baseSystemContractsHashes": {
                    "bootloader": "0x0100089b8a2f2e6a20ba28f02c9e0ed0c13d702932364561a0ea61621f65f0a8",
                    "default_aa": "0x0100067d16a5485875b4249040bf421f53e869337fe118ec747cf40a4c777e5f"
                  },
                  "operatorAddress": "0xa9232040bf0e0aea2578a5b2243f2916dbfc0a69",
                  "protocolVersion": "Version15"
                },
                "id": 0
              }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
        );

        let result = node
            .get_block_details(miniblock)
            .await
            .expect("get block details")
            .expect("block details");

        assert!(matches!(result.number, MiniblockNumber(16474138)));
        assert_eq!(result.l1_batch_number, L1BatchNumber(270435));
        assert_eq!(result.base.timestamp, 1697405098);
    }

    #[tokio::test]
    async fn test_get_bridge_contracts_uses_default_values_if_local() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let expected_bridge_addresses = BridgeAddresses {
            l1_erc20_default_bridge: Default::default(),
            l2_erc20_default_bridge: Default::default(),
            l1_weth_bridge: Default::default(),
            l2_weth_bridge: Default::default(),
        };

        let actual_bridge_addresses = node
            .get_bridge_contracts()
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
        let input_bridge_addresses = BridgeAddresses {
            l1_erc20_default_bridge: H160::repeat_byte(0x1),
            l2_erc20_default_bridge: H160::repeat_byte(0x2),
            l1_weth_bridge: Some(H160::repeat_byte(0x3)),
            l2_weth_bridge: Some(H160::repeat_byte(0x4)),
        };
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getBridgeContracts",
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
                    "l1Erc20DefaultBridge": format!("{:#x}", input_bridge_addresses.l1_erc20_default_bridge),
                    "l2Erc20DefaultBridge": format!("{:#x}", input_bridge_addresses.l2_erc20_default_bridge),
                    "l1WethBridge": format!("{:#x}", input_bridge_addresses.l1_weth_bridge.unwrap()),
                    "l2WethBridge": format!("{:#x}", input_bridge_addresses.l2_weth_bridge.unwrap())
                },
                "id": 0
            }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
        );

        let actual_bridge_addresses = node
            .get_bridge_contracts()
            .await
            .expect("get bridge addresses");

        // Assert
        testing::assert_bridge_addresses_eq(&input_bridge_addresses, &actual_bridge_addresses)
    }

    #[tokio::test]
    async fn test_get_bytecode_by_hash_returns_local_value_if_available() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let input_hash = H256::repeat_byte(0x1);
        let input_bytecode = vec![0x1];
        node.get_inner()
            .write()
            .unwrap()
            .fork_storage
            .store_factory_dep(input_hash, input_bytecode.clone());

        let actual = node
            .get_bytecode_by_hash(input_hash)
            .await
            .expect("failed fetching bytecode")
            .expect("no bytecode was found");

        // Assert
        assert_eq!(input_bytecode, actual);
    }

    #[tokio::test]
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
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getBytecodeByHash",
                "params": [
                    format!("{:#x}", input_hash)
                ],
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "result": input_bytecode,
            }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
        );

        let actual = node
            .get_bytecode_by_hash(input_hash)
            .await
            .expect("failed fetching bytecode")
            .expect("no bytecode was found");

        // Assert
        assert_eq!(input_bytecode, actual);
    }

    #[tokio::test]
    async fn test_get_raw_block_transactions_local() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            let mut block = Block::<TransactionVariant>::default();
            let txn = api::Transaction::default();
            writer.tx_results.insert(
                txn.hash,
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![],
                        gas_used: Some(U256::from(10_000)),
                        effective_gas_price: Some(U256::from(1_000_000_000)),
                        ..Default::default()
                    },
                    debug: testing::default_tx_debug_info(),
                },
            );
            block.transactions.push(TransactionVariant::Full(txn));
            writer.blocks.insert(H256::repeat_byte(0x1), block);
            writer.block_hashes.insert(0, H256::repeat_byte(0x1));
        }

        let txns = node
            .get_raw_block_transactions(MiniblockNumber(0))
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
        let miniblock = MiniblockNumber::from(16474138);
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getRawBlockTransactions",
                "params": [miniblock.0]
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": [
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
                ],
                "id": 0
              }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
        );

        let txns = node
            .get_raw_block_transactions(miniblock)
            .await
            .expect("get transaction details");
        assert_eq!(txns.len(), 1);
    }

    #[tokio::test]
    async fn test_get_all_account_balances_empty() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let balances = node
            .get_all_account_balances(Address::zero())
            .await
            .expect("get balances");
        assert!(balances.is_empty());
    }

    #[tokio::test]
    async fn test_get_confirmed_tokens_eth() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let balances = node
            .get_confirmed_tokens(0, 100)
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
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getBlockDetails",
                "params": [1]
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
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
                    "l2FairGasPrice": 250000000,
                    "l2TxCount": 0,
                    "number": 0,
                    "operatorAddress": "0x0000000000000000000000000000000000000000",
                    "protocolVersion": "Version16",
                    "proveTxHash": null,
                    "provenAt": null,
                    "rootHash": "0xdaa77426c30c02a43d9fba4e841a6556c524d47030762eb14dc4af897e605d9b",
                    "status": "verified",
                    "timestamp": 1000
                },
                "id": 0
            }),
        );
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "eth_getBlockByHash",
                "params": ["0xdaa77426c30c02a43d9fba4e841a6556c524d47030762eb14dc4af897e605d9b", true]
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
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
                },
                "id": 1
            }),
        );
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getConfirmedTokens",
                "params": [0, 100]
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": [
                    {
                        "decimals": 18,
                        "l1Address": "0xbe9895146f7af43049ca1c1ae358b0541ea49704",
                        "l2Address": "0x75af292c1c9a37b3ea2e6041168b4e48875b9ed5",
                        "name": "Coinbase Wrapped Staked ETH",
                        "symbol": "cbETH"
                      }
                ],
                "id": 0
            }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), Some(1), CacheConfig::None).await),
            None,
            Default::default(),
        );
        {
            let inner = node.get_inner();
            let writer = inner.write().unwrap();
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
            .get_all_account_balances(Address::repeat_byte(0x1))
            .await
            .expect("get balances");
        assert_eq!(balances.get(&cbeth_address).unwrap(), &U256::from(1337));
    }
}
