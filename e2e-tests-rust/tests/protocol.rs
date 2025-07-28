use alloy::network::ReceiptResponse;
use alloy_zksync::provider::ZksyncProvider;
use anvil_zksync_e2e_tests::AnvilZksyncTesterBuilder;

#[tokio::test]
async fn protocol_v27_by_default_and_on_demand() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default().build().await?;
    let receipt = tester.tx().finalize().await?;
    let block_number = receipt.block_number().unwrap();

    let block_details = tester
        .l2_provider()
        .get_block_details(block_number)
        .await?
        .unwrap();
    assert_eq!(
        block_details.protocol_version,
        Some("Version27".to_string())
    );

    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--protocol-version", "26"]))
        .build()
        .await?;
    let receipt = tester.tx().finalize().await?;
    let block_number = receipt.block_number().unwrap();

    let block_details = tester
        .l2_provider()
        .get_block_details(block_number)
        .await?
        .unwrap();
    assert_eq!(
        block_details.protocol_version,
        Some("Version26".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn protocol_v27_on_demand() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--protocol-version", "27"]))
        .build()
        .await?;
    let receipt = tester.tx().finalize().await?;
    let block_number = receipt.block_number().unwrap();

    let block_details = tester
        .l2_provider()
        .get_block_details(block_number)
        .await?
        .unwrap();
    assert_eq!(
        block_details.protocol_version,
        Some("Version27".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn protocol_v28_on_demand() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--protocol-version", "28"]))
        .build()
        .await?;
    let receipt = tester.tx().finalize().await?;
    let block_number = receipt.block_number().unwrap();

    let block_details = tester
        .l2_provider()
        .get_block_details(block_number)
        .await?
        .unwrap();
    assert_eq!(
        block_details.protocol_version,
        Some("Version28".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn protocol_v29_on_demand() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--protocol-version", "29"]))
        .build()
        .await?;

    let receipt = tester.tx().finalize().await?;
    let block_number = receipt.block_number().unwrap();

    let block_details = tester
        .l2_provider()
        .get_block_details(block_number)
        .await?
        .unwrap();

    assert_eq!(
        block_details.protocol_version,
        Some("Version29".to_string())
    );

    Ok(())
}
