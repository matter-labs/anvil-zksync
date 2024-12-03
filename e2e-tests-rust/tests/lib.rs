use alloy::network::{ReceiptResponse, TransactionBuilder};
use alloy::primitives::{address, Address, U256};
use alloy::providers::ext::AnvilApi;
use alloy::providers::{PendingTransaction, PendingTransactionError, Provider, WalletProvider};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::{reqwest, Http};
use alloy_zksync::network::transaction_request::TransactionRequest;
use alloy_zksync::network::Zksync;
use alloy_zksync::node_bindings::EraTestNode;
use alloy_zksync::provider::{zksync_provider, ProviderBuilderExt};
use alloy_zksync::wallet::ZksyncWallet;
use anvil_zksync_e2e_tests::utils::LockedPort;
use anvil_zksync_e2e_tests::AnvilZKsyncApiProvider;
use std::time::Duration;

async fn init(
    f: impl FnOnce(EraTestNode) -> EraTestNode,
) -> anyhow::Result<
    impl Provider<Http<reqwest::Client>, Zksync> + WalletProvider<Zksync, Wallet = ZksyncWallet> + Clone,
> {
    let locked_port = LockedPort::acquire_unused().await?;
    let provider = zksync_provider()
        .with_recommended_fillers()
        .on_era_test_node_with_wallet_and_config(|node| {
            f(node
                .path(
                    std::env::var("ANVIL_ZKSYNC_BINARY_PATH")
                        .unwrap_or("../target/release/anvil-zksync".to_string()),
                )
                .port(locked_port.port))
        });

    // Wait for anvil-zksync to get up and be able to respond
    provider.get_accounts().await?;
    // Explicitly unlock the port to showcase why we waited above
    drop(locked_port);

    Ok(provider)
}

const RICH_WALLET0: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const RICH_WALLET1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");

async fn test_finalize_two_txs_in_the_same_block(
    provider: impl Provider<Http<reqwest::Client>, Zksync> + WalletProvider<Zksync> + Clone + 'static,
    target: Address,
) -> anyhow::Result<()> {
    async fn submit_tx(
        provider: impl Provider<Http<reqwest::Client>, Zksync> + WalletProvider<Zksync> + Clone,
        rich_wallet: Address,
        target: Address,
    ) -> Result<PendingTransaction, PendingTransactionError> {
        let tx = TransactionRequest::default()
            .with_from(rich_wallet)
            .with_to(target)
            .with_value(U256::from(100));
        provider.send_transaction(tx).await?.register().await
    }

    // Submit two txs at the same time
    let handle0 = tokio::spawn(submit_tx(provider.clone(), RICH_WALLET0, target));
    let handle1 = tokio::spawn(submit_tx(provider.clone(), RICH_WALLET1, target));

    // Wait until both are finalized
    let (pending_tx0, pending_tx1) = tokio::join!(handle0, handle1);
    let pending_tx0 = pending_tx0??;
    let pending_tx1 = pending_tx1??;

    // Fetch their receipts
    let receipt0 = provider
        .get_transaction_receipt(pending_tx0.await?)
        .await?
        .unwrap();
    assert!(receipt0.status());
    let receipt1 = provider
        .get_transaction_receipt(pending_tx1.await?)
        .await?
        .unwrap();
    assert!(receipt1.status());

    // Assert that they are different txs but executed in the same block
    assert_eq!(receipt0.from(), RICH_WALLET0);
    assert_eq!(receipt1.from(), RICH_WALLET1);
    assert_ne!(receipt0.transaction_hash(), receipt1.transaction_hash());

    // But executed in the same block
    assert!(receipt0.block_number().is_some());
    assert_eq!(receipt0.block_number(), receipt1.block_number());
    assert!(receipt0.block_hash().is_some());
    assert_eq!(receipt0.block_hash(), receipt1.block_hash());

    Ok(())
}

#[tokio::test]
async fn interval_sealing_finalization() -> anyhow::Result<()> {
    // Test that we can submit a transaction and wait for it to finalize when anvil-zksync is
    // operating in interval sealing mode.
    let provider = init(|node| node.block_time(1)).await?;

    let tx = TransactionRequest::default()
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let receipt = provider.send_transaction(tx).await?.get_receipt().await?;
    assert!(receipt.status());

    Ok(())
}

