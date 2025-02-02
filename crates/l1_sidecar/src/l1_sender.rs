use crate::zkstack_config::ZkstackConfig;
use alloy::consensus::{SidecarBuilder, SimpleCoder};
use alloy::network::{ReceiptResponse, TransactionBuilder, TransactionBuilder4844};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use alloy::sol_types::{SolCall, SolValue};
use tokio::sync::{mpsc, oneshot};
use zksync_types::commitment::L1BatchWithMetadata;
use zksync_types::{Address, L2ChainId, H256};

/// Current commitment encoding version as per protocol.
pub const SUPPORTED_ENCODING_VERSION: u8 = 0;

alloy::sol!(
    "../../contracts/l1-contracts/contracts/state-transition/chain-interfaces/IExecutor.sol"
);

/// Node component responsible for sending transactions to L1.
pub struct L1Sender {
    provider: Box<dyn Provider>,
    l2_chain_id: L2ChainId,
    validator_timelock_addr: Address,
    command_receiver: mpsc::Receiver<Command>,
    last_committed_l1_batch: L1BatchWithMetadata,
}

impl L1Sender {
    /// Initializes a new [`L1Sender`] that will send transaction using supplied provider. Assumes
    /// that zkstack config matches L1 configuration at the other end of provider.
    ///
    /// Resulting [`L1Sender`] is expected to be consumed by calling [`Self::run`]. Additionally,
    /// returns a cloneable handle that can be used to send requests to this instance of [`L1Sender`].
    pub fn new(
        zkstack_config: &ZkstackConfig,
        genesis_metadata: L1BatchWithMetadata,
        provider: Box<dyn Provider>,
    ) -> (Self, L1SenderHandle) {
        let (command_sender, command_receiver) = mpsc::channel(128);
        let this = Self {
            provider,
            l2_chain_id: zkstack_config.genesis.l2_chain_id,
            validator_timelock_addr: zkstack_config.contracts.l1.validator_timelock_addr,
            command_receiver,
            last_committed_l1_batch: genesis_metadata,
        };
        let handle = L1SenderHandle { command_sender };
        (this, handle)
    }

    /// Runs L1 sender indefinitely thus processing requests received from any of the matching
    /// handles.
    pub async fn run(mut self) -> anyhow::Result<()> {
        while let Some(command) = self.command_receiver.recv().await {
            match command {
                Command::Commit(batch, reply) => self.commit(batch, reply).await,
            }
        }

        tracing::trace!("channel has been closed; stopping L1 sender");
        Ok(())
    }
}

impl L1Sender {
    async fn commit(
        &mut self,
        batch: L1BatchWithMetadata,
        reply: oneshot::Sender<anyhow::Result<H256>>,
    ) {
        let result = self.commit_fallible(&batch).await;
        if result.is_ok() {
            // Commitment was successful, update last committed batch
            self.last_committed_l1_batch = batch;
        }

        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Err(result) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
            result
        } else {
            return;
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to commit batch: {:#?}", err);
        }
    }

