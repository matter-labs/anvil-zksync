use alloy::contract::SolCallBuilder;
use alloy::network::{Ethereum, Network, ReceiptResponse};
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy_zksync::network::Zksync;
use alloy_zksync::network::transaction_request::TransactionRequest;
use std::fmt::Debug;

#[allow(clippy::all)]
mod private {
    alloy::sol!(
        #[sol(rpc)]
        Counter,
        "src/test_contracts/zk-artifacts/Counter.json"
    );
    alloy::sol!(
        #[sol(rpc)]
        CounterEvm,
        "src/test_contracts/evm-artifacts/Counter.json"
    );
}

pub struct Counter<N: Network, P: Provider<N>>(private::Counter::CounterInstance<P, N>);

impl<P: Provider<Zksync> + Clone> Counter<Zksync, P> {
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
}

impl<P: Provider<Ethereum> + Clone> Counter<Ethereum, P> {
    pub async fn deploy_evm(provider: P) -> anyhow::Result<Self> {
        let evm_contract = private::CounterEvm::deploy(provider.clone()).await?;

        Ok(Self(private::Counter::new(
            *evm_contract.address(),
            provider,
        )))
    }
}

impl<N: Network, P: Provider<N>> Counter<N, P> {
    pub fn address(&self) -> &Address {
        self.0.address()
    }

    pub async fn get(&self) -> alloy::contract::Result<U256> {
        self.0.get().call().await
    }

    pub fn increment(
        &self,
        x: impl TryInto<U256, Error = impl Debug>,
    ) -> SolCallBuilder<&P, private::Counter::incrementCall, N> {
        self.0.increment(x.try_into().unwrap())
    }

    pub fn increment_with_revert(
        &self,
        x: impl TryInto<U256, Error = impl Debug>,
        should_revert: bool,
    ) -> SolCallBuilder<&P, private::Counter::incrementWithRevertCall, N> {
        self.0
            .incrementWithRevert(x.try_into().unwrap(), should_revert)
    }
}
