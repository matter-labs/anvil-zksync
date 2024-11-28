use alloy::providers::{Provider, ProviderCall};
use alloy::rpc::client::NoParams;
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
}

impl<P, T> EraTestNodeApiProvider<T> for P
where
    T: Transport + Clone,
    P: Provider<T, Zksync>,
{
}
