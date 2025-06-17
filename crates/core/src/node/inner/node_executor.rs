use super::InMemoryNodeInner;
use crate::node::blockchain::ReadBlockchain;
use crate::node::fork::ForkConfig;
use crate::node::inner::fork::{ForkClient, ForkSource};
use crate::node::inner::vm_runner::VmRunner;
use crate::node::keys::StorageKeyLayout;
use crate::node::pool::TxBatch;
use anyhow::Context;
use indicatif::ProgressBar;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot, RwLock};
use url::Url;
use zk_os_forward_system::run::BatchContext;
use zksync_error::anvil_zksync;
use zksync_error::anvil_zksync::node::{AnvilNodeError, AnvilNodeResult};
use zksync_os_sequencer::model::BlockCommand;
use zksync_os_sequencer::storage::StateHandle;
use zksync_types::bytecode::{pad_evm_bytecode, BytecodeHash, BytecodeMarker};
use zksync_types::utils::nonces_to_full_nonce;
use zksync_types::{get_code_key, u256_to_h256, Address, L2BlockNumber, StorageKey, U256};

pub struct NodeExecutor {
    node_inner: Arc<RwLock<InMemoryNodeInner>>,
    vm_runner: VmRunner,
    state_handle: StateHandle,
    command_receiver: mpsc::Receiver<Command>,
    storage_key_layout: StorageKeyLayout,
}

impl NodeExecutor {
    pub fn new(
        node_inner: Arc<RwLock<InMemoryNodeInner>>,
        vm_runner: VmRunner,
        state_handle: StateHandle,
        storage_key_layout: StorageKeyLayout,
    ) -> (Self, NodeExecutorHandle) {
        let (command_sender, command_receiver) = mpsc::channel(128);
        let this = Self {
            node_inner,
            vm_runner,
            state_handle,
            command_receiver,
            storage_key_layout,
        };
        let handle = NodeExecutorHandle { command_sender };
        (this, handle)
    }

    pub async fn run(mut self) -> AnvilNodeResult<()> {
        while let Some(command) = self.command_receiver.recv().await {
            match command {
                Command::SealBlock(tx_batch, reply) => {
                    self.seal_block(tx_batch, reply).await?;
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
                Command::SetProgressReport(bar) => {
                    self.vm_runner.set_progress_report(bar);
                }
            }
        }

        tracing::trace!("channel has been closed; stopping node executor");
        Ok(())
    }
}

pub fn millis_since_epoch() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Incorrect system time")
        .as_millis()
}

impl NodeExecutor {
    async fn seal_block(
        &mut self,
        tx_batch: TxBatch,
        reply_sender: Option<oneshot::Sender<AnvilNodeResult<L2BlockNumber>>>,
    ) -> AnvilNodeResult<()> {
        let mut node_inner = self.node_inner.write().await;
        let result = async {
            let (batch_out, replay) =
                zksync_os_sequencer::execution::block_executor::execute_block(
                    BlockCommand::Produce(BatchContext {
                        chain_id: node_inner.config.chain_id.unwrap() as u64,
                        block_number: (self.state_handle.current_block_number().await.0 + 1) as u64,
                        block_hashes: Default::default(),
                        timestamp: (millis_since_epoch() / 1000) as u64,
                        eip1559_basefee: ruint::aliases::U256::from(1000),
                        gas_per_pubdata: Default::default(),
                        native_price: ruint::aliases::U256::from(1),
                        coinbase: Default::default(),
                        gas_limit: 100_000_000,
                    }),
                    Box::pin(futures::stream::iter(tx_batch.txs)),
                    self.state_handle.clone(),
                )
                .await
                .context("execute_block")?;

            self.state_handle
                .handle_block_output(batch_out.clone(), replay.transactions.clone());
            Ok(L2BlockNumber(batch_out.header.number as u32))
        }
        .await;
        drop(node_inner);

        // Reply to sender if we can, otherwise hold result for further processing
        let result = if let Some(reply_sender) = reply_sender {
            if let Err(error_result) = reply_sender.send(result) {
                tracing::info!("failed to reply as receiver has been dropped");
                error_result
            } else {
                return Ok(());
            }
        } else {
            result
        };
        // Not much we can do with an error at this level so we just print it
        if let Err(err) = result {
            tracing::error!("failed to seal a block: {:#?}", err);
        }
        Ok(())
    }

