use alloy::network::Network;
use alloy::primitives::{Address, TxHash};
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
    fn get_auto_mine(&self) -> ProviderCall<T, NoParams, bool> {
        self.client().request_noparams("anvil_getAutomine").into()
    }

    fn set_auto_mine(&self, enable: bool) -> ProviderCall<T, (bool,), ()> {
        self.client().request("anvil_setAutomine", (enable,)).into()
    }

    fn set_interval_mining(&self, seconds: u64) -> ProviderCall<T, (u64,), ()> {
        self.client()
            .request("anvil_setIntervalMining", (seconds,))
            .into()
    }

    fn drop_transaction(&self, hash: TxHash) -> ProviderCall<T, (TxHash,), Option<TxHash>> {
        self.client()
            .request("anvil_dropTransaction", (hash,))
            .into()
    }

    fn drop_all_transactions(&self) -> ProviderCall<T, NoParams, ()> {
        self.client()
            .request_noparams("anvil_dropAllTransactions")
            .into()
    }

    fn remove_pool_transactions(&self, address: Address) -> ProviderCall<T, (Address,), ()> {
        self.client()
            .request("anvil_removePoolTransactions", (address,))
            .into()
    }

    fn mine(
        &self,
        num_blocks: Option<u64>,
        interval: Option<u64>,
    ) -> ProviderCall<T, (Option<u64>, Option<u64>), ()> {
        self.client()
            .request("anvil_mine", (num_blocks, interval))
            .into()
    }

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
