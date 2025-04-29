//! In-memory node, that supports forking other networks.
use anvil_zksync_common::cache::CacheConfig;
use anvil_zksync_common::sh_println;
use anvil_zksync_common::shell::get_shell;
use anvil_zksync_config::constants::TEST_NODE_NETWORK_ID;
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_core::delegate_vm;
use anvil_zksync_core::node::fork::{ForkClient, ForkSource};
use anvil_zksync_core::node::inner::blockchain::ReadBlockchain;
use anvil_zksync_core::node::inner::node_executor::NodeExecutorHandle;
use anvil_zksync_core::node::inner::storage::ReadStorageDyn;
use anvil_zksync_core::node::inner::time::ReadTime;
use anvil_zksync_core::node::inner::InMemoryNodeInner;
use anvil_zksync_core::node::traces::call_error::CallErrorTracer;
use anvil_zksync_core::node::traces::decoder::CallTraceDecoderBuilder;
use anvil_zksync_core::node::VersionedState;
use anvil_zksync_core::node::{AnvilVM, StorageKeyLayout};
use anvil_zksync_core::node::{BlockSealer, BlockSealerMode, NodeExecutor, TxBatch, TxPool};
use anvil_zksync_core::node::{BlockSealerState, ImpersonationManager, Snapshot};
use anvil_zksync_core::node::{TestNodeFeeInputProvider, ZKOsVM};
use anvil_zksync_core::observability::Observability;
use anvil_zksync_core::system_contracts::SystemContracts;
use anvil_zksync_traces::{
    build_call_trace_arena, decode_trace_arena, filter_call_trace_arena,
    identifier::SignaturesIdentifier, render_trace_arena_inner,
};
use anvil_zksync_types::{
    traces::CallTraceArena, LogLevel, ShowGasDetails, ShowStorageLogs, ShowVMDetails,
};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use once_cell::sync::OnceCell;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use zksync_contracts::BaseSystemContracts;
use zksync_error::anvil_zksync::node::{
    generic_error, to_generic, AnvilNodeError, AnvilNodeResult,
};
use zksync_error::anvil_zksync::state::{StateLoaderError, StateLoaderResult};
use zksync_multivm::interface::storage::StorageView;
use zksync_multivm::interface::VmFactory;
use zksync_multivm::interface::{
    ExecutionResult, InspectExecutionMode, TxExecutionMode, VmInterface,
};
use zksync_multivm::tracers::CallTracer;
use zksync_multivm::vm_latest::Vm;

use zksync_multivm::vm_latest::{HistoryDisabled, ToTracerPointer};
use zksync_types::api::TransactionVariant;
use zksync_types::l2::L2Tx;
use zksync_types::web3::Bytes;
use zksync_types::{
    Address, L2ChainId, PackedEthSignature, ProtocolVersionId, Transaction, H160, U256,
};

/// In-memory node, that can be used for local & unit testing.
/// It also supports the option of forking testnet/mainnet.
/// All contents are removed when object is destroyed.
#[derive(Clone)]
pub struct InMemoryNode {
    /// A thread safe reference to the [InMemoryNodeInner].
    pub(crate) inner: Arc<RwLock<InMemoryNodeInner>>,
    pub(crate) blockchain: Box<dyn ReadBlockchain>,
    pub(crate) storage: Box<dyn ReadStorageDyn>,
    pub(crate) fork: Box<dyn ForkSource>,
    pub node_handle: NodeExecutorHandle,
    /// List of snapshots of the [InMemoryNodeInner]. This is bounded at runtime by [MAX_SNAPSHOTS].
    pub(crate) snapshots: Arc<RwLock<Vec<Snapshot>>>,
    pub(crate) time: Box<dyn ReadTime>,
    pub(crate) impersonation: ImpersonationManager,
    /// An optional handle to the observability stack
    pub(crate) observability: Option<Observability>,
    pub(crate) pool: TxPool,
    pub(crate) sealer_state: BlockSealerState,
    pub(crate) system_contracts: SystemContracts,
    pub(crate) storage_key_layout: StorageKeyLayout,
}

