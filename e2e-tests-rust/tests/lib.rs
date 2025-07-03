use alloy::network::ReceiptResponse;
use alloy::primitives::{address, keccak256, Address, B256};
use alloy::providers::ext::AnvilApi;
use alloy::providers::Provider;
use alloy::providers::WalletProvider;
use alloy::{primitives::U256, signers::local::PrivateKeySigner};
use alloy_zksync::node_bindings::AnvilZKsync;
use anvil_zksync_common::utils::io::write_json_file;
use anvil_zksync_core::node::VersionedState;
use anvil_zksync_e2e_tests::{
    get_node_binary_path, AnvilZKsyncApi, AnvilZksyncTesterBuilder, LockedPort, ReceiptExt,
    ResponseHeadersInspector, ZksyncWalletProviderExt, DEFAULT_TX_VALUE,
};
use anyhow::Context;
use flate2::read::GzDecoder;
use http::header::{
    HeaderMap, HeaderValue, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN, ORIGIN,
};
use std::fs;
use std::io::Read;
use std::time::Duration;
use tempdir::TempDir;

const SOME_ORIGIN: HeaderValue = HeaderValue::from_static("http://some.origin");
const OTHER_ORIGIN: HeaderValue = HeaderValue::from_static("http://other.origin");
const ANY_ORIGIN: HeaderValue = HeaderValue::from_static("*");

#[tokio::test]
async fn interval_sealing_finalization() -> anyhow::Result<()> {
    // Test that we can submit a transaction and wait for it to finalize when anvil-zksync is
    // operating in interval sealing mode.
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.block_time(1))
        .build()
        .await?;

    tester.tx().finalize().await?.assert_successful()?;

    Ok(())
}

#[tokio::test]
async fn interval_sealing_multiple_txs() -> anyhow::Result<()> {
    // Test that we can submit two transactions and wait for them to finalize in the same block when
    // anvil-zksync is operating in interval sealing mode. 3 seconds should be long enough for
    // the entire flow to execute before the first block is produced.
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.block_time(3))
        .build()
        .await?;

    tester
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
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.no_mine())
        .build()
        .await?;

    let pending_tx = tester.tx().register().await?;
    let pending_tx = pending_tx
        .assert_not_finalizable(Duration::from_secs(3))
        .await?;

    // Mine a block manually and assert that the transaction is finalized now
    tester.l2_provider().anvil_mine(None, None).await?;
    pending_tx
        .wait_until_finalized()
        .await?
        .assert_successful()?;

    Ok(())
}

