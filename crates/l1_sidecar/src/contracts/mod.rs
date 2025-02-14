// Hide ugly auto-generated alloy structs outside of this module.
mod private {
    // Macros that hide non-trivial implementations are not great. One considered alternative was to
    // use `alloy_sol_macro_expander` directly from `build.rs`, prettify generated code with
    // `prettyplease` and then output into a VCS-tracked directory. Although this works, unfortunately
    // the generated code is still very ugly, so I decided to not go forward with this for now.
    //
    // Once https://github.com/alloy-rs/core/issues/261 is resolved hopefully it will become much more
    // human-readable.
    //
    // Additionally, https://github.com/alloy-rs/core/issues/601 tracks proper support for output into
    // a file.
    alloy::sol!("src/contracts/sol/IExecutor.sol");
}

use self::private::IExecutor;
use alloy::sol_types::{SolCall, SolValue};
use zksync_types::commitment::L1BatchWithMetadata;
use zksync_types::L2ChainId;

/// Current commitment encoding version as per protocol.
pub const SUPPORTED_ENCODING_VERSION: u8 = 0;

/// Builds a Solidity function call to `commitBatchesSharedBridgeCall` as expected by `IExecutor.sol`.
///
/// Assumes system log verification and DA input verification are disabled.
pub fn commit_batches_shared_bridge_call(
    l2_chain_id: L2ChainId,
    last_committed_l1_batch: &L1BatchWithMetadata,
    batch: &L1BatchWithMetadata,
) -> impl SolCall {
    IExecutor::commitBatchesSharedBridgeCall::new((
        alloy::primitives::U256::from(l2_chain_id.as_u64()),
        alloy::primitives::U256::from(last_committed_l1_batch.header.number.0 + 1),
        alloy::primitives::U256::from(last_committed_l1_batch.header.number.0 + 1),
        commit_calldata(last_committed_l1_batch, batch).into(),
    ))
}

/// `commitBatchesSharedBridgeCall` expects the rest of calldata to be of very specific form. This
/// function makes sure last committed batch and new batch are encoded correctly.
fn commit_calldata(
    last_committed_l1_batch: &L1BatchWithMetadata,
    batch: &L1BatchWithMetadata,
) -> Vec<u8> {
    let stored_batch_info = IExecutor::StoredBatchInfo::from((
        last_committed_l1_batch.header.number.0 as u64,
        alloy::primitives::FixedBytes::<32>::from(last_committed_l1_batch.metadata.root_hash.0),
        last_committed_l1_batch.metadata.rollup_last_leaf_index,
        alloy::primitives::U256::from(last_committed_l1_batch.header.l1_tx_count),
        alloy::primitives::FixedBytes::<32>::from(
            last_committed_l1_batch
                .header
                .priority_ops_onchain_data_hash()
                .0,
        ),
        alloy::primitives::FixedBytes::<32>::from(
            last_committed_l1_batch.metadata.l2_l1_merkle_root.0,
        ),
        alloy::primitives::U256::from(last_committed_l1_batch.header.timestamp),
        alloy::primitives::FixedBytes::<32>::from(last_committed_l1_batch.metadata.commitment.0),
    ));
    let commit_batch_info = IExecutor::CommitBatchInfo::from((
        batch.header.number.0 as u64,
        batch.header.timestamp,
        batch.metadata.rollup_last_leaf_index,
        alloy::primitives::FixedBytes::<32>::from(batch.metadata.root_hash.0),
        alloy::primitives::U256::from(batch.header.l1_tx_count),
        alloy::primitives::FixedBytes::<32>::from(batch.header.priority_ops_onchain_data_hash().0),
        alloy::primitives::FixedBytes::<32>::from(
            batch
                .metadata
                .bootloader_initial_content_commitment
                .unwrap()
                .0,
        ),
        alloy::primitives::FixedBytes::<32>::from(
            batch.metadata.events_queue_commitment.unwrap().0,
        ),
        // System log verification is disabled on L1 so we pretend we don't have any
        alloy::primitives::Bytes::new(),
        // Same for DA input
        alloy::primitives::Bytes::new(),
    ));
    let mut commit_data = (stored_batch_info, vec![commit_batch_info]).abi_encode_params();
    // Prefixed by current encoding version as expected by protocol
    commit_data.insert(0, SUPPORTED_ENCODING_VERSION);

    commit_data
}
