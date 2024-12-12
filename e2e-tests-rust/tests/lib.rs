use alloy::network::ReceiptResponse;
use alloy::providers::ext::AnvilApi;
use alloy::providers::Provider;
use anvil_zksync_e2e_tests::{
    init_testing_provider, AnvilZKsyncApi, ReceiptExt, ZksyncWalletProviderExt, DEFAULT_TX_VALUE,
};
use alloy::{
    primitives::U256,
    signers::local::PrivateKeySigner,
};
use std::convert::identity;
use std::time::Duration;

#[tokio::test]
async fn interval_sealing_finalization() -> anyhow::Result<()> {
    // Test that we can submit a transaction and wait for it to finalize when anvil-zksync is
    // operating in interval sealing mode.
    let provider = init_testing_provider(|node| node.block_time(1)).await?;

    provider.tx().finalize().await?.assert_successful()?;

    Ok(())
}

#[tokio::test]
async fn interval_sealing_multiple_txs() -> anyhow::Result<()> {
    // Test that we can submit two transactions and wait for them to finalize in the same block when
    // anvil-zksync is operating in interval sealing mode. 3 seconds should be long enough for
    // the entire flow to execute before the first block is produced.
    let provider = init_testing_provider(|node| node.block_time(3)).await?;

    provider
        .race_n_txs_rich::<2>()
        .await?
        .assert_successful()?
        .assert_same_block()?;

    Ok(())
}

#[tokio::test]
async fn no_sealing_timeout() -> anyhow::Result<()> {
    // Test that we can submit a transaction and timeout while waiting for it to finalize when
    // anvil-zksync is operating in no sealing mode.
    let provider = init_testing_provider(|node| node.no_mine()).await?;

    let pending_tx = provider.tx().register().await?;
    let pending_tx = pending_tx
        .assert_not_finalizable(Duration::from_secs(3))
        .await?;

    // Mine a block manually and assert that the transaction is finalized now
    provider.anvil_mine(None, None).await?;
    pending_tx
        .wait_until_finalized()
        .await?
        .assert_successful()?;

    Ok(())
}

#[tokio::test]
async fn dynamic_sealing_mode() -> anyhow::Result<()> {
    // Test that we can successfully switch between different sealing modes
    let provider = init_testing_provider(|node| node.no_mine()).await?;
    assert_eq!(provider.anvil_get_auto_mine().await?, false);

    // Enable immediate block sealing
    provider.anvil_set_auto_mine(true).await?;
    assert_eq!(provider.anvil_get_auto_mine().await?, true);

    // Check that we can finalize transactions now
    let receipt = provider.tx().finalize().await?;
    assert!(receipt.status());

    // Enable interval block sealing
    provider.anvil_set_interval_mining(3).await?;
    assert_eq!(provider.anvil_get_auto_mine().await?, false);

    // Check that we can finalize two txs in the same block now
    provider
        .race_n_txs_rich::<2>()
        .await?
        .assert_successful()?
        .assert_same_block()?;

    // Disable block sealing entirely
    provider.anvil_set_auto_mine(false).await?;
    assert_eq!(provider.anvil_get_auto_mine().await?, false);

    // Check that transactions do not get finalized now
    provider
        .tx()
        .register()
        .await?
        .assert_not_finalizable(Duration::from_secs(3))
        .await?;

    Ok(())
}

#[tokio::test]
async fn drop_transaction() -> anyhow::Result<()> {
    // Test that we can submit two transactions and then remove one from the pool before it gets
    // finalized. 3 seconds should be long enough for the entire flow to execute before the first
    // block is produced.
    let provider = init_testing_provider(|node| node.block_time(3)).await?;

    let pending_tx0 = provider.tx().with_rich_from(0).register().await?;
    let pending_tx1 = provider.tx().with_rich_from(1).register().await?;

    // Drop first
    provider
        .anvil_drop_transaction(*pending_tx0.tx_hash())
        .await?;

    // Assert first never gets finalized but the second one does
    pending_tx0
        .assert_not_finalizable(Duration::from_secs(4))
        .await?;
    pending_tx1
        .wait_until_finalized()
        .await?
        .assert_successful()?;

    Ok(())
}

