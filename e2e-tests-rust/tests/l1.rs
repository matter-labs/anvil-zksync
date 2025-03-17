use alloy::network::{Ethereum, ReceiptResponse, TransactionBuilder};
use alloy::primitives::{Address, B256, U256};
use alloy::providers::{Provider, ProviderBuilder, WalletProvider};
use alloy::transports::BoxTransport;
use alloy_zksync::network::receipt_response::ReceiptResponse as ZkReceiptResponse;
use alloy_zksync::provider::ZksyncProvider;
use anvil_zksync_e2e_tests::contracts::{Bridgehub, L1Messenger, L2Message};
use anvil_zksync_e2e_tests::http_middleware::HttpWithMiddleware;
use anvil_zksync_e2e_tests::test_contracts::Counter;
use anvil_zksync_e2e_tests::{
    init_testing_provider, AnvilZKsyncApi, FullZksyncProvider, LockedPort, ReceiptExt,
    TestingProvider,
};
use anyhow::Context;
use std::str::FromStr;

async fn init_l1_l2_providers() -> anyhow::Result<(
    impl Provider<BoxTransport, Ethereum> + Clone,
    TestingProvider<impl FullZksyncProvider<HttpWithMiddleware>, HttpWithMiddleware>,
)> {
    let l1_locked_port = LockedPort::acquire_unused().await?;
    let l1_provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_anvil_with_config(|anvil| {
            anvil
                .port(l1_locked_port.port)
                .arg("--no-request-size-limit")
        });
    let l1_address = format!("http://localhost:{}", l1_locked_port.port);
    let l2_provider =
        init_testing_provider(move |node| node.args(["--external-l1", l1_address.as_str()]))
            .await?;

    // Pre-generate a few batches for the rest of the test
    for _ in 0..5 {
        l2_provider.tx().finalize().await?.assert_successful()?;
    }

    Ok((l1_provider, l2_provider))
}

#[tokio::test]
async fn commit_batch_to_l1() -> anyhow::Result<()> {
    let (l1_provider, l2_provider) = init_l1_l2_providers().await?;

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
    let (l1_provider, l2_provider) = init_l1_l2_providers().await?;

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
    let (l1_provider, l2_provider) = init_l1_l2_providers().await?;

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

#[tokio::test]
async fn send_l2_to_l1_message() -> anyhow::Result<()> {
    let (l1_provider, l2_provider) = init_l1_l2_providers().await?;

    let message = "Some L2->L1 message";
    let l1_messenger = L1Messenger::new(l2_provider.clone());
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

    let bridgehub = Bridgehub::new(l1_provider.clone(), &l2_provider).await?;
    let log_batch_number: u64 = msg_tx_receipt
        .l1_batch_number()
        .context("missing L1 batch number")?
        .try_into()?;
    let msg_proof = l2_provider
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
        sender: l2_provider.default_signer_address(),
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
        l2_provider.anvil_commit_batch(batch_number).await?;
        l2_provider.anvil_prove_batch(batch_number).await?;
        l2_provider.anvil_execute_batch(batch_number).await?;
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
    let (l1_provider, l2_provider) = init_l1_l2_providers().await?;

    // Deploy `Counter` contract and validate that it is initialized with `0`
    let counter = Counter::deploy(l2_provider.clone()).await?;
    assert_eq!(counter.get().await?, U256::from(0));

    // Prepare a transaction from a rich account that will increment `Counter` by 1
    let alice = Address::from_str("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?;
    let eip1559_est = l1_provider.estimate_eip1559_fees(None).await?;
    let tx = counter
        .increment(1)
        .into_transaction_request()
        .with_from(alice)
        .with_max_fee_per_gas(eip1559_est.max_fee_per_gas);

    // But submit it as an L1 transaction through Bridgehub
    let bridgehub = Bridgehub::new(l1_provider.clone(), &l2_provider).await?;
    bridgehub
        .request_execute(&l2_provider, tx.clone())
        .await?
        .watch()
        .await?;
    // Validate that the counter was increased
    assert_eq!(counter.get().await?, U256::from(1));

    Ok(())
}
