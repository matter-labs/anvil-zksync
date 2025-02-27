use alloy::contract::SolCallBuilder;
use alloy::network::Ethereum;
use alloy::primitives::{address, Address, Bytes, FixedBytes, U256};
use alloy::providers::Provider;
use alloy::transports::Transport;
use alloy_zksync::network::Zksync;
use std::fmt::Debug;

mod private {
    alloy::sol!(
        #[sol(rpc)]
        "../contracts/l1-contracts/contracts/common/interfaces/IL1Messenger.sol"
    );

    alloy::sol!(
        #[sol(rpc)]
        IBridgehub,
        "../contracts/l1-contracts/out/IBridgehub.sol/IBridgehub.json"
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

pub struct Bridgehub<T: Transport + Clone, P: Provider<T, Ethereum>>(
    private::IBridgehub::IBridgehubInstance<T, P, Ethereum>,
);

impl<T: Transport + Clone, P: Provider<T, Ethereum>> Bridgehub<T, P> {
    pub fn new(provider: P) -> Self {
        // TODO: Make bridgehub address dynamic depending on zkstack config
        Self(private::IBridgehub::new(
            address!("c209a42a0cf0ead398206d1feb4e8c314d753b92"),
            provider,
        ))
    }

    pub fn address(&self) -> &Address {
        self.0.address()
    }

    pub fn prove_l2_log_inclusion(
        &self,
        batch_number: impl TryInto<U256, Error = impl Debug>,
        index: impl TryInto<U256, Error = impl Debug>,
        log: L2Log,
        proof: Vec<FixedBytes<32>>,
    ) -> SolCallBuilder<T, &P, private::IBridgehub::proveL2LogInclusionCall, Ethereum> {
        // TODO: Make chain id dynamic depending on zkstack config
        self.0.proveL2LogInclusion(
            U256::from(260),
            batch_number.try_into().unwrap(),
            index.try_into().unwrap(),
            log,
            proof,
        )
    }

    pub fn prove_l2_message_inclusion(
        &self,
        batch_number: impl TryInto<U256, Error = impl Debug>,
        index: impl TryInto<U256, Error = impl Debug>,
        msg: L2Message,
        proof: Vec<FixedBytes<32>>,
    ) -> SolCallBuilder<T, &P, private::IBridgehub::proveL2MessageInclusionCall, Ethereum> {
        // TODO: Make chain id dynamic depending on zkstack config
        self.0.proveL2MessageInclusion(
            U256::from(260),
            batch_number.try_into().unwrap(),
            index.try_into().unwrap(),
            msg,
            proof,
        )
    }
}