#[tokio::test]
async fn drop_all_transactions() -> anyhow::Result<()> {
    // Test that we can submit two transactions and then remove them from the pool before they get
    // finalized. 3 seconds should be long enough for the entire flow to execute before the first
    // block is produced.
    let provider = init_testing_provider(|node| node.block_time(3)).await?;

    let pending_tx0 = provider.tx().with_rich_from(0).register().await?;
    let pending_tx1 = provider.tx().with_rich_from(1).register().await?;

    // Drop all transactions
    provider.anvil_drop_all_transactions().await?;

    // Neither transaction gets finalized
    pending_tx0
        .assert_not_finalizable(Duration::from_secs(4))
        .await?;
    pending_tx1
        .assert_not_finalizable(Duration::from_secs(4))
        .await?;

    Ok(())
}

#[tokio::test]
async fn remove_pool_transactions() -> anyhow::Result<()> {
    // Test that we can submit two transactions from two senders and then remove first sender's
    // transaction from the pool before it gets finalized. 3 seconds should be long enough for the
    // entire flow to execute before the first block is produced.
    let provider = init_testing_provider(|node| node.block_time(3)).await?;

    // Submit two transactions
    let pending_tx0 = provider.tx().with_rich_from(0).register().await?;
    let pending_tx1 = provider.tx().with_rich_from(1).register().await?;

    // Drop first
    provider
        .anvil_remove_pool_transactions(provider.rich_account(0))
        .await?;

    // Assert first never gets finalized but the second one does
    pending_tx0
        .assert_not_finalizable(Duration::from_secs(4))
        .await?;
    pending_tx1
        .wait_until_finalized()
        .await?
        .assert_successful()?;

    Ok(())
}

#[tokio::test]
async fn manual_mining_two_txs_in_one_block() -> anyhow::Result<()> {
    // Test that we can submit two transaction and then manually mine one block that contains both
    // transactions in it.
    let provider = init_testing_provider(|node| node.no_mine()).await?;

    let pending_tx0 = provider.tx().with_rich_from(0).register().await?;
    let pending_tx1 = provider.tx().with_rich_from(1).register().await?;

    // Mine a block manually and assert that both transactions are finalized now
    provider.anvil_mine(None, None).await?;

    let receipt0 = pending_tx0.wait_until_finalized().await?;
    receipt0.assert_successful()?;
    let receipt1 = pending_tx1.wait_until_finalized().await?;
    receipt1.assert_successful()?;
    receipt0.assert_same_block(&receipt1)?;

    Ok(())
}

#[tokio::test]
async fn detailed_mining_success() -> anyhow::Result<()> {
    // Test that we can call detailed mining after a successful transaction and match output from it.
    let provider = init_testing_provider(|node| node.no_mine()).await?;

    provider.tx().register().await?;

    // Mine a block manually and assert that it has our transaction with extra fields
    let block = provider.anvil_zksync_mine_detailed().await?;
    assert_eq!(block.transactions.len(), 1);
    let actual_tx = block
        .transactions
        .clone()
        .into_transactions()
        .next()
        .unwrap();

    assert_eq!(
        actual_tx.other.get("output").and_then(|x| x.as_str()),
        Some("0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000")
    );
    assert!(actual_tx.other.get("revertReason").is_none());

    Ok(())
}

#[tokio::test]
async fn seal_block_ignoring_halted_transaction() -> anyhow::Result<()> {
    // Test that we can submit three transactions (1 and 3 are successful, 2 is halting). And then
    // observe a block that finalizes 1 and 3 while ignoring 2.
    let mut provider = init_testing_provider(|node| node.block_time(3)).await?;
    let random_account = provider.register_random_signer();

    // Impersonate random account for now so that gas estimation works as expected
    provider.anvil_impersonate_account(random_account).await?;

    let pending_tx0 = provider.tx().with_rich_from(0).register().await?;
    let pending_tx1 = provider.tx().with_from(random_account).register().await?;
    let pending_tx2 = provider.tx().with_rich_from(1).register().await?;

    // Stop impersonating random account so that tx is going to halt
    provider
        .anvil_stop_impersonating_account(random_account)
        .await?;

    // Fetch their receipts and assert they are executed in the same block
    let receipt0 = pending_tx0.wait_until_finalized().await?;
    receipt0.assert_successful()?;
    let receipt2 = pending_tx2.wait_until_finalized().await?;
    receipt2.assert_successful()?;
    receipt0.assert_same_block(&receipt2)?;

    // Halted transaction never gets finalized
    pending_tx1
        .assert_not_finalizable(Duration::from_secs(4))
        .await?;

    Ok(())
}