#[tokio::test]
async fn interval_sealing_multiple_txs() -> anyhow::Result<()> {
    // Test that we can submit two transactions and wait for them to finalize in the same block when
    // anvil-zksync is operating in interval sealing mode. 3 seconds should be long enough for
    // the entire flow to execute before the first block is produced.
    let provider = init(|node| node.block_time(3)).await?;

    test_finalize_two_txs_in_the_same_block(
        provider,
        address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn no_sealing_timeout() -> anyhow::Result<()> {
    // Test that we can submit a transaction and timeout while waiting for it to finalize when
    // anvil-zksync is operating in no sealing mode.
    let provider = init(|node| node.no_mine()).await?;

    let tx = TransactionRequest::default()
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx = provider.send_transaction(tx).await?.register().await?;
    let tx_hash = pending_tx.tx_hash().clone();
    let finalization_result = tokio::time::timeout(Duration::from_secs(3), pending_tx).await;
    assert!(finalization_result.is_err());

    // Mine a block manually and assert that the transaction is finalized now
    provider.anvil_mine(None, None).await?;
    let receipt = provider.get_transaction_receipt(tx_hash).await?.unwrap();
    assert!(receipt.status());

    Ok(())
}

#[tokio::test]
async fn dynamic_sealing_mode() -> anyhow::Result<()> {
    // Test that we can successfully switch between different sealing modes
    let provider = init(|node| node.no_mine()).await?;
    assert_eq!(provider.anvil_get_auto_mine().await?, false);

    // Enable immediate block sealing
    provider.anvil_set_auto_mine(true).await?;
    assert_eq!(provider.anvil_get_auto_mine().await?, true);

    // Check that we can finalize transactions now
    let tx = TransactionRequest::default()
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let receipt = provider.send_transaction(tx).await?.get_receipt().await?;
    assert!(receipt.status());

    // Enable interval block sealing
    provider.anvil_set_interval_mining(3).await?;
    assert_eq!(provider.anvil_get_auto_mine().await?, false);

    // Check that we can finalize two txs in the same block now
    test_finalize_two_txs_in_the_same_block(
        provider.clone(),
        address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
    )
    .await?;

    // Disable block sealing entirely
    provider.anvil_set_auto_mine(false).await?;
    assert_eq!(provider.anvil_get_auto_mine().await?, false);

    // Check that transactions do not get finalized now
    let tx = TransactionRequest::default()
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx = provider.send_transaction(tx).await?.register().await?;
    let finalization_result = tokio::time::timeout(Duration::from_secs(3), pending_tx).await;
    assert!(finalization_result.is_err());

    Ok(())
}

#[tokio::test]
async fn drop_transaction() -> anyhow::Result<()> {
    // Test that we can submit two transactions and then remove one from the pool before it gets
    // finalized. 3 seconds should be long enough for the entire flow to execute before the first
    // block is produced.
    let provider = init(|node| node.block_time(3)).await?;

    // Submit two transactions
    let tx0 = TransactionRequest::default()
        .with_from(RICH_WALLET0)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx0 = provider.send_transaction(tx0).await?.register().await?;
    let tx1 = TransactionRequest::default()
        .with_from(RICH_WALLET1)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx1 = provider.send_transaction(tx1).await?.register().await?;

    // Drop first
    provider
        .anvil_drop_transaction(*pending_tx0.tx_hash())
        .await?;

    // Assert first never gets finalized but the second one does
    let finalization_result = tokio::time::timeout(Duration::from_secs(4), pending_tx0).await;
    assert!(finalization_result.is_err());
    let receipt = provider
        .get_transaction_receipt(pending_tx1.await?)
        .await?
        .unwrap();
    assert!(receipt.status());

    Ok(())
}

#[tokio::test]
async fn drop_all_transactions() -> anyhow::Result<()> {
    // Test that we can submit two transactions and then remove them from the pool before the get
    // finalized. 3 seconds should be long enough for the entire flow to execute before the first
    // block is produced.
    let provider = init(|node| node.block_time(3)).await?;

    // Submit two transactions
    let tx0 = TransactionRequest::default()
        .with_from(RICH_WALLET0)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx0 = provider.send_transaction(tx0).await?.register().await?;
    let tx1 = TransactionRequest::default()
        .with_from(RICH_WALLET1)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx1 = provider.send_transaction(tx1).await?.register().await?;

    // Drop all transactions
    provider.anvil_drop_all_transactions().await?;

    // Neither transaction gets finalized
    let finalization_result = tokio::time::timeout(Duration::from_secs(4), pending_tx0).await;
    assert!(finalization_result.is_err());
    let finalization_result = tokio::time::timeout(Duration::from_secs(4), pending_tx1).await;
    assert!(finalization_result.is_err());

    Ok(())
}

#[tokio::test]
async fn remove_pool_transactions() -> anyhow::Result<()> {
    // Test that we can submit two transactions from two senders and then remove first sender's
    // transaction from the pool before it gets finalized. 3 seconds should be long enough for the
    // entire flow to execute before the first block is produced.
    let provider = init(|node| node.block_time(3)).await?;

    // Submit two transactions
    let tx0 = TransactionRequest::default()
        .with_from(RICH_WALLET0)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx0 = provider.send_transaction(tx0).await?.register().await?;
    let tx1 = TransactionRequest::default()
        .with_from(RICH_WALLET1)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx1 = provider.send_transaction(tx1).await?.register().await?;

    // Drop first
    provider
        .anvil_remove_pool_transactions(RICH_WALLET0)
        .await?;

    // Assert first never gets finalized but the second one does
    let finalization_result = tokio::time::timeout(Duration::from_secs(4), pending_tx0).await;
    assert!(finalization_result.is_err());
    let receipt = provider
        .get_transaction_receipt(pending_tx1.await?)
        .await?
        .unwrap();
    assert!(receipt.status());

    Ok(())
}

#[tokio::test]
async fn manual_mining_two_txs_in_one_block() -> anyhow::Result<()> {
    // Test that we can submit two transaction and then manually mine one block that contains both
    // transactions in it.
    let provider = init(|node| node.no_mine()).await?;

    let tx0 = TransactionRequest::default()
        .with_from(RICH_WALLET0)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx0 = provider.send_transaction(tx0).await?.register().await?;
    let tx1 = TransactionRequest::default()
        .with_from(RICH_WALLET1)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx1 = provider.send_transaction(tx1).await?.register().await?;

    // Mine a block manually and assert that both transactions are finalized now
    provider.anvil_mine(None, None).await?;
    let receipt0 = provider
        .get_transaction_receipt(pending_tx0.await?)
        .await?
        .unwrap();
    assert!(receipt0.status());
    let receipt1 = provider
        .get_transaction_receipt(pending_tx1.await?)
        .await?
        .unwrap();
    assert!(receipt1.status());

    assert_eq!(receipt0.block_hash(), receipt1.block_hash());
    assert_eq!(receipt0.block_number(), receipt1.block_number());

    Ok(())
}

#[tokio::test]
async fn detailed_mining_success() -> anyhow::Result<()> {
    // Test that we can detailed mining on a successful transaction and get output from it.
    let provider = init(|node| node.no_mine()).await?;

    let tx = TransactionRequest::default()
        .with_from(RICH_WALLET0)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    provider.send_transaction(tx).await?.register().await?;

    // Mine a block manually and assert that it has our transaction with extra fields
    let block = provider.mine_detailed().await?;
    assert_eq!(block.transactions.len(), 1);
    let actual_tx = block
        .transactions
        .clone()
        .into_transactions()
        .next()
        .unwrap();

    assert_eq!(
        actual_tx.other.get("output").and_then(|x| x.as_str()),
        Some("0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000")
    );
    assert!(actual_tx.other.get("revertReason").is_none());

    Ok(())
}

#[tokio::test]
async fn seal_block_ignoring_halted_transaction() -> anyhow::Result<()> {
    // Test that we can submit three transactions (1 and 3 are successful, 2 is halting). And then
    // observe a block that finalizes 1 and 3 while ignoring 2.
    let mut provider = init(|node| node.block_time(3)).await?;
    let signer = PrivateKeySigner::random();
    let random_account = signer.address();
    provider.wallet_mut().register_signer(signer);

    // Impersonate random account for now so that gas estimation works as expected
    provider.anvil_impersonate_account(random_account).await?;

    // Submit three transactions
    let tx0 = TransactionRequest::default()
        .with_from(RICH_WALLET0)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx0 = provider.send_transaction(tx0).await?.register().await?;
    let tx1 = TransactionRequest::default()
        .with_from(random_account)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx1 = provider.send_transaction(tx1).await?.register().await?;
    let tx2 = TransactionRequest::default()
        .with_from(RICH_WALLET1)
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx2 = provider.send_transaction(tx2).await?.register().await?;

    // Stop impersonating random account so that tx is going to halt
    provider
        .anvil_stop_impersonating_account(random_account)
        .await?;

    // Fetch their receipts
    let receipt0 = provider
        .get_transaction_receipt(pending_tx0.await?)
        .await?
        .unwrap();
    assert!(receipt0.status());
    let receipt2 = provider
        .get_transaction_receipt(pending_tx2.await?)
        .await?
        .unwrap();
    assert!(receipt2.status());

    // Assert that they are different txs but executed in the same block
    assert_eq!(receipt0.from(), RICH_WALLET0);
    assert_eq!(receipt2.from(), RICH_WALLET1);
    assert_ne!(receipt0.transaction_hash(), receipt2.transaction_hash());
    assert_eq!(receipt0.block_hash(), receipt2.block_hash());
    assert_eq!(receipt0.block_number(), receipt2.block_number());

    // Halted transaction never gets finalized
    let finalization_result = tokio::time::timeout(Duration::from_secs(4), pending_tx1).await;
    assert!(finalization_result.is_err());

    Ok(())
}
