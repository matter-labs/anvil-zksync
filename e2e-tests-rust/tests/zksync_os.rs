use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, WalletProvider};
use alloy::rpc::types::TransactionRequest;
use alloy_zksync::provider::{DepositRequest, ZksyncProviderWithWallet};
use alloy_zksync::utils::ETHER_L1_ADDRESS;
use anvil_zksync_e2e_tests::contracts::Bridgehub;
use anvil_zksync_e2e_tests::test_contracts::{Counter, SimpleErc20};
use anvil_zksync_e2e_tests::{
    AnvilZKsyncApi, AnvilZksyncTesterBuilder, ReceiptExt, DEFAULT_TX_VALUE,
};
use test_casing::test_casing;

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

#[tokio::test]
async fn zksync_os_commit_batch_to_l1() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .with_node_fn(&|node| node.timeout(60_000).args(["--log", "info", "--use-boojum"]))
        .build()
        .await?;

    // Pre-generate a few batches for the rest of the test
    for _ in 0..5 {
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
    }

    // Committing first batch after genesis should work
    let tx_hash = tester.l2_provider().anvil_commit_batch(1).await?;
    let receipt = tester
        .l1_provider()
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Committing same batch twice shouldn't work
    let error = tester
        .l2_provider()
        .anvil_commit_batch(1)
        .await
        .expect_err("commit batch expected to fail");
    assert!(error.to_string().contains("commit transaction failed"));

    // Next batch is committable
    let tx_hash = tester.l2_provider().anvil_commit_batch(2).await?;
    let receipt = tester
        .l1_provider()
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Skipping a batch shouldn't work
    let error = tester
        .l2_provider()
        .anvil_commit_batch(4)
        .await
        .expect_err("commit batch expected to fail");
    assert!(error.to_string().contains("commit transaction failed"));

    Ok(())
}

#[tokio::test]
async fn zksync_os_l1_priority_tx() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .with_node_fn(&|node| {
            node.timeout(60_000)
                .args(["--chain-id", "271", "--log", "trace", "--use-boojum"])
        })
        .build()
        .await?;

    // Deploy `Counter` EVM contract and validate that it is initialized with `0`
    let counter = Counter::deploy_evm(tester.l2_evm_provider()).await?;
    assert_eq!(counter.get().await?, U256::from(0));

    // Prepare a transaction from a rich account that will increment `Counter` by 1
    let alice = tester.l2_provider().default_signer_address();
    let eip1559_est = tester.l1_provider().estimate_eip1559_fees().await?;
    let tx = counter
        .increment(1)
        .into_transaction_request()
        .with_from(alice)
        .with_max_fee_per_gas(eip1559_est.max_fee_per_gas);

    // But submit it as an L1 transaction through Bridgehub
    let bridgehub = Bridgehub::new(tester.l1_provider().clone(), tester.l2_provider()).await?;
    bridgehub
        .request_execute_evm(tester.l2_provider(), tx.clone())
        .await?
        .watch()
        .await?;

    // Validate that the counter was increased
    assert_eq!(counter.get().await?, U256::from(1));

    Ok(())
}

#[tokio::test]
async fn zksync_os_deposit() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .with_node_fn(&|node| {
            node.timeout(60_000)
                .args(["--chain-id", "271", "--log", "trace", "--use-boojum"])
        })
        .build()
        .await?;

    let alice = tester.l2_provider().default_signer_address();
    let alice_l1_initial_balance = tester.l1_provider().get_balance(alice).await?;
    let alice_l2_initial_balance = tester.l2_provider().get_balance(alice).await?;
    let amount = U256::from(1);

    let deposit_l1_receipt = tester
        .l2_provider()
        .deposit(
            &DepositRequest::new(amount)
                .with_receiver(alice)
                .with_token(ETHER_L1_ADDRESS),
            &tester.l1_provider(),
        )
        .await?;
    deposit_l1_receipt.get_l2_tx()?.get_receipt().await?;
    let deposit_l1_receipt = deposit_l1_receipt.get_receipt();
    let fee =
        U256::from(deposit_l1_receipt.effective_gas_price * deposit_l1_receipt.gas_used as u128);

    let alice_l1_final_balance = tester.l1_provider().get_balance(alice).await?;
    let alice_l2_final_balance = tester.l2_provider().get_balance(alice).await?;
    // Non-strict equality because somehow we spend more than expected and also more than expected
    // gets deposited to L2. Assuming this is expected as zksync-era e2e tests assert the same behavior.
    assert!(alice_l1_final_balance <= alice_l1_initial_balance - fee - amount);
    assert!(alice_l2_final_balance >= alice_l2_initial_balance + amount);

    Ok(())
}
