use anvil_zksync_api_decl::ZksNamespaceServer;
use anvil_zksync_core::node::InMemoryNode;
use anvil_zksync_l1_sidecar::L1Sidecar;
use function_name::named;
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

use crate::error::{rpc_unsupported, RpcErrorAdapter};

pub struct ZksNamespace {
    node: InMemoryNode,
    l1_sidecar: L1Sidecar,
}

impl ZksNamespace {
    pub fn new(node: InMemoryNode, l1_sidecar: L1Sidecar) -> Self {
        Self { node, l1_sidecar }
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
        self.node
            .estimate_fee_impl(req)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn estimate_gas_l1_to_l2(
        &self,
        req: CallRequest,
        // TODO: Support
        _state_override: Option<StateOverride>,
    ) -> RpcResult<U256> {
        self.node
            .estimate_gas_l1_to_l2(req)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn get_bridgehub_contract(&self) -> RpcResult<Option<Address>> {
        Ok(Some(
            self.l1_sidecar
                .contracts_config()
                .map_err(RpcErrorAdapter::into)?
                .ecosystem_contracts
                .bridgehub_proxy_addr,
        ))
    }

    #[named]
    async fn get_main_l1_contract(&self) -> RpcResult<Address> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_testnet_paymaster(&self) -> RpcResult<Option<Address>> {
        rpc_unsupported(function_name!())
    }

    async fn get_bridge_contracts(&self) -> RpcResult<BridgeAddresses> {
        if let Ok(contracts_config) = self.l1_sidecar.contracts_config() {
            return Ok(BridgeAddresses {
                l1_shared_default_bridge: Some(contracts_config.bridges.shared.l1_address),
                l2_shared_default_bridge: contracts_config.bridges.shared.l2_address,
                l1_erc20_default_bridge: Some(contracts_config.bridges.erc20.l1_address),
                l2_erc20_default_bridge: contracts_config.bridges.erc20.l2_address,
                l1_weth_bridge: None,
                l2_weth_bridge: None,
                l2_legacy_shared_bridge: contracts_config.l2.legacy_shared_bridge_addr,
            });
        }

        self.node
            .get_bridge_contracts_impl()
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn get_base_token_l1_address(&self) -> RpcResult<Address> {
        self.node
            .get_base_token_l1_address_impl()
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn l1_chain_id(&self) -> RpcResult<U64> {
        Ok(U64::from(
            self.l1_sidecar
                .genesis_config()
                .map_err(RpcErrorAdapter::into)?
                .l1_chain_id
                .0,
        ))
    }

    async fn get_confirmed_tokens(&self, from: u32, limit: u8) -> RpcResult<Vec<Token>> {
        self.node
            .get_confirmed_tokens_impl(from, limit)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn get_all_account_balances(
        &self,
        address: Address,
    ) -> RpcResult<HashMap<Address, U256>> {
        self.node
            .get_all_account_balances_impl(address)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    #[named]
    async fn get_l2_to_l1_msg_proof(
        &self,
        _block: L2BlockNumber,
        _sender: Address,
        _msg: H256,
        _l2_log_position: Option<usize>,
    ) -> RpcResult<Option<L2ToL1LogProof>> {
        rpc_unsupported(function_name!())
    }

    async fn get_l2_to_l1_log_proof(
        &self,
        tx_hash: H256,
        index: Option<usize>,
    ) -> RpcResult<Option<L2ToL1LogProof>> {
        self.node
            .get_l2_to_l1_log_proof_impl(tx_hash, index)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    #[named]
    async fn get_l1_batch_number(&self) -> RpcResult<U64> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_l2_block_range(&self, _batch: L1BatchNumber) -> RpcResult<Option<(U64, U64)>> {
        rpc_unsupported(function_name!())
    }

    async fn get_block_details(
        &self,
        block_number: L2BlockNumber,
    ) -> RpcResult<Option<BlockDetails>> {
        self.node
            .get_block_details_impl(block_number)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn get_transaction_details(&self, hash: H256) -> RpcResult<Option<TransactionDetails>> {
        self.node
            .get_transaction_details_impl(hash)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    async fn get_raw_block_transactions(
        &self,
        block_number: L2BlockNumber,
    ) -> RpcResult<Vec<Transaction>> {
        self.node
            .get_raw_block_transactions_impl(block_number)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    #[named]
    async fn get_l1_batch_details(
        &self,
        _batch: L1BatchNumber,
    ) -> RpcResult<Option<L1BatchDetails>> {
        rpc_unsupported(function_name!())
    }

    async fn get_bytecode_by_hash(&self, hash: H256) -> RpcResult<Option<Vec<u8>>> {
        self.node
            .get_bytecode_by_hash_impl(hash)
            .await
            .map_err(RpcErrorAdapter::into)
    }

    #[named]
    async fn get_l1_gas_price(&self) -> RpcResult<U64> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_fee_params(&self) -> RpcResult<FeeParams> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_protocol_version(
        &self,
        _version_id: Option<u16>,
    ) -> RpcResult<Option<ProtocolVersion>> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_proof(
        &self,
        _address: Address,
        _keys: Vec<H256>,
        _l1_batch_number: L1BatchNumber,
    ) -> RpcResult<Option<Proof>> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_batch_fee_input(&self) -> RpcResult<PubdataIndependentBatchFeeModelInput> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn send_raw_transaction_with_detailed_output(
        &self,
        _tx_bytes: Bytes,
    ) -> RpcResult<TransactionDetailedResult> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_timestamp_asserter(&self) -> RpcResult<Option<Address>> {
        rpc_unsupported(function_name!())
    }

    #[named]
    async fn get_l2_multicall3(&self) -> RpcResult<Option<Address>> {
        rpc_unsupported(function_name!())
    }
}
