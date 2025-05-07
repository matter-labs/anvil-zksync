use alloy::primitives::U256;
use anvil_zksync_e2e_tests::test_contracts::Counter;
use anvil_zksync_e2e_tests::{get_node_binary_path, AnvilZksyncTesterBuilder, ReceiptExt};
use std::process::Stdio;
use std::time::Duration;
use test_casing::{cases, test_casing, TestCases};
use tokio::process::Command as CommandAsync;

const EVM_SUPPORTED_PROTOCOL_VERSIONS: TestCases<u16> = cases! {
    [27, 28]
};

#[test_casing(2, EVM_SUPPORTED_PROTOCOL_VERSIONS)]
#[tokio::test]
async fn deploy_evm_counter(protocol_version: u16) -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| {
            node.args([
                "--evm-interpreter",
                "--protocol-version",
                &protocol_version.to_string(),
            ])
        })
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

    // EVM `Counter` should have different address from ZKsync `Counter`
    let counter_zksync = Counter::deploy(tester.l2_provider()).await?;
    assert_ne!(counter_zksync.address(), counter.address());

    Ok(())
}

#[tokio::test]
async fn no_evm_emulator_on_v26() -> anyhow::Result<()> {
    let child = CommandAsync::new(get_node_binary_path())
        .stderr(Stdio::piped())
        .args(["--evm-interpreter", "--protocol-version", "26"])
        .spawn()?;
    let output = tokio::time::timeout(Duration::from_secs(10), child.wait_with_output()).await??;
    let error = std::str::from_utf8(&output.stderr)?;
    assert!(error.contains("EVM interpreter requires protocol version 27 or higher"));

    Ok(())
}

#[test_casing(2, EVM_SUPPORTED_PROTOCOL_VERSIONS)]
#[tokio::test]
async fn deploy_evm_counter_with_l1(protocol_version: u16) -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .with_node_fn(&|node| {
            node.args([
                "--evm-interpreter",
                "--protocol-version",
                &protocol_version.to_string(),
            ])
        })
        .build()
        .await?;
    // TODO: For now the rest of the test is equivalent to the base case but the original intention
    //       was to deploy EVM contract from L1 via a priority transaction. This does not seems to
    //       be possible however. To be confirmed with protocol.

    // Deploy `Counter` EVM contract and validate that it is initialized with `0`
    let counter = Counter::deploy_evm(tester.l2_evm_provider()).await?;
    assert_eq!(counter.get().await?, U256::from(0));

    // Increment counter by 1
    let tx_receipt = counter.increment(1).send().await?.get_receipt().await?;
    tx_receipt.assert_successful()?;

    // Validate that the counter was increased
    assert_eq!(counter.get().await?, U256::from(1));

    // EVM `Counter` should have different address from ZKsync `Counter`
    let counter_zksync = Counter::deploy(tester.l2_provider()).await?;
    assert_ne!(counter_zksync.address(), counter.address());

    Ok(())
}
