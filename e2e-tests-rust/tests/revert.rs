use alloy::primitives::U256;
use anvil_zksync_e2e_tests::AnvilZksyncTesterBuilder;
use anvil_zksync_e2e_tests::test_contracts::Counter;

#[tokio::test]
async fn revert_during_estimation() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default().build().await?;

    // Deploy `Counter` contract and validate that it is initialized with `0`
    let counter = Counter::deploy(tester.l2_provider()).await?;
    assert_eq!(counter.get().await?, U256::from(0));

    // Increment counter by 1
    let estimate_gas_result = counter.increment_with_revert(1, true).estimate_gas().await;
    let Err(err) = estimate_gas_result else {
        anyhow::bail!("gas estimation should fail for reverting transactions")
    };
    println!("err: {err:?}");
    assert!(
        err.to_string()
            .contains(&hex::encode("This method always reverts"))
    );

    // Validate that the counter stayed the same
    assert_eq!(counter.get().await?, U256::from(0));

    Ok(())
}
