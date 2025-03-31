use super::InMemoryNodeInner;
use crate::node::fork::ForkConfig;
use crate::node::inner::fork::{ForkClient, ForkSource};
use crate::node::inner::vm_runner::VmRunner;
use crate::node::keys::StorageKeyLayout;
use crate::node::pool::TxBatch;
use indicatif::ProgressBar;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use url::Url;
use zksync_types::bytecode::BytecodeHash;
use zksync_types::utils::nonces_to_full_nonce;
use zksync_types::{get_code_key, u256_to_h256, Address, L2BlockNumber, StorageKey, U256};

pub struct NodeExecutor {
    node_inner: Arc<RwLock<InMemoryNodeInner>>,
    vm_runner: VmRunner,
    command_receiver: mpsc::Receiver<Command>,
    storage_key_layout: StorageKeyLayout,
}

impl NodeExecutor {
    pub fn new(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        vm_runner: VmRunner,
        storage_key_layout: StorageKeyLayout,
    ) -> (Self, NodeExecutorHandle) {
        let (command_sender, command_receiver) = mpsc::channel(128);
        let this = Self {
            node_inner,
            vm_runner,
            command_receiver,
            storage_key_layout,
        };
        let handle = NodeExecutorHandle { command_sender };
        (this, handle)
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        while let Some(command) = self.command_receiver.recv().await {
            match command {
                Command::SealBlock(tx_batch, reply) => {
                    self.seal_block(tx_batch, reply).await;
                }
                Command::SealBlocks(tx_batches, interval, reply) => {
                    self.seal_blocks(tx_batches, interval, reply).await;
                }
                Command::SetCode(address, code, reply) => {
                    self.set_code(address, code, reply).await;
                }
                Command::SetStorage(key, value, reply) => {
                    self.set_storage(key, value, reply).await;
                }
                Command::SetBalance(address, balance, reply) => {
                    self.set_balance(address, balance, reply).await;
                }
                Command::SetNonce(address, nonce, reply) => {
                    self.set_nonce(address, nonce, reply).await;
                }
                Command::ResetFork(url, block_number, reply) => {
                    self.reset_fork(url, block_number, reply).await;
                }
                Command::ResetForkBlockNumber(block_number, reply) => {
                    self.reset_fork_block_number(block_number, reply).await;
                }
                Command::SetForkUrl(url, reply) => {
                    self.set_fork_url(url, reply).await;
                }
                Command::RemoveFork(reply) => {
                    self.remove_fork(reply).await;
                }
                Command::IncreaseTime(delta, reply) => {
                    self.increase_time(delta, reply).await;
                }
                Command::EnforceNextTimestamp(timestamp, reply) => {
                    self.enforce_next_timestamp(timestamp, reply).await;
                }
                Command::SetCurrentTimestamp(timestamp, reply) => {
                    self.set_current_timestamp(timestamp, reply).await;
                }
                Command::SetTimestampInterval(seconds) => {
                    self.set_timestamp_interval(seconds).await;
                }
                Command::RemoveTimestampInterval(reply) => {
                    self.remove_timestamp_interval(reply).await;
                }
                Command::EnforceNextBaseFeePerGas(base_fee, reply) => {
                    self.enforce_next_base_fee_per_gas(base_fee, reply).await;
                }
                Command::SetProgressBar(bar) => {
                    self.vm_runner.set_progress_bar(bar);
                }
            }
        }

        tracing::trace!("channel has been closed; stopping node executor");
        Ok(())
    }
}