#[tokio::test]
async fn dynamic_sealing_mode() -> anyhow::Result<()> {
    // Test that we can successfully switch between different sealing modes
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.no_mine())
        .build()
        .await?;
    assert!(!tester.l2_provider().anvil_get_auto_mine().await?);

    // Enable immediate block sealing
    tester.l2_provider().anvil_set_auto_mine(true).await?;
    assert!(tester.l2_provider().anvil_get_auto_mine().await?);

    // Check that we can finalize transactions now
    let receipt = tester.tx().finalize().await?;
    assert!(receipt.status());

    // Enable interval block sealing
    tester.l2_provider().anvil_set_interval_mining(3).await?;
    assert!(!tester.l2_provider().anvil_get_auto_mine().await?);

    // Check that we can finalize two txs in the same block now
    tester
        .race_n_txs_rich::<2>()
        .await?
        .assert_successful()?
        .assert_same_block()?;

    // Disable block sealing entirely
    tester.l2_provider().anvil_set_auto_mine(false).await?;
    assert!(!tester.l2_provider().anvil_get_auto_mine().await?);

    // Check that transactions do not get finalized now
    tester
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
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.block_time(3))
        .build()
        .await?;

    let pending_tx0 = tester.tx().with_rich_from(0).register().await?;
    let pending_tx1 = tester.tx().with_rich_from(1).register().await?;

    // Drop first
    tester
        .l2_provider()
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
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.block_time(3))
        .build()
        .await?;

    let pending_tx0 = tester.tx().with_rich_from(0).register().await?;
    let pending_tx1 = tester.tx().with_rich_from(1).register().await?;

    // Drop all transactions
    tester.l2_provider().anvil_drop_all_transactions().await?;

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
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.block_time(3))
        .build()
        .await?;

    // Submit two transactions
    let pending_tx0 = tester.tx().with_rich_from(0).register().await?;
    let pending_tx1 = tester.tx().with_rich_from(1).register().await?;

    // Drop first
    tester
        .l2_provider()
        .anvil_remove_pool_transactions(tester.rich_account(0))
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
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.no_mine())
        .build()
        .await?;

    let pending_tx0 = tester.tx().with_rich_from(0).register().await?;
    let pending_tx1 = tester.tx().with_rich_from(1).register().await?;

    // Mine a block manually and assert that both transactions are finalized now
    tester.l2_provider().anvil_mine(None, None).await?;

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
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.no_mine())
        .build()
        .await?;

    tester.tx().register().await?;

    // Mine a block manually and assert that it has our transaction with extra fields
    let block = tester.l2_provider().anvil_zksync_mine_detailed().await?;
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
    let mut tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.block_time(3))
        .build()
        .await?;
    let random_account = tester.l2_provider_mut().register_random_signer();

    // Impersonate random account for now so that gas estimation works as expected
    tester
        .l2_provider()
        .anvil_impersonate_account(random_account)
        .await?;

    let pending_tx0 = tester.tx().with_rich_from(0).register().await?;
    let pending_tx1 = tester.tx().with_from(random_account).register().await?;
    let pending_tx2 = tester.tx().with_rich_from(1).register().await?;

    // Stop impersonating random account so that tx is going to halt
    tester
        .l2_provider()
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
    let tester = AnvilZksyncTesterBuilder::default().build().await?;

    let receipts = [tester.tx().finalize().await?, tester.tx().finalize().await?];
    let blocks = tester.get_blocks_by_receipts(&receipts).await?;

    // Dump node's state, re-create it and load old state
    let state = tester.l2_provider().anvil_dump_state().await?;
    let tester = AnvilZksyncTesterBuilder::default().build().await?;
    tester.l2_provider().anvil_load_state(state).await?;

    // Assert that new node has pre-restart receipts, blocks and state
    tester.assert_has_receipts(&receipts).await?;
    tester.assert_has_blocks(&blocks).await?;
    tester
        .assert_balance(receipts[0].sender()?, DEFAULT_TX_VALUE)
        .await?;
    tester
        .assert_balance(receipts[1].sender()?, DEFAULT_TX_VALUE)
        .await?;

    // Assert we can still finalize transactions after loading state
    tester.tx().finalize().await?;

    Ok(())
}

#[tokio::test]
async fn cant_load_into_existing_state() -> anyhow::Result<()> {
    // Test that we can't load new state into a node with existing state.
    let tester = AnvilZksyncTesterBuilder::default().build().await?;

    let old_receipts = [tester.tx().finalize().await?, tester.tx().finalize().await?];
    let old_blocks = tester.get_blocks_by_receipts(&old_receipts).await?;

    // Dump node's state and re-create it
    let state = tester.l2_provider().anvil_dump_state().await?;
    let tester = AnvilZksyncTesterBuilder::default().build().await?;

    let new_receipts = [tester.tx().finalize().await?, tester.tx().finalize().await?];
    let new_blocks = tester.get_blocks_by_receipts(&new_receipts).await?;

    // Load state into the new node, make sure it fails and assert that the node still has new
    // receipts, blocks and state.
    assert!(tester.l2_provider().anvil_load_state(state).await.is_err());
    tester.assert_has_receipts(&new_receipts).await?;
    tester.assert_has_blocks(&new_blocks).await?;
    tester
        .assert_balance(new_receipts[0].sender()?, DEFAULT_TX_VALUE)
        .await?;
    tester
        .assert_balance(new_receipts[1].sender()?, DEFAULT_TX_VALUE)
        .await?;

    // Assert the node does not have old state
    tester.assert_no_receipts(&old_receipts).await?;
    tester.assert_no_blocks(&old_blocks).await?;
    tester.assert_balance(old_receipts[0].sender()?, 0).await?;
    tester.assert_balance(old_receipts[1].sender()?, 0).await?;

    Ok(())
}

