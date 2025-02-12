use crate::zkstack_config::ZkstackConfig;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use zksync_contracts::BaseSystemContractsHashes;
use zksync_types::blob::num_blobs_required;
use zksync_types::block::L1BatchHeader;
use zksync_types::commitment::{
    AuxCommitments, CommitmentCommonInput, CommitmentInput, L1BatchCommitment, L1BatchMetadata,
    L1BatchWithMetadata,
};
use zksync_types::fee_model::BatchFeeInput;
use zksync_types::{Address, Bloom, H256};
use zksync_types::{L1BatchNumber, ProtocolVersionId};

/// Node component that can generate batch's metadata (with commitment) on demand.
#[derive(Debug, Clone)]
pub struct CommitmentGenerator {
    /// System contracts hashes expected by L1. Might be different from the actual contract hashes used by anvil-zksync.
    base_system_contracts_hashes: BaseSystemContractsHashes,
    /// Fee address expected by L1.
    fee_address: Address,
    /// Batches with already known metadata.
    batches: Arc<RwLock<HashMap<L1BatchNumber, L1BatchWithMetadata>>>,
}

impl CommitmentGenerator {
    /// Initializes a new [`CommitmentGenerator`] matching L1 from the provided zkstack config.
    ///
    /// Additionally, returns genesis' metadata.
    pub fn new(zkstack_config: &ZkstackConfig) -> (Self, L1BatchWithMetadata) {
        assert_eq!(
            zkstack_config.genesis.genesis_protocol_version,
            ProtocolVersionId::latest(),
            "L1 setup is not using the latest protocol version. \
            Likely protocol version was upgraded in anvil-zksync but not in L1 setup. \
            To re-generate the setup please follow instructions in `./l1-setup/README.md`."
        );
        // L1 expects a specific hash for bootloader/AA contracts which might be different from what
        // anvil-zksync is actually using (e.g. it is running with a user-provided bootloader). Thus,
        // we use hash from zkstack genesis config, i.e. what L1 was initialized with.
        let base_system_contracts_hashes = BaseSystemContractsHashes {
            bootloader: zkstack_config.genesis.bootloader_hash,
            default_aa: zkstack_config.genesis.default_aa_hash,
            evm_emulator: None,
        };
        let this = Self {
            base_system_contracts_hashes,
            fee_address: zkstack_config.genesis.fee_account,
            batches: Arc::new(RwLock::new(HashMap::new())),
        };

        // Run realistic genesis commitment computation that should match value from zkstack config
        let commitment_input = CommitmentInput::for_genesis_batch(
            zkstack_config.genesis.genesis_root,
            zkstack_config.genesis.genesis_rollup_leaf_index,
            base_system_contracts_hashes,
            ProtocolVersionId::latest(),
        );
        let genesis_metadata = this.generate_metadata_inner(L1BatchNumber(0), commitment_input);
        assert_eq!(
            zkstack_config.genesis.genesis_batch_commitment, genesis_metadata.metadata.commitment,
            "Computed genesis batch commitment does not match zkstack config"
        );
        this.batches
            .write()
            .unwrap()
            .insert(L1BatchNumber(0), genesis_metadata.clone());

        (this, genesis_metadata)
    }

    /// Retrieve batch's existing metadata or generate it if there is none.
    pub fn get_metadata(&self, batch_number: L1BatchNumber) -> L1BatchWithMetadata {
        let mut writer = self.batches.write().unwrap();
        writer
            .entry(batch_number)
            .or_insert_with(|| self.generate_metadata(batch_number))
            .clone()
    }

    fn generate_metadata(&self, batch_number: L1BatchNumber) -> L1BatchWithMetadata {
        // anvil-zksync does not store batches right now so we just generate dummy commitment input.
        // Root hash is random purely so that different batches have different commitments.
        let root_hash = H256::random();
        let rollup_last_leaf_index = 42;
        let common = CommitmentCommonInput {
            l2_to_l1_logs: Vec::new(),
            rollup_last_leaf_index,
            rollup_root_hash: root_hash,
            bootloader_code_hash: self.base_system_contracts_hashes.bootloader,
            default_aa_code_hash: self.base_system_contracts_hashes.default_aa,
            evm_emulator_code_hash: None,
            protocol_version: ProtocolVersionId::latest(),
        };
        let commitment_input = CommitmentInput::PostBoojum {
            common,
            system_logs: Vec::new(),
            state_diffs: Vec::new(),
            aux_commitments: AuxCommitments {
                events_queue_commitment: H256::zero(),
                bootloader_initial_content_commitment: H256::zero(),
            },
            blob_hashes: {
                let num_blobs = num_blobs_required(&ProtocolVersionId::latest());
                vec![Default::default(); num_blobs]
            },
            aggregation_root: H256::zero(),
        };

        self.generate_metadata_inner(batch_number, commitment_input)
    }

