#![allow(dead_code)]
#![allow(unused_variables)]

//! Interfaces that use zkos for VM execution.
//! This is still experimental code.
use zksync_multivm::{
    interface::{
        storage::{StoragePtr, WriteStorage},
        L1BatchEnv, SystemEnv, VmExecutionResultAndLogs, VmInterface, VmInterfaceHistoryEnabled,
    },
    vm_latest::TracerPointer,
    HistoryMode,
};
use zksync_types::{Address, StorageKey, Transaction};

use crate::deps::InMemoryStorage;

pub fn boojumos_get_nonce_key(account: &Address) -> StorageKey {
    todo!()
}

pub fn boojumos_storage_key_for_eth_balance(address: &Address) -> StorageKey {
    todo!();
}

pub struct BoojumOsVM<S: WriteStorage, H: HistoryMode> {
    pub storage: StoragePtr<S>,

    _phantom: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> BoojumOsVM<S, H> {
    pub fn new(
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
        storage: StoragePtr<S>,
        raw_storage: &InMemoryStorage,
    ) -> Self {
        todo!()
    }

    /// If any keys are updated in storage externally, but not reflected in internal tree.
    pub fn update_inconsistent_keys(&mut self, inconsistent_nodes: &[&StorageKey]) {
        todo!()
    }
}

pub struct BoojumOsTracerDispatcher<S: WriteStorage, H: HistoryMode> {
    _tracers: Vec<S>,
    _marker: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> Default for BoojumOsTracerDispatcher<S, H> {
    fn default() -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> From<Vec<TracerPointer<S, H>>>
    for BoojumOsTracerDispatcher<S, H>
{
    fn from(_value: Vec<TracerPointer<S, H>>) -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmInterface for BoojumOsVM<S, H> {
    type TracerDispatcher = BoojumOsTracerDispatcher<S, H>;

    fn push_transaction(
        &mut self,
        tx: Transaction,
    ) -> zksync_multivm::interface::PushTransactionResult<'_> {
        todo!()
    }

    fn inspect(
        &mut self,
        _dispatcher: &mut Self::TracerDispatcher,
        execution_mode: zksync_multivm::interface::InspectExecutionMode,
    ) -> VmExecutionResultAndLogs {
        todo!()
    }

    fn start_new_l2_block(&mut self, _l2_block_env: zksync_multivm::interface::L2BlockEnv) {
        todo!()
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

impl<S: WriteStorage, H: HistoryMode> VmInterfaceHistoryEnabled for BoojumOsVM<S, H> {
    fn make_snapshot(&mut self) {}

    fn rollback_to_the_latest_snapshot(&mut self) {
        panic!("Not implemented for zkos");
    }

    fn pop_snapshot_no_rollback(&mut self) {}
}
