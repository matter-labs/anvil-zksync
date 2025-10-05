use alloy::contract::SolCallBuilder;
use alloy::network::ReceiptResponse as _;
use alloy::network::{Ethereum, TransactionBuilder};
use alloy::primitives::{Address, Bytes, ChainId, FixedBytes, U256, address};
use alloy::providers::{DynProvider, PendingTransactionBuilder, Provider};
use alloy::rpc::types::TransactionReceipt;
use alloy_zksync::network::Zksync;
use alloy_zksync::network::receipt_response::ReceiptResponse;
use alloy_zksync::network::transaction_request::TransactionRequest;
use alloy_zksync::provider::ZksyncProvider;
use anyhow::Context;
use std::fmt::Debug;

#[allow(clippy::all)]
mod private {
    alloy::sol!(
        #[sol(rpc)]
        #[derive(Debug)]
        IL1Messenger,
        "src/contracts/artifacts/IL1Messenger.json"
    );

    alloy::sol!(
        #[sol(rpc)]
        IBridgehub,
        "src/contracts/artifacts/IBridgehubBase.json"
    );

    alloy::sol!(
        #[sol(rpc)]
        IBaseToken,
        "src/contracts/artifacts/IBaseToken.json"
    );

    alloy::sol!(
        #[sol(rpc)]
        IL1Nullifier,
        "src/contracts/artifacts/IL1Nullifier.json"
    );

    alloy::sol!(
        #[sol(rpc)]
        IL1AssetRouter,
        "src/contracts/artifacts/IL1AssetRouter.json"
    );

    alloy::sol!(IMailbox, "src/contracts/artifacts/IMailbox.json");

    alloy::sol!(IExecutor, "src/contracts/artifacts/IExecutor.json");
}

pub use private::IExecutor::BlockExecution;

const L1_MESSENGER_ADDRESS: Address = address!("0000000000000000000000000000000000008008");
const L2_BASE_TOKEN_ADDRESS: Address = address!("000000000000000000000000000000000000800a");

pub struct L1Messenger<P: Provider<Zksync>>(private::IL1Messenger::IL1MessengerInstance<P, Zksync>);

impl<P: Provider<Zksync>> L1Messenger<P> {
    pub fn new(provider: P) -> Self {
        Self(private::IL1Messenger::new(L1_MESSENGER_ADDRESS, provider))
    }

    pub fn address(&self) -> &Address {
        self.0.address()
    }

    pub fn send_to_l1(
        &self,
        bytes: impl Into<Bytes>,
    ) -> SolCallBuilder<&P, private::IL1Messenger::sendToL1Call, Zksync> {
        self.0.sendToL1(bytes.into())
    }
}

pub type L2Log = private::IBridgehub::L2Log;
pub type L2Message = private::IBridgehub::L2Message;

pub struct Bridgehub<P: Provider<Ethereum>> {
    instance: private::IBridgehub::IBridgehubInstance<P, Ethereum>,
    chain_id: ChainId,
}

