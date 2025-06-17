use alloy::network::{ReceiptResponse, TransactionBuilder};
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use anvil_zksync_e2e_tests::test_contracts::Counter;
use anvil_zksync_e2e_tests::{AnvilZksyncTesterBuilder, ReceiptExt, DEFAULT_TX_VALUE};

#[tokio::test]
async fn transfer() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--log", "info", "--use-boojum"]))
        .build()
        .await?;

    // Check that we can finalize transactions now
    let receipt = tester
        .l2_evm_provider()
        .send_transaction(
            TransactionRequest::default()
                .with_to(Address::random())
                .with_value(U256::from(DEFAULT_TX_VALUE))
                .gas_limit(100_000)
                .max_fee_per_gas(1000)
                .max_priority_fee_per_gas(1000),
        )
        .await?
        .get_receipt()
        .await?;
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
    let tx_receipt = counter
        .increment(1)
        .gas(100_000)
        .max_fee_per_gas(1000)
        .max_priority_fee_per_gas(1000)
        .send()
        .await?
        .get_receipt()
        .await?;
    tx_receipt.assert_successful()?;

    // Validate that the counter was increased
    assert_eq!(counter.get().await?, U256::from(1));

    Ok(())
}
