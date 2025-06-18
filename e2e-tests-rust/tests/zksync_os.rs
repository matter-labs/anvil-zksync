use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use anvil_zksync_e2e_tests::test_contracts::{Counter, SimpleErc20};
use anvil_zksync_e2e_tests::{AnvilZksyncTesterBuilder, ReceiptExt, DEFAULT_TX_VALUE};

#[tokio::test]
async fn simple_transfer() -> anyhow::Result<()> {
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
async fn counter() -> anyhow::Result<()> {
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

#[tokio::test]
async fn erc20() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--log", "info", "--use-boojum"]))
        .build()
        .await?;
    let alice = tester.rich_account(0);

    // Deploy ERC20 EVM contract and mint some for alice
    let erc20 = SimpleErc20::deploy_evm(
        tester.l2_evm_provider(),
        "TestToken".to_string(),
        "TTK".to_string(),
    )
    .await?;
    let tx_receipt = erc20
        .mint(alice, U256::from(1000000000000000000000000u128))
        .gas(500_000)
        .max_fee_per_gas(1000)
        .max_priority_fee_per_gas(1000)
        .send()
        .await?
        .get_receipt()
        .await?;
    tx_receipt.assert_successful()?;

    // Check that we can transfer ERC20 tokens to a random account
    let to = Address::random();
    let tx_receipt = erc20
        .transfer(to, U256::from(100u128))
        .from(alice)
        .gas(500_000)
        .max_fee_per_gas(1000)
        .max_priority_fee_per_gas(1000)
        .send()
        .await?
        .get_receipt()
        .await?;
    tx_receipt.assert_successful()?;

    Ok(())
}