    fn generate_metadata_inner(
        &self,
        batch_number: L1BatchNumber,
        commitment_input: CommitmentInput,
    ) -> L1BatchWithMetadata {
        let root_hash = commitment_input.common().rollup_root_hash;
        let rollup_last_leaf_index = commitment_input.common().rollup_last_leaf_index;
        let l2_to_l1_logs = commitment_input.common().l2_to_l1_logs.clone();

        let commitment = L1BatchCommitment::new(commitment_input);
        let mut commitment_artifacts = commitment.artifacts();
        if batch_number == L1BatchNumber(0) {
            // `l2_l1_merkle_root` for genesis batch is set to 0 on L1 contract, same must be here.
            commitment_artifacts.l2_l1_merkle_root = H256::zero();
        } else {
            // We don't send system logs containing the actual `l2_l1_merkle_root` so Executor contract
            // is assuming `l2_l1_merkle_root` zeroed out hash for all batches by default. In reality
            // even an empty Merkle tree has a non-trivial hash.
            commitment_artifacts.l2_l1_merkle_root = H256::zero();
        }
        tracing::debug!(
            batch = batch_number.0,
            commitment_hash = ?commitment_artifacts.commitment_hash.commitment,
            "generated a new batch commitment",
        );

        // Our version of Executor contract has system log and data availability verification disabled.
        // Hence, we are free to create a zeroed out dummy header without any logs whatsoever.
        let batch_header = L1BatchHeader {
            number: batch_number,
            timestamp: 0,
            l1_tx_count: 0,
            l2_tx_count: 0,
            priority_ops_onchain_data: vec![],
            l2_to_l1_logs,
            l2_to_l1_messages: vec![],
            bloom: Bloom::zero(),
            used_contract_hashes: vec![],
            base_system_contracts_hashes: self.base_system_contracts_hashes,
            system_logs: vec![],
            protocol_version: Some(ProtocolVersionId::latest()),
            pubdata_input: None,
            fee_address: self.fee_address,
            batch_fee_input: BatchFeeInput::pubdata_independent(0, 0, 0),
        };
        // Unlike above, this is a realistic metadata calculation based on code from zksync-era.
        let batch_metadata = L1BatchMetadata {
            root_hash,
            rollup_last_leaf_index,
            initial_writes_compressed: commitment_artifacts.compressed_initial_writes,
            repeated_writes_compressed: commitment_artifacts.compressed_repeated_writes,
            commitment: commitment_artifacts.commitment_hash.commitment,
            l2_l1_merkle_root: commitment_artifacts.l2_l1_merkle_root,
            block_meta_params: commitment.meta_parameters(),
            aux_data_hash: commitment_artifacts.commitment_hash.aux_output,
            meta_parameters_hash: commitment_artifacts.commitment_hash.meta_parameters,
            pass_through_data_hash: commitment_artifacts.commitment_hash.pass_through_data,
            events_queue_commitment: commitment_artifacts
                .aux_commitments
                .map(|a| a.events_queue_commitment),
            bootloader_initial_content_commitment: commitment_artifacts
                .aux_commitments
                .map(|a| a.bootloader_initial_content_commitment),
            state_diffs_compressed: commitment_artifacts
                .compressed_state_diffs
                .unwrap_or_default(),
            state_diff_hash: Some(commitment_artifacts.state_diff_hash),
            local_root: Some(commitment_artifacts.local_root),
            aggregation_root: Some(commitment_artifacts.aggregation_root),
            // anvil-zksync can only be run in rollup mode which does not have DA inclusion
            da_inclusion_data: None,
        };

        // Pretend there were no used factory deps and no published bytecode.
        L1BatchWithMetadata::new(batch_header, batch_metadata, HashMap::new(), &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_proper_genesis() {
        let config = ZkstackConfig::builtin();
        let (_, genesis_metadata) = CommitmentGenerator::new(&config);
        // Basic invariants expected by the protocol
        assert_eq!(genesis_metadata.header.number, L1BatchNumber(0));
        assert_eq!(genesis_metadata.header.timestamp, 0);
        assert_eq!(genesis_metadata.header.l1_tx_count, 0);
        assert_eq!(genesis_metadata.header.l2_tx_count, 0);

        // Computed genesis should match provided config
        assert_eq!(
            genesis_metadata.metadata.root_hash,
            config.genesis.genesis_root
        );
        assert_eq!(
            genesis_metadata.metadata.rollup_last_leaf_index,
            config.genesis.genesis_rollup_leaf_index
        );
        assert_eq!(
            genesis_metadata.metadata.commitment,
            config.genesis.genesis_batch_commitment
        );
    }

    #[test]
    fn generates_valid_commitment_for_random_batch() {
        let config = ZkstackConfig::builtin();
        let (commitment_generator, _) = CommitmentGenerator::new(&config);
        let metadata = commitment_generator.generate_metadata(L1BatchNumber(42));

        // Really this is all we can check without making assumptions about the implementation
        assert_eq!(metadata.header.number, L1BatchNumber(42));
    }
}
