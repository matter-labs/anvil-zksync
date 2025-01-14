use crate::error::RpcError;
use anvil_zksync_api_decl::ZksNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use jsonrpsee::core::{async_trait, RpcResult};
use std::collections::HashMap;
use zksync_types::api::state_override::StateOverride;
use zksync_types::api::{
    BlockDetails, BridgeAddresses, L1BatchDetails, L2ToL1LogProof, Proof, ProtocolVersion,
    TransactionDetailedResult, TransactionDetails,
};
use zksync_types::fee::Fee;
use zksync_types::fee_model::{FeeParams, PubdataIndependentBatchFeeModelInput};
use zksync_types::transaction_request::CallRequest;
use zksync_types::web3::Bytes;
use zksync_types::{Address, L1BatchNumber, L2BlockNumber, Transaction, H256, U256, U64};
use zksync_web3_decl::types::Token;

pub struct ZksNamespace {
    node: InMemoryNode,
}

impl ZksNamespace {
    pub fn new(node: InMemoryNode) -> Self {
        Self { node }
    }
}

#[async_trait]
impl ZksNamespaceServer for ZksNamespace {
    async fn estimate_fee(
        &self,
        req: CallRequest,
        // TODO: Support
        _state_override: Option<StateOverride>,
    ) -> RpcResult<Fee> {
        Ok(self
            .node
            .estimate_fee_impl(req)
            .await
            .map_err(RpcError::from)?)
    }

    async fn estimate_gas_l1_to_l2(
        &self,
        _req: CallRequest,
        _state_override: Option<StateOverride>,
    ) -> RpcResult<U256> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_bridgehub_contract(&self) -> RpcResult<Option<Address>> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_main_contract(&self) -> RpcResult<Address> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_testnet_paymaster(&self) -> RpcResult<Option<Address>> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_bridge_contracts(&self) -> RpcResult<BridgeAddresses> {
        Ok(self
            .node
            .get_bridge_contracts_impl()
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_base_token_l1_address(&self) -> RpcResult<Address> {
        Ok(self
            .node
            .get_base_token_l1_address_impl()
            .await
            .map_err(RpcError::from)?)
    }

    async fn l1_chain_id(&self) -> RpcResult<U64> {
        Ok(self
            .node
            .get_chain_id()
            .await
            .map(U64::from)
            .map_err(RpcError::from)?)
    }

    async fn get_confirmed_tokens(&self, from: u32, limit: u8) -> RpcResult<Vec<Token>> {
        Ok(self
            .node
            .get_confirmed_tokens_impl(from, limit)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_all_account_balances(
        &self,
        address: Address,
    ) -> RpcResult<HashMap<Address, U256>> {
        Ok(self
            .node
            .get_all_account_balances_impl(address)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_l2_to_l1_msg_proof(
        &self,
        _block: L2BlockNumber,
        _sender: Address,
        _msg: H256,
        _l2_log_position: Option<usize>,
    ) -> RpcResult<Option<L2ToL1LogProof>> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_l2_to_l1_log_proof(
        &self,
        _tx_hash: H256,
        _index: Option<usize>,
    ) -> RpcResult<Option<L2ToL1LogProof>> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_l1_batch_number(&self) -> RpcResult<U64> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_l2_block_range(&self, _batch: L1BatchNumber) -> RpcResult<Option<(U64, U64)>> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_block_details(
        &self,
        block_number: L2BlockNumber,
    ) -> RpcResult<Option<BlockDetails>> {
        Ok(self
            .node
            .get_block_details_impl(block_number)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_transaction_details(&self, hash: H256) -> RpcResult<Option<TransactionDetails>> {
        Ok(self
            .node
            .get_transaction_details_impl(hash)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_raw_block_transactions(
        &self,
        block_number: L2BlockNumber,
    ) -> RpcResult<Vec<Transaction>> {
        Ok(self
            .node
            .get_raw_block_transactions_impl(block_number)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_l1_batch_details(
        &self,
        _batch: L1BatchNumber,
    ) -> RpcResult<Option<L1BatchDetails>> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_bytecode_by_hash(&self, hash: H256) -> RpcResult<Option<Vec<u8>>> {
        Ok(self
            .node
            .get_bytecode_by_hash_impl(hash)
            .await
            .map_err(RpcError::from)?)
    }

    async fn get_l1_gas_price(&self) -> RpcResult<U64> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_fee_params(&self) -> RpcResult<FeeParams> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_protocol_version(
        &self,
        _version_id: Option<u16>,
    ) -> RpcResult<Option<ProtocolVersion>> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_proof(
        &self,
        _address: Address,
        _keys: Vec<H256>,
        _l1_batch_number: L1BatchNumber,
    ) -> RpcResult<Option<Proof>> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_batch_fee_input(&self) -> RpcResult<PubdataIndependentBatchFeeModelInput> {
        Err(RpcError::Unsupported.into())
    }

    async fn send_raw_transaction_with_detailed_output(
        &self,
        _tx_bytes: Bytes,
    ) -> RpcResult<TransactionDetailedResult> {
        Err(RpcError::Unsupported.into())
    }

    async fn get_timestamp_asserter(&self) -> RpcResult<Option<Address>> {
        Err(RpcError::Unsupported.into())
    }
}
