use alloy::network::{ReceiptResponse, TransactionBuilder};
use alloy::primitives::{Address, B256, U256};
use alloy::providers::{Provider, WalletProvider};
use alloy_zksync::network::receipt_response::ReceiptResponse as ZkReceiptResponse;
use alloy_zksync::provider::ZksyncProvider;
use anvil_zksync_e2e_tests::contracts::{Bridgehub, L1Messenger, L2Message};
use anvil_zksync_e2e_tests::test_contracts::Counter;
use anvil_zksync_e2e_tests::{AnvilZKsyncApi, AnvilZksyncTesterBuilder, ReceiptExt};
use anyhow::Context;
use std::str::FromStr;

#[tokio::test]
async fn commit_batch_to_l1() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .build()
        .await?;

    // Pre-generate a few batches for the rest of the test
    for _ in 0..5 {
        tester.tx().finalize().await?.assert_successful()?;
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
async fn prove_batch_on_l1() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .build()
        .await?;

    // Pre-generate a few batches for the rest of the test
    for _ in 0..5 {
        tester.tx().finalize().await?.assert_successful()?;
    }

    // Proving batch without committing shouldn't work
    let error = tester
        .l2_provider()
        .anvil_prove_batch(1)
        .await
        .expect_err("prove batch expected to fail");
    assert!(error.to_string().contains("prove transaction failed"));

    // Commit & prove first batch after genesis
    tester.l2_provider().anvil_commit_batch(1).await?;
    let tx_hash = tester.l2_provider().anvil_prove_batch(1).await?;
    let receipt = tester
        .l1_provider()
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Proving same batch twice shouldn't work
    let error = tester
        .l2_provider()
        .anvil_prove_batch(1)
        .await
        .expect_err("prove batch expected to fail");
    assert!(error.to_string().contains("prove transaction failed"));

    // Commit & prove next batch
    tester.l2_provider().anvil_commit_batch(2).await?;
    let tx_hash = tester.l2_provider().anvil_prove_batch(2).await?;
    let receipt = tester
        .l1_provider()
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Skipping a batch shouldn't work
    let error = tester
        .l2_provider()
        .anvil_prove_batch(4)
        .await
        .expect_err("prove batch expected to fail");
    assert!(error.to_string().contains("prove transaction failed"));

    Ok(())
}

#[tokio::test]
async fn execute_batch_on_l1() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .build()
        .await?;

    // Pre-generate a few batches for the rest of the test
    for _ in 0..5 {
        tester.tx().finalize().await?.assert_successful()?;
    }

    // Executing batch without committing shouldn't work
    let error = tester
        .l2_provider()
        .anvil_execute_batch(1)
        .await
        .expect_err("execute batch expected to fail");
    assert!(error.to_string().contains("execute transaction failed"));

    // Committing is not enough for executing
    tester.l2_provider().anvil_commit_batch(1).await?;
    let error = tester
        .l2_provider()
        .anvil_execute_batch(1)
        .await
        .expect_err("execute batch expected to fail");
    assert!(error.to_string().contains("execute transaction failed"));

    // Prove & commit first batch after genesis
    tester.l2_provider().anvil_prove_batch(1).await?;
    let tx_hash = tester.l2_provider().anvil_execute_batch(1).await?;
    let receipt = tester
        .l1_provider()
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Executing same batch twice shouldn't work
    let error = tester
        .l2_provider()
        .anvil_execute_batch(1)
        .await
        .expect_err("execute batch expected to fail");
    assert!(error.to_string().contains("execute transaction failed"));

    // Commit & prove & execute next batch
    tester.l2_provider().anvil_commit_batch(2).await?;
    tester.l2_provider().anvil_prove_batch(2).await?;
    let tx_hash = tester.l2_provider().anvil_execute_batch(2).await?;
    let receipt = tester
        .l1_provider()
        .get_transaction_receipt(tx_hash)
        .await?
        .expect("receipt not found on L1");
    assert!(receipt.status());

    // Skipping a batch shouldn't work
    let error = tester
        .l2_provider()
        .anvil_execute_batch(4)
        .await
        .expect_err("execute batch expected to fail");
    assert!(error.to_string().contains("execute transaction failed"));

    Ok(())
}

