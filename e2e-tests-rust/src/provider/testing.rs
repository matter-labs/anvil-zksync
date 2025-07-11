use crate::ReceiptExt;
use crate::http_middleware::HttpWithMiddleware;
use crate::utils::{LockedPort, get_node_binary_path};
use alloy::network::primitives::HeaderResponse as _;
use alloy::network::{Ethereum, Network, ReceiptResponse as _, TransactionBuilder};
use alloy::primitives::{Address, U256};
use alloy::providers::{
    DynProvider, PendingTransaction, PendingTransactionError, Provider, ProviderBuilder,
    RootProvider, WalletProvider,
};
use alloy::rpc::{
    client::RpcClient,
    types::{Block, TransactionRequest},
};
use alloy::signers::Signer;
use alloy::signers::local::LocalSigner;
use alloy::transports::{RpcError, TransportErrorKind};
use alloy_zksync::network::Zksync;
use alloy_zksync::network::header_response::HeaderResponse;
use alloy_zksync::network::receipt_response::ReceiptResponse;
use alloy_zksync::network::transaction_response::TransactionResponse;
use alloy_zksync::node_bindings::{AnvilZKsync, AnvilZKsyncError::NoKeysAvailable};
use alloy_zksync::provider::{layers::anvil_zksync::AnvilZKsyncLayer, zksync_provider};
use alloy_zksync::wallet::ZksyncWallet;
use anyhow::Context as _;
use itertools::Itertools;
use std::convert::identity;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::task::JoinHandle;

pub const DEFAULT_TX_VALUE: u64 = 100;

/// Full requirements for the underlying Zksync provider.
pub trait FullZksyncProvider:
    Provider<Zksync> + WalletProvider<Zksync, Wallet = ZksyncWallet> + Clone
{
}
impl<P> FullZksyncProvider for P where
    P: Provider<Zksync> + WalletProvider<Zksync, Wallet = ZksyncWallet> + Clone
{
}

/// Testing provider that redirects all alloy functionality to the underlying provider but also provides
/// extra functionality for testing.
///
/// It is also aware of rich accounts. It is a bit different from [`WalletProvider::signer_addresses`]
/// as signer set can change dynamically over time if, for example, user registers a new signer on
/// their side.
#[derive(Debug, Clone)]
pub struct AnvilZksyncTester<P>
where
    P: FullZksyncProvider,
{
    /// Optional L1 provider when anvil-zksync is being run with L1 sidecar
    l1_provider: Option<DynProvider<Ethereum>>,
    /// ZKsync L2 provider for anvil-zksync node
    l2_provider: P,
    /// EVM-compatible L2 provider for anvil-zksync node
    l2_evm_provider: DynProvider<Ethereum>,
    rich_accounts: Vec<Address>,

    /// Underlying anvil-zksync instance's URL
    pub l2_url: reqwest::Url,
}

#[derive(Default)]
pub struct AnvilZksyncTesterBuilder<'a> {
    spawn_l1: bool,
    node_fn: Option<&'a dyn Fn(AnvilZKsync) -> AnvilZKsync>,
    client_fn: Option<&'a dyn Fn(reqwest::ClientBuilder) -> reqwest::ClientBuilder>,
    client_middleware_fn:
        Option<&'a dyn Fn(reqwest_middleware::ClientBuilder) -> reqwest_middleware::ClientBuilder>,
}

impl<'a> AnvilZksyncTesterBuilder<'a> {
    pub fn with_l1(mut self) -> Self {
        self.spawn_l1 = true;
        self
    }

    pub fn with_node_fn(mut self, node_fn: &'a dyn Fn(AnvilZKsync) -> AnvilZKsync) -> Self {
        self.node_fn = Some(node_fn);
        self
    }

    pub fn with_client_fn(
        mut self,
        client_fn: &'a dyn Fn(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
    ) -> Self {
        self.client_fn = Some(client_fn);
        self
    }