    async fn seal_blocks(
        &mut self,
        tx_batches: Vec<TxBatch>,
        interval: u64,
        reply: oneshot::Sender<AnvilNodeResult<Vec<L2BlockNumber>>>,
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
            Ok(block_numbers)
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

    async fn set_code(
        &mut self,
        address: Address,
        mut bytecode: Vec<u8>,
        reply: oneshot::Sender<()>,
    ) {
        let code_key = get_code_key(&address);
        let marker = BytecodeMarker::detect(&bytecode);
        let bytecode_hash = match marker {
            BytecodeMarker::EraVm => BytecodeHash::for_bytecode(&bytecode),
            BytecodeMarker::Evm => BytecodeHash::for_raw_evm_bytecode(&bytecode),
        }
        .value();
        if marker == BytecodeMarker::Evm {
            bytecode = pad_evm_bytecode(&bytecode);
        }

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
        reply: oneshot::Sender<AnvilNodeResult<()>>,
    ) {
        let result = async {
            // We don't know what chain this is so we assume default scale configuration.
            let fork_client =
                ForkClient::at_block_number(ForkConfig::unknown(url), block_number).await?;
            self.node_inner.write().await.reset(Some(fork_client)).await;

            Ok(())
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
        reply: oneshot::Sender<AnvilNodeResult<()>>,
    ) {
        let result = async {
            let node_inner = self.node_inner.write().await;
            let url = node_inner
                .fork
                .url()
                .ok_or_else(|| anvil_zksync::node::generic_error!("no existing fork found"))?;
            // Keep scale factors as this is the same chain.
            let details = node_inner
                .fork
                .details()
                .ok_or_else(|| anvil_zksync::node::generic_error!("no existing fork found"))?;
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

            Ok(())
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
        reply: oneshot::Sender<AnvilNodeResult<()>>,
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
    pub async fn seal_block(&self, tx_batch: TxBatch) -> AnvilNodeResult<()> {
        execute_without_response(&self.command_sender, Command::SealBlock(tx_batch, None)).await
    }

    /// Request [`NodeExecutor`] to seal a new block from the provided transaction batch. Waits for
    /// the block to be produced and returns its number.
    ///
    /// It is sender's responsibility to make sure [`TxBatch`] is constructed correctly (see its
    /// docs).
    pub async fn seal_block_sync(&self, tx_batch: TxBatch) -> AnvilNodeResult<L2BlockNumber> {
        execute_with_response(&self.command_sender, |response_sender| {
            Command::SealBlock(tx_batch, Some(response_sender))
        })
        .await?
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
    ) -> AnvilNodeResult<Vec<L2BlockNumber>> {
        execute_with_response(&self.command_sender, |response_sender| {
            Command::SealBlocks(tx_batches, interval, response_sender)
        })
        .await?
    }

    /// Request [`NodeExecutor`] to set bytecode for given address. Waits for the change to take place.
    pub async fn set_code_sync(&self, address: Address, bytecode: Vec<u8>) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, |response_sender| {
            Command::SetCode(address, bytecode, response_sender)
        })
        .await
    }

    /// Request [`NodeExecutor`] to set storage key-value pair. Waits for the change to take place.
    pub async fn set_storage_sync(&self, key: StorageKey, value: U256) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, |response_sender| {
            Command::SetStorage(key, value, response_sender)
        })
        .await
    }

