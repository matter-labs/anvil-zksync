use std::sync::{Arc, RwLock};

use bigdecimal::BigDecimal;
use futures::FutureExt;
use zksync_basic_types::{Address, L1BatchNumber, MiniblockNumber, U256};
use zksync_core::api_server::web3::backend_jsonrpc::{
    error::into_jsrpc_error, namespaces::zks::ZksNamespaceT,
};
use zksync_types::{
    api::{
        BlockDetails, BlockDetailsBase, BlockStatus, BridgeAddresses, ProtocolVersion,
        TransactionDetails, TransactionStatus,
    },
    fee::Fee,
    ProtocolVersionId,
};
use zksync_web3_decl::{
    error::Web3Error,
    types::{Filter, Log},
};

use crate::{
    fork::ForkSource,
    node::{InMemoryNodeInner, TransactionResult, L2_GAS_PRICE},
    utils::{not_implemented, utc_datetime_from_epoch_ms, IntoBoxedFuture},
};
use colored::Colorize;

/// Mock implementation of ZksNamespace - used only in the test node.
pub struct ZkMockNamespaceImpl<S> {
    node: Arc<RwLock<InMemoryNodeInner<S>>>,
}

impl<S> ZkMockNamespaceImpl<S> {
    /// Creates a new `Zks` instance with the given `node`.
    pub fn new(node: Arc<RwLock<InMemoryNodeInner<S>>>) -> Self {
        Self { node }
    }
}