#[tokio::test]
async fn set_chain_id() -> anyhow::Result<()> {
    let mut tester = AnvilZksyncTesterBuilder::default().build().await?;

    let random_signer = PrivateKeySigner::random();
    let random_signer_address = random_signer.address();

    // Send transaction before changing chain id
    tester
        .tx()
        .with_to(random_signer_address)
        .with_value(U256::from(1e18 as u64))
        .finalize()
        .await?;

    // Change chain id
    let new_chain_id = 123;
    tester
        .l2_provider()
        .anvil_set_chain_id(new_chain_id)
        .await?;

    // Verify new chain id
    assert_eq!(new_chain_id, tester.l2_provider().get_chain_id().await?);

    // Verify transactions can be executed after chain id change
    // Registering and using new signer to get new chain id applied
    tester.l2_provider_mut().register_signer(random_signer);
    tester
        .tx()
        .with_from(random_signer_address)
        .with_chain_id(new_chain_id)
        .finalize()
        .await?;

    Ok(())
}

#[tokio::test]
async fn cli_no_cors() -> anyhow::Result<()> {
    // Verify all origins are allowed by default
    let response_inspector = ResponseHeadersInspector::default();
    let tester = AnvilZksyncTesterBuilder::default()
        .with_client_middleware_fn(&|client_builder| {
            client_builder.with(response_inspector.clone())
        })
        .build()
        .await?;
    tester.l2_provider().get_chain_id().await?;
    let resp_headers = response_inspector.last_unwrap();
    assert_eq!(
        resp_headers.get(ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&ANY_ORIGIN)
    );

    // Making OPTIONS request reveals all access control settings
    let client = reqwest::Client::new();
    let l2_url = tester.l2_url.clone();
    let resp = client
        .request(reqwest::Method::OPTIONS, l2_url)
        .send()
        .await?;
    assert_eq!(
        resp.headers().get(ACCESS_CONTROL_ALLOW_METHODS),
        Some(&HeaderValue::from_static("GET,POST"))
    );
    assert_eq!(
        resp.headers().get(ACCESS_CONTROL_ALLOW_HEADERS),
        Some(&HeaderValue::from_static("content-type"))
    );
    assert_eq!(
        resp.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&ANY_ORIGIN)
    );
    drop(tester);

    // Verify access control is disabled with --no-cors
    let response_inspector_no_cors = ResponseHeadersInspector::default();
    let tester_no_cors = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.arg("--no-cors"))
        .with_client_middleware_fn(&|client_builder| {
            client_builder.with(response_inspector_no_cors.clone())
        })
        .build()
        .await?;
    tester_no_cors.l2_provider().get_chain_id().await?;
    let resp_headers = response_inspector_no_cors.last_unwrap();
    assert_eq!(resp_headers.get(ACCESS_CONTROL_ALLOW_ORIGIN), None);

    Ok(())
}

#[tokio::test]
async fn cli_allow_origin() -> anyhow::Result<()> {
    let req_headers = HeaderMap::from_iter([(ORIGIN, SOME_ORIGIN)]);

    // Verify allowed origin can make requests
    let response_inspector = ResponseHeadersInspector::default();
    let tester_with_allowed_origin = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.arg(format!("--allow-origin={}", SOME_ORIGIN.to_str().unwrap())))
        .with_client_fn(&|client| client.default_headers(req_headers.clone()))
        .with_client_middleware_fn(&|client_builder| {
            client_builder.with(response_inspector.clone())
        })
        .build()
        .await?;
    tester_with_allowed_origin
        .l2_provider()
        .get_chain_id()
        .await?;
    let resp_headers = response_inspector.last_unwrap();
    assert_eq!(
        resp_headers.get(ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&SOME_ORIGIN)
    );
    drop(tester_with_allowed_origin);

    // Verify different origin are also allowed to make requests. CORS is reliant on the browser
    // to respect access control headers reported by the server.
    let response_inspector_not_allowed = ResponseHeadersInspector::default();
    let tester_with_not_allowed_origin = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| {
            node.arg(format!("--allow-origin={}", OTHER_ORIGIN.to_str().unwrap()))
        })
        .with_client_fn(&|client| client.default_headers(req_headers.clone()))
        .with_client_middleware_fn(&|client_builder| {
            client_builder.with(response_inspector_not_allowed.clone())
        })
        .build()
        .await?;
    tester_with_not_allowed_origin
        .l2_provider()
        .get_chain_id()
        .await?;
    let resp_headers = response_inspector_not_allowed.last_unwrap();
    assert_eq!(
        resp_headers.get(ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&OTHER_ORIGIN)
    );

    Ok(())
}