    /// Request [`NodeExecutor`] to set account's balance to the given value. Waits for the change
    /// to take place.
    pub async fn set_balance_sync(&self, address: Address, balance: U256) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, move |response_sender| {
            Command::SetBalance(address, balance, response_sender)
        })
        .await
    }

    /// Request [`NodeExecutor`] to set account's nonce to the given value. Waits for the change
    /// to take place.
    pub async fn set_nonce_sync(&self, address: Address, nonce: U256) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, move |response_sender| {
            Command::SetNonce(address, nonce, response_sender)
        })
        .await
    }

    /// Request [`NodeExecutor`] to reset fork to given url and block number. All local state will
    /// be wiped. Waits for the change to take place.
    pub async fn reset_fork_sync(
        &self,
        url: Url,
        block_number: Option<L2BlockNumber>,
    ) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, move |response_sender| {
            Command::ResetFork(url, block_number, response_sender)
        })
        .await?
    }

    /// Request [`NodeExecutor`] to reset fork at the given block number. All state will be wiped.
    /// Fails if there is no existing fork. Waits for the change to take place.
    pub async fn reset_fork_block_number_sync(
        &self,
        block_number: L2BlockNumber,
    ) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, |response_sender| {
            Command::ResetForkBlockNumber(block_number, response_sender)
        })
        .await?
    }

    /// Request [`NodeExecutor`] to set fork's RPC URL without resetting the state. Waits for the
    /// change to take place. Returns `Some(previous_url)` if fork existed and `None` otherwise.
    pub async fn set_fork_url_sync(&self, url: Url) -> AnvilNodeResult<Option<Url>> {
        execute_with_response(&self.command_sender, move |response_sender| {
            Command::SetForkUrl(url, response_sender)
        })
        .await
    }

    /// Request [`NodeExecutor`] to remove fork if there is one. Waits for the change to take place.
    pub async fn remove_fork_sync(&self) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, Command::RemoveFork).await
    }

    /// Request [`NodeExecutor`] to increase time by the given delta (in seconds). Waits for the
    /// change to take place.
    pub async fn increase_time_sync(&self, delta: u64) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, |response_sender| {
            Command::IncreaseTime(delta, response_sender)
        })
        .await
    }

    /// Request [`NodeExecutor`] to enforce next block's timestamp (in seconds). Waits for the
    /// timestamp validity to be confirmed. Block might still not be produced by then.
    pub async fn enforce_next_timestamp_sync(&self, timestamp: u64) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, |response_sender| {
            Command::EnforceNextTimestamp(timestamp, response_sender)
        })
        .await?
    }

    /// Request [`NodeExecutor`] to set current timestamp (in seconds). Waits for the
    /// change to take place.
    pub async fn set_current_timestamp_sync(&self, timestamp: u64) -> AnvilNodeResult<i128> {
        execute_with_response(&self.command_sender, |response_sender| {
            Command::SetCurrentTimestamp(timestamp, response_sender)
        })
        .await
    }

    /// Request [`NodeExecutor`] to set block timestamp interval (in seconds). Does not wait for the
    /// change to take place.
    pub async fn set_block_timestamp_interval(&self, seconds: u64) -> AnvilNodeResult<()> {
        execute_without_response(&self.command_sender, Command::SetTimestampInterval(seconds)).await
    }

    /// Request [`NodeExecutor`] to remove block timestamp interval. Waits for the change to take
    /// place. Returns `true` if an existing interval was removed, `false` otherwise.
    pub async fn remove_block_timestamp_interval_sync(&self) -> AnvilNodeResult<bool> {
        execute_with_response(&self.command_sender, Command::RemoveTimestampInterval).await
    }

    /// Request [`NodeExecutor`] to enforce next block's base fee per gas. Waits for the change to take
    /// place. Block might still not be produced by then.
    pub async fn enforce_next_base_fee_per_gas_sync(&self, base_fee: U256) -> AnvilNodeResult<()> {
        execute_with_response(&self.command_sender, |sender| {
            Command::EnforceNextBaseFeePerGas(base_fee, sender)
        })
        .await
    }

    /// Request [`NodeExecutor`] to set (or unset) the progress bar for transaction replay.
    pub async fn set_progress_report(&self, bar: Option<ProgressBar>) -> AnvilNodeResult<()> {
        execute_without_response(&self.command_sender, Command::SetProgressReport(bar)).await
    }
}

async fn execute_without_response(
    command_sender: &mpsc::Sender<Command>,
    command: Command,
) -> AnvilNodeResult<()> {
    let action_name = command.readable_action_name();
    command_sender
        .send(command)
        .await
        .map_err(|_| node_executor_dropped_error("request to", &action_name))
}

