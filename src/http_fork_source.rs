use eyre::Context;
use zksync_web3_decl::{
    jsonrpsee::http_client::{HttpClient, HttpClientBuilder},
    namespaces::{EthNamespaceClient, ZksNamespaceClient},
};

use crate::fork::{block_on, ForkSource};

#[derive(Debug)]
/// Fork source that gets the data via HTTP requests.
pub struct HttpForkSource {
    /// URL for the network to fork.
    pub fork_url: String,
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
        let client = self.create_client();
        block_on(async move { client.get_transaction_by_hash(hash).await })
            .wrap_err("fork http client failed")
    }

    fn get_raw_block_transactions(
        &self,
        block_number: zksync_basic_types::MiniblockNumber,
    ) -> eyre::Result<Vec<zksync_types::Transaction>> {
        let client = self.create_client();
        block_on(async move { client.get_raw_block_transactions(block_number).await })
            .wrap_err("fork http client failed")
    }
}