impl<S: Send + Sync + 'static + ForkSource + std::fmt::Debug> ZksNamespaceT
    for ZkMockNamespaceImpl<S>
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
    fn estimate_fee(
        &self,
        req: zksync_types::transaction_request::CallRequest,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_types::fee::Fee>> {
        let reader = match self.node.read() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError)).boxed()
            }
        };

        let result: jsonrpc_core::Result<Fee> = reader.estimate_gas_impl(req);
        match result {
            Ok(fee) => Ok(fee).into_boxed_future(),
            Err(err) => return futures::future::err(err).boxed(),
        }
    }

    fn get_raw_block_transactions(
        &self,
        _block_number: MiniblockNumber,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<zksync_types::Transaction>>> {
        not_implemented("zks_getRawBlockTransactions")
    }

    fn estimate_gas_l1_to_l2(
        &self,
        _req: zksync_types::transaction_request::CallRequest,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<U256>> {
        not_implemented("zks_estimateGasL1ToL2")
    }

    fn get_main_contract(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::Address>> {
        not_implemented("zks_getMainContract")
    }

    fn get_testnet_paymaster(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_basic_types::Address>>> {
        not_implemented("zks_getTestnetPaymaster")
    }

    fn get_bridge_contracts(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<BridgeAddresses>> {
        not_implemented("zks_getBridgeContracts")
    }

    fn l1_chain_id(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::U64>> {
        not_implemented("zks_L1ChainId")
    }

    fn get_confirmed_tokens(
        &self,
        _from: u32,
        _limit: u8,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<zksync_web3_decl::types::Token>>> {
        not_implemented("zks_getConfirmedTokens")
    }

    fn get_token_price(
        &self,
        token_address: zksync_basic_types::Address,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<BigDecimal>> {
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

    fn get_all_account_balances(
        &self,
        _address: zksync_basic_types::Address,
    ) -> jsonrpc_core::BoxFuture<
        jsonrpc_core::Result<std::collections::HashMap<zksync_basic_types::Address, U256>>,
    > {
        not_implemented("zks_getAllAccountBalances")
    }

    fn get_l2_to_l1_msg_proof(
        &self,
        _block: zksync_basic_types::MiniblockNumber,
        _sender: zksync_basic_types::Address,
        _msg: zksync_basic_types::H256,
        _l2_log_position: Option<usize>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::L2ToL1LogProof>>>
    {
        not_implemented("zks_getL2ToL1MsgProof")
    }

    fn get_l2_to_l1_log_proof(
        &self,
        _tx_hash: zksync_basic_types::H256,
        _index: Option<usize>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::L2ToL1LogProof>>>
    {
        not_implemented("zks_getL2ToL1LogProof")
    }

    fn get_l1_batch_number(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::U64>> {
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
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::BlockDetails>>>
    {
        let inner = self.node.clone();
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
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::TransactionDetails>>>
    {
        let inner = self.node.clone();
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
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::L1BatchDetails>>>
    {
        Box::pin(async { Ok(None) })
    }

    fn get_bytecode_by_hash(
        &self,
        _hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<Vec<u8>>>> {
        not_implemented("zks_getBytecodeByHash")
    }

    fn get_l1_gas_price(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::U64>> {
        not_implemented("zks_getL1GasPrice")
    }

    fn get_protocol_version(
        &self,
        _version_id: Option<u16>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<ProtocolVersion>>> {
        not_implemented("zks_getProtocolVersion")
    }

    fn get_logs_with_virtual_blocks(
        &self,
        _filter: Filter,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<Log>>> {
        not_implemented("zks_getLogs")
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::cache::CacheConfig;
    use crate::fork::ForkDetails;
    use crate::node::{ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
    use crate::testing::{ForkBlockConfig, MockServer};
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};
    use crate::{system_contracts, testing};

    use super::*;
    use zksync_basic_types::{Address, H256};
    use zksync_types::api::{Block, TransactionReceipt, TransactionVariant};
    use zksync_types::transaction_request::CallRequest;

    #[tokio::test]
    async fn test_estimate_fee() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());

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

        let result = namespace.estimate_fee(mock_request).await.unwrap();

        assert_eq!(result.gas_limit, U256::from(730662));
        assert_eq!(result.max_fee_per_gas, U256::from(250000000));
        assert_eq!(result.max_priority_fee_per_gas, U256::from(0));
        assert_eq!(result.gas_per_pubdata_limit, U256::from(4080));
    }

    #[tokio::test]
    async fn test_get_token_price_given_eth_should_return_price() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());

        let mock_address = Address::from_str("0x0000000000000000000000000000000000000000")
            .expect("Failed to parse address");

        // Act
        let result = namespace.get_token_price(mock_address).await.unwrap();

        // Assert
        assert_eq!(result, BigDecimal::from(1_500));
    }

    #[tokio::test]
    async fn test_get_token_price_given_capitalized_link_address_should_return_price() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::new(
            None,
            ShowCalls::None,
            crate::node::ShowStorageLogs::None,
            crate::node::ShowVMDetails::None,
            crate::node::ShowGasDetails::None,
            false,
            &system_contracts::Options::BuiltIn,
        );
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());

        let mock_address = Address::from_str("0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78")
            .expect("Failed to parse address");

        // Act
        let result = namespace.get_token_price(mock_address).await.unwrap();

        // Assert
        assert_eq!(result, BigDecimal::from(1));
    }

    #[tokio::test]
    async fn test_get_token_price_given_unknown_address_should_return_error() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());

        let mock_address = Address::from_str("0x0000000000000000000000000000000000000042")
            .expect("Failed to parse address");

        // Act
        let result = namespace.get_token_price(mock_address).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_transaction_details_local() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());
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
        // Act
        let result = namespace
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
            crate::node::ShowCalls::None,
            ShowStorageLogs::None,
            ShowVMDetails::None,
            ShowGasDetails::None,
            false,
            &system_contracts::Options::BuiltIn,
        );

        let namespace = ZkMockNamespaceImpl::new(node.get_inner());
        let result = namespace
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
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());
        let inner = node.get_inner();
        {
            let mut writer = inner.write().unwrap();
            let block = Block::<TransactionVariant>::default();
            writer.blocks.insert(H256::repeat_byte(0x1), block);
            writer.block_hashes.insert(0, H256::repeat_byte(0x1));
        }
        // Act
        let result = namespace
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
            crate::node::ShowCalls::None,
            ShowStorageLogs::None,
            ShowVMDetails::None,
            ShowGasDetails::None,
            false,
            &system_contracts::Options::BuiltIn,
        );

        let namespace = ZkMockNamespaceImpl::new(node.get_inner());
        let result = namespace
            .get_block_details(miniblock)
            .await
            .expect("get block details")
            .expect("block details");

        assert!(matches!(result.number, MiniblockNumber(16474138)));
        assert_eq!(result.l1_batch_number, L1BatchNumber(270435));
        assert_eq!(result.base.timestamp, 1697405098);
    }
}