impl NodeExecutor {
    async fn seal_block(
        &mut self,
        tx_batch: TxBatch,
        reply: Option<oneshot::Sender<anyhow::Result<L2BlockNumber>>>,
    ) {
        let mut node_inner = self.node_inner.write().await;
        let tx_batch_execution_result = self
            .vm_runner
            .run_tx_batch(tx_batch, &mut node_inner)
            .await
            .unwrap();
        let result = node_inner.seal_block(tx_batch_execution_result).await;
        drop(node_inner);
        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Some(reply) = reply {
            if let Err(result) = reply.send(result) {
                tracing::info!("failed to reply as receiver has been dropped");
                result
            } else {
                return;
            }
        } else {
            result
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to seal a block: {:#?}", err);
        }
    }

    async fn seal_blocks(
        &mut self,
        tx_batches: Vec<TxBatch>,
        interval: u64,
        reply: oneshot::Sender<anyhow::Result<Vec<L2BlockNumber>>>,
    ) {
        let mut node_inner = self.node_inner.write().await;

        // Save old interval to restore later: it might get replaced with `interval` below
        let old_interval = node_inner.time.get_block_timestamp_interval();
        let result = async {
            let mut block_numbers = Vec::with_capacity(tx_batches.len());
            // Processing the entire vector is essentially atomic here because `NodeExecutor` is
            // the only component that seals blocks.
            for (i, tx_batch) in tx_batches.into_iter().enumerate() {
                // Enforce provided interval starting from the second block (i.e. first block should
                // use the existing interval).
                if i == 1 {
                    node_inner.time.set_block_timestamp_interval(Some(interval));
                }
                let tx_batch_execution_result = self
                    .vm_runner
                    .run_tx_batch(tx_batch, &mut node_inner)
                    .await?;
                let number = node_inner.seal_block(tx_batch_execution_result).await?;
                block_numbers.push(number);
            }
            anyhow::Ok(block_numbers)
        }
        .await;
        // Restore old interval
        node_inner.time.set_block_timestamp_interval(old_interval);

        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Err(result) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
            result
        } else {
            return;
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to seal blocks: {:#?}", err);
        }
    }

    async fn set_code(&mut self, address: Address, bytecode: Vec<u8>, reply: oneshot::Sender<()>) {
        let code_key = get_code_key(&address);
        let bytecode_hash = BytecodeHash::for_bytecode(&bytecode).value();
        // TODO: Likely fork_storage can be moved to `NodeExecutor` instead
        let node_inner = self.node_inner.read().await;
        node_inner
            .fork_storage
            .store_factory_dep(bytecode_hash, bytecode);
        node_inner.fork_storage.set_value(code_key, bytecode_hash);
        drop(node_inner);
        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn set_storage(&mut self, key: StorageKey, value: U256, reply: oneshot::Sender<()>) {
        // TODO: Likely fork_storage can be moved to `NodeExecutor` instead
        self.node_inner
            .read()
            .await
            .fork_storage
            .set_value(key, u256_to_h256(value));
        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn set_balance(&mut self, address: Address, balance: U256, reply: oneshot::Sender<()>) {
        let balance_key = self
            .storage_key_layout
            .get_storage_key_for_base_token(&address);
        // TODO: Likely fork_storage can be moved to `NodeExecutor` instead
        self.node_inner
            .read()
            .await
            .fork_storage
            .set_value(balance_key, u256_to_h256(balance));
        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn set_nonce(&mut self, address: Address, nonce: U256, reply: oneshot::Sender<()>) {
        let nonce_key = self.storage_key_layout.get_nonce_key(&address);
        let enforced_full_nonce = nonces_to_full_nonce(nonce, nonce);
        // TODO: Likely fork_storage can be moved to `NodeExecutor` instead
        self.node_inner
            .read()
            .await
            .fork_storage
            .set_value(nonce_key, u256_to_h256(enforced_full_nonce));
        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn reset_fork(
        &mut self,
        url: Url,
        block_number: Option<L2BlockNumber>,
        reply: oneshot::Sender<anyhow::Result<()>>,
    ) {
        let result = async {
            // We don't know what chain this is so we assume default scale configuration.
            let fork_client =
                ForkClient::at_block_number(ForkConfig::unknown(url), block_number).await?;
            self.node_inner.write().await.reset(Some(fork_client)).await;

            anyhow::Ok(())
        }
        .await;

        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Err(result) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
            result
        } else {
            return;
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to reset fork: {:#?}", err);
        }
    }

    async fn reset_fork_block_number(
        &mut self,
        block_number: L2BlockNumber,
        reply: oneshot::Sender<anyhow::Result<()>>,
    ) {
        let result = async {
            let node_inner = self.node_inner.write().await;
            let url = node_inner
                .fork
                .url()
                .ok_or_else(|| anyhow::anyhow!("no existing fork found"))?;
            // Keep scale factors as this is the same chain.
            let details = node_inner
                .fork
                .details()
                .ok_or_else(|| anyhow::anyhow!("no existing fork found"))?;
            let fork_client = ForkClient::at_block_number(
                ForkConfig {
                    url,
                    estimate_gas_price_scale_factor: details.estimate_gas_price_scale_factor,
                    estimate_gas_scale_factor: details.estimate_gas_scale_factor,
                },
                Some(block_number),
            )
            .await?;
            self.node_inner.write().await.reset(Some(fork_client)).await;

            anyhow::Ok(())
        }
        .await;

        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Err(result) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
            result
        } else {
            return;
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to reset fork: {:#?}", err);
        }
    }

    async fn set_fork_url(&mut self, url: Url, reply: oneshot::Sender<Option<Url>>) {
        let node_inner = self.node_inner.write().await;
        let old_url = node_inner.fork.set_fork_url(url);

        // Reply to sender if we can
        if reply.send(old_url).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn remove_fork(&mut self, reply: oneshot::Sender<()>) {
        self.node_inner.write().await.reset(None).await;

        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn increase_time(&mut self, delta: u64, reply: oneshot::Sender<()>) {
        self.node_inner.write().await.time.increase_time(delta);
        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn enforce_next_timestamp(
        &mut self,
        timestamp: u64,
        reply: oneshot::Sender<anyhow::Result<()>>,
    ) {
        let result = self
            .node_inner
            .write()
            .await
            .time
            .enforce_next_timestamp(timestamp);
        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Err(result) = reply.send(result) {
            tracing::info!("failed to reply as receiver has been dropped");
            result
        } else {
            return;
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to enforce next timestamp: {:#?}", err);
        }
    }

    async fn set_current_timestamp(&mut self, timestamp: u64, reply: oneshot::Sender<i128>) {
        let result = self
            .node_inner
            .write()
            .await
            .time
            .set_current_timestamp_unchecked(timestamp);
        // Reply to sender if we can
        if reply.send(result).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn set_timestamp_interval(&mut self, delta: u64) {
        self.node_inner
            .write()
            .await
            .time
            .set_block_timestamp_interval(Some(delta));
    }

    async fn remove_timestamp_interval(&mut self, reply: oneshot::Sender<bool>) {
        let result = self
            .node_inner
            .write()
            .await
            .time
            .remove_block_timestamp_interval();
        // Reply to sender if we can
        if reply.send(result).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }

    async fn enforce_next_base_fee_per_gas(&mut self, base_fee: U256, reply: oneshot::Sender<()>) {
        self.node_inner
            .write()
            .await
            .fee_input_provider
            .set_base_fee(base_fee.as_u64());
        // Reply to sender if we can
        if reply.send(()).is_err() {
            tracing::info!("failed to reply as receiver has been dropped");
        }
    }
}

#[derive(Clone, Debug)]
pub struct NodeExecutorHandle {
    command_sender: mpsc::Sender<Command>,
}

impl NodeExecutorHandle {
    /// Request [`NodeExecutor`] to seal a new block from the provided transaction batch. Does not
    /// wait for the block to actually be produced.
    ///
    /// It is sender's responsibility to make sure [`TxBatch`] is constructed correctly (see its
    /// docs).
    pub async fn seal_block(&self, tx_batch: TxBatch) -> anyhow::Result<()> {
        Ok(self
            .command_sender
            .send(Command::SealBlock(tx_batch, None))
            .await?)
    }

    /// Request [`NodeExecutor`] to seal a new block from the provided transaction batch. Waits for
    /// the block to be produced and returns its number.
    ///
    /// It is sender's responsibility to make sure [`TxBatch`] is constructed correctly (see its
    /// docs).
    pub async fn seal_block_sync(&self, tx_batch: TxBatch) -> anyhow::Result<L2BlockNumber> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SealBlock(tx_batch, Some(response_sender)))
            .await
            .map_err(|_| anyhow::anyhow!("failed to seal a block as node executor is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to seal a block as node executor is dropped"),
        }
    }

    /// Request [`NodeExecutor`] to seal multiple blocks from the provided transaction batches with
    /// `interval` seconds in-between of two consecutive blocks.
    /// Waits for the blocks to be produced and returns their numbers.
    ///
    /// Guarantees that the resulting block numbers will be sequential (i.e. no other blocks can
    /// be produced in-between).
    ///
    /// It is sender's responsibility to make sure [`TxBatch`]es are constructed correctly (see
    /// docs).
    pub async fn seal_blocks_sync(
        &self,
        tx_batches: Vec<TxBatch>,
        interval: u64,
    ) -> anyhow::Result<Vec<L2BlockNumber>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SealBlocks(tx_batches, interval, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to seal a block as node executor is dropped"))?;

        match response_receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("failed to seal a block as node executor is dropped"),
        }
    }

    /// Request [`NodeExecutor`] to set bytecode for given address. Waits for the change to take place.
    pub async fn set_code_sync(&self, address: Address, bytecode: Vec<u8>) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SetCode(address, bytecode, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to set code as node executor is dropped"))?;
        match response_receiver.await {
            Ok(()) => Ok(()),
            Err(_) => {
                anyhow::bail!("failed to set code as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to set storage key-value pair. Waits for the change to take place.
    pub async fn set_storage_sync(&self, key: StorageKey, value: U256) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SetStorage(key, value, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to set storage as node executor is dropped"))?;
        match response_receiver.await {
            Ok(()) => Ok(()),
            Err(_) => {
                anyhow::bail!("failed to set storage as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to set account's balance to the given value. Waits for the change
    /// to take place.
    pub async fn set_balance_sync(&self, address: Address, balance: U256) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SetBalance(address, balance, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to set balance as node executor is dropped"))?;
        match response_receiver.await {
            Ok(()) => Ok(()),
            Err(_) => {
                anyhow::bail!("failed to set balance as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to set account's nonce to the given value. Waits for the change
    /// to take place.
    pub async fn set_nonce_sync(&self, address: Address, nonce: U256) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SetNonce(address, nonce, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to set nonce as node executor is dropped"))?;
        match response_receiver.await {
            Ok(()) => Ok(()),
            Err(_) => {
                anyhow::bail!("failed to set nonce as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to reset fork to given url and block number. All local state will
    /// be wiped. Waits for the change to take place.
    pub async fn reset_fork_sync(
        &self,
        url: Url,
        block_number: Option<L2BlockNumber>,
    ) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::ResetFork(url, block_number, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to reset fork as node executor is dropped"))?;
        match response_receiver.await {
            Ok(result) => result,
            Err(_) => {
                anyhow::bail!("failed to reset fork as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to reset fork at the given block number. All state will be wiped.
    /// Fails if there is no existing fork. Waits for the change to take place.
    pub async fn reset_fork_block_number_sync(
        &self,
        block_number: L2BlockNumber,
    ) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::ResetForkBlockNumber(block_number, response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to reset fork block number as node executor is dropped")
            })?;
        match response_receiver.await {
            Ok(result) => result,
            Err(_) => {
                anyhow::bail!("failed to reset fork block number as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to set fork's RPC URL without resetting the state. Waits for the
    /// change to take place. Returns `Some(previous_url)` if fork existed and `None` otherwise.
    pub async fn set_fork_url_sync(&self, url: Url) -> anyhow::Result<Option<Url>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SetForkUrl(url, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to set fork URL as node executor is dropped"))?;
        match response_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => {
                anyhow::bail!("failed to set fork URL as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to remove fork if there is one. Waits for the change to take place.
    pub async fn remove_fork_sync(&self) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::RemoveFork(response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to remove fork as node executor is dropped"))?;
        match response_receiver.await {
            Ok(()) => Ok(()),
            Err(_) => {
                anyhow::bail!("failed to remove fork as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to increase time by the given delta (in seconds). Waits for the
    /// change to take place.
    pub async fn increase_time_sync(&self, delta: u64) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::IncreaseTime(delta, response_sender))
            .await
            .map_err(|_| anyhow::anyhow!("failed to increase time as node executor is dropped"))?;
        match response_receiver.await {
            Ok(()) => Ok(()),
            Err(_) => {
                anyhow::bail!("failed to increase time as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to enforce next block's timestamp (in seconds). Waits for the
    /// timestamp validity to be confirmed. Block might still not be produced by then.
    pub async fn enforce_next_timestamp_sync(&self, timestamp: u64) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::EnforceNextTimestamp(timestamp, response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to enforce next timestamp as node executor is dropped")
            })?;
        match response_receiver.await {
            Ok(result) => result,
            Err(_) => {
                anyhow::bail!("failed to enforce next timestamp as node executor is dropped")
            }
        }
    }

    /// Request [`NodeExecutor`] to set current timestamp (in seconds). Waits for the
    /// change to take place.
    pub async fn set_current_timestamp_sync(&self, timestamp: u64) -> anyhow::Result<i128> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::SetCurrentTimestamp(timestamp, response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to set current timestamp as node executor is dropped")
            })?;

        match response_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => anyhow::bail!("failed to set current timestamp as node executor is dropped"),
        }
    }

    /// Request [`NodeExecutor`] to set block timestamp interval (in seconds). Does not wait for the
    /// change to take place.
    pub async fn set_block_timestamp_interval(&self, seconds: u64) -> anyhow::Result<()> {
        Ok(self
            .command_sender
            .send(Command::SetTimestampInterval(seconds))
            .await?)
    }

    /// Request [`NodeExecutor`] to remove block timestamp interval. Waits for the change to take
    /// place. Returns `true` if an existing interval was removed, `false` otherwise.
    pub async fn remove_block_timestamp_interval_sync(&self) -> anyhow::Result<bool> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::RemoveTimestampInterval(response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!("failed to remove block interval as node executor is dropped")
            })?;

        match response_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => anyhow::bail!("failed to remove block interval as node executor is dropped"),
        }
    }

    /// Request [`NodeExecutor`] to enforce next block's base fee per gas. Waits for the change to take
    /// place. Block might still not be produced by then.
    pub async fn enforce_next_base_fee_per_gas_sync(&self, base_fee: U256) -> anyhow::Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.command_sender
            .send(Command::EnforceNextBaseFeePerGas(base_fee, response_sender))
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "failed to enforce next base fee per gas as node executor is dropped"
                )
            })?;

        match response_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => {
                anyhow::bail!("failed to enforce next base fee per gas as node executor is dropped")
            }
        }
    }

    /// Tell the node executor to set (or unset) the progress bar for transaction replay.
    pub async fn set_progress_bar(&self, bar: Option<ProgressBar>) -> anyhow::Result<()> {
        // We don't need a response, so we ignore a potential oneshot.
        self.command_sender
            .send(Command::SetProgressBar(bar))
            .await
            .map_err(|_| anyhow::anyhow!("failed to set progress bar - node executor dropped"))?;
        Ok(())
    }
}

#[derive(Debug)]
enum Command {
    // Block sealing commands
    SealBlock(
        TxBatch,
        Option<oneshot::Sender<anyhow::Result<L2BlockNumber>>>,
    ),
    SealBlocks(
        Vec<TxBatch>,
        u64,
        oneshot::Sender<anyhow::Result<Vec<L2BlockNumber>>>,
    ),
    // Storage manipulation commands
    SetCode(Address, Vec<u8>, oneshot::Sender<()>),
    SetStorage(StorageKey, U256, oneshot::Sender<()>),
    SetBalance(Address, U256, oneshot::Sender<()>),
    SetNonce(Address, U256, oneshot::Sender<()>),
    // Fork manipulation commands
    ResetFork(
        Url,
        Option<L2BlockNumber>,
        oneshot::Sender<anyhow::Result<()>>,
    ),
    ResetForkBlockNumber(L2BlockNumber, oneshot::Sender<anyhow::Result<()>>),
    SetForkUrl(Url, oneshot::Sender<Option<Url>>),
    RemoveFork(oneshot::Sender<()>),
    // Time manipulation commands. Caveat: reply-able commands can hold user connections alive for
    // a long time (until the command is processed).
    IncreaseTime(u64, oneshot::Sender<()>),
    EnforceNextTimestamp(u64, oneshot::Sender<anyhow::Result<()>>),
    SetCurrentTimestamp(u64, oneshot::Sender<i128>),
    SetTimestampInterval(u64),
    RemoveTimestampInterval(oneshot::Sender<bool>),
    // Fee manipulation commands
    EnforceNextBaseFeePerGas(U256, oneshot::Sender<()>),
    // Replay tx progress indicator
    SetProgressBar(Option<ProgressBar>),
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use backon::{ConstantBuilder, ExponentialBuilder, Retryable};
    use std::time::Duration;
    use tokio::sync::mpsc::error::TryRecvError;

    pub struct NodeExecutorTester {
        receiver: Arc<RwLock<mpsc::Receiver<Command>>>,
    }

    impl NodeExecutorTester {
        pub fn new() -> (Self, NodeExecutorHandle) {
            let (command_sender, command_receiver) = mpsc::channel(128);
            (
                Self {
                    receiver: Arc::new(RwLock::new(command_receiver)),
                },
                NodeExecutorHandle { command_sender },
            )
        }

        async fn recv(&self) -> anyhow::Result<Command> {
            let mut receiver = self.receiver.write().await;
            tokio::time::timeout(Duration::from_millis(100), receiver.recv())
                .await
                .map_err(|_| anyhow::anyhow!("no command received"))
                .and_then(|res| res.ok_or(anyhow::anyhow!("disconnected")))
        }

        /// Assert that the next command is sealing provided tx batch. Waits with a timeout for the
        /// next command to arrive if the command queue is empty.
        pub async fn expect_seal_block(&self, expected_tx_batch: TxBatch) -> anyhow::Result<()> {
            let command = (|| self.recv())
                .retry(ExponentialBuilder::default())
                .await?;
            match command {
                Command::SealBlock(actual_tx_batch, _) if actual_tx_batch == expected_tx_batch => {
                    Ok(())
                }
                _ => anyhow::bail!("unexpected command: {:?}", command),
            }
        }

        /// Assert that the next command is sealing provided tx batch. Unlike `expect_seal_block`
        /// this method does not retry.
        pub async fn expect_seal_block_immediate(
            &self,
            expected_tx_batch: TxBatch,
        ) -> anyhow::Result<()> {
            let result = self.receiver.write().await.try_recv();
            match result {
                Ok(Command::SealBlock(actual_tx_batch, _))
                    if actual_tx_batch == expected_tx_batch =>
                {
                    Ok(())
                }
                Ok(command) => anyhow::bail!("unexpected command: {:?}", command),
                Err(TryRecvError::Empty) => anyhow::bail!("no command received"),
                Err(TryRecvError::Disconnected) => anyhow::bail!("disconnected"),
            }
        }

        /// Assert that there are no command currently in receiver queue. Waits up to a timeout for
        /// the next command to potentially arrive.
        pub async fn expect_empty(&self) -> anyhow::Result<()> {
            let result = (|| self.recv())
                .retry(
                    ConstantBuilder::default()
                        .with_delay(Duration::from_millis(100))
                        .with_max_times(3),
                )
                .await;
            match result {
                Ok(command) => {
                    anyhow::bail!("unexpected command: {:?}", command)
                }
                Err(err) if err.to_string().contains("no command received") => Ok(()),
                Err(err) => anyhow::bail!("unexpected error: {:?}", err),
            }
        }

        /// Assert that there are no command currently in receiver queue. Unlike `expect_empty`
        /// this method does not retry.
        pub async fn expect_empty_immediate(&self) -> anyhow::Result<()> {
            let result = self.receiver.write().await.try_recv();
            match result {
                Ok(command) => {
                    anyhow::bail!("unexpected command: {:?}", command)
                }
                Err(TryRecvError::Empty) => Ok(()),
                Err(TryRecvError::Disconnected) => anyhow::bail!("disconnected"),
            }
        }
    }
}
