use alloy::providers::{Provider, ProviderBuilder};
use alloy::transports::Transport;
use anvil_zksync_e2e_tests::{
    init_testing_provider, AnvilZKsyncApi, FullZksyncProvider, LockedPort, ReceiptExt,
    TestingProvider,
};

async fn init_l1_provider<P: FullZksyncProvider<T> + 'static, T: Transport + Clone>(
    l2_provider: &TestingProvider<P, T>,
    l1_port: u16,
) -> anyhow::Result<Box<dyn Provider>> {
    let l1_provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_builtin(&format!("http://localhost:{}", l1_port))
        .await?;

    // Pre-generate a few batches for the rest of the test
    for _ in 0..5 {
        l2_provider.tx().finalize().await?.assert_successful()?;
    }

    Ok(Box::new(l1_provider))
}

#[tokio::test]
async fn commit_batch_to_l1() -> anyhow::Result<()> {
    let l1_locked_port = LockedPort::acquire_unused().await?;

    let l1_port = l1_locked_port.port.to_string();
    let l2_provider =
        init_testing_provider(move |node| node.args(["--with-l1", "--l1-port", l1_port.as_str()]))
            .await?;
    let l1_provider = init_l1_provider(&l2_provider, l1_locked_port.port).await?;

    // Committing first batch after genesis should work
    let tx_hash = l2_provider.anvil_commit_batch(1).await?;
    let receipt = l1_provider
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Committing same batch twice shouldn't work
    let error = l2_provider
        .anvil_commit_batch(1)
        .await
        .expect_err("commit batch expected to fail");
    assert!(error.to_string().contains("commit transaction failed"));

    // Next batch is committable
    let tx_hash = l2_provider.anvil_commit_batch(2).await?;
    let receipt = l1_provider
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Skipping a batch shouldn't work
    let error = l2_provider
        .anvil_commit_batch(4)
        .await
        .expect_err("commit batch expected to fail");
    assert!(error.to_string().contains("commit transaction failed"));

    Ok(())
}

#[tokio::test]
async fn prove_batch_on_l1() -> anyhow::Result<()> {
    let l1_locked_port = LockedPort::acquire_unused().await?;

    let l1_port = l1_locked_port.port.to_string();
    let l2_provider =
        init_testing_provider(move |node| node.args(["--with-l1", "--l1-port", l1_port.as_str()]))
            .await?;
    let l1_provider = init_l1_provider(&l2_provider, l1_locked_port.port).await?;

    // Proving batch without committing shouldn't work
    let error = l2_provider
        .anvil_prove_batch(1)
        .await
        .expect_err("prove batch expected to fail");
    assert!(error.to_string().contains("prove transaction failed"));

    // Commit & prove first batch after genesis
    l2_provider.anvil_commit_batch(1).await?;
    let tx_hash = l2_provider.anvil_prove_batch(1).await?;
    let receipt = l1_provider
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Proving same batch twice shouldn't work
    let error = l2_provider
        .anvil_prove_batch(1)
        .await
        .expect_err("prove batch expected to fail");
    assert!(error.to_string().contains("prove transaction failed"));

    // Commit & prove next batch
    l2_provider.anvil_commit_batch(2).await?;
    let tx_hash = l2_provider.anvil_prove_batch(2).await?;
    let receipt = l1_provider
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Skipping a batch shouldn't work
    let error = l2_provider
        .anvil_prove_batch(4)
        .await
        .expect_err("prove batch expected to fail");
    assert!(error.to_string().contains("prove transaction failed"));

    Ok(())
}

#[tokio::test]
async fn execute_batch_on_l1() -> anyhow::Result<()> {
    let l1_locked_port = LockedPort::acquire_unused().await?;

    let l1_port = l1_locked_port.port.to_string();
    let l2_provider =
        init_testing_provider(move |node| node.args(["--with-l1", "--l1-port", l1_port.as_str()]))
            .await?;
    let l1_provider = init_l1_provider(&l2_provider, l1_locked_port.port).await?;

    // Executing batch without committing shouldn't work
    let error = l2_provider
        .anvil_execute_batch(1)
        .await
        .expect_err("execute batch expected to fail");
    assert!(error.to_string().contains("execute transaction failed"));

    // Committing is not enough for executing
    l2_provider.anvil_commit_batch(1).await?;
    let error = l2_provider
        .anvil_execute_batch(1)
        .await
        .expect_err("execute batch expected to fail");
    assert!(error.to_string().contains("execute transaction failed"));

    // Prove & commit first batch after genesis
    l2_provider.anvil_prove_batch(1).await?;
    let tx_hash = l2_provider.anvil_execute_batch(1).await?;
    let receipt = l1_provider
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Executing same batch twice shouldn't work
    let error = l2_provider
        .anvil_execute_batch(1)
        .await
        .expect_err("execute batch expected to fail");
    assert!(error.to_string().contains("execute transaction failed"));

    // Commit & prove & execute next batch
    l2_provider.anvil_commit_batch(2).await?;
    l2_provider.anvil_prove_batch(2).await?;
    let tx_hash = l2_provider.anvil_execute_batch(2).await?;
    let receipt = l1_provider
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Skipping a batch shouldn't work
    let error = l2_provider
        .anvil_execute_batch(4)
        .await
        .expect_err("execute batch expected to fail");
    assert!(error.to_string().contains("execute transaction failed"));

    Ok(())
}