#[tokio::test]
async fn pool_txs_order_fifo() -> anyhow::Result<()> {
    let tester_fifo = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.no_mine())
        .build()
        .await?;

    let pending_tx0 = tester_fifo
        .tx()
        .with_rich_from(0)
        .with_max_fee_per_gas(50_000_000)
        .register()
        .await?;
    let pending_tx1 = tester_fifo
        .tx()
        .with_rich_from(1)
        .with_max_fee_per_gas(100_000_000)
        .register()
        .await?;
    let pending_tx2 = tester_fifo
        .tx()
        .with_rich_from(2)
        .with_max_fee_per_gas(150_000_000)
        .register()
        .await?;

    tester_fifo.l2_provider().anvil_mine(Some(1), None).await?;

    let block = tester_fifo
        .l2_provider()
        .get_block(1.into())
        .await?
        .unwrap();
    let tx_hashes = block.transactions.as_hashes().unwrap();
    assert_eq!(&tx_hashes[0], pending_tx0.tx_hash());
    assert_eq!(&tx_hashes[1], pending_tx1.tx_hash());
    assert_eq!(&tx_hashes[2], pending_tx2.tx_hash());
    Ok(())
}

#[tokio::test]
async fn pool_txs_order_fees() -> anyhow::Result<()> {
    let tester_fees = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.no_mine().arg("--order=fees"))
        .build()
        .await?;

    let pending_tx0 = tester_fees
        .tx()
        .with_rich_from(0)
        .with_max_fee_per_gas(50_000_000)
        .register()
        .await?;
    let pending_tx1 = tester_fees
        .tx()
        .with_rich_from(1)
        .with_max_fee_per_gas(100_000_000)
        .register()
        .await?;
    let pending_tx2 = tester_fees
        .tx()
        .with_rich_from(2)
        .with_max_fee_per_gas(150_000_000)
        .register()
        .await?;

    tester_fees.l2_provider().anvil_mine(Some(1), None).await?;

    let block = tester_fees
        .l2_provider()
        .get_block(1.into())
        .await?
        .unwrap();
    let tx_hashes = block.transactions.as_hashes().unwrap();
    assert_eq!(&tx_hashes[0], pending_tx2.tx_hash());
    assert_eq!(&tx_hashes[1], pending_tx1.tx_hash());
    assert_eq!(&tx_hashes[2], pending_tx0.tx_hash());
    Ok(())
}

#[tokio::test]
async fn transactions_have_index() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.no_mine())
        .build()
        .await?;
    let tx1 = tester.tx().with_rich_from(0).register().await?;
    let tx2 = tester.tx().with_rich_from(1).register().await?;

    tester.l2_provider().anvil_mine(Some(1), None).await?;

    let receipt1 = tx1.wait_until_finalized().await?;
    let receipt2 = tx2.wait_until_finalized().await?;

    assert_eq!(receipt1.transaction_index(), 0.into());
    assert_eq!(receipt2.transaction_index(), 1.into());
    Ok(())
}

