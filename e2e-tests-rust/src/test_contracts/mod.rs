use alloy::contract::SolCallBuilder;
use alloy::network::{Ethereum, Network, ReceiptResponse, TransactionBuilder};
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy_zksync::network::transaction_request::TransactionRequest;
use alloy_zksync::network::Zksync;
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
    alloy::sol!(
        #[sol(rpc)]
        SimpleErc20Evm,
        "src/test_contracts/evm-artifacts/SimpleERC20.json"
    );
}

pub struct Counter<N: Network, P: Provider<N>>(private::Counter::CounterInstance<(), P, N>);

impl<P: Provider<Zksync> + Clone> Counter<Zksync, P> {
    pub async fn deploy(provider: P) -> anyhow::Result<Self> {
        let tx = TransactionRequest::default()
            .with_create_params(private::Counter::BYTECODE.clone().into(), vec![], vec![])?
            .with_gas_limit(100_000)
            .with_gas_per_pubdata(U256::from(100))
            .with_max_fee_per_gas(1000)
            .with_max_priority_fee_per_gas(1000);
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
        Ok(self.0.get().call().await?._0)
    }

    pub fn increment(
        &self,
        x: impl TryInto<U256, Error = impl Debug>,
    ) -> SolCallBuilder<(), &P, private::Counter::incrementCall, N> {
        self.0.increment(x.try_into().unwrap())
    }
}

pub struct SimpleErc20<N: Network, P: Provider<N>>(
    private::SimpleErc20Evm::SimpleErc20EvmInstance<(), P, N>,
);

impl<P: Provider<Ethereum> + Clone> SimpleErc20<Ethereum, P> {
    pub async fn deploy_evm(provider: P, name: String, symbol: String) -> anyhow::Result<Self> {
        let evm_contract = private::SimpleErc20Evm::deploy(provider.clone(), name, symbol).await?;

        Ok(Self(private::SimpleErc20Evm::new(
            *evm_contract.address(),
            provider,
        )))
    }
}

impl<N: Network, P: Provider<N>> SimpleErc20<N, P> {
    pub fn address(&self) -> &Address {
        self.0.address()
    }

    pub async fn balance_of(&self, address: Address) -> alloy::contract::Result<U256> {
        Ok(self.0.balanceOf(address).call().await?._0)
    }

    pub fn transfer(
        &self,
        to: Address,
        amount: U256,
    ) -> SolCallBuilder<(), &P, private::SimpleErc20Evm::transferCall, N> {
        self.0.transfer(to, amount)
    }

    pub fn mint(
        &self,
        to: Address,
        amount: U256,
    ) -> SolCallBuilder<(), &P, private::SimpleErc20Evm::mintCall, N> {
        self.0.mint(to, amount)
    }
}
