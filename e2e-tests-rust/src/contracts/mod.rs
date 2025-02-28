use alloy::contract::SolCallBuilder;
use alloy::network::Ethereum;
use alloy::primitives::{address, Address, Bytes, ChainId, FixedBytes, U256};
use alloy::providers::Provider;
use alloy::transports::Transport;
use alloy_zksync::network::Zksync;
use alloy_zksync::provider::ZksyncProvider;
use anyhow::Context;
use std::fmt::Debug;

#[allow(clippy::all)]
mod private {
    alloy::sol!(
        #[sol(rpc)]
        IL1Messenger,
        "src/contracts/artifacts/IL1Messenger.json"
    );

    alloy::sol!(
        #[sol(rpc)]
        IBridgehub,
        "src/contracts/artifacts/IBridgehub.json"
    );
}

const L1_MESSENGER_ADDRESS: Address = address!("0000000000000000000000000000000000008008");

pub struct L1Messenger<T: Transport + Clone, P: Provider<T, Zksync>>(
    private::IL1Messenger::IL1MessengerInstance<T, P, Zksync>,
);

impl<T: Transport + Clone, P: Provider<T, Zksync>> L1Messenger<T, P> {
    pub fn new(provider: P) -> Self {
        Self(private::IL1Messenger::new(L1_MESSENGER_ADDRESS, provider))
    }

    pub fn address(&self) -> &Address {
        self.0.address()
    }

    pub fn send_to_l1(
        &self,
        bytes: impl Into<Bytes>,
    ) -> SolCallBuilder<T, &P, private::IL1Messenger::sendToL1Call, Zksync> {
        self.0.sendToL1(bytes.into())
    }
}

pub type L2Log = private::IBridgehub::L2Log;
pub type L2Message = private::IBridgehub::L2Message;

pub struct Bridgehub<T: Transport + Clone, P: Provider<T, Ethereum>> {
    instance: private::IBridgehub::IBridgehubInstance<T, P, Ethereum>,
    chain_id: ChainId,
}

impl<T: Transport + Clone, P: Provider<T, Ethereum>> Bridgehub<T, P> {
    pub async fn new<T2: Transport + Clone>(
        l1_provider: P,
        l2_provider: &impl Provider<T2, Zksync>,
    ) -> anyhow::Result<Self> {
        let chain_id = l2_provider.get_chain_id().await?;
        let birdgehub_address = l2_provider
            .get_bridgehub_contract()
            .await?
            .context("anvil-zksync does not have bridgehub contract; is it running in L1 mode?")?;
        Ok(Self {
            instance: private::IBridgehub::new(birdgehub_address, l1_provider),
            chain_id,
        })
    }

    pub fn address(&self) -> &Address {
        self.instance.address()
    }

    pub fn prove_l2_message_inclusion(
        &self,
        batch_number: impl TryInto<U256, Error = impl Debug>,
        index: impl TryInto<U256, Error = impl Debug>,
        msg: L2Message,
        proof: Vec<FixedBytes<32>>,
    ) -> SolCallBuilder<T, &P, private::IBridgehub::proveL2MessageInclusionCall, Ethereum> {
        self.instance.proveL2MessageInclusion(
            U256::from(self.chain_id),
            batch_number.try_into().unwrap(),
            index.try_into().unwrap(),
            msg,
            proof,
        )
    }
}