#[tokio::test]
async fn dump_state_on_run() -> anyhow::Result<()> {
    let temp_dir = TempDir::new("state-test").expect("failed creating temporary dir");
    let dump_path = temp_dir.path().join("state_dump.json");

    let dump_path_clone = dump_path.clone();
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| {
            node.path(get_node_binary_path())
                .arg("--state-interval")
                .arg("1")
                .arg("--dump-state")
                .arg(dump_path_clone.to_str().unwrap())
        })
        .build()
        .await?;

    let receipt = tester.tx().finalize().await?;
    let tx_hash = receipt.transaction_hash().to_string();

    tokio::time::sleep(Duration::from_secs(2)).await;

    drop(tester);

    assert!(
        dump_path.exists(),
        "State dump file should exist at {dump_path:?}"
    );

    let dumped_data = fs::read_to_string(&dump_path)?;
    let state: VersionedState =
        serde_json::from_str(&dumped_data).context("Failed to deserialize state")?;

    match state {
        VersionedState::V1 { version: _, state } => {
            assert!(
                !state.blocks.is_empty(),
                "state_dump.json should contain at least one block"
            );
            assert!(
                !state.transactions.is_empty(),
                "state_dump.json should contain at least one transaction"
            );
            let tx_exists = state.transactions.iter().any(|tx| {
                let tx_hash_full =
                    format!("0x{}", hex::encode(tx.receipt.transaction_hash.as_bytes()));
                tx_hash_full == tx_hash
            });

            assert!(
                tx_exists,
                "The state dump should contain the transaction with hash: {tx_hash:?}"
            );
        }
        VersionedState::Unknown { version } => {
            panic!("Encountered unknown state version: {version}");
        }
    }

    Ok(())
}

#[tokio::test]
#[ignore]
// TODO: Investigate a better way to test against fork to avoid flakiness. See: https://github.com/matter-labs/anvil-zksync/issues/508
async fn dump_state_on_fork() -> anyhow::Result<()> {
    let temp_dir = TempDir::new("state-fork-test").expect("failed creating temporary dir");
    let dump_path = temp_dir.path().join("state_dump_fork.json");

    let dump_path_clone = dump_path.clone();
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| {
            node.path(get_node_binary_path())
                .arg("--state-interval")
                .arg("1")
                .arg("--dump-state")
                .arg(dump_path_clone.to_str().unwrap())
                .fork("sepolia-testnet")
        })
        .build()
        .await?;

    let receipt = tester.tx().finalize().await?;
    let tx_hash = receipt.transaction_hash().to_string();

    tokio::time::sleep(Duration::from_secs(2)).await;

    drop(tester);

    assert!(
        dump_path.exists(),
        "State dump file should exist at {dump_path:?}"
    );

    let dumped_data = fs::read_to_string(&dump_path)?;
    let state: VersionedState =
        serde_json::from_str(&dumped_data).context("Failed to deserialize state")?;

    match state {
        VersionedState::V1 { version: _, state } => {
            assert!(
                !state.blocks.is_empty(),
                "state_dump_fork.json should contain at least one block"
            );
            assert!(
                !state.transactions.is_empty(),
                "state_dump_fork.json should contain at least one transaction"
            );
            let tx_exists = state.transactions.iter().any(|tx| {
                let tx_hash_full =
                    format!("0x{}", hex::encode(tx.receipt.transaction_hash.as_bytes()));
                tx_hash_full == tx_hash
            });
            assert!(
                tx_exists,
                "The state dump should contain the transaction with hash: {tx_hash:?}",
            );
        }
        VersionedState::Unknown { version } => {
            panic!("Encountered unknown state version: {version}");
        }
    }

    Ok(())
}

#[tokio::test]
async fn load_state_on_run() -> anyhow::Result<()> {
    let temp_dir = TempDir::new("load-state-test").expect("failed creating temporary dir");
    let dump_path = temp_dir.path().join("load_state_run.json");
    let tester = AnvilZksyncTesterBuilder::default().build().await?;
    let receipts = [tester.tx().finalize().await?, tester.tx().finalize().await?];
    let blocks = tester.get_blocks_by_receipts(&receipts).await?;
    let state_bytes = tester.l2_provider().anvil_dump_state().await?;
    drop(tester);

    let mut decoder = GzDecoder::new(&state_bytes.0[..]);
    let mut json_str = String::new();
    decoder.read_to_string(&mut json_str).unwrap();
    let state: VersionedState = serde_json::from_str(&json_str).unwrap();
    write_json_file(&dump_path, &state)?;

    let dump_path_clone = dump_path.clone();
    let new_provider = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| {
            node.path(get_node_binary_path())
                .arg("--state-interval")
                .arg("1")
                .arg("--load-state")
                .arg(dump_path_clone.to_str().unwrap())
        })
        .build()
        .await?;

    new_provider.assert_has_receipts(&receipts).await?;
    new_provider.assert_has_blocks(&blocks).await?;
    new_provider
        .assert_balance(receipts[0].sender()?, DEFAULT_TX_VALUE)
        .await?;
    new_provider
        .assert_balance(receipts[1].sender()?, DEFAULT_TX_VALUE)
        .await?;

    drop(new_provider);

    assert!(
        dump_path.exists(),
        "State dump file should still exist at {dump_path:?}"
    );

    Ok(())
}

