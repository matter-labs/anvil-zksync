use crate::http_middleware::HttpWithMiddleware;
use crate::utils::{get_node_binary_path, LockedPort};
use crate::ReceiptExt;
use alloy::network::primitives::{BlockTransactionsKind, HeaderResponse as _};
use alloy::network::{Network, ReceiptResponse as _, TransactionBuilder};
use alloy::primitives::{Address, U256};
use alloy::providers::{
    PendingTransaction, PendingTransactionBuilder, PendingTransactionError, Provider, RootProvider,
    SendableTx, WalletProvider,
};
use alloy::rpc::{
    client::RpcClient,
    types::{Block, TransactionRequest},
};
use alloy::signers::local::LocalSigner;
use alloy::signers::Signer;
use alloy::transports::{RpcError, Transport, TransportErrorKind, TransportResult};
use alloy_zksync::network::header_response::HeaderResponse;
use alloy_zksync::network::receipt_response::ReceiptResponse;
use alloy_zksync::network::transaction_response::TransactionResponse;
use alloy_zksync::network::Zksync;
use alloy_zksync::node_bindings::{AnvilZKsync, AnvilZKsyncError::NoKeysAvailable};
use alloy_zksync::provider::{zksync_provider, layers::anvil_zksync::AnvilZKsyncLayer};
use alloy_zksync::wallet::ZksyncWallet;
use anyhow::Context as _;
use async_trait::async_trait;
use http::HeaderMap;
use itertools::Itertools;
use reqwest_middleware::{Middleware, Next};
use std::convert::identity;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

pub const DEFAULT_TX_VALUE: u64 = 100;

/// Full requirements for the underlying Zksync provider.
pub trait FullZksyncProvider<T>:
    Provider<T, Zksync> + WalletProvider<Zksync, Wallet = ZksyncWallet> + Clone
where
    T: Transport + Clone,
{
}
impl<P, T> FullZksyncProvider<T> for P
where
    P: Provider<T, Zksync> + WalletProvider<Zksync, Wallet = ZksyncWallet> + Clone,
    T: Transport + Clone,
{
}

/// Testing provider that redirects all alloy functionality to the underlying provider but also provides
/// extra functionality for testing.
///
/// It is also aware of rich accounts. It is a bit different from [`WalletProvider::signer_addresses`]
/// as signer set can change dynamically over time if, for example, user registers a new signer on
/// their side.
#[derive(Debug, Clone)]
pub struct TestingProvider<P, T>
where
    P: FullZksyncProvider<T>,
    T: Transport + Clone,
{
    inner: P,
    rich_accounts: Vec<Address>,
    /// Last seen response headers
    last_response_headers: Arc<RwLock<Option<HeaderMap>>>,
    _pd: PhantomData<T>,

    /// Underlying anvil-zksync instance's URL
    pub url: reqwest::Url,
}

// TODO: Consider creating a builder pattern
pub async fn init_testing_provider(
    node_fn: impl FnOnce(AnvilZKsync) -> AnvilZKsync,
) -> anyhow::Result<TestingProvider<impl FullZksyncProvider<HttpWithMiddleware>, HttpWithMiddleware>>
{
    init_testing_provider_with_client(node_fn, identity).await
}

