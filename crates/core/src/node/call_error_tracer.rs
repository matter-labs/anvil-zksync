use once_cell::sync::OnceCell;
use std::sync::Arc;
use zksync_multivm::interface::storage::WriteStorage;
use zksync_multivm::{
    tracers::dynamic::vm_1_5_0::DynTracer,
    vm_latest::{HistoryMode, SimpleMemory, VmTracer},
    zk_evm_latest::{
        tracing::{AfterDecodingData, VmLocalStateData},
        vm_state::ErrorFlags,
    },
};

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
