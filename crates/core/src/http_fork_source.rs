use std::{
    str::FromStr,
    sync::{Arc, RwLock},
};

use crate::{
    cache::Cache,
    fork::{block_on, ForkSource},
};
use anvil_zksync_config::types::CacheConfig;
use eyre::Context;
use zksync_types::{
    api::{BridgeAddresses, Transaction},
    url::SensitiveUrl,
};
use zksync_types::{H256, U256};
use zksync_web3_decl::{
    client::Client,
    namespaces::{EthNamespaceClient, ZksNamespaceClient},
    types::Index,
};
use zksync_web3_decl::{client::L2, types::Token};

#[derive(Debug, Clone)]
/// Fork source that gets the data via HTTP requests.
pub struct HttpForkSource {
    /// URL for the network to fork.
    pub fork_url: String,
    /// Cache for network data.
    pub(crate) cache: Arc<RwLock<Cache>>,
}

impl HttpForkSource {
    pub fn new(fork_url: String, cache_config: CacheConfig) -> Self {
        Self {
            fork_url,
            cache: Arc::new(RwLock::new(Cache::new(cache_config))),
        }
    }

    pub fn create_client(&self) -> Client<L2> {
        let url = SensitiveUrl::from_str(&self.fork_url)
            .unwrap_or_else(|_| panic!("Unable to parse client URL: {}", &self.fork_url));
        Client::http(url)
            .unwrap_or_else(|_| panic!("Unable to create a client for fork: {}", self.fork_url))
            .build()
    }
}

impl ForkSource for HttpForkSource {
    fn get_fork_url(&self) -> eyre::Result<String> {
        Ok(self.fork_url.clone())
    }

    fn get_storage_at(
        &self,
        address: zksync_types::Address,
        idx: zksync_types::U256,
        block: Option<zksync_types::api::BlockIdVariant>,
    ) -> eyre::Result<zksync_types::H256> {
        let client = self.create_client();
        block_on(async move { client.get_storage_at(address, idx, block).await })
            .wrap_err("fork http client failed")
    }

    fn get_bytecode_by_hash(&self, hash: zksync_types::H256) -> eyre::Result<Option<Vec<u8>>> {
        let client = self.create_client();
        block_on(async move { client.get_bytecode_by_hash(hash).await })
            .wrap_err("fork http client failed")
    }

    fn get_transaction_by_hash(
        &self,
        hash: zksync_types::H256,
    ) -> eyre::Result<Option<zksync_types::api::Transaction>> {
        if let Ok(Some(transaction)) = self
            .cache
            .read()
            .map(|guard| guard.get_transaction(&hash).cloned())
        {
            tracing::debug!("using cached transaction for {hash}");
            return Ok(Some(transaction));
        }

        let client = self.create_client();
        block_on(async move { client.get_transaction_by_hash(hash).await })
            .inspect(|maybe_transaction| {
                if let Some(transaction) = &maybe_transaction {
                    self.cache
                        .write()
                        .map(|mut guard| guard.insert_transaction(hash, transaction.clone()))
                        .unwrap_or_else(|err| {
                            tracing::warn!(
                                "failed writing to cache for 'get_transaction_by_hash': {:?}",
                                err
                            )
                        });
                }
            })
            .wrap_err("fork http client failed")
    }

    fn get_transaction_details(
        &self,
        hash: H256,
    ) -> eyre::Result<Option<zksync_types::api::TransactionDetails>> {
        let client = self.create_client();
        // n.b- We don't cache these responses as they will change through the lifecycle of the transaction
        // and caching could be error-prone. in theory we could cache responses once the txn status
        // is `final` or `failed` but currently this does not warrant the additional complexity.
        block_on(async move { client.get_transaction_details(hash).await })
            .wrap_err("fork http client failed")
    }

