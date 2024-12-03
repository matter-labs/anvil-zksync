use alloy::network::Network;
use alloy::providers::{Provider, ProviderCall};
use alloy::rpc::client::NoParams;
use alloy::serde::WithOtherFields;
use alloy::transports::Transport;
use alloy_zksync::network::Zksync;

pub mod utils;

pub trait EraTestNodeApiProvider<T>: Provider<T, Zksync>
where
    T: Transport + Clone,
{
    /// Custom version of [`alloy::providers::ext::AnvilApi::anvil_mine_detailed`] that returns
    /// block representation with transactions that contain extra custom fields.
    fn mine_detailed(
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
}

impl<P, T> EraTestNodeApiProvider<T> for P
where
    T: Transport + Clone,
    P: Provider<T, Zksync>,
{
}
