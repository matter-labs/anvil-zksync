use alloy::providers::Provider;
use alloy::transports::Transport;
use alloy_zksync::network::Zksync;

pub mod utils;

pub trait EraTestNodeApiProvider<T>: Provider<T, Zksync>
where
    T: Transport + Clone,
{
    // Empty but can be extended with custom RPC methods as below

    // fn get_auto_mine(&self) -> ProviderCall<T, NoParams, bool> {
    //     self.client().request_noparams("anvil_getAutomine").into()
    // }
}

impl<P, T> EraTestNodeApiProvider<T> for P
where
    T: Transport + Clone,
    P: Provider<T, Zksync>,
{
}