    fn get_raw_block_transactions(
        &self,
        block_number: zksync_types::L2BlockNumber,
    ) -> eyre::Result<Vec<zksync_types::Transaction>> {
        let number = block_number.0 as u64;
        if let Ok(Some(transaction)) = self
            .cache
            .read()
            .map(|guard| guard.get_block_raw_transactions(&number).cloned())
        {
            tracing::debug!("using cached raw transactions for block {block_number}");
            return Ok(transaction);
        }

        let client = self.create_client();
        block_on(async move { client.get_raw_block_transactions(block_number).await })
            .wrap_err("fork http client failed")
            .inspect(|transactions| {
                if !transactions.is_empty() {
                    self.cache
                        .write()
                        .map(|mut guard| {
                            guard.insert_block_raw_transactions(number, transactions.clone())
                        })
                        .unwrap_or_else(|err| {
                            tracing::warn!(
                                "failed writing to cache for 'get_raw_block_transactions': {:?}",
                                err
                            )
                        });
                }
            })
    }

    fn get_block_by_hash(
        &self,
        hash: zksync_types::H256,
        full_transactions: bool,
    ) -> eyre::Result<Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>> {
        if let Ok(Some(block)) = self
            .cache
            .read()
            .map(|guard| guard.get_block(&hash, full_transactions).cloned())
        {
            tracing::debug!("using cached block for {hash}");
            return Ok(Some(block));
        }

        let client = self.create_client();
        block_on(async move { client.get_block_by_hash(hash, full_transactions).await })
            .inspect(|block| {
                if let Some(block) = &block {
                    self.cache
                        .write()
                        .map(|mut guard| guard.insert_block(hash, full_transactions, block.clone()))
                        .unwrap_or_else(|err| {
                            tracing::warn!(
                                "failed writing to cache for 'get_block_by_hash': {:?}",
                                err
                            )
                        });
                }
            })
            .wrap_err("fork http client failed")
    }

