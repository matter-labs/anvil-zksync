use eyre::Context;
use zksync_web3_decl::{
    jsonrpsee::http_client::{HttpClient, HttpClientBuilder},
    namespaces::{EthNamespaceClient, ZksNamespaceClient},
};

use crate::{
    cache::Cache,
    fork::{block_on, ForkSource},
};

#[derive(Debug)]
/// Fork source that gets the data via HTTP requests.
pub struct HttpForkSource {
    /// URL for the network to fork.
    pub fork_url: String,
    cache: Cache,
}

impl HttpForkSource {
    pub fn create_client(&self) -> HttpClient {
        HttpClientBuilder::default()
            .build(self.fork_url.clone())
            .unwrap_or_else(|_| panic!("Unable to create a client for fork: {}", self.fork_url))
    }
}

impl ForkSource for HttpForkSource {
    fn get_storage_at(
        &self,
        address: zksync_basic_types::Address,
        idx: zksync_basic_types::U256,
        block: Option<zksync_types::api::BlockIdVariant>,
    ) -> eyre::Result<zksync_basic_types::H256> {
        let client = self.create_client();
        block_on(async move { client.get_storage_at(address, idx, block).await })
            .wrap_err("fork http client failed")
    }

    fn get_bytecode_by_hash(
        &self,
        hash: zksync_basic_types::H256,
    ) -> eyre::Result<Option<Vec<u8>>> {
        let client = self.create_client();
        block_on(async move { client.get_bytecode_by_hash(hash).await })
            .wrap_err("fork http client failed")
    }

    fn get_transaction_by_hash(
        &self,
        hash: zksync_basic_types::H256,
    ) -> eyre::Result<Option<zksync_types::api::Transaction>> {
        self.cache
            .transactions
            .get(&hash)
            .cloned()
            .map(|value| Ok(Some(value)))
            .unwrap_or_else(|| {
                let client = self.create_client();
                block_on(async move { client.get_transaction_by_hash(hash).await })
                    .wrap_err("fork http client failed")
                    .and_then(|result| {
                        if let Some(transaction) = &result {
                            self.cache.transactions.insert(hash, transaction.clone());
                        }
                        Ok(result)
                    })
            })
    }

    fn get_raw_block_transactions(
        &self,
        block_number: zksync_basic_types::MiniblockNumber,
    ) -> eyre::Result<Vec<zksync_types::Transaction>> {
        let mut block_hash = zksync_basic_types::H256::zero();
        let mut block_number_mapped = false;

        self.cache
            .block_hashes
            .get(&(block_number.0 as u64))
            .and_then(|hash| {
                block_number_mapped = true;
                block_hash = *hash;
                self.cache.block_raw_transactions.get(hash)
            })
            .cloned()
            .map(|value| Ok(value))
            .unwrap_or_else(|| {
                let client = self.create_client();
                block_on(async move { client.get_raw_block_transactions(block_number).await })
                    .wrap_err("fork http client failed")
                    .and_then(|result| {
                        if !block_number_mapped {
                            self.cache
                                .block_hashes
                                .insert(block_number.0 as u64, block_hash);
                        }

                        self.cache
                            .block_raw_transactions
                            .insert(block_hash, result.clone());
                        Ok(result)
                    })
            })
    }

    fn get_block_by_hash(
        &self,
        hash: zksync_basic_types::H256,
        full_transactions: bool,
    ) -> eyre::Result<Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>> {
        let mut cache = if full_transactions {
            self.cache.blocks_full
        } else {
            self.cache.blocks_min
        };

        cache
            .get(&hash)
            .cloned()
            .map(|value| Ok(Some(value)))
            .unwrap_or_else(|| {
                let client = self.create_client();
                block_on(async move { client.get_block_by_hash(hash, full_transactions).await })
                    .wrap_err("fork http client failed")
                    .and_then(|result| {
                        if let Some(transaction) = &result {
                            cache.insert(hash, transaction.clone());
                        }
                        Ok(result)
                    })
            })
    }

    fn get_block_by_number(
        &self,
        block_number: zksync_types::api::BlockNumber,
        full_transactions: bool,
    ) -> eyre::Result<Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>> {
        let number = match block_number {
            zksync_types::api::BlockNumber::Number(block_number) => Some(block_number),
            _ => None,
        };
        let mut cache = if full_transactions {
            self.cache.blocks_full
        } else {
            self.cache.blocks_min
        };

        let mut block_hash = zksync_basic_types::H256::zero();
        let mut block_number_mapped = false;
        number
            .and_then(|number| {
                self.cache.block_hashes.get(&number.as_u64()).map(|hash| {
                    block_number_mapped = true;
                    block_hash = *hash;
                    *hash
                })
            })
            .and_then(|hash| cache.get(&hash))
            .cloned()
            .map(|value| Ok(Some(value)))
            .unwrap_or_else(|| {
                let client = self.create_client();
                block_on(async move {
                    client
                        .get_block_by_number(block_number, full_transactions)
                        .await
                })
                .wrap_err("fork http client failed")
                .and_then(|result| {
                    if !block_number_mapped {
                        if let Some(number) = number {
                            self.cache.block_hashes.insert(number.as_u64(), block_hash);
                        }
                    }
                    if let Some(block) = &result {
                        cache.insert(block_hash, block.clone());
                    }
                    Ok(result)
                })
            })
    }
}
