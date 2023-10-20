use zksync_basic_types::{Address, U256, U64};
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_web3_decl::error::Web3Error;

use crate::{
    fork::ForkSource,
    namespaces::{HardhatNamespaceT, RpcResult},
    node::InMemoryNode,
    utils::IntoBoxedFuture,
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> HardhatNamespaceT
    for InMemoryNode<S>
{
    fn set_balance(&self, address: Address, balance: U256) -> RpcResult<bool> {
        self.set_balance(address, balance)
            .map_err(|err| {
                tracing::error!("failed setting balance : {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn set_nonce(&self, address: Address, balance: U256) -> RpcResult<bool> {
        self.set_nonce(address, balance)
            .map_err(|err| {
                tracing::error!("failed setting nonce: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn hardhat_mine(&self, num_blocks: Option<U64>, interval: Option<U64>) -> RpcResult<bool> {
        self.mine_blocks(num_blocks, interval)
            .map_err(|err| {
                tracing::error!("failed mining blocks: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn impersonate_account(&self, address: Address) -> RpcResult<bool> {
        self.impersonate_account(address)
            .map_err(|err| {
                tracing::error!("failed impersonating account: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn stop_impersonating_account(&self, address: Address) -> RpcResult<bool> {
        InMemoryNode::<S>::stop_impersonating_account(self, address)
            .map_err(|err| {
                tracing::error!("failed stopping to impersonate account: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }

    fn set_code(&self, address: Address, code: Vec<u8>) -> RpcResult<()> {
        self.set_code(address, code)
            .map_err(|err| {
                tracing::error!("failed setting code: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .into_boxed_future()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};
    use std::str::FromStr;
    use zksync_basic_types::{Nonce, H256};
    use zksync_core::api_server::web3::backend_jsonrpc::namespaces::eth::EthNamespaceT;
    use zksync_types::{api::BlockNumber, fee::Fee, l2::L2Tx, PackedEthSignature};

    #[tokio::test]
    async fn test_set_balance() {
        let address = Address::from_str("0x36615Cf349d7F6344891B1e7CA7C72883F5dc049").unwrap();
        let node = InMemoryNode::<HttpForkSource>::default();

        let balance_before = node.get_balance(address, None).await.unwrap();

        let result = HardhatNamespaceT::set_balance(&node, address, U256::from(1337))
            .await
            .unwrap();
        assert!(result);

        let balance_after = node.get_balance(address, None).await.unwrap();
        assert_eq!(balance_after, U256::from(1337));
        assert_ne!(balance_before, balance_after);
    }

    #[tokio::test]
    async fn test_set_nonce() {
        let address = Address::from_str("0x36615Cf349d7F6344891B1e7CA7C72883F5dc049").unwrap();
        let node = InMemoryNode::<HttpForkSource>::default();

        let nonce_before = node.get_transaction_count(address, None).await.unwrap();

        let result = HardhatNamespaceT::set_nonce(&node, address, U256::from(1337))
            .await
            .unwrap();
        assert!(result);

        let nonce_after = node.get_transaction_count(address, None).await.unwrap();
        assert_eq!(nonce_after, U256::from(1337));
        assert_ne!(nonce_before, nonce_after);

        // setting nonce lower than the current one should fail
        let result = HardhatNamespaceT::set_nonce(&node, address, U256::from(1336)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_hardhat_mine_default() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        // test with defaults
        let result = node.hardhat_mine(None, None).await.expect("hardhat_mine");
        assert!(result);

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 1, current_block.number);
        assert_eq!(start_block.timestamp + 1, current_block.timestamp);
        let result = node.hardhat_mine(None, None).await.expect("hardhat_mine");
        assert!(result);

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 2, current_block.number);
        assert_eq!(start_block.timestamp + 2, current_block.timestamp);
    }

    #[tokio::test]
    async fn test_hardhat_mine_custom() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        let num_blocks = 5;
        let interval = 3;
        let start_timestamp = start_block.timestamp + 1;

        let result = HardhatNamespaceT::hardhat_mine(
            &node,
            Some(U64::from(num_blocks)),
            Some(U64::from(interval)),
        )
        .await
        .expect("hardhat_mine");
        assert!(result);

        for i in 0..num_blocks {
            let current_block = node
                .get_block_by_number(BlockNumber::Number(start_block.number + i + 1), false)
                .await
                .unwrap()
                .expect("block exists");
            assert_eq!(start_block.number + i + 1, current_block.number);
            assert_eq!(
                start_timestamp + i * interval * 1_000,
                current_block.timestamp
            );
        }
    }

    #[tokio::test]
    async fn test_impersonate_account() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let to_impersonate =
            Address::from_str("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

        // give impersonated account some balance
        let result = HardhatNamespaceT::set_balance(&node, to_impersonate, U256::exp10(18))
            .await
            .unwrap();
        assert!(result);

        // construct a tx
        let mut tx = L2Tx::new(
            Address::random(),
            vec![],
            Nonce(0),
            Fee {
                gas_limit: U256::from(1_000_000),
                max_fee_per_gas: U256::from(250_000_000),
                max_priority_fee_per_gas: U256::from(250_000_000),
                gas_per_pubdata_limit: U256::from(20000),
            },
            to_impersonate,
            U256::one(),
            None,
            Default::default(),
        );
        tx.set_input(vec![], H256::random());
        if tx.common_data.signature.is_empty() {
            tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        // try to execute the tx- should fail without signature
        assert!(node.apply_txs(vec![tx.clone()]).is_err());

        // impersonate the account
        let result = HardhatNamespaceT::impersonate_account(&node, to_impersonate)
            .await
            .expect("impersonate_account");

        // result should be true
        assert!(result);

        // impersonating the same account again should return false
        let result = HardhatNamespaceT::impersonate_account(&node, to_impersonate)
            .await
            .expect("impersonate_account");
        assert!(!result);

        // execution should now succeed
        assert!(node.apply_txs(vec![tx.clone()]).is_ok());

        // stop impersonating the account
        let result = HardhatNamespaceT::stop_impersonating_account(&node, to_impersonate)
            .await
            .expect("stop_impersonating_account");

        // result should be true
        assert!(result);

        // stop impersonating the same account again should return false
        let result = HardhatNamespaceT::stop_impersonating_account(&node, to_impersonate)
            .await
            .expect("stop_impersonating_account");
        assert!(!result);

        // execution should now fail again
        assert!(node.apply_txs(vec![tx]).is_err());
    }

    #[tokio::test]
    async fn test_set_code() {
        let address = Address::repeat_byte(0x1);
        let node = InMemoryNode::<HttpForkSource>::default();
        let new_code = vec![0x1u8; 32];

        let code_before = node
            .get_code(address, None)
            .await
            .expect("failed getting code")
            .0;
        assert_eq!(Vec::<u8>::default(), code_before);

        HardhatNamespaceT::set_code(&node, address, new_code.clone())
            .await
            .expect("failed setting code");

        let code_after = node
            .get_code(address, None)
            .await
            .expect("failed getting code")
            .0;
        assert_eq!(new_code, code_after);
    }
}
