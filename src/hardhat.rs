use std::sync::{Arc, RwLock};

use crate::{fork::ForkSource, node::InMemoryNodeInner};
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_basic_types::{AccountTreeId, Address, U256};
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_types::{utils::storage_key_for_standard_token_balance, L2_ETH_TOKEN_ADDRESS};
use zksync_utils::u256_to_h256;
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
    #[rpc(name = "hardhat_setBalance")]
    fn set_balance(
        &self,
        address: Address, // The address whose balance will be edited
        balance: U256,    // The new balance to set for the given address, in wei
    ) -> BoxFuture<Result<bool>>;
}

impl<S: Send + Sync + 'static + ForkSource + std::fmt::Debug> HardhatNamespaceT
    for HardhatNamespaceImpl<S>
{
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
    fn set_balance(
        &self,
        address: Address,
        balance: U256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<bool>> {
        let inner = Arc::clone(&self.node);

        Box::pin(async move {
            let balance_key = storage_key_for_standard_token_balance(
                AccountTreeId::new(L2_ETH_TOKEN_ADDRESS),
                &address,
            );

            match inner.write() {
                Ok(mut inner_guard) => {
                    inner_guard
                        .fork_storage
                        .set_value(balance_key, u256_to_h256(balance));
                    Ok(true)
                }
                Err(_) => {
                    let web3_error = Web3Error::InternalError;
                    Err(into_jsrpc_error(web3_error))
                }
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
}
