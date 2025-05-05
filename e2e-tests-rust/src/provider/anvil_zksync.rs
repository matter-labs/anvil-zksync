use alloy::network::Network;
use alloy::primitives::TxHash;
use alloy::providers::{Provider, ProviderCall};
use alloy::rpc::client::NoParams;
use alloy::serde::WithOtherFields;
use alloy::transports::TransportResult;
use alloy_zksync::network::Zksync;

/// RPC interface that gives access to methods specific to anvil-zksync.
#[allow(clippy::type_complexity)]
pub trait AnvilZKsyncApi: Provider<Zksync> {
    /// Custom version of [`alloy::providers::ext::AnvilApi::anvil_mine_detailed`] that returns
    /// block representation with transactions that contain extra custom fields.
    fn anvil_zksync_mine_detailed(
        &self,
    ) -> ProviderCall<
        NoParams,
        alloy::rpc::types::Block<
            WithOtherFields<<Zksync as Network>::TransactionResponse>,
            <Zksync as Network>::HeaderResponse,
        >,
    > {
        self.client().request_noparams("anvil_mine_detailed").into()
    }

    /// Commits batch with given number to L1
    async fn anvil_commit_batch(&self, batch_number: u64) -> TransportResult<TxHash> {
        self.client()
            .request("anvil_zks_commitBatch", (batch_number,))
            .await
    }

    /// Proves batch with given number on L1
    async fn anvil_prove_batch(&self, batch_number: u64) -> TransportResult<TxHash> {
        self.client()
            .request("anvil_zks_proveBatch", (batch_number,))
            .await
    }

    /// Executes batch with given number on L1
    async fn anvil_execute_batch(&self, batch_number: u64) -> TransportResult<TxHash> {
        self.client()
            .request("anvil_zks_executeBatch", (batch_number,))
            .await
    }
}

impl<P> AnvilZKsyncApi for P where P: Provider<Zksync> {}
