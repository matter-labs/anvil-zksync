use zksync_web3_decl::{
    jsonrpsee::http_client::{HttpClient, HttpClientBuilder},
    namespaces::{EthNamespaceClient, ZksNamespaceClient},
};

use crate::fork::{block_on, ForkSource};

/// Fork source that gets the data via HTTP requests.
pub struct HttpForkSource {
    pub fork_url: String,
}

impl HttpForkSource {
    pub fn create_client(&self) -> HttpClient {
        HttpClientBuilder::default()
            .build(self.fork_url.clone())
            .expect("Unable to create a client for fork")
    }
}

impl ForkSource for HttpForkSource {
    fn get_storage_at(
        &self,
        address: zksync_basic_types::Address,
        idx: zksync_basic_types::U256,
        block: Option<zksync_types::api::BlockIdVariant>,
    ) -> zksync_basic_types::H256 {
        let client = self.create_client();
        block_on(async move { client.get_storage_at(address, idx, block).await }).unwrap()
    }

    fn get_bytecode_by_hash(&self, hash: zksync_basic_types::H256) -> Option<Vec<u8>> {
        let client = self.create_client();
        block_on(async move { client.get_bytecode_by_hash(hash).await }).unwrap()
    }

    fn get_transaction_by_hash(
        &self,
        hash: zksync_basic_types::H256,
    ) -> Option<zksync_types::api::Transaction> {
        let client = self.create_client();
        block_on(async move { client.get_transaction_by_hash(hash).await }).unwrap()
    }

    fn get_raw_block_transactions(
        &self,
        block_number: zksync_basic_types::MiniblockNumber,
    ) -> Vec<zksync_types::Transaction> {
        let client = self.create_client();
        block_on(async move { client.get_raw_block_transactions(block_number).await }).unwrap()
    }
}
