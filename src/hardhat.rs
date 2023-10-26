use std::sync::{Arc, RwLock};

use crate::{
    fork::ForkSource,
    node::InMemoryNodeInner,
    utils::{bytecode_to_factory_dep, mine_empty_blocks, IntoBoxedFuture},
};
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_basic_types::{Address, U256, U64};
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_state::ReadStorage;
use zksync_types::{
    get_code_key, get_nonce_key,
    utils::{decompose_full_nonce, nonces_to_full_nonce, storage_key_for_eth_balance},
};
use zksync_utils::{h256_to_u256, u256_to_h256};
use zksync_web3_decl::error::Web3Error;

/// Implementation of HardhatNamespaceImpl
pub struct HardhatNamespaceImpl<S> {
    node: Arc<RwLock<InMemoryNodeInner<S>>>,
}

impl<S> HardhatNamespaceImpl<S> {
    /// Creates a new `Hardhat` instance with the given `node`.
    pub fn new(node: Arc<RwLock<InMemoryNodeInner<S>>>) -> Self {
        Self { node }
    }
}

#[rpc]
pub trait HardhatNamespaceT {
    /// Sets the balance of the given address to the given balance.
    ///
    /// # Arguments
    ///
    /// * `address` - The `Address` whose balance will be edited
    /// * `balance` - The new balance to set for the given address, in wei
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "hardhat_setBalance")]
    fn set_balance(&self, address: Address, balance: U256) -> BoxFuture<Result<bool>>;

    /// Modifies an account's nonce by overwriting it.
    ///
    /// # Arguments
    ///
    /// * `address` - The `Address` whose nonce is to be changed
    /// * `nonce` - The new nonce
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "hardhat_setNonce")]
    fn set_nonce(&self, address: Address, balance: U256) -> BoxFuture<Result<bool>>;

    /// Sometimes you may want to advance the latest block number of the network by a large number of blocks.
    /// One way to do this would be to call the evm_mine RPC method multiple times, but this is too slow if you want to mine thousands of blocks.
    /// The hardhat_mine method can mine any number of blocks at once, in constant time. (It exhibits the same performance no matter how many blocks are mined.)
    ///
    /// # Arguments
    ///
    /// * `num_blocks` - The number of blocks to mine, defaults to 1
    /// * `interval` - The interval between the timestamps of each block, in seconds, and it also defaults to 1
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "hardhat_mine")]
    fn hardhat_mine(
        &self,
        num_blocks: Option<U64>,
        interval: Option<U64>,
    ) -> BoxFuture<Result<bool>>;

    /// Hardhat Network allows you to send transactions impersonating specific account and contract addresses.
    /// To impersonate an account use this method, passing the address to impersonate as its parameter.
    /// After calling this method, any transactions with this sender will be executed without verification.
    /// Multiple addresses can be impersonated at once.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to impersonate
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "hardhat_impersonateAccount")]
    fn impersonate_account(&self, address: Address) -> BoxFuture<Result<bool>>;

    /// Use this method to stop impersonating an account after having previously used `hardhat_impersonateAccount`
    /// The method returns `true` if the account was being impersonated and `false` otherwise.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to stop impersonating.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "hardhat_stopImpersonatingAccount")]
    fn stop_impersonating_account(&self, address: Address) -> BoxFuture<Result<bool>>;

    /// Modifies the bytecode stored at an account's address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address where the given code should be stored.
    /// * `code` - The code to be stored.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `bool` representing the success of the operation.
    #[rpc(name = "hardhat_setCode")]
    fn set_code(&self, address: Address, code: Vec<u8>) -> BoxFuture<Result<()>>;
}