impl InMemoryNode {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        inner: Arc<RwLock<InMemoryNodeInner>>,
        blockchain: Box<dyn ReadBlockchain>,
        storage: Box<dyn ReadStorageDyn>,
        fork: Box<dyn ForkSource>,
        node_handle: NodeExecutorHandle,
        observability: Option<Observability>,
        time: Box<dyn ReadTime>,
        impersonation: ImpersonationManager,
        pool: TxPool,
        sealer_state: BlockSealerState,
        system_contracts: SystemContracts,
        storage_key_layout: StorageKeyLayout,
    ) -> Self {
        InMemoryNode {
            inner,
            blockchain,
            storage,
            fork,
            node_handle,
            snapshots: Default::default(),
            time,
            impersonation,
            observability,
            pool,
            sealer_state,
            system_contracts,
            storage_key_layout,
        }
    }

    /// Replays transactions consequently in a new block. All transactions are expected to be
    /// executable and will become a part of the resulting block.
    pub async fn replay_txs(&self, txs: Vec<Transaction>) -> AnvilNodeResult<()> {
        let tx_batch = TxBatch {
            impersonating: false,
            txs,
        };
        let expected_tx_hashes = tx_batch
            .txs
            .iter()
            .map(|tx| tx.hash())
            .collect::<HashSet<_>>();
        let block_number = self.node_handle.seal_block_sync(tx_batch).await?;
        // Fetch the block that was just sealed
        let block = self
            .blockchain
            .get_block_by_number(block_number)
            .await
            .expect("freshly sealed block could not be found in storage");

        // Calculate tx hash set from that block
        let actual_tx_hashes = block
            .transactions
            .iter()
            .map(|tx| match tx {
                TransactionVariant::Full(tx) => tx.hash,
                TransactionVariant::Hash(tx_hash) => *tx_hash,
            })
            .collect::<HashSet<_>>();

        // Calculate the difference between expected transaction hash set and the actual one.
        // If the difference is not empty it means some transactions were not executed (i.e.
        // were halted).
        let diff_tx_hashes = expected_tx_hashes
            .difference(&actual_tx_hashes)
            .collect::<Vec<_>>();
        if !diff_tx_hashes.is_empty() {
            return Err(generic_error!(
                "Failed to replay transactions: {diff_tx_hashes:?}. Please report this."
            ));
        }

        Ok(())
    }

    /// Adds a lot of tokens to a given account with a specified balance.
    pub async fn set_rich_account(&self, address: H160, balance: U256) {
        self.inner.write().await.set_rich_account(address, balance)
    }

    /// Runs L2 'eth call' method - that doesn't commit to a block.
    pub async fn run_l2_call(
        &self,
        mut l2_tx: L2Tx,
        base_contracts: BaseSystemContracts,
    ) -> AnvilNodeResult<ExecutionResult> {
        let execution_mode = TxExecutionMode::EthCall;

        let inner = self.inner.read().await;

        // init vm

        let (batch_env, _) = inner.create_l1_batch_env().await;
        let system_env = inner.create_system_env(base_contracts, execution_mode);

        let storage = StorageView::new(inner.read_storage()).to_rc_ptr();

        let mut vm = if self.system_contracts.use_zkos {
            AnvilVM::ZKOs(ZKOsVM::<_, HistoryDisabled>::new(
                batch_env,
                system_env,
                storage,
                // TODO: this might be causing a deadlock.. check..
                &inner.fork_storage.inner.read().unwrap().raw_storage,
            ))
        } else {
            AnvilVM::ZKSync(Vm::new(batch_env, system_env, storage))
        };

        // We must inject *some* signature (otherwise bootloader code fails to generate hash).
        if l2_tx.common_data.signature.is_empty() {
            l2_tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        let tx: Transaction = l2_tx.into();
        delegate_vm!(vm, push_transaction(tx.clone()));

        let call_tracer_result = Arc::new(OnceCell::default());
        let error_flags_result = Arc::new(OnceCell::new());

        let tracers = vec![
            CallErrorTracer::new(error_flags_result.clone()).into_tracer_pointer(),
            CallTracer::new(call_tracer_result.clone()).into_tracer_pointer(),
        ];
        let tx_result = delegate_vm!(
            vm,
            inspect(&mut tracers.into(), InspectExecutionMode::OneTx)
        );

        let call_traces = Arc::try_unwrap(call_tracer_result)
            .unwrap()
            .take()
            .unwrap_or_default();

        let verbosity = get_shell().verbosity;
        if !call_traces.is_empty() && verbosity >= 2 {
            let tx_result_for_arena = tx_result.clone();
            let mut builder = CallTraceDecoderBuilder::default();
            builder = builder.with_signature_identifier(
                SignaturesIdentifier::new(
                    Some(inner.config.get_cache_dir().into()),
                    inner.config.offline,
                )
                .map_err(|err| {
                    generic_error!("Failed to create SignaturesIdentifier: {:#}", err)
                })?,
            );

            let decoder = builder.build();
            let arena: CallTraceArena = futures::executor::block_on(async {
                let blocking_result = tokio::task::spawn_blocking(move || {
                    let mut arena = build_call_trace_arena(&call_traces, &tx_result_for_arena);
                    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                    rt.block_on(async {
                        decode_trace_arena(&mut arena, &decoder)
                            .await
                            .map_err(|e| generic_error!("Failed to decode trace arena: {e}"))?;
                        Ok(arena)
                    })
                })
                .await;

                let inner_result: Result<CallTraceArena, AnvilNodeError> =
                    blocking_result.expect("spawn_blocking failed");
                inner_result
            })?;

            let filtered_arena = filter_call_trace_arena(&arena, verbosity);
            let trace_output = render_trace_arena_inner(&filtered_arena, false);
            sh_println!("\nTraces:\n{}", trace_output);
        }

        Ok(tx_result.result)
    }

    // Forcefully stores the given bytecode at a given account.
    pub async fn override_bytecode(
        &self,
        address: Address,
        bytecode: Vec<u8>,
    ) -> AnvilNodeResult<()> {
        self.node_handle.set_code_sync(address, bytecode).await
    }

    pub async fn dump_state(&self, preserve_historical_states: bool) -> AnvilNodeResult<Bytes> {
        let state = self
            .inner
            .read()
            .await
            .dump_state(preserve_historical_states)
            .await?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&serde_json::to_vec(&state).map_err(to_generic)?)
            .map_err(to_generic)?;
        Ok(encoder.finish().map_err(to_generic)?.into())
    }

    pub async fn load_state(&self, buf: Bytes) -> StateLoaderResult<bool> {
        let orig_buf = &buf.0[..];
        let mut decoder = GzDecoder::new(orig_buf);
        let mut decoded_data = Vec::new();

        // Support both compressed and non-compressed state format
        let decoded = if decoder.header().is_some() {
            tracing::trace!(bytes = buf.0.len(), "decompressing state");
            decoder.read_to_end(decoded_data.as_mut()).map_err(|e| {
                StateLoaderError::StateDecompression {
                    details: e.to_string(),
                }
            })?;
            &decoded_data
        } else {
            &buf.0
        };
        tracing::trace!(bytes = decoded.len(), "deserializing state");
        let state: VersionedState = serde_json::from_slice(decoded).map_err(|e| {
            StateLoaderError::StateDeserialization {
                details: e.to_string(),
            }
        })?;

        self.inner.write().await.load_state(state).await
    }

    pub async fn get_chain_id(&self) -> AnvilNodeResult<u32> {
        Ok(self
            .inner
            .read()
            .await
            .config
            .chain_id
            .unwrap_or(TEST_NODE_NETWORK_ID))
    }

    pub fn get_current_timestamp(&self) -> AnvilNodeResult<u64> {
        Ok(self.time.current_timestamp())
    }

    pub async fn set_show_storage_logs(
        &self,
        show_storage_logs: ShowStorageLogs,
    ) -> AnvilNodeResult<String> {
        self.inner.write().await.config.show_storage_logs = show_storage_logs;
        Ok(show_storage_logs.to_string())
    }

    pub async fn set_show_vm_details(
        &self,
        show_vm_details: ShowVMDetails,
    ) -> AnvilNodeResult<String> {
        self.inner.write().await.config.show_vm_details = show_vm_details;
        Ok(show_vm_details.to_string())
    }

    pub async fn set_show_gas_details(
        &self,
        show_gas_details: ShowGasDetails,
    ) -> AnvilNodeResult<String> {
        self.inner.write().await.config.show_gas_details = show_gas_details;
        Ok(show_gas_details.to_string())
    }

    pub async fn set_show_node_config(&self, value: bool) -> AnvilNodeResult<bool> {
        self.inner.write().await.config.show_node_config = value;
        Ok(value)
    }

    pub fn set_log_level(&self, level: LogLevel) -> AnvilNodeResult<bool> {
        let Some(observability) = &self.observability else {
            return Err(generic_error!("Node's logging is not set up."));
        };
        tracing::debug!("setting log level to '{}'", level);
        observability.set_log_level(level)?;
        Ok(true)
    }

    pub fn set_logging(&self, directive: String) -> AnvilNodeResult<bool> {
        let Some(observability) = &self.observability else {
            return Err(generic_error!("Node's logging is not set up."));
        };
        tracing::debug!("setting logging to '{}'", directive);
        observability.set_logging(directive)?;
        Ok(true)
    }

    pub async fn chain_id(&self) -> L2ChainId {
        self.inner.read().await.chain_id()
    }
}

