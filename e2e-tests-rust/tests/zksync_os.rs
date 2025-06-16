use alloy::primitives::U256;
use anvil_zksync_e2e_tests::test_contracts::Counter;
use anvil_zksync_e2e_tests::{AnvilZksyncTesterBuilder, ReceiptExt};

#[tokio::test]
async fn deploy_counter() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--log", "info", "--use-boojum"]))
        .build()
        .await?;

    // Deploy `Counter` contract and validate that it is initialized with `0`
    let counter = Counter::deploy(tester.l2_provider()).await?;
    assert_eq!(counter.get().await?, U256::from(0));

    // Increment counter by 1
    let tx_receipt = counter.increment(1).send().await?.get_receipt().await?;
    tx_receipt.assert_successful()?;

    // Validate that the counter was increased
    assert_eq!(counter.get().await?, U256::from(1));

    Ok(())
}