#[tokio::test]
async fn dump_and_load_state() -> anyhow::Result<()> {
    // Test that we can submit transactions, then dump state and shutdown the node. Following that we
    // should be able to spin up a new node and load state into it. Previous transactions/block should
    // be present on the new node along with the old state.
    let provider = init_testing_provider(identity).await?;

    let receipts = [
        provider.tx().finalize().await?,
        provider.tx().finalize().await?,
    ];
    let blocks = provider.get_blocks_by_receipts(&receipts).await?;

    // Dump node's state, re-create it and load old state
    let state = provider.anvil_dump_state().await?;
    let provider = init_testing_provider(identity).await?;
    provider.anvil_load_state(state).await?;

    // Assert that new node has pre-restart receipts, blocks and state
    provider.assert_has_receipts(&receipts).await?;
    provider.assert_has_blocks(&blocks).await?;
    provider
        .assert_balance(receipts[0].sender()?, DEFAULT_TX_VALUE)
        .await?;
    provider
        .assert_balance(receipts[1].sender()?, DEFAULT_TX_VALUE)
        .await?;

    // Assert we can still finalize transactions after loading state
    provider.tx().finalize().await?;

    Ok(())
}

#[tokio::test]
async fn cant_load_into_existing_state() -> anyhow::Result<()> {
    // Test that we can't load new state into a node with existing state.
    let provider = init_testing_provider(identity).await?;

    let old_receipts = [
        provider.tx().finalize().await?,
        provider.tx().finalize().await?,
    ];
    let old_blocks = provider.get_blocks_by_receipts(&old_receipts).await?;

    // Dump node's state and re-create it
    let state = provider.anvil_dump_state().await?;
    let provider = init_testing_provider(identity).await?;

    let new_receipts = [
        provider.tx().finalize().await?,
        provider.tx().finalize().await?,
    ];
    let new_blocks = provider.get_blocks_by_receipts(&new_receipts).await?;

    // Load state into the new node, make sure it fails and assert that the node still has new
    // receipts, blocks and state.
    assert!(provider.anvil_load_state(state).await.is_err());
    provider.assert_has_receipts(&new_receipts).await?;
    provider.assert_has_blocks(&new_blocks).await?;
    provider
        .assert_balance(new_receipts[0].sender()?, DEFAULT_TX_VALUE)
        .await?;
    provider
        .assert_balance(new_receipts[1].sender()?, DEFAULT_TX_VALUE)
        .await?;

    // Assert the node does not have old state
    provider.assert_no_receipts(&old_receipts).await?;
    provider.assert_no_blocks(&old_blocks).await?;
    provider
        .assert_balance(old_receipts[0].sender()?, 0)
        .await?;
    provider
        .assert_balance(old_receipts[1].sender()?, 0)
        .await?;

    Ok(())
}

#[tokio::test]
async fn set_chain_id() -> anyhow::Result<()> {
    let mut provider = init_testing_provider(identity).await?;

    let random_signer = PrivateKeySigner::random();
    let random_signer_address = random_signer.address();

    // Send transaction before changing chain id
    provider.tx().with_to(random_signer_address).with_value(U256::from(1e18 as u64)).finalize().await?;

    // Change chain id
    let new_chain_id = 123;
    provider.anvil_set_chain_id(new_chain_id).await?;

    // Verify new chain id
    assert_eq!(new_chain_id, provider.get_chain_id().await?);

    // Verify transactions can be executed after chain id change
    // Registering and using new signer to get new chain id applied
    provider.register_signer(random_signer);
    provider.tx().with_from(random_signer_address).with_chain_id(new_chain_id).finalize().await?;

    Ok(())
}
