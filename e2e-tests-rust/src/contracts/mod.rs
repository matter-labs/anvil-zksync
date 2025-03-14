use alloy::contract::SolCallBuilder;
use alloy::network::{Ethereum, TransactionBuilder};
use alloy::primitives::{address, Address, Bytes, ChainId, FixedBytes, U256};
use alloy::providers::{PendingTransactionBuilder, Provider};
use alloy::transports::Transport;
use alloy_zksync::network::transaction_request::TransactionRequest;
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

    alloy::sol!(IMailbox, "src/contracts/artifacts/IMailbox.json");
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

    // TODO: Port logic to alloy-zksync
    /// Requests execution of an L2 transaction from L1.
    pub async fn request_execute<T2: Transport + Clone>(
        &self,
        l2_provider: &impl Provider<T2, Zksync>,
        tx: TransactionRequest,
    ) -> anyhow::Result<PendingTransactionBuilder<T2, Zksync>> {
        let max_fee_per_gas = tx.max_fee_per_gas().unwrap();
        let max_priority_fee_per_gas = tx.max_priority_fee_per_gas().unwrap_or(0);
        let gas_per_pubdata_byte_limit = tx.gas_per_pubdata().unwrap_or(U256::from(800));
        let l2_value = tx.value().unwrap_or(U256::from(0));
        let to = tx.to().unwrap();
        let from = tx.from().unwrap();
        let calldata = tx.input().cloned().unwrap_or(Bytes::new());
        let factory_deps = tx.factory_deps().cloned().unwrap_or(vec![]);

        let gas_limit = l2_provider.estimate_gas_l1_to_l2(tx).await?;
        let base_cost = self
            .instance
            .l2TransactionBaseCost(
                U256::from(self.chain_id),
                U256::from(max_fee_per_gas),
                gas_limit,
                gas_per_pubdata_byte_limit,
            )
            .call()
            .await?
            ._0;
        // Assuming no operator tip for now
        let operator_tip = U256::from(0);
        let l2_costs = base_cost + operator_tip + l2_value;
        let request = private::IBridgehub::L2TransactionRequestDirect {
            chainId: U256::from(self.chain_id),
            mintValue: l2_costs,
            l2Contract: to,
            l2Value: l2_value,
            l2Calldata: calldata,
            l2GasLimit: gas_limit,
            l2GasPerPubdataByteLimit: gas_per_pubdata_byte_limit,
            factoryDeps: factory_deps,
            refundRecipient: from,
        };
        let l1_value = base_cost + l2_value;
        let l1_tx = self
            .instance
            .requestL2TransactionDirect(request)
            .from(from)
            .value(l1_value)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas)
            .into_transaction_request();
        let l1_tx_receipt = self
            .instance
            .provider()
            .send_transaction(l1_tx)
            .await?
            .get_receipt()
            .await?;
        let l2_tx_hash = l1_tx_receipt
            .inner
            .logs()
            .iter()
            .find_map(|log| {
                // TODO: Check that the log came from diamond proxy
                if let Ok(request) = log.log_decode::<private::IMailbox::NewPriorityRequest>() {
                    Some(request.inner.data.txHash)
                } else {
                    None
                }
            })
            .context("NewPriorityRequest event log was not found in L1 -> L2 transaction")?;
        Ok(PendingTransactionBuilder::new(
            l2_provider.root().clone(),
            l2_tx_hash,
        ))
    }
}