impl<S: Send + Sync + 'static + ForkSource + std::fmt::Debug> HardhatNamespaceT
    for HardhatNamespaceImpl<S>
{
    fn set_balance(
        &self,
        address: Address,
        balance: U256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<bool>> {
        let inner = Arc::clone(&self.node);

        Box::pin(async move {
            match inner.write() {
                Ok(mut inner_guard) => {
                    let balance_key = storage_key_for_eth_balance(&address);
                    inner_guard
                        .fork_storage
                        .set_value(balance_key, u256_to_h256(balance));
                    tracing::info!(
                        "👷 Balance for address {:?} has been manually set to {} Wei",
                        address,
                        balance
                    );
                    Ok(true)
                }
                Err(_) => {
                    let web3_error = Web3Error::InternalError;
                    Err(into_jsrpc_error(web3_error))
                }
            }
        })
    }

    fn set_nonce(
        &self,
        address: Address,
        nonce: U256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<bool>> {
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            match inner.write() {
                Ok(mut inner_guard) => {
                    let nonce_key = get_nonce_key(&address);
                    let full_nonce = inner_guard.fork_storage.read_value(&nonce_key);
                    let (mut account_nonce, mut deployment_nonce) =
                        decompose_full_nonce(h256_to_u256(full_nonce));
                    if account_nonce >= nonce {
                        return Err(jsonrpc_core::Error::invalid_params(format!(
                            "Account Nonce is already set to a higher value ({}, requested {})",
                            account_nonce, nonce
                        )));
                    }
                    account_nonce = nonce;
                    if deployment_nonce >= nonce {
                        return Err(jsonrpc_core::Error::invalid_params(format!(
                            "Deployment Nonce is already set to a higher value ({}, requested {})",
                            deployment_nonce, nonce
                        )));
                    }
                    deployment_nonce = nonce;
                    let enforced_full_nonce = nonces_to_full_nonce(account_nonce, deployment_nonce);
                    tracing::info!(
                        "👷 Nonces for address {:?} have been set to {}",
                        address,
                        nonce
                    );
                    inner_guard
                        .fork_storage
                        .set_value(nonce_key, u256_to_h256(enforced_full_nonce));
                    Ok(true)
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn hardhat_mine(
        &self,
        num_blocks: Option<U64>,
        interval: Option<U64>,
    ) -> BoxFuture<Result<bool>> {
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            match inner.write() {
                Ok(mut inner) => {
                    let num_blocks = num_blocks.unwrap_or_else(|| U64::from(1));
                    let interval_ms = interval
                        .unwrap_or_else(|| U64::from(1))
                        .saturating_mul(1_000.into());
                    if num_blocks.is_zero() {
                        return Err(jsonrpc_core::Error::invalid_params(
                            "Number of blocks must be greater than 0".to_string(),
                        ));
                    }
                    mine_empty_blocks(&mut inner, num_blocks.as_u64(), interval_ms.as_u64());
                    tracing::info!("👷 Mined {} blocks", num_blocks);
                    Ok(true)
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn impersonate_account(&self, address: Address) -> BoxFuture<jsonrpc_core::Result<bool>> {
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            match inner.write() {
                Ok(mut inner) => {
                    if inner.set_impersonated_account(address) {
                        tracing::info!("🕵️ Account {:?} has been impersonated", address);
                        Ok(true)
                    } else {
                        tracing::info!("🕵️ Account {:?} was already impersonated", address);
                        Ok(false)
                    }
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn stop_impersonating_account(&self, address: Address) -> BoxFuture<Result<bool>> {
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            match inner.write() {
                Ok(mut inner) => {
                    if inner.stop_impersonating_account(address) {
                        tracing::info!("🕵️ Stopped impersonating account {:?}", address);
                        Ok(true)
                    } else {
                        tracing::info!(
                            "🕵️ Account {:?} was not impersonated, nothing to stop",
                            address
                        );
                        Ok(false)
                    }
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn set_code(&self, address: Address, code: Vec<u8>) -> BoxFuture<Result<()>> {
        let inner = Arc::clone(&self.node);
        inner
            .write()
            .map(|mut writer| {
                let code_key = get_code_key(&address);
                tracing::info!("set code for address {address:#x}");
                let (hash, code) = bytecode_to_factory_dep(code);
                let hash = u256_to_h256(hash);
                writer.fork_storage.store_factory_dep(
                    hash,
                    code.iter()
                        .flat_map(|entry| {
                            let mut bytes = vec![0u8; 32];
                            entry.to_big_endian(&mut bytes);
                            bytes.to_vec()
                        })
                        .collect(),
                );
                writer.fork_storage.set_value(code_key, hash);
            })
            .map_err(|_| into_jsrpc_error(Web3Error::InternalError))
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
        let hardhat = HardhatNamespaceImpl::new(node.get_inner());

        let balance_before = node.get_balance(address, None).await.unwrap();

        let result = hardhat
            .set_balance(address, U256::from(1337))
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
        let hardhat = HardhatNamespaceImpl::new(node.get_inner());

        let nonce_before = node.get_transaction_count(address, None).await.unwrap();

        let result = hardhat.set_nonce(address, U256::from(1337)).await.unwrap();
        assert!(result);

        let nonce_after = node.get_transaction_count(address, None).await.unwrap();
        assert_eq!(nonce_after, U256::from(1337));
        assert_ne!(nonce_before, nonce_after);

        // setting nonce lower than the current one should fail
        let result = hardhat.set_nonce(address, U256::from(1336)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_hardhat_mine_default() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let hardhat = HardhatNamespaceImpl::new(node.get_inner());

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        // test with defaults
        let result = hardhat
            .hardhat_mine(None, None)
            .await
            .expect("hardhat_mine");
        assert!(result);

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        assert_eq!(start_block.number + 1, current_block.number);
        assert_eq!(start_block.timestamp + 1, current_block.timestamp);
        let result = hardhat
            .hardhat_mine(None, None)
            .await
            .expect("hardhat_mine");
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
        let hardhat: HardhatNamespaceImpl<HttpForkSource> =
            HardhatNamespaceImpl::new(node.get_inner());

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, false)
            .await
            .unwrap()
            .expect("block exists");

        let num_blocks = 5;
        let interval = 3;
        let start_timestamp = start_block.timestamp + 1;

        let result = hardhat
            .hardhat_mine(Some(U64::from(num_blocks)), Some(U64::from(interval)))
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
        let hardhat: HardhatNamespaceImpl<HttpForkSource> =
            HardhatNamespaceImpl::new(node.get_inner());
        let to_impersonate =
            Address::from_str("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

        // give impersonated account some balance
        let result = hardhat
            .set_balance(to_impersonate, U256::exp10(18))
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
        let result = hardhat
            .impersonate_account(to_impersonate)
            .await
            .expect("impersonate_account");

        // result should be true
        assert!(result);

        // impersonating the same account again should return false
        let result = hardhat
            .impersonate_account(to_impersonate)
            .await
            .expect("impersonate_account");
        assert!(!result);

        // execution should now succeed
        assert!(node.apply_txs(vec![tx.clone()]).is_ok());

        // stop impersonating the account
        let result = hardhat
            .stop_impersonating_account(to_impersonate)
            .await
            .expect("stop_impersonating_account");

        // result should be true
        assert!(result);

        // stop impersonating the same account again should return false
        let result = hardhat
            .stop_impersonating_account(to_impersonate)
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
        let hardhat = HardhatNamespaceImpl::new(node.get_inner());
        let new_code = vec![0x1u8; 32];

        let code_before = node
            .get_code(address, None)
            .await
            .expect("failed getting code")
            .0;
        assert_eq!(Vec::<u8>::default(), code_before);

        hardhat
            .set_code(address, new_code.clone())
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
