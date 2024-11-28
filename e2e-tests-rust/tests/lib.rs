use alloy::network::{ReceiptResponse, TransactionBuilder};
use alloy::primitives::{address, Address, U256};
use alloy::providers::{PendingTransaction, PendingTransactionError, Provider, WalletProvider};
use alloy::transports::http::{reqwest, Http};
use alloy_zksync::network::transaction_request::TransactionRequest;
use alloy_zksync::network::Zksync;
use alloy_zksync::node_bindings::EraTestNode;
use alloy_zksync::provider::{zksync_provider, ProviderBuilderExt};
use era_test_node_e2e_tests::utils::LockedPort;
use std::time::Duration;

async fn init(
    f: impl FnOnce(EraTestNode) -> EraTestNode,
) -> anyhow::Result<impl Provider<Http<reqwest::Client>, Zksync> + WalletProvider<Zksync> + Clone> {
    let locked_port = LockedPort::acquire_unused().await?;
    let provider = zksync_provider()
        .with_recommended_fillers()
        .on_era_test_node_with_wallet_and_config(|node| {
            f(node
                .path(
                    std::env::var("ERA_TEST_NODE_BINARY_PATH")
                        .unwrap_or("../target/release/era_test_node".to_string()),
                )
                .port(locked_port.port))
        });

    // Wait for era-test-node to get up and be able to respond
    provider.get_accounts().await?;
    // Explicitly unlock the port to showcase why we waited above
    drop(locked_port);

    Ok(provider)
}

#[tokio::test]
async fn interval_sealing_finalization() -> anyhow::Result<()> {
    // Test that we can submit a transaction and wait for it to finalize when era-test-node is
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
    // era-test-node is operating in interval sealing mode. 3 seconds should be long enough for
    // the entire flow to execute before the first block is produced.
    let provider = init(|node| node.block_time(3)).await?;
    const RICH_WALLET0: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
    const RICH_WALLET1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
    const TARGET: Address = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");

    async fn submit_tx(
        provider: impl Provider<Http<reqwest::Client>, Zksync> + WalletProvider<Zksync> + Clone,
        rich_wallet: Address,
    ) -> Result<PendingTransaction, PendingTransactionError> {
        let tx = TransactionRequest::default()
            .with_from(rich_wallet)
            .with_to(TARGET)
            .with_value(U256::from(100));
        provider.send_transaction(tx).await?.register().await
    }

    // Submit two txs at the same time
    let handle0 = tokio::spawn(submit_tx(provider.clone(), RICH_WALLET0));
    let handle1 = tokio::spawn(submit_tx(provider.clone(), RICH_WALLET1));

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
async fn no_sealing_timeout() -> anyhow::Result<()> {
    // Test that we can submit a transaction and timeout while waiting for it to finalize when
    // era-test-node is operating in no sealing mode.
    let provider = init(|node| node.no_mine()).await?;

    let tx = TransactionRequest::default()
        .with_to(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
        .with_value(U256::from(100));
    let pending_tx = provider.send_transaction(tx).await?.register().await?;
    let finalization_result = tokio::time::timeout(Duration::from_secs(3), pending_tx).await;
    assert!(finalization_result.is_err());

    Ok(())
}
