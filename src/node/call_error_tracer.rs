use zksync_multivm::interface::storage::WriteStorage;
use zksync_multivm::{
    tracers::dynamic::vm_1_5_0::DynTracer,
    vm_latest::{HistoryMode, SimpleMemory, VmTracer},
    zk_evm_latest::{
        tracing::{AfterDecodingData, VmLocalStateData},
        vm_state::ErrorFlags,
    },
};

pub struct CallErrorTracer {}

impl CallErrorTracer {
    pub fn new() -> Self {
        Self {}
    }
}

impl<S, H: HistoryMode> DynTracer<S, SimpleMemory<H>> for CallErrorTracer {
    fn after_decoding(
        &mut self,
        _state: VmLocalStateData<'_>,
        data: AfterDecodingData,
        _memory: &SimpleMemory<H>,
    ) {
        if !data.error_flags_accumulated.is_empty() {
            tracing::error!("!! Got error flags: ");
            if data
                .error_flags_accumulated
                .contains(ErrorFlags::INVALID_OPCODE)
            {
                tracing::error!("INVALID OPCODE");
            }
            if data
                .error_flags_accumulated
                .contains(ErrorFlags::NOT_ENOUGH_ERGS)
            {
                tracing::error!("NOT ENOUGH ERGS");
            }
            if data
                .error_flags_accumulated
                .contains(ErrorFlags::PRIVILAGED_ACCESS_NOT_FROM_KERNEL)
            {
                tracing::error!("PRIVILEGED ACCESS");
            }
            if data
                .error_flags_accumulated
                .contains(ErrorFlags::WRITE_IN_STATIC_CONTEXT)
            {
                tracing::error!("WRITE IN STATIC");
            }
            if data
                .error_flags_accumulated
                .contains(ErrorFlags::CALLSTACK_IS_FULL)
            {
                tracing::error!("CALLSTACK IS FULL");
            }
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmTracer<S, H> for CallErrorTracer {}
