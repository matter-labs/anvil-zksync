use crate::zkstack_config::ZkstackConfig;
use anvil_zksync_core::node::blockchain::ReadBlockchain;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use zksync_contracts::BaseSystemContractsHashes;
use zksync_types::blob::num_blobs_required;
use zksync_types::block::{L1BatchHeader, L1BatchTreeData};
use zksync_types::commitment::{
    AuxCommitments, CommitmentCommonInput, CommitmentInput, L1BatchCommitment, L1BatchMetadata,
    L1BatchWithMetadata,
};
use zksync_types::{Address, H256};
use zksync_types::{L1BatchNumber, ProtocolVersionId};

/// Node component that can generate batch's metadata (with commitment) on demand.
#[derive(Debug, Clone)]
pub struct CommitmentGenerator {
    /// System contracts hashes expected by L1. Might be different from the actual contract hashes used by anvil-zksync.
    base_system_contracts_hashes: BaseSystemContractsHashes,
    /// Fee address expected by L1.
    fee_address: Address,
    blockchain: Box<dyn ReadBlockchain>,
    /// Batches with already known metadata.
    batches: Arc<RwLock<HashMap<L1BatchNumber, L1BatchWithMetadata>>>,
}

impl CommitmentGenerator {
    /// Initializes a new [`CommitmentGenerator`] matching L1 from the provided zkstack config.
    ///
    /// Additionally, returns genesis' metadata.
    pub fn new(zkstack_config: &ZkstackConfig, blockchain: Box<dyn ReadBlockchain>) -> Self {
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

        // Run realistic genesis commitment computation that should match value from zkstack config
        let mut genesis_batch_header = L1BatchHeader::new(
            L1BatchNumber(0),
            0,
            base_system_contracts_hashes,
            ProtocolVersionId::latest(),
        );
        genesis_batch_header.fee_address = zkstack_config.genesis.fee_account;
        let commitment_input = CommitmentInput::for_genesis_batch(
            zkstack_config.genesis.genesis_root,
            zkstack_config.genesis.genesis_rollup_leaf_index,
            base_system_contracts_hashes,
            ProtocolVersionId::latest(),
        );
        let genesis_metadata =
            Self::generate_metadata_inner(genesis_batch_header, commitment_input);
        assert_eq!(
            zkstack_config.genesis.genesis_batch_commitment, genesis_metadata.metadata.commitment,
            "Computed genesis batch commitment does not match zkstack config"
        );

        Self {
            base_system_contracts_hashes,
            fee_address: zkstack_config.genesis.fee_account,
            blockchain,
            batches: Arc::new(RwLock::new(HashMap::from_iter([(
                L1BatchNumber(0),
                genesis_metadata,
            )]))),
        }
    }

    /// Retrieve batch's existing metadata or generate it if there is none. Returns `None` if batch
    /// with this number does not exist.
    pub async fn get_or_generate_metadata(
        &self,
        batch_number: L1BatchNumber,
    ) -> Option<L1BatchWithMetadata> {
        if let Some(metadata) = self.batches.read().unwrap().get(&batch_number) {
            return Some(metadata.clone());
        }

        // Fetch batch header from storage and patch its fee_address/base_system_contract_hashes as
        // those might be different from what L1 expects (e.g. impersonated execution, custom
        // user-supplied contracts etc).
        let mut header = self.blockchain.get_batch_header(batch_number).await?;
        header.fee_address = self.fee_address;
        header.base_system_contracts_hashes = self.base_system_contracts_hashes;

        // anvil-zksync does not store batches right now so we just generate dummy commitment input.
        // Root hash is random purely so that different batches have different commitments.
        let tree_data = L1BatchTreeData {
            hash: H256::random(),
            rollup_last_leaf_index: 42,
        };
        let metadata = self.generate_metadata(header, tree_data);
        self.batches
            .write()
            .unwrap()
            .insert(batch_number, metadata.clone());
        Some(metadata)
    }