    pub fn with_client_middleware_fn(
        mut self,
        client_middleware_fn: &'a dyn Fn(
            reqwest_middleware::ClientBuilder,
        ) -> reqwest_middleware::ClientBuilder,
    ) -> Self {
        self.client_middleware_fn = Some(client_middleware_fn);
        self
    }

    pub async fn build(self) -> anyhow::Result<AnvilZksyncTester<impl FullZksyncProvider>> {
        let node_fn = self.node_fn.unwrap_or(&identity);
        let client_fn = self.client_fn.unwrap_or(&identity);
        let client_middleware_fn = self.client_middleware_fn.unwrap_or(&identity);

        let (l1_provider, l1_address) = if self.spawn_l1 {
            let l1_locked_port = LockedPort::acquire_unused().await?;
            let l1_provider = ProviderBuilder::new().connect_anvil_with_wallet_and_config(|anvil| {
                anvil
                    .port(l1_locked_port.port)
                    .arg("--no-request-size-limit")
            })?;
            let l1_address = format!("http://localhost:{}", l1_locked_port.port);
            (Some(DynProvider::new(l1_provider)), Some(l1_address))
        } else {
            (None, None)
        };

        let locked_port = LockedPort::acquire_unused().await?;
        let node_layer = AnvilZKsyncLayer::from(node_fn({
            let mut node = AnvilZKsync::new()
                .path(get_node_binary_path())
                .port(locked_port.port);
            if let Some(l1_address) = l1_address {
                node = node.args(["--external-l1", l1_address.as_str()])
            }
            node
        }));

        let client = client_fn(reqwest::Client::builder()).build()?;
        let client = client_middleware_fn(reqwest_middleware::ClientBuilder::new(client)).build();
        let l2_url = node_layer.endpoint_url();
        let http = HttpWithMiddleware::with_client(client, l2_url.clone());
        let rpc_client = RpcClient::new(http, true);

        let rich_accounts = node_layer.instance().addresses().to_vec();
        let default_keys = node_layer.instance().keys().to_vec();
        let (default_key, remaining_keys) = default_keys.split_first().ok_or(NoKeysAvailable)?;

        let default_signer = LocalSigner::from(default_key.clone())
            .with_chain_id(Some(node_layer.instance().chain_id()));
        let mut wallet = ZksyncWallet::from(default_signer);

        for key in remaining_keys {
            let signer = LocalSigner::from(key.clone());
            wallet.register_signer(signer)
        }

        let l2_provider = zksync_provider()
            .with_recommended_fillers()
            .wallet(wallet.clone())
            .layer(node_layer)
            .connect_client(rpc_client);
        let l2_evm_provider = DynProvider::new(
            ProviderBuilder::new()
                .wallet(wallet)
                .connect(l2_url.as_str())
                .await?,
        );

        // Wait for anvil-zksync to get up and be able to respond.
        // Ignore error response (should not fail here if provider is used with intentionally wrong
        // configuration for testing purposes).
        let _ = l2_provider.get_chain_id().await;
        // Explicitly unlock the port to showcase why we waited above
        drop(locked_port);

        Ok(AnvilZksyncTester {
            l1_provider,
            l2_provider,
            l2_evm_provider,
            rich_accounts,

            l2_url,
        })
    }
}

impl<P> AnvilZksyncTester<P>
where
    P: FullZksyncProvider,
{
    /// Returns a rich account under the requested index. Rich accounts returned from this method
    /// are guaranteed to not change over the node's lifetime.
    pub fn rich_account(&self, index: usize) -> Address {
        *self
            .rich_accounts
            .get(index)
            .unwrap_or_else(|| panic!("not enough rich accounts (#{index} was requested)",))
    }

    pub fn l1_provider(&self) -> DynProvider<Ethereum> {
        self.l1_provider
            .clone()
            .expect("anvil-zksync is not running in L1 mode")
    }

    pub fn l2_provider(&self) -> &P {
        &self.l2_provider
    }

    pub fn l2_provider_mut(&mut self) -> &mut P {
        &mut self.l2_provider
    }

    pub fn l2_evm_provider(&self) -> DynProvider<Ethereum> {
        self.l2_evm_provider.clone()
    }
}