    fn get_block_by_number(
        &self,
        block_number: zksync_types::api::BlockNumber,
        full_transactions: bool,
    ) -> eyre::Result<Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>> {
        let maybe_number = match block_number {
            zksync_types::api::BlockNumber::Number(block_number) => Some(block_number),
            _ => None,
        };

        if let Some(block) = maybe_number.and_then(|number| {
            self.cache.read().ok().and_then(|guard| {
                guard
                    .get_block_hash(&number.as_u64())
                    .and_then(|hash| guard.get_block(hash, full_transactions).cloned())
            })
        }) {
            tracing::debug!("using cached block for {block_number}");
            return Ok(Some(block));
        }

        let client = self.create_client();
        block_on(async move {
            client
                .get_block_by_number(block_number, full_transactions)
                .await
        })
        .inspect(|block| {
            if let Some(block) = &block {
                self.cache
                    .write()
                    .map(|mut guard| {
                        guard.insert_block(block.hash, full_transactions, block.clone())
                    })
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            "failed writing to cache for 'get_block_by_number': {:?}",
                            err
                        )
                    });
            }
        })
        .wrap_err("fork http client failed")
    }

    /// Returns the  transaction count for a given block hash.
    fn get_block_transaction_count_by_hash(&self, block_hash: H256) -> eyre::Result<Option<U256>> {
        let client = self.create_client();
        block_on(async move { client.get_block_transaction_count_by_hash(block_hash).await })
            .wrap_err("fork http client failed")
    }

    /// Returns the transaction count for a given block number.
    fn get_block_transaction_count_by_number(
        &self,
        block_number: zksync_types::api::BlockNumber,
    ) -> eyre::Result<Option<U256>> {
        let client = self.create_client();
        block_on(async move {
            client
                .get_block_transaction_count_by_number(block_number)
                .await
        })
        .wrap_err("fork http client failed")
    }

    /// Returns information about a transaction by block hash and transaction index position.
    fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> eyre::Result<Option<Transaction>> {
        let client = self.create_client();
        block_on(async move {
            client
                .get_transaction_by_block_hash_and_index(block_hash, index)
                .await
        })
        .wrap_err("fork http client failed")
    }

    /// Returns information about a transaction by block number and transaction index position.
    fn get_transaction_by_block_number_and_index(
        &self,
        block_number: zksync_types::api::BlockNumber,
        index: Index,
    ) -> eyre::Result<Option<Transaction>> {
        let client = self.create_client();
        block_on(async move {
            client
                .get_transaction_by_block_number_and_index(block_number, index)
                .await
        })
        .wrap_err("fork http client failed")
    }

    /// Returns details of a block, given miniblock number
    fn get_block_details(
        &self,
        miniblock: zksync_types::L2BlockNumber,
    ) -> eyre::Result<Option<zksync_types::api::BlockDetails>> {
        let client = self.create_client();
        block_on(async move { client.get_block_details(miniblock).await }).wrap_err(format!(
            "Failed to get block details for {} l2 block in fork http client",
            miniblock
        ))
    }

    /// Returns fee parameters for the give source.
    fn get_fee_params(&self) -> eyre::Result<zksync_types::fee_model::FeeParams> {
        let client = self.create_client();
        block_on(async move { client.get_fee_params().await }).wrap_err("fork http client failed")
    }

    /// Returns addresses of the default bridge contracts.
    fn get_bridge_contracts(&self) -> eyre::Result<BridgeAddresses> {
        if let Some(bridge_addresses) = self
            .cache
            .read()
            .ok()
            .and_then(|guard| guard.get_bridge_addresses().cloned())
        {
            tracing::debug!("using cached bridge contracts");
            return Ok(bridge_addresses);
        };

        let client = self.create_client();
        block_on(async move { client.get_bridge_contracts().await })
            .inspect(|bridge_addresses| {
                self.cache
                    .write()
                    .map(|mut guard| guard.set_bridge_addresses(bridge_addresses.clone()))
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            "failed writing to cache for 'get_bridge_contracts': {:?}",
                            err
                        )
                    });
            })
            .wrap_err("fork http client failed")
    }

    /// Returns known token addresses
    fn get_confirmed_tokens(&self, from: u32, limit: u8) -> eyre::Result<Vec<Token>> {
        if let Some(confirmed_tokens) = self
            .cache
            .read()
            .ok()
            .and_then(|guard| guard.get_confirmed_tokens(from, limit).cloned())
        {
            tracing::debug!("using cached confirmed_tokens");
            return Ok(confirmed_tokens);
        };

        let client = self.create_client();
        block_on(async move { client.get_confirmed_tokens(from, limit).await })
            .inspect(|confirmed_tokens| {
                self.cache
                    .write()
                    .map(|mut guard| {
                        guard.set_confirmed_tokens(from, limit, confirmed_tokens.clone())
                    })
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            "failed writing to cache for 'set_confirmed_tokens': {:?}",
                            err
                        )
                    });
            })
            .wrap_err("fork http client failed")
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use zksync_types::api::BlockNumber;
    use zksync_types::{Address, L2BlockNumber, H160, H256, U64};

    use crate::testing;

    use super::*;

    #[test]
    fn test_get_block_by_hash_full_is_cached() {
        let input_block_hash = H256::repeat_byte(0x01);
        let input_block_number = 8;

        let mock_server = testing::MockServer::run();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockByHash",
                "params": [
                    format!("{input_block_hash:#x}"),
                    true
                ],
            }),
            testing::BlockResponseBuilder::new()
                .set_hash(input_block_hash)
                .set_number(input_block_number)
                .build(),
        );

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);

        let actual_block = fork_source
            .get_block_by_hash(input_block_hash, true)
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(input_block_number), actual_block.number);

        let actual_block = fork_source
            .get_block_by_hash(input_block_hash, true)
            .expect("failed fetching cached block by hash")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(input_block_number), actual_block.number);
    }

    #[test]
    fn test_get_block_by_hash_minimal_is_cached() {
        let input_block_hash = H256::repeat_byte(0x01);
        let input_block_number = 8;

        let mock_server = testing::MockServer::run();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockByHash",
                "params": [
                    format!("{input_block_hash:#x}"),
                    false
                ],
            }),
            testing::BlockResponseBuilder::new()
                .set_hash(input_block_hash)
                .set_number(input_block_number)
                .build(),
        );

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);

        let actual_block = fork_source
            .get_block_by_hash(input_block_hash, false)
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(input_block_number), actual_block.number);

        let actual_block = fork_source
            .get_block_by_hash(input_block_hash, false)
            .expect("failed fetching cached block by hash")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(input_block_number), actual_block.number);
    }

    #[test]
    fn test_get_block_by_number_full_is_cached() {
        let input_block_hash = H256::repeat_byte(0x01);
        let input_block_number = 8;

        let mock_server = testing::MockServer::run();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockByNumber",
                "params": [
                    format!("{input_block_number:#x}"),
                    true
                ],
            }),
            testing::BlockResponseBuilder::new()
                .set_hash(input_block_hash)
                .set_number(input_block_number)
                .build(),
        );

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);

        let actual_block = fork_source
            .get_block_by_number(
                zksync_types::api::BlockNumber::Number(U64::from(input_block_number)),
                true,
            )
            .expect("failed fetching block by number")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(input_block_number), actual_block.number);

        let actual_block = fork_source
            .get_block_by_number(
                zksync_types::api::BlockNumber::Number(U64::from(input_block_number)),
                true,
            )
            .expect("failed fetching cached block by number")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(input_block_number), actual_block.number);
    }

    #[test]
    fn test_get_block_by_number_minimal_is_cached() {
        let input_block_hash = H256::repeat_byte(0x01);
        let input_block_number = 8;

        let mock_server = testing::MockServer::run();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockByNumber",
                "params": [
                    format!("{input_block_number:#x}"),
                    false
                ],
            }),
            testing::BlockResponseBuilder::new()
                .set_hash(input_block_hash)
                .set_number(input_block_number)
                .build(),
        );

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);

        let actual_block = fork_source
            .get_block_by_number(BlockNumber::Number(U64::from(input_block_number)), false)
            .expect("failed fetching block by number")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(input_block_number), actual_block.number);

        let actual_block = fork_source
            .get_block_by_number(BlockNumber::Number(U64::from(input_block_number)), false)
            .expect("failed fetching cached block by number")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(input_block_number), actual_block.number);
    }

    #[test]
    fn test_get_raw_block_transactions_is_cached() {
        let input_block_number = 8u32;

        let mock_server = testing::MockServer::run();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getRawBlockTransactions",
                "params": [
                    input_block_number,
                ],
            }),
            testing::RawTransactionsResponseBuilder::new()
                .add(1)
                .build(),
        );

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);

        let actual_raw_transactions = fork_source
            .get_raw_block_transactions(L2BlockNumber(input_block_number))
            .expect("failed fetching block raw transactions");
        assert_eq!(1, actual_raw_transactions.len());

        let actual_raw_transactions = fork_source
            .get_raw_block_transactions(L2BlockNumber(input_block_number))
            .expect("failed fetching cached block raw transactions");
        assert_eq!(1, actual_raw_transactions.len());
    }

    #[test]
    fn test_get_transactions_is_cached() {
        let input_tx_hash = H256::repeat_byte(0x01);

        let mock_server = testing::MockServer::run();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getTransactionByHash",
                "params": [
                    input_tx_hash,
                ],
            }),
            testing::TransactionResponseBuilder::new()
                .set_hash(input_tx_hash)
                .build(),
        );

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);

        let actual_transaction = fork_source
            .get_transaction_by_hash(input_tx_hash)
            .expect("failed fetching transaction")
            .expect("no transaction");
        assert_eq!(input_tx_hash, actual_transaction.hash);

        let actual_transaction = fork_source
            .get_transaction_by_hash(input_tx_hash)
            .expect("failed fetching cached transaction")
            .expect("no transaction");
        assert_eq!(input_tx_hash, actual_transaction.hash);
    }

    #[test]
    fn test_get_transaction_details() {
        let input_tx_hash = H256::repeat_byte(0x01);
        let mock_server = testing::MockServer::run();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getTransactionDetails",
                "params": [
                    input_tx_hash,
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

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);
        let transaction_details = fork_source
            .get_transaction_details(input_tx_hash)
            .expect("failed fetching transaction")
            .expect("no transaction");
        assert_eq!(
            transaction_details.initiator_address,
            Address::from_str("0x63ab285cd87a189f345fed7dd4e33780393e01f0").unwrap()
        );
    }

    #[test]
    fn test_get_block_details() {
        let miniblock = L2BlockNumber::from(16474138);
        let mock_server = testing::MockServer::run();
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
                  "l2FairGasPrice": 50000000u64,
                  "fairPubdataPrice": 100u64,
                  "baseSystemContractsHashes": {
                    "bootloader": "0x0100089b8a2f2e6a20ba28f02c9e0ed0c13d702932364561a0ea61621f65f0a8",
                    "default_aa": "0x0100067d16a5485875b4249040bf421f53e869337fe118ec747cf40a4c777e5f"
                  },
                  "operatorAddress": "0xa9232040bf0e0aea2578a5b2243f2916dbfc0a69",
                  "protocolVersion": "Version15",
                },
                "id": 0
              }),
        );

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);
        let block_details = fork_source
            .get_block_details(miniblock)
            .expect("failed fetching transaction")
            .expect("no transaction");
        assert_eq!(
            block_details.operator_address,
            Address::from_str("0xa9232040bf0e0aea2578a5b2243f2916dbfc0a69").unwrap()
        );
    }

    #[test]
    fn test_get_bridge_contracts_is_cached() {
        let input_bridge_addresses = BridgeAddresses {
            l1_erc20_default_bridge: Some(H160::repeat_byte(0x1)),
            l2_erc20_default_bridge: Some(H160::repeat_byte(0x2)),
            l1_shared_default_bridge: Some(H160::repeat_byte(0x5)),
            l2_shared_default_bridge: Some(H160::repeat_byte(0x2)),
            l1_weth_bridge: Some(H160::repeat_byte(0x3)),
            l2_weth_bridge: Some(H160::repeat_byte(0x4)),
            l2_legacy_shared_bridge: Some(H160::repeat_byte(0x6)),
        };
        let mock_server = testing::MockServer::run();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "zks_getBridgeContracts",
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
                    "l1Erc20SharedBridge": format!("{:#x}", input_bridge_addresses.l1_shared_default_bridge.unwrap()),
                    "l2Erc20SharedBridge": format!("{:#x}", input_bridge_addresses.l2_shared_default_bridge.unwrap()),
                    "l1Erc20DefaultBridge": format!("{:#x}", input_bridge_addresses.l1_erc20_default_bridge.unwrap()),
                    "l2Erc20DefaultBridge": format!("{:#x}", input_bridge_addresses.l2_erc20_default_bridge.unwrap()),
                    "l1WethBridge": format!("{:#x}", input_bridge_addresses.l1_weth_bridge.unwrap()),
                    "l2WethBridge": format!("{:#x}", input_bridge_addresses.l2_weth_bridge.unwrap())
                },
                "id": 0
            }),
        );

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);

        let actual_bridge_addresses = fork_source
            .get_bridge_contracts()
            .expect("failed fetching bridge addresses");
        testing::assert_bridge_addresses_eq(&input_bridge_addresses, &actual_bridge_addresses);

        let actual_bridge_addresses = fork_source
            .get_bridge_contracts()
            .expect("failed fetching bridge addresses");
        testing::assert_bridge_addresses_eq(&input_bridge_addresses, &actual_bridge_addresses);
    }

    #[test]
    fn test_get_confirmed_tokens_is_cached() {
        let mock_server = testing::MockServer::run();
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

        let fork_source = HttpForkSource::new(mock_server.url(), CacheConfig::Memory);

        let tokens = fork_source
            .get_confirmed_tokens(0, 100)
            .expect("failed fetching tokens");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].symbol, "cbETH");

        let tokens = fork_source
            .get_confirmed_tokens(0, 100)
            .expect("failed fetching tokens");
        assert_eq!(tokens.len(), 1);
    }
}