    fn generate_metadata(
        &self,
        header: L1BatchHeader,
        tree_data: L1BatchTreeData,
    ) -> L1BatchWithMetadata {
        let common = CommitmentCommonInput {
            l2_to_l1_logs: header.l2_to_l1_logs.clone(),
            rollup_last_leaf_index: tree_data.rollup_last_leaf_index,
            rollup_root_hash: tree_data.hash,
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

        Self::generate_metadata_inner(header, commitment_input)
    }

    fn generate_metadata_inner(
        header: L1BatchHeader,
        commitment_input: CommitmentInput,
    ) -> L1BatchWithMetadata {
        let root_hash = commitment_input.common().rollup_root_hash;
        let rollup_last_leaf_index = commitment_input.common().rollup_last_leaf_index;

        let commitment = L1BatchCommitment::new(commitment_input);
        let mut commitment_artifacts = commitment.artifacts();
        if header.number == L1BatchNumber(0) {
            // `l2_l1_merkle_root` for genesis batch is set to 0 on L1 contract, same must be here.
            commitment_artifacts.l2_l1_merkle_root = H256::zero();
        } else {
            // We don't send system logs containing the actual `l2_l1_merkle_root` so Executor contract
            // is assuming `l2_l1_merkle_root` zeroed out hash for all batches by default. In reality
            // even an empty Merkle tree has a non-trivial hash.
            commitment_artifacts.l2_l1_merkle_root = H256::zero();
        }
        tracing::debug!(
            batch = header.number.0,
            commitment_hash = ?commitment_artifacts.commitment_hash.commitment,
            "generated a new batch commitment",
        );
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
        L1BatchWithMetadata::new(header, batch_metadata, HashMap::new(), &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_zksync_core::filters::LogFilter;
    use async_trait::async_trait;
    use zksync_types::api::{
        Block, BlockDetails, BlockId, DebugCall, Log, Transaction, TransactionDetails,
        TransactionReceipt, TransactionVariant,
    };
    use zksync_types::L2BlockNumber;

    // TODO: Consider moving to a separate testing crate
    #[derive(Clone, Debug)]
    struct MockBlockchain(HashMap<L1BatchNumber, L1BatchHeader>);

    impl MockBlockchain {
        pub fn new(batches: impl IntoIterator<Item = L1BatchHeader>) -> Self {
            let genesis = L1BatchHeader::new(
                L1BatchNumber(0),
                0,
                BaseSystemContractsHashes::default(),
                ProtocolVersionId::latest(),
            );
            Self(HashMap::from_iter(
                batches
                    .into_iter()
                    .map(|h| (h.number, h))
                    .chain([(L1BatchNumber(0), genesis)]),
            ))
        }
    }

    #[async_trait]
    impl ReadBlockchain for MockBlockchain {
        fn dyn_cloned(&self) -> Box<dyn ReadBlockchain> {
            unimplemented!()
        }

        async fn current_batch(&self) -> L1BatchNumber {
            unimplemented!()
        }

        async fn current_block_number(&self) -> L2BlockNumber {
            unimplemented!()
        }

        async fn current_block_hash(&self) -> H256 {
            unimplemented!()
        }

        async fn get_block_by_hash(&self, _hash: &H256) -> Option<Block<TransactionVariant>> {
            unimplemented!()
        }

        async fn get_block_by_number(
            &self,
            _number: L2BlockNumber,
        ) -> Option<Block<TransactionVariant>> {
            unimplemented!()
        }

        async fn get_block_by_id(&self, _block_id: BlockId) -> Option<Block<TransactionVariant>> {
            unimplemented!()
        }

        async fn get_block_hash_by_number(&self, _number: L2BlockNumber) -> Option<H256> {
            unimplemented!()
        }

        async fn get_block_hash_by_id(&self, _block_id: BlockId) -> Option<H256> {
            unimplemented!()
        }

        async fn get_block_number_by_hash(&self, _hash: &H256) -> Option<L2BlockNumber> {
            unimplemented!()
        }

        async fn get_block_number_by_id(&self, _block_id: BlockId) -> Option<L2BlockNumber> {
            unimplemented!()
        }

        async fn get_block_tx_hashes_by_number(&self, _number: L2BlockNumber) -> Option<Vec<H256>> {
            unimplemented!()
        }

        async fn get_block_tx_hashes_by_id(&self, _block_id: BlockId) -> Option<Vec<H256>> {
            unimplemented!()
        }

        async fn get_block_tx_by_id(
            &self,
            _block_id: BlockId,
            _index: usize,
        ) -> Option<Transaction> {
            unimplemented!()
        }

        async fn get_block_tx_count_by_id(&self, _block_id: BlockId) -> Option<usize> {
            unimplemented!()
        }

        async fn get_block_details_by_number(
            &self,
            _number: L2BlockNumber,
            _l2_fair_gas_price: u64,
            _fair_pubdata_price: Option<u64>,
            _base_system_contracts_hashes: BaseSystemContractsHashes,
        ) -> Option<BlockDetails> {
            unimplemented!()
        }

        async fn get_tx_receipt(&self, _tx_hash: &H256) -> Option<TransactionReceipt> {
            unimplemented!()
        }

        async fn get_tx_debug_info(&self, _tx_hash: &H256, _only_top: bool) -> Option<DebugCall> {
            unimplemented!()
        }

        async fn get_tx_api(&self, _tx_hash: &H256) -> anyhow::Result<Option<Transaction>> {
            unimplemented!()
        }

        async fn get_detailed_tx(
            &self,
            _tx: Transaction,
        ) -> Option<anvil_zksync_types::api::DetailedTransaction> {
            unimplemented!()
        }

        async fn get_tx_details(&self, _tx_hash: &H256) -> Option<TransactionDetails> {
            unimplemented!()
        }

        async fn get_zksync_tx(&self, _tx_hash: &H256) -> Option<zksync_types::Transaction> {
            unimplemented!()
        }

        async fn get_filter_logs(&self, _log_filter: &LogFilter) -> Vec<Log> {
            unimplemented!()
        }

        async fn get_batch_header(&self, batch_number: L1BatchNumber) -> Option<L1BatchHeader> {
            self.0.get(&batch_number).cloned()
        }
    }

    #[tokio::test]
    async fn generates_proper_genesis() {
        let config = ZkstackConfig::builtin();
        let blockchain = MockBlockchain::new([]);
        let commitment_generator = CommitmentGenerator::new(&config, Box::new(blockchain));
        let genesis_metadata = commitment_generator
            .get_or_generate_metadata(L1BatchNumber(0))
            .await
            .unwrap();
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

    #[tokio::test]
    async fn returns_none_for_unknown_batch() {
        let config = ZkstackConfig::builtin();
        let blockchain = MockBlockchain::new([]);
        let commitment_generator = CommitmentGenerator::new(&config, Box::new(blockchain));
        let metadata = commitment_generator
            .get_or_generate_metadata(L1BatchNumber(42))
            .await;

        assert_eq!(metadata, None);
    }

    #[tokio::test]
    async fn generates_valid_commitment_for_random_batch() {
        let config = ZkstackConfig::builtin();
        let batch_42_header = L1BatchHeader::new(
            L1BatchNumber(42),
            1042,
            BaseSystemContractsHashes::default(),
            ProtocolVersionId::latest(),
        );
        let blockchain = MockBlockchain::new([batch_42_header.clone()]);
        let commitment_generator = CommitmentGenerator::new(&config, Box::new(blockchain));
        let metadata = commitment_generator
            .get_or_generate_metadata(L1BatchNumber(42))
            .await
            .unwrap();

        // Really this is all we can check without making assumptions about the implementation
        assert_eq!(metadata.header.number, batch_42_header.number);
        assert_eq!(metadata.header.timestamp, batch_42_header.timestamp);
    }
}