impl<P> AnvilZksyncTester<P>
where
    P: FullZksyncProvider,
    Self: 'static,
{
    /// Creates a default transaction (transfers 100 wei to a random account from the default signer)
    /// and returns it as a builder. The builder can then be used to populate transaction with custom
    /// data and then to register it or wait until it is finalized.
    pub fn tx(&self) -> TestTxBuilder<P> {
        let tx = TransactionRequest::default()
            .with_to(Address::random())
            .with_value(U256::from(DEFAULT_TX_VALUE));
        TestTxBuilder {
            inner: tx,
            tester: (*self).clone(),
        }
    }

    /// Submit `N` concurrent transactions and wait for all of them to finalize. Returns an array of
    /// receipts packed as [`RacedReceipts`] (helper structure for asserting conditions on all receipts
    /// at the same time).
    pub async fn race_n_txs<const N: usize>(
        &self,
        f: impl Fn(usize, TestTxBuilder<P>) -> TestTxBuilder<P>,
    ) -> Result<RacedReceipts<N>, PendingTransactionError> {
        let pending_txs: [JoinHandle<
            Result<PendingTransactionFinalizable<Zksync>, PendingTransactionError>,
        >; N] = std::array::from_fn(|i| {
            let tx = f(i, self.tx());
            tokio::spawn(tx.register())
        });

        let receipt_futures = futures::future::try_join_all(pending_txs)
            .await
            .expect("failed to join a handle")
            .into_iter()
            .map_ok(|pending_tx| pending_tx.wait_until_finalized())
            .collect::<Result<Vec<_>, _>>()?;

        let receipts = futures::future::join_all(receipt_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        // Unwrap is safe as we are sure `receipts` contains exactly `N` elements
        Ok(RacedReceipts {
            receipts: receipts.try_into().unwrap(),
        })
    }

    /// Convenience method over [`Self::race_n_txs`] that builds `N` default transactions but uses
    /// a different rich signer for each of them. Panics if there is not enough rich accounts.
    pub async fn race_n_txs_rich<const N: usize>(
        &self,
    ) -> Result<RacedReceipts<N>, PendingTransactionError> {
        self.race_n_txs(|i, tx| tx.with_rich_from(i)).await
    }

    pub async fn get_block_by_receipt(
        &self,
        receipt: &ReceiptResponse,
    ) -> anyhow::Result<Block<TransactionResponse, HeaderResponse>> {
        let hash = receipt.block_hash_ext()?;
        self.l2_provider
            .get_block_by_hash(receipt.block_hash_ext()?)
            .full()
            .await?
            .with_context(|| format!("block (hash={hash}) not found"))
    }

    pub async fn get_blocks_by_receipts(
        &self,
        receipts: impl IntoIterator<Item = &ReceiptResponse>,
    ) -> anyhow::Result<Vec<Block<TransactionResponse, HeaderResponse>>> {
        futures::future::join_all(
            receipts
                .into_iter()
                .map(|receipt| self.get_block_by_receipt(receipt)),
        )
        .await
        .into_iter()
        .collect()
    }

    pub async fn assert_has_receipt(
        &self,
        expected_receipt: &ReceiptResponse,
    ) -> anyhow::Result<()> {
        let Some(actual_receipt) = self
            .l2_provider
            .get_transaction_receipt(expected_receipt.transaction_hash())
            .await?
        else {
            anyhow::bail!(
                "receipt (hash={}) not found",
                expected_receipt.transaction_hash()
            );
        };
        assert_eq!(expected_receipt, &actual_receipt);
        Ok(())
    }

    pub async fn assert_has_receipts(
        &self,
        receipts: impl IntoIterator<Item = &ReceiptResponse>,
    ) -> anyhow::Result<()> {
        for receipt in receipts {
            self.assert_has_receipt(receipt).await?;
        }
        Ok(())
    }

    pub async fn assert_no_receipt(
        &self,
        expected_receipt: &ReceiptResponse,
    ) -> anyhow::Result<()> {
        if let Some(actual_receipt) = self
            .l2_provider
            .get_transaction_receipt(expected_receipt.transaction_hash())
            .await?
        {
            anyhow::bail!(
                "receipt (hash={}) expected to be missing but was found",
                actual_receipt.transaction_hash()
            );
        } else {
            Ok(())
        }
    }

    pub async fn assert_no_receipts(
        &self,
        receipts: impl IntoIterator<Item = &ReceiptResponse>,
    ) -> anyhow::Result<()> {
        for receipt in receipts {
            self.assert_no_receipt(receipt).await?;
        }
        Ok(())
    }

    pub async fn assert_has_block(
        &self,
        expected_block: &Block<TransactionResponse, HeaderResponse>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            expected_block.transactions.is_full(),
            "expected block did not have full transactions"
        );
        let Some(actual_block) = self
            .l2_provider
            .get_block_by_hash(expected_block.header.hash())
            .full()
            .await?
        else {
            anyhow::bail!("block (hash={}) not found", expected_block.header.hash());
        };
        assert_eq!(expected_block, &actual_block);
        Ok(())
    }

    pub async fn assert_has_blocks(
        &self,
        blocks: impl IntoIterator<Item = &Block<TransactionResponse, HeaderResponse>>,
    ) -> anyhow::Result<()> {
        for block in blocks {
            self.assert_has_block(block).await?;
        }
        Ok(())
    }

    pub async fn assert_no_block(
        &self,
        expected_block: &Block<TransactionResponse, HeaderResponse>,
    ) -> anyhow::Result<()> {
        if let Some(actual_block) = self
            .l2_provider
            .get_block_by_hash(expected_block.header.hash())
            .full()
            .await?
        {
            anyhow::bail!(
                "block (hash={}) expected to be missing but was found",
                actual_block.header.hash()
            );
        } else {
            Ok(())
        }
    }

    pub async fn assert_no_blocks(
        &self,
        blocks: impl IntoIterator<Item = &Block<TransactionResponse, HeaderResponse>>,
    ) -> anyhow::Result<()> {
        for block in blocks {
            self.assert_no_block(block).await?;
        }
        Ok(())
    }

    pub async fn assert_balance(
        &self,
        address: Address,
        expected_balance: u64,
    ) -> anyhow::Result<()> {
        let actual_balance = self.l2_provider.get_balance(address).await?;
        let expected_balance = U256::from(expected_balance);
        anyhow::ensure!(
            actual_balance == expected_balance,
            "account's ({}) balance ({}) did not match expected value ({})",
            address,
            actual_balance,
            expected_balance,
        );
        Ok(())
    }
}