#[tokio::test]
async fn send_l2_to_l1_message() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .build()
        .await?;

    let message = "Some L2->L1 message";
    let l1_messenger = L1Messenger::new(tester.l2_provider().clone());
    let msg_tx_receipt: ZkReceiptResponse = l1_messenger
        .send_to_l1(message)
        .send()
        .await?
        .get_receipt()
        .await?;
    assert_eq!(
        msg_tx_receipt.l2_to_l1_logs().len(),
        1,
        "expected exactly one L2-to-L1 user log"
    );
    let log = &msg_tx_receipt.l2_to_l1_logs()[0];
    assert_eq!(&log.sender, l1_messenger.address());

    let bridgehub = Bridgehub::new(tester.l1_provider(), tester.l2_provider()).await?;
    let log_batch_number: u64 = msg_tx_receipt
        .l1_batch_number()
        .context("missing L1 batch number")?
        .try_into()?;
    let msg_proof = tester
        .l2_provider()
        .get_l2_to_l1_log_proof(
            msg_tx_receipt.transaction_hash(),
            Some(log.log_index.try_into()?),
        )
        .await?
        .unwrap();
    let l2_message = L2Message {
        txNumberInBatch: msg_tx_receipt
            .l1_batch_tx_index()
            .context("missing L1 batch tx index")?
            .try_into()?,
        sender: tester.l2_provider().default_signer_address(),
        data: message.into(),
    };
    let prove_inclusion_call = bridgehub.prove_l2_message_inclusion(
        log_batch_number,
        msg_proof.id,
        l2_message.clone(),
        msg_proof.proof,
    );

    // Inclusion check fails as the batch has not been executed yet
    assert!(prove_inclusion_call.call().await.is_err());

    // Execute all batches up to the one including the log
    for batch_number in 1..=log_batch_number {
        tester
            .l2_provider()
            .anvil_commit_batch(batch_number)
            .await?;
        tester.l2_provider().anvil_prove_batch(batch_number).await?;
        tester
            .l2_provider()
            .anvil_execute_batch(batch_number)
            .await?;
    }

    // Inclusion check succeeds as the batch has been executed
    let (is_included,) = prove_inclusion_call.call().await?.into();
    assert!(is_included);

    // Inclusion check with fake proof fails
    let fake_prove_inclusion_call = bridgehub.prove_l2_message_inclusion(
        log_batch_number,
        msg_proof.id,
        l2_message,
        vec![B256::random()],
    );
    let (is_included,) = fake_prove_inclusion_call.call().await?.into();
    assert!(!is_included);

    Ok(())
}

#[tokio::test]
async fn l1_priority_tx() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_l1()
        .build()
        .await?;

    // Deploy `Counter` contract and validate that it is initialized with `0`
    let counter = Counter::deploy(tester.l2_provider().clone()).await?;
    assert_eq!(counter.get().await?, U256::from(0));

    // Prepare a transaction from a rich account that will increment `Counter` by 1
    let alice = Address::from_str("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?;
    let eip1559_est = tester.l1_provider().estimate_eip1559_fees().await?;
    let tx = counter
        .increment(1)
        .into_transaction_request()
        .with_from(alice)
        .with_max_fee_per_gas(eip1559_est.max_fee_per_gas);

    // But submit it as an L1 transaction through Bridgehub
    let bridgehub = Bridgehub::new(tester.l1_provider().clone(), tester.l2_provider()).await?;
    bridgehub
        .request_execute(tester.l2_provider(), tx.clone())
        .await?
        .watch()
        .await?;
    // Validate that the counter was increased
    assert_eq!(counter.get().await?, U256::from(1));

    Ok(())
}
