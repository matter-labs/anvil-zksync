// Hide ugly auto-generated alloy structs outside of this module.
mod private {
    use zksync_types::commitment::L1BatchWithMetadata;

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
    // Copied from `PriorityTree.sol` as the entire file has imports that are unprocessable by `alloy::sol!`
    alloy::sol! {
        struct PriorityOpsBatchInfo {
            bytes32[] leftPath;
            bytes32[] rightPath;
            bytes32[] itemHashes;
        }
    }
    alloy::sol!(IZKChain, "src/contracts/artifacts/IZKChain.json");

    impl From<&L1BatchWithMetadata> for IExecutor::StoredBatchInfo {
        fn from(value: &L1BatchWithMetadata) -> Self {
            Self::from((
                value.header.number.0 as u64,
                alloy::primitives::FixedBytes::<32>::from(value.metadata.root_hash.0),
                value.metadata.rollup_last_leaf_index,
                alloy::primitives::U256::from(value.header.l1_tx_count),
                alloy::primitives::FixedBytes::<32>::from(
                    value.header.priority_ops_onchain_data_hash().0,
                ),
                alloy::primitives::FixedBytes::<32>::from(value.metadata.l2_l1_merkle_root.0),
                alloy::primitives::U256::from(value.header.timestamp),
                alloy::primitives::FixedBytes::<32>::from(value.metadata.commitment.0),
            ))
        }
    }
}

pub use self::private::IZKChain::NewPriorityRequest;
use alloy::primitives::TxHash;

use self::private::{IExecutor, PriorityOpsBatchInfo};
use alloy::sol_types::{SolCall, SolValue};
use zksync_mini_merkle_tree::MiniMerkleTree;
use zksync_types::commitment::{serialize_commitments, L1BatchWithMetadata};
use zksync_types::l1::L1Tx;
use zksync_types::web3::keccak256;
use zksync_types::{L2ChainId, H256};

/// Current commitment encoding version as per protocol.
pub const SUPPORTED_ENCODING_VERSION: u8 = 0;

/// Builds a Solidity function call to `commitBatchesSharedBridge` as expected by `IExecutor.sol`.
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

/// `commitBatchesSharedBridge` expects the rest of calldata to be of very specific form. This
/// function makes sure last committed batch and new batch are encoded correctly (assumes post gateway).
fn commit_calldata(
    last_committed_l1_batch: &L1BatchWithMetadata,
    batch: &L1BatchWithMetadata,
) -> Vec<u8> {
    let stored_batch_info = IExecutor::StoredBatchInfo::from(last_committed_l1_batch);
    let last_batch_hash = H256(keccak256(stored_batch_info.abi_encode_params().as_slice()));
    tracing::info!(?last_batch_hash, "preparing commit calldata");

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
        alloy::primitives::Bytes::from(serialize_commitments(&batch.header.system_logs)),
        // Our DA input consists only of state diff hash. Executor is patched to not use anything else.
        alloy::primitives::Bytes::from(
            batch
                .metadata
                .state_diff_hash
                .expect("Failed to get state_diff_hash from metadata")
                .0,
        ),
    ));
    let encoded_data = (stored_batch_info, vec![commit_batch_info]).abi_encode_params();

    // Prefixed by current encoding version as expected by protocol
    [[SUPPORTED_ENCODING_VERSION].to_vec(), encoded_data]
        .concat()
        .to_vec()
}

/// Builds a Solidity function call to `proveBatchesSharedBridge` as expected by `IExecutor.sol`.
///
/// Assumes `TestnetVerifier` was deployed (thus verification for empty proofs is disabled).
pub fn prove_batches_shared_bridge_call(
    l2_chain_id: L2ChainId,
    last_proved_l1_batch: &L1BatchWithMetadata,
    batch: &L1BatchWithMetadata,
) -> impl SolCall {
    IExecutor::proveBatchesSharedBridgeCall::new((
        alloy::primitives::U256::from(l2_chain_id.as_u64()),
        alloy::primitives::U256::from(last_proved_l1_batch.header.number.0 + 1),
        alloy::primitives::U256::from(last_proved_l1_batch.header.number.0 + 1),
        prove_calldata(last_proved_l1_batch, batch).into(),
    ))
}

/// `proveBatchesSharedBridge` expects the rest of calldata to be of very specific form. This
/// function makes sure last proved batch and new batch are encoded correctly (assumes post gateway).
fn prove_calldata(
    last_proved_l1_batch: &L1BatchWithMetadata,
    batch: &L1BatchWithMetadata,
) -> Vec<u8> {
    let prev_l1_batch_info = IExecutor::StoredBatchInfo::from(last_proved_l1_batch);
    let batches_arg = vec![IExecutor::StoredBatchInfo::from(batch)];
    let proof_input = Vec::<alloy::primitives::U256>::new();
    let encoded_data = (prev_l1_batch_info, batches_arg, proof_input).abi_encode_params();

    // Prefixed by current encoding version as expected by protocol
    [[SUPPORTED_ENCODING_VERSION].to_vec(), encoded_data]
        .concat()
        .to_vec()
}

/// Builds a Solidity function call to `executeBatchesSharedBridge` as expected by `IExecutor.sol`.
pub fn execute_batches_shared_bridge_call(
    l2_chain_id: L2ChainId,
    batch: &L1BatchWithMetadata,
    l1_tx_merkle_tree: &MiniMerkleTree<L1Tx>,
) -> impl SolCall {
    IExecutor::executeBatchesSharedBridgeCall::new((
        alloy::primitives::U256::from(l2_chain_id.as_u64()),
        alloy::primitives::U256::from(batch.header.number.0),
        alloy::primitives::U256::from(batch.header.number.0),
        execute_calldata(batch, l1_tx_merkle_tree).into(),
    ))
}

/// `executeBatchesSharedBridge` expects the rest of calldata to be of very specific form. This
/// function makes sure batch and its priority operations are encoded correctly (assumes post gateway).
fn execute_calldata(
    batch: &L1BatchWithMetadata,
    l1_tx_merkle_tree: &MiniMerkleTree<L1Tx>,
) -> Vec<u8> {
    let count = batch.header.l1_tx_count as usize;
    let priority_ops_proofs = if count > 0 {
        let (_, left, right) = l1_tx_merkle_tree.merkle_root_and_paths_for_range(..count);
        let hashes = l1_tx_merkle_tree.hashes_prefix(count);
        vec![PriorityOpsBatchInfo {
            leftPath: left
                .into_iter()
                .map(Option::unwrap_or_default)
                .map(|hash| TxHash::from(hash.0))
                .collect(),
            rightPath: right
                .into_iter()
                .map(Option::unwrap_or_default)
                .map(|hash| TxHash::from(hash.0))
                .collect(),
            itemHashes: hashes
                .into_iter()
                .map(|hash| TxHash::from(hash.0))
                .collect(),
        }]
    } else {
        vec![PriorityOpsBatchInfo {
            leftPath: vec![],
            rightPath: vec![],
            itemHashes: vec![],
        }]
    };
    let batches_arg = vec![IExecutor::StoredBatchInfo::from(batch)];
    let encoded_data = (batches_arg, priority_ops_proofs).abi_encode_params();

    // Prefixed by current encoding version as expected by protocol
    [[SUPPORTED_ENCODING_VERSION].to_vec(), encoded_data]
        .concat()
        .to_vec()
}
