//! Interfaces that use zksync_os for VM execution.
//! This is still experimental code.
use std::vec;

use anvil_zksync_config::types::ZKsyncOsConfig;
use forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use zksync_multivm::{
    interface::{
        storage::{StoragePtr, WriteStorage},
        ExecutionResult, InspectExecutionMode, L1BatchEnv, PushTransactionResult, SystemEnv,
        TxExecutionMode, VmExecutionResultAndLogs, VmInterface, VmInterfaceHistoryEnabled,
    },
    tracers::TracerDispatcher,
    vm_latest::TracerPointer,
    HistoryMode,
};

use zksync_multivm::MultiVmTracerPointer;
use zksync_types::{StorageKey, Transaction};

use crate::deps::InMemoryStorage;

mod helpers;

// Only put things that _have_ to be public here.
pub use self::helpers::{
    zksync_os_get_batch_witness, zksync_os_get_nonce_key, zksync_os_storage_key_for_eth_balance,
    ZKSYNC_OS_CALL_GAS_LIMIT,
};

use self::helpers::*;

#[derive(Debug)]
pub struct ZKsyncOsVM<S: WriteStorage, H: HistoryMode> {
    pub storage: StoragePtr<S>,
    pub tree: InMemoryTree,
    preimage: InMemoryPreimageSource,
    transactions: Vec<Transaction>,
    system_env: SystemEnv,
    batch_env: L1BatchEnv,
    config: ZKsyncOsConfig,
    witness: Option<Vec<u8>>,
    _phantom: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> ZKsyncOsVM<S, H> {
    pub fn new(
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
        storage: StoragePtr<S>,
        raw_storage: &InMemoryStorage,
        config: &ZKsyncOsConfig,
    ) -> Self {
        let (tree, preimage) = { create_tree_from_full_state(raw_storage) };
        ZKsyncOsVM {
            storage,
            tree,
            preimage,
            transactions: vec![],
            system_env,
            batch_env,
            witness: None,
            config: config.clone(),
            _phantom: Default::default(),
        }
    }

    /// If any keys are updated in storage externally, but not reflected in internal tree.
    pub fn update_inconsistent_keys(&mut self, inconsistent_nodes: &[&StorageKey]) {
        for key in inconsistent_nodes {
            let value = self.storage.borrow_mut().read_value(key);
            add_elem_to_tree(&mut self.tree, key, &value);
        }
    }
}

pub struct ZkSyncOSTracerDispatcher<S: WriteStorage, H: HistoryMode> {
    _tracers: Vec<S>,
    _marker: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> Default for ZkSyncOSTracerDispatcher<S, H> {
    fn default() -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> From<Vec<TracerPointer<S, H>>>
    for ZkSyncOSTracerDispatcher<S, H>
{
    fn from(_value: Vec<TracerPointer<S, H>>) -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> From<Vec<MultiVmTracerPointer<S, H>>>
    for ZkSyncOSTracerDispatcher<S, H>
{
    fn from(_value: Vec<MultiVmTracerPointer<S, H>>) -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> From<TracerDispatcher<S, H>>
    for ZkSyncOSTracerDispatcher<S, H>
{
    fn from(_value: TracerDispatcher<S, H>) -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmInterface for ZKsyncOsVM<S, H> {
    type TracerDispatcher = ZkSyncOSTracerDispatcher<S, H>;

    fn push_transaction(
        &mut self,
        tx: Transaction,
    ) -> zksync_multivm::interface::PushTransactionResult<'_> {
        self.transactions.push(tx);
        PushTransactionResult {
            compressed_bytecodes: Default::default(),
        }
    }

    fn inspect(
        &mut self,
        _dispatcher: &mut Self::TracerDispatcher,
        execution_mode: zksync_multivm::interface::InspectExecutionMode,
    ) -> VmExecutionResultAndLogs {
        if let InspectExecutionMode::Bootloader = execution_mode {
            // This is called at the end of seal block.
            // Now is the moment to collect the witness and store it.

            // TODO: add support for multiple transactions.

            if let Some(witness) = self.witness.clone() {
                set_batch_witness(self.batch_env.number.0, witness);
            }

            return VmExecutionResultAndLogs {
                result: ExecutionResult::Success { output: vec![] },
                logs: Default::default(),
                statistics: Default::default(),
                refunds: Default::default(),
                dynamic_factory_deps: Default::default(),
            };
        }
        let simulate_only = match self.system_env.execution_mode {
            TxExecutionMode::VerifyExecute => false,
            TxExecutionMode::EstimateFee => true,
            TxExecutionMode::EthCall => true,
        };

        // For now we only support one transaction.
        assert_eq!(
            1,
            self.transactions.len(),
            "only one tx per batch supported for now"
        );

        // TODO: add support for multiple transactions.
        let tx = self.transactions[0].clone();
        let (result, witness) = execute_tx_in_zkos(
            &tx,
            &self.tree,
            &self.preimage,
            &mut self.storage,
            simulate_only,
            &self.batch_env,
            self.system_env.chain_id.as_u64(),
            self.config.zksync_os_bin_path.clone(),
        );

        self.witness = witness;
        result
    }

    fn start_new_l2_block(&mut self, _l2_block_env: zksync_multivm::interface::L2BlockEnv) {
        // no-op
    }

    fn inspect_transaction_with_bytecode_compression(
        &mut self,
        _tracer: &mut Self::TracerDispatcher,
        _tx: Transaction,
        _with_compression: bool,
    ) -> (
        zksync_multivm::interface::BytecodeCompressionResult<'_>,
        VmExecutionResultAndLogs,
    ) {
        todo!()
    }

    fn finish_batch(
        &mut self,
        _pubdata_builder: std::rc::Rc<dyn zksync_multivm::interface::pubdata::PubdataBuilder>,
    ) -> zksync_multivm::interface::FinishedL1Batch {
        todo!()
    }
}

impl<S: WriteStorage, H: HistoryMode> VmInterfaceHistoryEnabled for ZKsyncOsVM<S, H> {
    fn make_snapshot(&mut self) {}

    fn rollback_to_the_latest_snapshot(&mut self) {
        panic!("Not implemented for zksync_os");
    }

    fn pop_snapshot_no_rollback(&mut self) {}

    fn pop_front_snapshot_no_rollback(&mut self) {}
}
