use alloy::network::Network;
use alloy::primitives::TxHash;
use alloy::providers::{Provider, ProviderCall};
use alloy::rpc::client::NoParams;
use alloy::serde::WithOtherFields;
use alloy::transports::{Transport, TransportResult};
use alloy_zksync::network::Zksync;

/// RPC interface that gives access to methods specific to anvil-zksync.
#[allow(clippy::type_complexity)]
pub trait AnvilZKsyncApi<T>: Provider<T, Zksync>
where
    T: Transport + Clone,
{
    /// Custom version of [`alloy::providers::ext::AnvilApi::anvil_mine_detailed`] that returns
    /// block representation with transactions that contain extra custom fields.
    fn anvil_zksync_mine_detailed(
        &self,
    ) -> ProviderCall<
        T,
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
}

impl<P, T> AnvilZKsyncApi<T> for P
where
    T: Transport + Clone,
    P: Provider<T, Zksync>,
{
}
