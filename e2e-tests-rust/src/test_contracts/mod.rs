use alloy::contract::SolCallBuilder;
use alloy::network::ReceiptResponse;
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::transports::Transport;
use alloy_zksync::network::transaction_request::TransactionRequest;
use alloy_zksync::network::Zksync;
use std::fmt::Debug;

#[allow(clippy::all)]
mod private {
    alloy::sol!(
        #[sol(rpc)]
        Counter,
        "src/test_contracts/artifacts/Counter.json"
    );
}

pub struct Counter<T: Transport + Clone, P: Provider<T, Zksync>>(
    private::Counter::CounterInstance<T, P, Zksync>,
);

impl<T: Transport + Clone, P: Provider<T, Zksync>> Counter<T, P> {
    pub async fn deploy(provider: P) -> anyhow::Result<Self> {
        let tx = TransactionRequest::default().with_create_params(
            private::Counter::BYTECODE.clone().into(),
            vec![],
            vec![],
        )?;
        let receipt = provider.send_transaction(tx).await?.get_receipt().await?;
        let contract_address = receipt
            .contract_address()
            .expect("Failed to get contract address");

        Ok(Self(private::Counter::new(contract_address, provider)))
    }

    pub fn address(&self) -> &Address {
        self.0.address()
    }

    pub async fn get(&self) -> alloy::contract::Result<U256> {
        Ok(self.0.get().call().await?._0)
    }

    pub fn increment(
        &self,
        x: impl TryInto<U256, Error = impl Debug>,
    ) -> SolCallBuilder<T, &P, private::Counter::incrementCall, Zksync> {
        self.0.increment(x.try_into().unwrap())
    }
}