async fn execute_with_response<R>(
    command_sender: &mpsc::Sender<Command>,
    command_gen: impl FnOnce(oneshot::Sender<R>) -> Command,
) -> AnvilNodeResult<R> {
    let (response_sender, response_receiver) = oneshot::channel();

    let command = command_gen(response_sender);
    let action_name = command.readable_action_name();
    command_sender
        .send(command)
        .await
        .map_err(|_| node_executor_dropped_error("request to", &action_name))?;

    match response_receiver.await {
        Ok(result) => Ok(result),
        Err(_) => Err(node_executor_dropped_error(
            "receive a response to the request to",
            &action_name,
        )),
    }
}
fn node_executor_dropped_error(request_or_receive: &str, action_name: &str) -> AnvilNodeError {
    anvil_zksync::node::generic_error!(
        r"Failed to {request_or_receive} {action_name} because node executor is dropped. \
         Another error was likely propagated from the main execution loop. \
         If this is not the case, please, report this as a bug."
    )
}

#[derive(Debug)]
enum Command {
    // Block sealing commands
    SealBlock(
        TxBatch,
        Option<oneshot::Sender<AnvilNodeResult<L2BlockNumber>>>,
    ),
    SealBlocks(
        Vec<TxBatch>,
        u64,
        oneshot::Sender<AnvilNodeResult<Vec<L2BlockNumber>>>,
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
        oneshot::Sender<AnvilNodeResult<()>>,
    ),
    ResetForkBlockNumber(L2BlockNumber, oneshot::Sender<AnvilNodeResult<()>>),
    SetForkUrl(Url, oneshot::Sender<Option<Url>>),
    RemoveFork(oneshot::Sender<()>),
    // Time manipulation commands. Caveat: reply-able commands can hold user connections alive for
    // a long time (until the command is processed).
    IncreaseTime(u64, oneshot::Sender<()>),
    EnforceNextTimestamp(u64, oneshot::Sender<AnvilNodeResult<()>>),
    SetCurrentTimestamp(u64, oneshot::Sender<i128>),
    SetTimestampInterval(u64),
    RemoveTimestampInterval(oneshot::Sender<bool>),
    // Fee manipulation commands
    EnforceNextBaseFeePerGas(U256, oneshot::Sender<()>),
    // Replay tx progress indicator
    SetProgressReport(Option<ProgressBar>),
}

impl Command {
    /// Human-readable command description used for diagnostics.
    fn readable_action_name(&self) -> String {
        fn batch_repr(batch: &TxBatch) -> String {
            format!(
                "{:?}",
                batch
                    .txs
                    .iter()
                    .map(|tx| tx.hash().to_string())
                    .collect::<Vec<_>>()
            )
        }
        match self {
            Command::SealBlock(tx_batch, _) => {
                format!("seal a block with transactions {}", batch_repr(tx_batch))
            }
            Command::SealBlocks(vec, interval, _) => format!(
                "seal blocks with intervals of {interval} seconds between consecutive blocks: {:?}",
                vec.iter().map(batch_repr).collect::<Vec<_>>()
            ),
            Command::SetCode(address, _bytecode, _) => {
                format!("set bytecode for address {address}")
            }
            Command::SetStorage(storage_key, value, _) => {
                format!("set storage {}={value}", storage_key.hashed_key())
            }
            Command::SetBalance(account, new_value, _) => {
                format!("set balance of account {account} to {new_value}")
            }
            Command::SetNonce(account, nonce, _) => {
                format!("set nonce of account {account} to {nonce}")
            }
            Command::ResetFork(url, l2_block_number, _) => {
                format!("reset fork to url {url} and block number {l2_block_number:?}")
            }
            Command::ResetForkBlockNumber(l2_block_number, _) => {
                format!("reset fork block number to {l2_block_number}")
            }
            Command::SetForkUrl(url, _) => format!("set fork RPC URL to {url}"),
            Command::RemoveFork(_) => "remove fork if there was one".into(),
            Command::IncreaseTime(delta, _) => format!("increase time by {delta} seconds"),
            Command::EnforceNextTimestamp(timestamp, _) => {
                format!("enforce next block's timestamp to {timestamp} seconds")
            }
            Command::SetCurrentTimestamp(timestamp, _) => {
                format!("set current timestamp to {timestamp} seconds")
            }
            Command::SetTimestampInterval(interval) => {
                format!("set block timestamp interval to {interval} seconds")
            }
            Command::RemoveTimestampInterval(_) => "remove timestamp interval".into(),
            Command::EnforceNextBaseFeePerGas(base_fee, _) => {
                format!("enforce next block's base fee per gas to {base_fee}")
            }
            Command::SetProgressReport(_) => "set progress report".into(),
        }
    }
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
