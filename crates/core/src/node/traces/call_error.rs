use once_cell::sync::OnceCell;
use std::sync::Arc;
use zksync_multivm::interface::storage::WriteStorage;
use zksync_multivm::interface::tracer::VmExecutionStopReason;
use zksync_multivm::tracers::old::OldTracers;
use zksync_multivm::{
    tracers::dynamic::vm_1_5_0::DynTracer,
    vm_latest::{HistoryMode, SimpleMemory, VmTracer},
    zk_evm_latest::{
        tracing::{AfterDecodingData, VmLocalStateData},
        vm_state::ErrorFlags,
    },
    IntoOldVmTracer,
};

#[derive(Debug, Clone)]
pub struct CallErrorTracer {
    result: Arc<OnceCell<ErrorFlags>>,
}

impl CallErrorTracer {
    pub fn new(result: Arc<OnceCell<ErrorFlags>>) -> Self {
        Self { result }
    }
}

impl<S, H: HistoryMode> DynTracer<S, SimpleMemory<H>> for CallErrorTracer {
    fn after_decoding(
        &mut self,
        _state: VmLocalStateData<'_>,
        data: AfterDecodingData,
        _memory: &SimpleMemory<H>,
    ) {
        // The top frame is processed last, its error flags
        // overwrite any previously observed ones.
        if !data.error_flags_accumulated.is_empty() {
            let _ = self.result.set(data.error_flags_accumulated);
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmTracer<S, H> for CallErrorTracer {}

//
// The rest of the file contains stub tracer implementations for older VM versions.
// Reasoning: `CallErrorTracer` needs to implement `MultiVmTracer` to be compatible with era
// abstractions such as `BatchExecutor` and `BatchExecutorFactory`.
//

impl<S, H: zksync_multivm::vm_1_4_1::HistoryMode>
    zksync_multivm::tracers::dynamic::vm_1_4_1::DynTracer<
        S,
        zksync_multivm::vm_1_4_1::SimpleMemory<H>,
    > for CallErrorTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_1_4_1::HistoryMode>
    zksync_multivm::vm_1_4_1::VmTracer<S, H> for CallErrorTracer
{
    fn after_vm_execution(
        &mut self,
        _state: &mut zksync_multivm::vm_1_4_1::ZkSyncVmState<S, H>,
        _bootloader_state: &zksync_multivm::vm_1_4_1::BootloaderState,
        _stop_reason: VmExecutionStopReason,
    ) {
        todo!()
    }
}

impl<S, H: zksync_multivm::vm_1_4_2::HistoryMode>
    zksync_multivm::tracers::dynamic::vm_1_4_1::DynTracer<
        S,
        zksync_multivm::vm_1_4_2::SimpleMemory<H>,
    > for CallErrorTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_1_4_2::HistoryMode>
    zksync_multivm::vm_1_4_2::VmTracer<S, H> for CallErrorTracer
{
    fn after_vm_execution(
        &mut self,
        _state: &mut zksync_multivm::vm_1_4_2::ZkSyncVmState<S, H>,
        _bootloader_state: &zksync_multivm::vm_1_4_2::BootloaderState,
        _stop_reason: VmExecutionStopReason,
    ) {
        todo!()
    }
}

impl<S: WriteStorage, H: zksync_multivm::vm_boojum_integration::HistoryMode>
    zksync_multivm::tracers::dynamic::vm_1_4_0::DynTracer<
        S,
        zksync_multivm::vm_boojum_integration::SimpleMemory<H>,
    > for CallErrorTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_boojum_integration::HistoryMode>
    zksync_multivm::vm_boojum_integration::VmTracer<S, H> for CallErrorTracer
{
    fn after_vm_execution(
        &mut self,
        _state: &mut zksync_multivm::vm_boojum_integration::ZkSyncVmState<S, H>,
        _bootloader_state: &zksync_multivm::vm_boojum_integration::BootloaderState,
        _stop_reason: VmExecutionStopReason,
    ) {
        todo!()
    }
}

impl<S: WriteStorage, H: zksync_multivm::vm_refunds_enhancement::HistoryMode>
    zksync_multivm::tracers::dynamic::vm_1_3_3::DynTracer<
        S,
        zksync_multivm::vm_refunds_enhancement::SimpleMemory<H>,
    > for CallErrorTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_refunds_enhancement::HistoryMode>
    zksync_multivm::vm_refunds_enhancement::VmTracer<S, H> for CallErrorTracer
{
    fn after_vm_execution(
        &mut self,
        _state: &mut zksync_multivm::vm_refunds_enhancement::ZkSyncVmState<S, H>,
        _bootloader_state: &zksync_multivm::vm_refunds_enhancement::BootloaderState,
        _stop_reason: VmExecutionStopReason,
    ) {
        todo!()
    }
}

impl<S: WriteStorage, H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::tracers::dynamic::vm_1_3_3::DynTracer<
        S,
        zksync_multivm::vm_virtual_blocks::SimpleMemory<H>,
    > for CallErrorTracer
{
}

impl<H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::ExecutionEndTracer<H> for CallErrorTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::ExecutionProcessing<S, H> for CallErrorTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::VmTracer<S, H> for CallErrorTracer
{
}

impl IntoOldVmTracer for CallErrorTracer {
    fn old_tracer(&self) -> OldTracers {
        todo!()
    }
}