/// Helper struct for building and submitting transactions. Main idea here is to reduce the amount
/// of boilerplate for users who just want to submit default transactions (see [`TestingProvider::tx`])
/// most of the time. Also returns wrapped pending transaction in the form of [`PendingTransactionFinalizable`],
/// which can be finalized without a user-supplied provider instance.
pub struct TestTxBuilder<P>
where
    P: FullZksyncProvider,
{
    inner: TransactionRequest,
    tester: AnvilZksyncTester<P>,
}

impl<P> TestTxBuilder<P>
where
    P: FullZksyncProvider,
{
    /// Builder-pattern method for setting the sender.
    pub fn with_from(mut self, from: Address) -> Self {
        self.inner = self.inner.with_from(from);
        self
    }

    /// Sets the sender to an indexed rich account (see [`TestingProvider::rich_account`]).
    pub fn with_rich_from(mut self, index: usize) -> Self {
        let from = self.tester.rich_account(index);
        self.inner = self.inner.with_from(from);
        self
    }

    /// Builder-pattern method for setting the receiver.
    pub fn with_to(mut self, to: Address) -> Self {
        self.inner = self.inner.with_to(to);
        self
    }

    /// Builder-pattern method for setting the value.
    pub fn with_value(mut self, value: U256) -> Self {
        self.inner = self.inner.with_value(value);
        self
    }

    /// Builder-pattern method for setting the chain id.
    pub fn with_chain_id(mut self, id: u64) -> Self {
        self.inner = self.inner.with_chain_id(id);
        self
    }

    /// Builder-pattern method for setting max fee per gas.
    pub fn with_max_fee_per_gas(mut self, max_fee_per_gas: u128) -> Self {
        self.inner = self.inner.with_max_fee_per_gas(max_fee_per_gas);
        self
    }

    /// Submits transaction to the node.
    ///
    /// This does not wait for the transaction to be confirmed, but returns a [`PendingTransactionFinalizable`]
    /// that can be awaited at a later moment.
    pub async fn register(
        self,
    ) -> Result<PendingTransactionFinalizable<Zksync>, PendingTransactionError> {
        let pending_tx = self
            .tester
            .l2_provider
            .send_transaction(self.inner.into())
            .await?
            .register()
            .await?;
        Ok(PendingTransactionFinalizable {
            inner: pending_tx,
            provider: self.tester.l2_provider.root().clone(),
        })
    }

    /// Waits for the transaction to finalize with the given number of confirmations and then fetches
    /// its receipt.
    pub async fn finalize(self) -> Result<ReceiptResponse, PendingTransactionError> {
        self.tester
            .l2_provider
            .send_transaction(self.inner.into())
            .await?
            .get_receipt()
            .await
    }
}

