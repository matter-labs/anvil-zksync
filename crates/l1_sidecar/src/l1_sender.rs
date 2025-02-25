use crate::contracts;
use crate::zkstack_config::ZkstackConfig;
use alloy::consensus::{SidecarBuilder, SimpleCoder};
use alloy::network::{ReceiptResponse, TransactionBuilder, TransactionBuilder4844};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use tokio::sync::{mpsc, oneshot};
use zksync_types::commitment::L1BatchWithMetadata;
use zksync_types::{Address, L2ChainId, H256};

/// Node component responsible for sending transactions to L1.
pub struct L1Sender {
    provider: Box<dyn Provider>,
    l2_chain_id: L2ChainId,
    validator_timelock_addr: Address,
    command_receiver: mpsc::Receiver<Command>,
    last_committed_l1_batch: L1BatchWithMetadata,
    last_proved_l1_batch: L1BatchWithMetadata,
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
            last_committed_l1_batch: genesis_metadata.clone(),
            last_proved_l1_batch: genesis_metadata,
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
                Command::Prove(batch, reply) => self.prove(batch, reply).await,
                Command::Execute(batch, reply) => self.execute(batch, reply).await,
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

        let call = contracts::commit_batches_shared_bridge_call(
            self.l2_chain_id,
            &self.last_committed_l1_batch,
            batch,
        );

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

    async fn prove(
        &mut self,
        batch: L1BatchWithMetadata,
        reply: oneshot::Sender<anyhow::Result<H256>>,
    ) {
        let result = self.prove_fallible(&batch).await;
        if result.is_ok() {
            // Proving was successful, update last proved batch
            self.last_proved_l1_batch = batch;
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
            tracing::error!("failed to prove batch: {:#?}", err);
        }
    }

    async fn prove_fallible(&self, batch: &L1BatchWithMetadata) -> anyhow::Result<H256> {
        // Create a blob sidecar with empty data
        let sidecar = SidecarBuilder::<SimpleCoder>::from_slice(&[]).build()?;

        let call = contracts::prove_batches_shared_bridge_call(
            self.l2_chain_id,
            &self.last_proved_l1_batch,
            batch,
        );

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
            "batch prove transaction sent to L1"
        );

        let receipt = pending_tx.get_receipt().await?;
        if receipt.status() {
            // We could also look at tx receipt's logs for a corresponding `BlocksVerification` event but
            // the existing logic is likely good enough for a test node.
            tracing::info!(
                batch = batch.header.number.0,
                tx_hash = ?receipt.transaction_hash,
                block_number = receipt.block_number.unwrap(),
                "batch proved on L1",
            );
        } else {
            tracing::error!(
                batch = batch.header.number.0,
                tx_hash = ?receipt.transaction_hash,
                block_number = receipt.block_number.unwrap(),
                "prove transaction failed"
            );
            anyhow::bail!(
                "prove transaction failed, see L1 transaction's trace for more details (tx_hash='{:?}')",
                receipt.transaction_hash
            );
        }

        Ok(receipt.transaction_hash().0.into())
    }

    async fn execute(
        &mut self,
        batch: L1BatchWithMetadata,
        reply: oneshot::Sender<anyhow::Result<H256>>,
    ) {
        let result = self.execute_fallible(&batch).await;

        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Err(result) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
            result
        } else {
            return;
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to execute batch: {:#?}", err);
        }
    }

    async fn execute_fallible(&self, batch: &L1BatchWithMetadata) -> anyhow::Result<H256> {
        // Create a blob sidecar with empty data
        let sidecar = SidecarBuilder::<SimpleCoder>::from_slice(&[]).build()?;

        let call = contracts::execute_batches_shared_bridge_call(self.l2_chain_id, batch);

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
            "batch execute transaction sent to L1"
        );

        let receipt = pending_tx.get_receipt().await?;
        if receipt.status() {
            // We could also look at tx receipt's logs for a corresponding `BlocksVerification` event but
            // the existing logic is likely good enough for a test node.
            tracing::info!(
                batch = batch.header.number.0,
                tx_hash = ?receipt.transaction_hash,
                block_number = receipt.block_number.unwrap(),
                "batch executed on L1",
            );
        } else {
            tracing::error!(
                batch = batch.header.number.0,
                tx_hash = ?receipt.transaction_hash,
                block_number = receipt.block_number.unwrap(),
                "execute transaction failed"
            );
            anyhow::bail!(
                "execute transaction failed, see L1 transaction's trace for more details (tx_hash='{:?}')",
                receipt.transaction_hash
            );
        }

        Ok(receipt.transaction_hash().0.into())
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

    /// Request [`L1Sender`] to prove provided batch. Waits until an L1 transaction proving the
    /// batch is submitted to L1 and returns its hash.
    pub async fn prove_sync(&self, batch: L1BatchWithMetadata) -> anyhow::Result<H256> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::Prove(batch, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to prove a batch as L1 sender is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to prove a batch as L1 sender is dropped"),
        }
    }

    /// Request [`L1Sender`] to execute provided batch. Waits until an L1 transaction executing the
    /// batch is submitted to L1 and returns its hash.
    pub async fn execute_sync(&self, batch: L1BatchWithMetadata) -> anyhow::Result<H256> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::Execute(batch, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to execute a batch as L1 sender is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to execute a batch as L1 sender is dropped"),
        }
    }
}

#[derive(Debug)]
enum Command {
    Commit(L1BatchWithMetadata, oneshot::Sender<anyhow::Result<H256>>),
    Prove(L1BatchWithMetadata, oneshot::Sender<anyhow::Result<H256>>),
    Execute(L1BatchWithMetadata, oneshot::Sender<anyhow::Result<H256>>),
}