pub async fn init_testing_provider_with_client(
    node_fn: impl FnOnce(AnvilZKsync) -> AnvilZKsync,
    client_fn: impl FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
) -> anyhow::Result<TestingProvider<impl FullZksyncProvider<HttpWithMiddleware>, HttpWithMiddleware>>
{
    let locked_port = LockedPort::acquire_unused().await?;
    let node_layer = AnvilZKsyncLayer::from(node_fn(
        AnvilZKsync::new()
            .path(get_node_binary_path())
            .port(locked_port.port),
    ));

    let last_response_headers = Arc::new(RwLock::new(None));
    let client =
        reqwest_middleware::ClientBuilder::new(client_fn(reqwest::Client::builder()).build()?)
            .with(ResponseHeadersInspector(last_response_headers.clone()))
            .build();
    let url = node_layer.endpoint_url();
    let http = HttpWithMiddleware::with_client(client, url.clone());
    let rpc_client = RpcClient::new(http, true);

    let rich_accounts = node_layer.instance().addresses().iter().cloned().collect();
    let default_keys = node_layer.instance().keys().to_vec();
    let (default_key, remaining_keys) = default_keys.split_first().ok_or(NoKeysAvailable)?;

    let default_signer = LocalSigner::from(default_key.clone())
        .with_chain_id(Some(node_layer.instance().chain_id()));
    let mut wallet = ZksyncWallet::from(default_signer);

    for key in remaining_keys {
        let signer = LocalSigner::from(key.clone());
        wallet.register_signer(signer)
    }

    let provider = zksync_provider()
        .with_recommended_fillers()
        .wallet(wallet)
        .layer(node_layer)
        .on_client(rpc_client);

    // Wait for anvil-zksync to get up and be able to respond.
    // Ignore error response (should not fail here if provider is used with intentionally wrong
    // configuration for testing purposes).
    let _ = provider.get_chain_id().await;
    // Explicitly unlock the port to showcase why we waited above
    drop(locked_port);

    Ok(TestingProvider {
        inner: provider,
        rich_accounts,
        last_response_headers,
        _pd: Default::default(),

        url,
    })
}

impl<P, T> TestingProvider<P, T>
where
    P: FullZksyncProvider<T>,
    T: Transport + Clone,
{
    /// Returns a rich account under the requested index. Rich accounts returned from this method
    /// are guaranteed to not change over the node's lifetime.
    pub fn rich_account(&self, index: usize) -> Address {
        *self
            .rich_accounts
            .get(index)
            .unwrap_or_else(|| panic!("not enough rich accounts (#{} was requested)", index,))
    }

    /// Returns last seen response headers. Panics if there is none.
    pub async fn last_response_headers_unwrap(&self) -> HeaderMap {
        self.last_response_headers
            .read()
            .await
            .clone()
            .expect("no headers found")
    }
}

