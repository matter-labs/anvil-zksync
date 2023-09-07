use std::sync::{Arc, RwLock};

use crate::{fork::ForkSource, node::InMemoryNodeInner};
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_basic_types::{Address, U256};
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_state::ReadStorage;
use zksync_types::{
    get_nonce_key,
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
                    println!(
                        "👷 Balance for address {:?} has been manually set to {} Wei",
                        address, balance
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
                        return Err(jsonrpc_core::Error::invalid_params(&format!(
                            "Account Nonce is already set to a higher value ({}, requested {})",
                            account_nonce, nonce
                        )));
                    }
                    account_nonce = nonce;
                    if deployment_nonce >= nonce {
                        return Err(jsonrpc_core::Error::invalid_params(&format!(
                            "Deployment Nonce is already set to a higher value ({}, requested {})",
                            deployment_nonce, nonce
                        )));
                    }
                    deployment_nonce = nonce;
                    let enforced_full_nonce = nonces_to_full_nonce(account_nonce, deployment_nonce);
                    println!(
                        "👷 Nonces for address {:?} have been set to {}",
                        address, nonce
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};
    use std::str::FromStr;
    use zksync_core::api_server::web3::backend_jsonrpc::namespaces::eth::EthNamespaceT;

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
}
