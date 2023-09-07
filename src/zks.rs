use std::sync::{Arc, RwLock};

use bigdecimal::BigDecimal;
use futures::FutureExt;
use zksync_basic_types::{MiniblockNumber, U256};
use zksync_core::api_server::web3::backend_jsonrpc::{
    error::into_jsrpc_error, namespaces::zks::ZksNamespaceT,
};
use zksync_types::{
    api::{BridgeAddresses, ProtocolVersion},
    fee::Fee,
};
use zksync_web3_decl::error::Web3Error;

use crate::{fork::ForkSource, node::InMemoryNodeInner, utils::IntoBoxedFuture};
use colored::Colorize;

/// Mock implementation of ZksNamespace - used only in the test node.
pub struct ZkMockNamespaceImpl<S> {
    node: Arc<RwLock<InMemoryNodeInner<S>>>,
}

impl<S> ZkMockNamespaceImpl<S> {
    /// Creates a new `Zks` instance with the given `node`.
    pub fn new(node: Arc<RwLock<InMemoryNodeInner<S>>>) -> Self {
        Self { node }
    }
}

macro_rules! not_implemented {
    () => {
        Box::pin(async move { Err(jsonrpc_core::Error::method_not_found()) })
    };
}
impl<S: Send + Sync + 'static + ForkSource + std::fmt::Debug> ZksNamespaceT
    for ZkMockNamespaceImpl<S>
{
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
        token_address: zksync_basic_types::Address,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<BigDecimal>> {
        match format!("{:?}", token_address).to_lowercase().as_str() {
            "0x0000000000000000000000000000000000000000" => {
                // ETH
                Ok(1_500.into()).into_boxed_future()
            }
            "0x40609141db628beee3bfab8034fc2d8278d0cc78" => {
                // LINK
                Ok(1.into()).into_boxed_future()
            }
            "0x0bfce1d53451b4a8175dd94e6e029f7d8a701e9c" => {
                // wBTC
                Ok(1.into()).into_boxed_future()
            }
            "0x0faf6df7054946141266420b43783387a78d82a9" => {
                // USDC
                Ok(1.into()).into_boxed_future()
            }
            "0x3e7676937a7e96cfb7616f255b9ad9ff47363d4b" => {
                // DAI
                Ok(1.into()).into_boxed_future()
            }
            address => {
                println!(
                    "{}",
                    format!("Token price requested for unknown address {:?}", address).red()
                );
                futures::future::err(into_jsrpc_error(Web3Error::InternalError)).boxed()
            }
        }
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
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::BlockDetails>>>
    {
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
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::L1BatchDetails>>>
    {
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

    fn get_protocol_version(
        &self,
        _version_id: Option<u16>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<ProtocolVersion>>> {
        not_implemented!()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::node::ShowCalls;
    use crate::system_contracts;
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};

    use super::*;
    use zksync_basic_types::Address;
    use zksync_types::transaction_request::CallRequest;

    #[tokio::test]
    async fn test_estimate_fee() {
        let node = InMemoryNode::<HttpForkSource>::default();
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

        assert_eq!(result.gas_limit, U256::from(1083285));
        assert_eq!(result.max_fee_per_gas, U256::from(250000000));
        assert_eq!(result.max_priority_fee_per_gas, U256::from(0));
        assert_eq!(result.gas_per_pubdata_limit, U256::from(4080));
    }

    #[tokio::test]
    async fn test_get_token_price_given_eth_should_return_price() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());

        let mock_address = Address::from_str("0x0000000000000000000000000000000000000000")
            .expect("Failed to parse address");

        // Act
        let result = namespace.get_token_price(mock_address).await.unwrap();

        // Assert
        assert_eq!(result, BigDecimal::from(1_500));
    }

    #[tokio::test]
    async fn test_get_token_price_given_capitalized_link_address_should_return_price() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::new(
            None,
            ShowCalls::None,
            crate::node::ShowStorageLogs::None,
            crate::node::ShowVMDetails::None,
            crate::node::ShowGasDetails::None,
            false,
            &system_contracts::Options::BuiltIn,
        );
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());

        let mock_address = Address::from_str("0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78")
            .expect("Failed to parse address");

        // Act
        let result = namespace.get_token_price(mock_address).await.unwrap();

        // Assert
        assert_eq!(result, BigDecimal::from(1));
    }

    #[tokio::test]
    async fn test_get_token_price_given_unknown_address_should_return_error() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        let namespace = ZkMockNamespaceImpl::new(node.get_inner());

        let mock_address = Address::from_str("0x0000000000000000000000000000000000000042")
            .expect("Failed to parse address");

        // Act
        let result = namespace.get_token_price(mock_address).await;

        // Assert
        assert!(result.is_err());
    }
}