    async fn commit_fallible(&self, batch: &L1BatchWithMetadata) -> anyhow::Result<H256> {
        // Create a blob sidecar with empty data
        let sidecar = SidecarBuilder::<SimpleCoder>::from_slice(&[]).build()?;

        let call = IExecutor::commitBatchesSharedBridgeCall::new((
            alloy::primitives::U256::from(self.l2_chain_id.as_u64()),
            alloy::primitives::U256::from(self.last_committed_l1_batch.header.number.0 + 1),
            alloy::primitives::U256::from(self.last_committed_l1_batch.header.number.0 + 1),
            self.commit_calldata(batch).into(),
        ));

        let gas_price = self.provider.get_gas_price().await?;
        let eip1559_est = self.provider.estimate_eip1559_fees(None).await?;
        let tx = TransactionRequest::default()
            .with_to(self.validator_timelock_addr.0.into())
            .with_max_fee_per_blob_gas(gas_price)
            .with_max_fee_per_gas(eip1559_est.max_fee_per_gas)
            .with_max_priority_fee_per_gas(eip1559_est.max_priority_fee_per_gas)
            // Default value for `max_aggregated_tx_gas` from zksync-era, should always be enough
            .with_gas_limit(15000000)
            .with_call(&call)
            .with_blob_sidecar(sidecar);

        let pending_tx = self.provider.send_transaction(tx).await?;
        tracing::debug!(
            batch = batch.header.number.0,
            pending_tx_hash = ?pending_tx.tx_hash(),
            "batch commit transaction sent to L1"
        );

        let receipt = pending_tx.get_receipt().await?;
        if receipt.status() {
            // We could also look at tx receipt's logs for a corresponding `BlockCommit` event but
            // the existing logic is likely good enough for a test node.
            tracing::info!(
                batch = batch.header.number.0,
                tx_hash = ?receipt.transaction_hash,
                block_number = receipt.block_number.unwrap(),
                "batch committed to L1",
            );
        } else {
            tracing::error!(
                batch = batch.header.number.0,
                tx_hash = ?receipt.transaction_hash,
                block_number = receipt.block_number.unwrap(),
                "commit transaction failed"
            );
            anyhow::bail!(
                "commit transaction failed, see L1 transaction's trace for more details (tx_hash='{:?}')",
                receipt.transaction_hash
            );
        }

        Ok(receipt.transaction_hash().0.into())
    }

    /// ABI encode new batch into calldata as expected by `IExecutor::commitBatchesSharedBridgeCall`.
    fn commit_calldata(&self, batch: &L1BatchWithMetadata) -> Vec<u8> {
        let stored_batch_info = IExecutor::StoredBatchInfo::from((
            self.last_committed_l1_batch.header.number.0 as u64,
            alloy::primitives::FixedBytes::<32>::from(
                &self.last_committed_l1_batch.metadata.root_hash.0,
            ),
            self.last_committed_l1_batch.metadata.rollup_last_leaf_index,
            alloy::primitives::U256::from(self.last_committed_l1_batch.header.l1_tx_count),
            alloy::primitives::FixedBytes::<32>::from(
                self.last_committed_l1_batch
                    .header
                    .priority_ops_onchain_data_hash()
                    .0,
            ),
            alloy::primitives::FixedBytes::<32>::from(
                self.last_committed_l1_batch.metadata.l2_l1_merkle_root.0,
            ),
            alloy::primitives::U256::from(self.last_committed_l1_batch.header.timestamp),
            alloy::primitives::FixedBytes::<32>::from(
                self.last_committed_l1_batch.metadata.commitment.0,
            ),
        ));
        let commit_batch_info = IExecutor::CommitBatchInfo::from((
            batch.header.number.0 as u64,
            batch.header.timestamp,
            batch.metadata.rollup_last_leaf_index,
            alloy::primitives::FixedBytes::<32>::from(batch.metadata.root_hash.0),
            alloy::primitives::U256::from(batch.header.l1_tx_count),
            alloy::primitives::FixedBytes::<32>::from(
                batch.header.priority_ops_onchain_data_hash().0,
            ),
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
}

/// A cheap cloneable handle to a [`L1Sender`] instance that can send requests and await for them to
/// be processed.
#[derive(Clone, Debug)]
pub struct L1SenderHandle {
    command_sender: mpsc::Sender<Command>,
}

impl L1SenderHandle {
    /// Request [`L1Sender`] to commit provided batch. Waits until an L1 transaction commiting the
    /// batch is submitted to L1 and returns its hash.
    pub async fn commit_sync(&self, batch: L1BatchWithMetadata) -> anyhow::Result<H256> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::Commit(batch, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to commit a batch as L1 sender is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to commit a batch as L1 sender is dropped"),
        }
    }
}

#[derive(Debug)]
enum Command {
    Commit(L1BatchWithMetadata, oneshot::Sender<anyhow::Result<H256>>),
}
