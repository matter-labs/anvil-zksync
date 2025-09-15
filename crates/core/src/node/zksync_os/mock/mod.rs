//! Interfaces that use zksync_os for VM execution.
//! This is still experimental code.

use anvil_zksync_config::types::ZKsyncOsConfig;
use zksync_multivm::{
    HistoryMode,
    interface::{
        L1BatchEnv, SystemEnv, VmExecutionResultAndLogs, VmInterface, VmInterfaceHistoryEnabled,
        storage::{StoragePtr, WriteStorage},
    },
    tracers::TracerDispatcher,
    vm_latest::TracerPointer,
};

use zksync_multivm::MultiVmTracerPointer;
use zksync_types::{Address, StorageKey, Transaction};

use crate::deps::InMemoryStorage;

#[derive(Debug)]
pub struct MockZKsyncOsVM<S: WriteStorage, H: HistoryMode> {
    _s: std::marker::PhantomData<S>,
    _h: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> MockZKsyncOsVM<S, H> {
    pub fn new(
        _batch_env: L1BatchEnv,
        _system_env: SystemEnv,
        _storage: StoragePtr<S>,
        _raw_storage: &InMemoryStorage,
        _config: &ZKsyncOsConfig,
    ) -> Self {
        panic!(
            "Cannot instantiate mock ZKsyncOsVM, make sure 'zksync-os' feature is enabled in Cargo.toml"
        );
    }

    /// If any keys are updated in storage externally, but not reflected in internal tree.
    pub fn update_inconsistent_keys(&mut self, _inconsistent_nodes: &[&StorageKey]) {
        unimplemented!();
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

impl<S: WriteStorage, H: HistoryMode> VmInterface for MockZKsyncOsVM<S, H> {
    type TracerDispatcher = ZkSyncOSTracerDispatcher<S, H>;

    fn push_transaction(
        &mut self,
        _tx: Transaction,
    ) -> zksync_multivm::interface::PushTransactionResult<'_> {
        unimplemented!()
    }

    fn inspect(
        &mut self,
        _dispatcher: &mut Self::TracerDispatcher,
        _execution_mode: zksync_multivm::interface::InspectExecutionMode,
    ) -> VmExecutionResultAndLogs {
        unimplemented!()
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

impl<S: WriteStorage, H: HistoryMode> VmInterfaceHistoryEnabled for MockZKsyncOsVM<S, H> {
    fn make_snapshot(&mut self) {}

    fn rollback_to_the_latest_snapshot(&mut self) {
        panic!("Not implemented for zksync_os");
    }

    fn pop_snapshot_no_rollback(&mut self) {}

    fn pop_front_snapshot_no_rollback(&mut self) {}
}