impl<P, T> TestingProvider<P, T>
where
    P: FullZksyncProvider<T>,
    T: Transport + Clone,
    Self: 'static,
{
    /// Creates a default transaction (transfers 100 wei to a random account from the default signer)
    /// and returns it as a builder. The builder can then be used to populate transaction with custom
    /// data and then to register it or wait until it is finalized.
    pub fn tx(&self) -> TestTxBuilder<P, T> {
        let tx = TransactionRequest::default()
            .with_to(Address::random())
            .with_value(U256::from(DEFAULT_TX_VALUE));
        TestTxBuilder {
            inner: tx,
            provider: (*self).clone(),
            _pd: Default::default(),
        }
    }

    /// Submit `N` concurrent transactions and wait for all of them to finalize. Returns an array of
    /// receipts packed as [`RacedReceipts`] (helper structure for asserting conditions on all receipts
    /// at the same time).
    pub async fn race_n_txs<const N: usize>(
        &self,
        f: impl Fn(usize, TestTxBuilder<P, T>) -> TestTxBuilder<P, T>,
    ) -> Result<RacedReceipts<N>, PendingTransactionError> {
        let pending_txs: [JoinHandle<
            Result<PendingTransactionFinalizable<T, Zksync>, PendingTransactionError>,
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
        self.get_block_by_hash(receipt.block_hash_ext()?, BlockTransactionsKind::Full)
            .await?
            .with_context(|| format!("block (hash={}) not found", hash))
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
            .get_block_by_hash(expected_block.header.hash(), BlockTransactionsKind::Full)
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
            .get_block_by_hash(expected_block.header.hash(), BlockTransactionsKind::Full)
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
        let actual_balance = self.get_balance(address).await?;
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

#[async_trait::async_trait]
impl<P, T> Provider<T, Zksync> for TestingProvider<P, T>
where
    P: FullZksyncProvider<T>,
    T: Transport + Clone,
{
    fn root(&self) -> &RootProvider<T, Zksync> {
        self.inner.root()
    }

    async fn send_transaction_internal(
        &self,
        tx: SendableTx<Zksync>,
    ) -> TransportResult<PendingTransactionBuilder<T, Zksync>> {
        self.inner.send_transaction_internal(tx).await
    }
}

impl<P: FullZksyncProvider<T>, T: Transport + Clone> WalletProvider<Zksync>
    for TestingProvider<P, T>
{
    type Wallet = ZksyncWallet;

    fn wallet(&self) -> &Self::Wallet {
        self.inner.wallet()
    }

    fn wallet_mut(&mut self) -> &mut Self::Wallet {
        self.inner.wallet_mut()
    }
}

/// Helper struct for building and submitting transactions. Main idea here is to reduce the amount
/// of boilerplate for users who just want to submit default transactions (see [`TestingProvider::tx`])
/// most of the time. Also returns wrapped pending transaction in the form of [`PendingTransactionFinalizable`],
/// which can be finalized without a user-supplied provider instance.
pub struct TestTxBuilder<P, T>
where
    P: FullZksyncProvider<T>,
    T: Transport + Clone,
{
    inner: TransactionRequest,
    provider: TestingProvider<P, T>,
    _pd: PhantomData<T>,
}

impl<P, T> TestTxBuilder<P, T>
where
    T: Transport + Clone,
    P: FullZksyncProvider<T>,
{
    /// Builder-pattern method for setting the sender.
    pub fn with_from(mut self, from: Address) -> Self {
        self.inner = self.inner.with_from(from);
        self
    }

    /// Sets the sender to an indexed rich account (see [`TestingProvider::rich_account`]).
    pub fn with_rich_from(mut self, index: usize) -> Self {
        let from = self.provider.rich_account(index);
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
    ) -> Result<PendingTransactionFinalizable<T, Zksync>, PendingTransactionError> {
        let pending_tx = self
            .provider
            .send_transaction(self.inner.into())
            .await?
            .register()
            .await?;
        Ok(PendingTransactionFinalizable {
            inner: pending_tx,
            provider: self.provider.root().clone(),
        })
    }

    /// Waits for the transaction to finalize with the given number of confirmations and then fetches
    /// its receipt.
    pub async fn finalize(self) -> Result<ReceiptResponse, PendingTransactionError> {
        self.provider
            .send_transaction(self.inner.into())
            .await?
            .get_receipt()
            .await
    }
}

/// A wrapper around [`PendingTransaction`] that holds a provider instance which can be used to check
/// if the transaction is finalized or not without user supplying it again. Also contains helper
/// methods to assert different finalization scenarios.
pub struct PendingTransactionFinalizable<T, N: Network> {
    inner: PendingTransaction,
    provider: RootProvider<T, N>,
}

impl<T, N: Network> AsRef<PendingTransaction> for PendingTransactionFinalizable<T, N> {
    fn as_ref(&self) -> &PendingTransaction {
        &self.inner
    }
}

impl<T, N: Network> AsMut<PendingTransaction> for PendingTransactionFinalizable<T, N> {
    fn as_mut(&mut self) -> &mut PendingTransaction {
        &mut self.inner
    }
}

impl<T, N: Network> Deref for PendingTransactionFinalizable<T, N> {
    type Target = PendingTransaction;

    fn deref(&self) -> &PendingTransaction {
        &self.inner
    }
}

impl<T, N: Network> DerefMut for PendingTransactionFinalizable<T, N> {
    fn deref_mut(&mut self) -> &mut PendingTransaction {
        &mut self.inner
    }
}

impl<T, N: Network> Future for PendingTransactionFinalizable<T, N> {
    type Output = <PendingTransaction as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(cx)
    }
}

impl<T: Transport + Clone, N: Network> PendingTransactionFinalizable<T, N> {
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

/// A [`reqwest_middleware`]-compliant middleware that allows to inspect last seen response headers.
struct ResponseHeadersInspector(Arc<RwLock<Option<HeaderMap>>>);

#[async_trait]
impl Middleware for ResponseHeadersInspector {
    async fn handle(
        &self,
        req: reqwest::Request,
        extensions: &mut http::Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<reqwest::Response> {
        let resp = next.run(req, extensions).await?;
        *self.0.write().await = Some(resp.headers().clone());
        Ok(resp)
    }
}