/// A wrapper around [`PendingTransaction`] that holds a provider instance which can be used to check
/// if the transaction is finalized or not without user supplying it again. Also contains helper
/// methods to assert different finalization scenarios.
pub struct PendingTransactionFinalizable<N: Network> {
    inner: PendingTransaction,
    provider: RootProvider<N>,
}

impl<N: Network> AsRef<PendingTransaction> for PendingTransactionFinalizable<N> {
    fn as_ref(&self) -> &PendingTransaction {
        &self.inner
    }
}

impl<N: Network> AsMut<PendingTransaction> for PendingTransactionFinalizable<N> {
    fn as_mut(&mut self) -> &mut PendingTransaction {
        &mut self.inner
    }
}

impl<N: Network> Deref for PendingTransactionFinalizable<N> {
    type Target = PendingTransaction;

    fn deref(&self) -> &PendingTransaction {
        &self.inner
    }
}

impl<N: Network> DerefMut for PendingTransactionFinalizable<N> {
    fn deref_mut(&mut self) -> &mut PendingTransaction {
        &mut self.inner
    }
}

impl<N: Network> Future for PendingTransactionFinalizable<N> {
    type Output = <PendingTransaction as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(cx)
    }
}

impl<N: Network> PendingTransactionFinalizable<N> {
    /// Asserts that transaction is finalizable by waiting until its receipt gets resolved.
    pub async fn wait_until_finalized(self) -> Result<N::ReceiptResponse, PendingTransactionError> {
        let tx_hash = self.inner.await?;
        let receipt = self.provider.get_transaction_receipt(tx_hash).await?;
        if let Some(receipt) = receipt {
            Ok(receipt)
        } else {
            Err(RpcError::<TransportErrorKind>::NullResp.into())
        }
    }

    /// Asserts that transaction is not finalizable by expecting to timeout in the given duration
    /// while trying to resolve its receipt.
    pub async fn assert_not_finalizable(mut self, duration: Duration) -> anyhow::Result<Self> {
        let timeout = tokio::time::timeout(duration, &mut self);
        match timeout.await {
            Ok(Ok(tx_hash)) => {
                anyhow::bail!(
                    "expected transaction (hash={}) to not be finalizable, but it was",
                    tx_hash
                );
            }
            Ok(Err(e)) => {
                anyhow::bail!("failed to wait for a pending transaction: {}", e);
            }
            Err(_) => Ok(self),
        }
    }
}

/// Helper wrapper of `N` receipts of transactions that were raced together (see
/// [`TestingProvider::race_n_txs`]). Contains method for asserting different conditions for all receipts.
pub struct RacedReceipts<const N: usize> {
    pub receipts: [ReceiptResponse; N],
}

impl<const N: usize> RacedReceipts<N> {
    /// Asserts that all transactions were successful.
    pub fn assert_successful(self) -> anyhow::Result<Self> {
        for receipt in &self.receipts {
            receipt.assert_successful()?;
        }
        Ok(self)
    }

    /// Asserts that all transactions were sealed in the same block.
    pub fn assert_same_block(self) -> anyhow::Result<Self> {
        if N == 0 {
            return Ok(self);
        }
        let first = &self.receipts[0];
        for receipt in &self.receipts[1..] {
            receipt.assert_same_block(first)?;
        }

        Ok(self)
    }
}
