use alloy::network::ReceiptResponse;
use alloy::primitives::U256;
use anvil_zksync_e2e_tests::test_contracts::Counter;
use anvil_zksync_e2e_tests::{AnvilZksyncTesterBuilder, ReceiptExt};

#[tokio::test]
async fn transfer() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--log", "info", "--use-boojum"]))
        .build()
        .await?;

    // Check that we can finalize transactions now
    let receipt = tester.tx().finalize().await?;
    assert!(receipt.status());

    Ok(())
}

#[tokio::test]
async fn deploy_counter() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--log", "info", "--use-boojum"]))
        .build()
        .await?;

    // Deploy `Counter` EVM contract and validate that it is initialized with `0`
    let counter = Counter::deploy_evm(tester.l2_evm_provider()).await?;
    assert_eq!(counter.get().await?, U256::from(0));

    // Increment counter by 1
    let tx_receipt = counter.increment(1).send().await?.get_receipt().await?;
    tx_receipt.assert_successful()?;

    // Validate that the counter was increased
    assert_eq!(counter.get().await?, U256::from(1));

    Ok(())
}
