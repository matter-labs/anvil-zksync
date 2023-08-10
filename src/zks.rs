use std::sync::{Arc, RwLock};

use bigdecimal::BigDecimal;
use futures::FutureExt;
use zksync_basic_types::{MiniblockNumber, U256};
use zksync_core::api_server::web3::backend_jsonrpc::{namespaces::zks::ZksNamespaceT, error::into_jsrpc_error};
use zksync_types::{api::BridgeAddresses, fee::Fee};
use zksync_web3_decl::error::Web3Error;

use crate::{node::InMemoryNodeInner, utils::IntoBoxedFuture};

/// Mock implementation of ZksNamespace - used only in the test node.
pub struct ZkMockNamespaceImpl {
    node: Arc<RwLock<InMemoryNodeInner>>,
}

impl ZkMockNamespaceImpl {
     /// Creates a new `Zks` instance with the given `node`.
    pub fn new(node: Arc<RwLock<InMemoryNodeInner>>) -> Self {
        Self { node }
    }
}

macro_rules! not_implemented {
    () => {
        Box::pin(async move { Err(jsonrpc_core::Error::method_not_found()) })
    };
}
impl ZksNamespaceT for ZkMockNamespaceImpl {
    /// Estimates the gas fee data required for a given call request.
    ///
    /// # Arguments
    ///
    /// * `req` - A `CallRequest` struct representing the call request to estimate gas for.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `Fee` representing the estimated gas data required.
    fn estimate_fee(
        &self,
        req: zksync_types::transaction_request::CallRequest,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_types::fee::Fee>> {
        let reader = match self.node.read() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError)).boxed()
            }
        };

        let result: jsonrpc_core::Result<Fee> = reader.estimate_gas_impl(req);
        match result {
            Ok(fee) => Ok(fee).into_boxed_future(),
            Err(err) => return futures::future::err(err).boxed(),
        }
    }

    fn get_raw_block_transactions(
        &self,
        _block_number: MiniblockNumber,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<zksync_types::Transaction>>> {
        not_implemented!()
    }

    fn estimate_gas_l1_to_l2(
        &self,
        _req: zksync_types::transaction_request::CallRequest,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<U256>> {
        not_implemented!()
    }

    fn get_main_contract(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::Address>> {
        not_implemented!()
    }

    fn get_testnet_paymaster(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_basic_types::Address>>> {
        not_implemented!()
    }

    fn get_bridge_contracts(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<BridgeAddresses>> {
        not_implemented!()
    }

    fn l1_chain_id(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::U64>> {
        not_implemented!()
    }

    fn get_confirmed_tokens(
        &self,
        _from: u32,
        _limit: u8,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<zksync_web3_decl::types::Token>>> {
        not_implemented!()
    }

    fn get_token_price(
        &self,
        _token_address: zksync_basic_types::Address,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<BigDecimal>> {
        not_implemented!()
    }

    fn get_all_account_balances(
        &self,
        _address: zksync_basic_types::Address,
    ) -> jsonrpc_core::BoxFuture<
        jsonrpc_core::Result<std::collections::HashMap<zksync_basic_types::Address, U256>>,
    > {
        not_implemented!()
    }

    fn get_l2_to_l1_msg_proof(
        &self,
        _block: zksync_basic_types::MiniblockNumber,
        _sender: zksync_basic_types::Address,
        _msg: zksync_basic_types::H256,
        _l2_log_position: Option<usize>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::L2ToL1LogProof>>>
    {
        not_implemented!()
    }

    fn get_l2_to_l1_log_proof(
        &self,
        _tx_hash: zksync_basic_types::H256,
        _index: Option<usize>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::L2ToL1LogProof>>>
    {
        not_implemented!()
    }

    fn get_l1_batch_number(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::U64>> {
        not_implemented!()
    }

    fn get_block_details(
        &self,
        _block_number: zksync_basic_types::MiniblockNumber,
    ) -> jsonrpc_core::BoxFuture<
        jsonrpc_core::Result<Option<zksync_types::explorer_api::BlockDetails>>,
    > {
        not_implemented!()
    }

    fn get_miniblock_range(
        &self,
        _batch: zksync_basic_types::L1BatchNumber,
    ) -> jsonrpc_core::BoxFuture<
        jsonrpc_core::Result<Option<(zksync_basic_types::U64, zksync_basic_types::U64)>>,
    > {
        not_implemented!()
    }

    fn set_known_bytecode(
        &self,
        _bytecode: zksync_basic_types::Bytes,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<bool>> {
        not_implemented!()
    }

    fn get_transaction_details(
        &self,
        _hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::TransactionDetails>>>
    {
        not_implemented!()
    }

    fn get_l1_batch_details(
        &self,
        _batch: zksync_basic_types::L1BatchNumber,
    ) -> jsonrpc_core::BoxFuture<
        jsonrpc_core::Result<Option<zksync_types::explorer_api::L1BatchDetails>>,
    > {
        not_implemented!()
    }

    fn get_bytecode_by_hash(
        &self,
        _hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<Vec<u8>>>> {
        not_implemented!()
    }

    fn get_l1_gas_price(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::U64>> {
        not_implemented!()
    }
}

#[cfg(test)]
mod tests {
    use crate::node::InMemoryNode;

    use super::*;
    use zksync_types::transaction_request::CallRequest;

    #[tokio::test]
    async fn test_estimate_fee() {
        let node = InMemoryNode::new(None, crate::ShowCalls::None, false, false);
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());

        let mock_request = CallRequest {
            from: Some(
                "0xa61464658afeaf65cccaafd3a512b69a83b77618"
                    .parse()
                    .unwrap(),
            ),
            to: Some(
                "0x36615cf349d7f6344891b1e7ca7c72883f5dc049"
                    .parse()
                    .unwrap(),
            ),
            gas: Some(U256::from(0)),
            gas_price: Some(U256::from(0)),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            value: Some(U256::from(0)),
            data: Some(vec![0, 0].into()),
            nonce: Some(U256::from(0)),
            transaction_type: None,
            access_list: None,
            eip712_meta: None,
        };

        let result = namespace.estimate_fee(mock_request).await.unwrap();
        println!("result: {:?}", result);

        assert_eq!(result.gas_limit, U256::from(904431));
        assert_eq!(result.max_fee_per_gas, U256::from(250000000));
        assert_eq!(result.max_priority_fee_per_gas, U256::from(0));
        assert_eq!(result.gas_per_pubdata_limit, U256::from(4080));
    }
}