// Test utils
// TODO: Consider builder pattern with sensible defaults
// #[cfg(test)]
// TODO: Mark with #[cfg(test)] once it is not used in other modules
impl InMemoryNode {
    pub fn test_config(fork_client_opt: Option<ForkClient>, config: TestNodeConfig) -> Self {
        let fee_provider = TestNodeFeeInputProvider::from_fork(
            fork_client_opt.as_ref().map(|client| &client.details),
            &config.base_token_config,
        );
        let impersonation = ImpersonationManager::default();
        let system_contracts = SystemContracts::from_options(
            config.system_contracts_options,
            config.system_contracts_path.clone(),
            ProtocolVersionId::latest(),
            config.use_evm_emulator,
            config.use_zkos,
        );
        let storage_key_layout = if config.use_zkos {
            StorageKeyLayout::ZkOs
        } else {
            StorageKeyLayout::ZkEra
        };
        let (inner, storage, blockchain, time, fork, vm_runner) = InMemoryNodeInner::init(
            fork_client_opt,
            fee_provider,
            Arc::new(RwLock::new(Default::default())),
            config,
            impersonation.clone(),
            system_contracts.clone(),
            storage_key_layout,
            false,
        );
        let (node_executor, node_handle) =
            NodeExecutor::new(inner.clone(), vm_runner, storage_key_layout);
        let pool = TxPool::new(
            impersonation.clone(),
            anvil_zksync_types::TransactionOrder::Fifo,
        );
        let tx_listener = pool.add_tx_listener();
        let (block_sealer, block_sealer_state) = BlockSealer::new(
            BlockSealerMode::immediate(1000, tx_listener),
            pool.clone(),
            node_handle.clone(),
        );
        tokio::spawn(node_executor.run());
        tokio::spawn(block_sealer.run());
        Self::new(
            inner,
            blockchain,
            storage,
            fork,
            node_handle,
            None,
            time,
            impersonation,
            pool,
            block_sealer_state,
            system_contracts,
            storage_key_layout,
        )
    }

    pub fn test(fork_client_opt: Option<ForkClient>) -> Self {
        let config = TestNodeConfig {
            cache_config: CacheConfig::None,
            ..Default::default()
        };
        Self::test_config(fork_client_opt, config)
    }
}

#[cfg(test)]
impl InMemoryNode {
    pub async fn apply_txs(
        &self,
        txs: impl IntoIterator<Item = Transaction>,
    ) -> AnvilNodeResult<Vec<zksync_types::api::TransactionReceipt>> {
        use backon::{ConstantBuilder, Retryable};
        use std::time::Duration;

        let txs = Vec::from_iter(txs);
        let expected_tx_hashes = txs.iter().map(|tx| tx.hash()).collect::<Vec<_>>();
        self.pool.add_txs(txs);

        let mut receipts = Vec::with_capacity(expected_tx_hashes.len());
        for tx_hash in expected_tx_hashes {
            let receipt = (|| async {
                self.blockchain
                    .get_tx_receipt(&tx_hash)
                    .await
                    .ok_or(generic_error!("missing tx receipt"))
            })
            .retry(
                ConstantBuilder::default()
                    .with_delay(Duration::from_millis(500))
                    .with_max_times(5),
            )
            .await?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }
}