impl<P: Provider<Ethereum>> Bridgehub<P> {
    pub async fn new(l1_provider: P, l2_provider: &impl Provider<Zksync>) -> anyhow::Result<Self> {
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
    ) -> SolCallBuilder<&P, private::IBridgehub::proveL2MessageInclusionCall, Ethereum> {
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
    pub async fn request_execute(
        &self,
        l2_provider: &impl Provider<Zksync>,
        tx: TransactionRequest,
    ) -> anyhow::Result<PendingTransactionBuilder<Zksync>> {
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
            .await?;
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

pub struct L2BaseToken<P: Provider<Zksync>>(private::IBaseToken::IBaseTokenInstance<P, Zksync>);

impl<P: Provider<Zksync>> L2BaseToken<P> {
    pub fn new(l2_provider: P) -> Self {
        Self(private::IBaseToken::new(L2_BASE_TOKEN_ADDRESS, l2_provider))
    }

    pub fn address(&self) -> &Address {
        self.0.address()
    }

    pub async fn withdraw(
        &self,
        l1_receiver: Address,
        value: U256,
    ) -> anyhow::Result<ReceiptResponse> {
        let receipt = self
            .0
            .withdraw(l1_receiver)
            .value(value)
            .send()
            .await?
            .get_receipt()
            .await?;
        assert!(receipt.status(), "L2->L1 withdraw failed");
        Ok(receipt)
    }
}

pub struct L1Nullifier<P: Provider<Ethereum>> {
    instance: private::IL1Nullifier::IL1NullifierInstance<P, Ethereum>,
    l2_provider: DynProvider<Zksync>,
}

impl<P: Provider<Ethereum>> L1Nullifier<P> {
    pub fn new(address: Address, l1_provider: P, l2_provider: DynProvider<Zksync>) -> Self {
        Self {
            instance: private::IL1Nullifier::new(address, l1_provider),
            l2_provider,
        }
    }

    pub fn address(&self) -> &Address {
        self.instance.address()
    }

    pub async fn finalize_withdrawal(
        &self,
        withdrawal_l2_receipt: ReceiptResponse,
    ) -> anyhow::Result<TransactionReceipt> {
        let l1_message_sent = withdrawal_l2_receipt
            .logs()
            .iter()
            .find_map(|log| {
                if log.address() != L1_MESSENGER_ADDRESS {
                    return None;
                }
                log.log_decode::<private::IL1Messenger::L1MessageSent>()
                    .ok()
            })
            .expect("no `L1MessageSent` events found in withdrawal receipt");
        let (l2_to_l1_log_index, _) = withdrawal_l2_receipt
            .l2_to_l1_logs()
            .iter()
            .enumerate()
            .find(|(_, log)| log.sender == L1_MESSENGER_ADDRESS)
            .expect("no L2->L1 logs found in withdrawal receipt");
        let proof = self
            .l2_provider
            .get_l2_to_l1_log_proof(
                withdrawal_l2_receipt.transaction_hash(),
                Some(l2_to_l1_log_index),
            )
            .await?
            .expect("anvil-zksync failed to provide proof for withdrawal log");
        let finalize_withdrawal_l1_receipt = self
            .instance
            .finalizeDeposit(private::IL1Nullifier::FinalizeL1DepositParams {
                chainId: U256::from(self.l2_provider.get_chain_id().await?),
                l2BatchNumber: U256::from(withdrawal_l2_receipt.l1_batch_number().unwrap()),
                l2MessageIndex: U256::from(proof.id),
                l2Sender: l1_message_sent.inner._sender,
                l2TxNumberInBatch: withdrawal_l2_receipt
                    .l1_batch_tx_index()
                    .unwrap()
                    .try_into()
                    .unwrap(),
                message: l1_message_sent.inner._message.clone(),
                merkleProof: proof.proof,
            })
            .send()
            .await?
            .get_receipt()
            .await?;
        assert!(
            finalize_withdrawal_l1_receipt.status(),
            "failed to finalize L2->L1 withdrawal"
        );
        Ok(finalize_withdrawal_l1_receipt)
    }
}

pub struct L1AssetRouter<P: Provider<Ethereum>> {
    instance: private::IL1AssetRouter::IL1AssetRouterInstance<P, Ethereum>,
    l2_provider: DynProvider<Zksync>,
}

impl<P: Provider<Ethereum> + Clone> L1AssetRouter<P> {
    pub async fn new(l1_provider: P, l2_provider: DynProvider<Zksync>) -> Self {
        let bridge_addresses = l2_provider.get_bridge_contracts().await.unwrap();
        Self {
            instance: private::IL1AssetRouter::new(
                bridge_addresses.l1_shared_default_bridge.unwrap(),
                l1_provider,
            ),
            l2_provider,
        }
    }

    pub fn address(&self) -> &Address {
        self.instance.address()
    }

    pub async fn l1_nullifier(&self) -> anyhow::Result<L1Nullifier<P>> {
        let l1_nullifier_address = self.instance.L1_NULLIFIER().call().await?;
        Ok(L1Nullifier::new(
            l1_nullifier_address,
            self.instance.provider().clone(),
            self.l2_provider.clone(),
        ))
    }
}