#[tokio::test]
#[ignore]
// TODO: Investigate a better way to test against fork to avoid flakiness. See: https://github.com/matter-labs/anvil-zksync/issues/508
async fn load_state_on_fork() -> anyhow::Result<()> {
    let temp_dir = TempDir::new("load-state-fork-test").expect("failed creating temporary dir");
    let dump_path = temp_dir.path().join("load_state_fork.json");
    let tester = AnvilZksyncTesterBuilder::default().build().await?;
    let receipts = [tester.tx().finalize().await?, tester.tx().finalize().await?];
    let blocks = tester.get_blocks_by_receipts(&receipts).await?;
    let state_bytes = tester.l2_provider().anvil_dump_state().await?;
    drop(tester);

    let mut decoder = GzDecoder::new(&state_bytes.0[..]);
    let mut json_str = String::new();
    decoder.read_to_string(&mut json_str).unwrap();
    let state: VersionedState = serde_json::from_str(&json_str).unwrap();
    write_json_file(&dump_path, &state)?;

    let dump_path_clone = dump_path.clone();
    let new_provider = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| {
            node.path(get_node_binary_path())
                .arg("--state-interval")
                .arg("1")
                .arg("--load-state")
                .arg(dump_path_clone.to_str().unwrap())
                .fork("sepolia-testnet")
        })
        .build()
        .await?;

    new_provider.assert_has_receipts(&receipts).await?;
    new_provider.assert_has_blocks(&blocks).await?;
    new_provider
        .assert_balance(receipts[0].sender()?, DEFAULT_TX_VALUE)
        .await?;
    new_provider
        .assert_balance(receipts[1].sender()?, DEFAULT_TX_VALUE)
        .await?;

    drop(new_provider);

    assert!(
        dump_path.exists(),
        "State dump file should still exist at {dump_path:?}"
    );

    Ok(())
}

#[tokio::test]
async fn test_server_port_fallback() -> anyhow::Result<()> {
    let locked_port = LockedPort::acquire_unused().await?;

    let node1 = AnvilZKsync::new()
        .path(get_node_binary_path())
        .port(locked_port.port)
        .spawn();
    let port1 = node1.port();

    let node2 = AnvilZKsync::new()
        .path(get_node_binary_path())
        .port(locked_port.port)
        .spawn();
    let port2 = node2.port();

    assert_ne!(
        port1, port2,
        "The second instance should have a different port due to fallback"
    );

    drop(node1);
    drop(node2);

    Ok(())
}

#[tokio::test]
async fn storage_at_historical() -> anyhow::Result<()> {
    // Test that we can query historical storage slots
    let tester = AnvilZksyncTesterBuilder::default().build().await?;

    tester.tx().finalize().await?;
    tester.tx().finalize().await?;
    tester.tx().finalize().await?;

    const NONCE_HOLDER_ADDRESS: Address = address!("0000000000000000000000000000000000008003");
    let address = B256::left_padding_from(tester.l2_provider().default_signer_address().as_ref());
    let key = keccak256(address.concat_const::<32, 64>(B256::ZERO));
    let fetch_nonce = async |n: u64| {
        tester
            .l2_provider()
            .get_storage_at(NONCE_HOLDER_ADDRESS, key.into())
            .number(n)
            .await
    };
    // Make sure nonce starts from 0 and then increases by 1 every block (excluding virtual ones).
    assert_eq!(fetch_nonce(0).await?, U256::from(0));
    assert_eq!(fetch_nonce(1).await?, U256::from(1));
    assert_eq!(fetch_nonce(2).await?, U256::from(1));
    assert_eq!(fetch_nonce(3).await?, U256::from(2));
    assert_eq!(fetch_nonce(4).await?, U256::from(2));
    assert_eq!(fetch_nonce(5).await?, U256::from(3));
    assert_eq!(fetch_nonce(6).await?, U256::from(3));

    Ok(())
}
