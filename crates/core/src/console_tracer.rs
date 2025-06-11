use std::sync::{Arc, RwLock};
use zksync_multivm::zk_evm_latest::tracing::{
    AfterExecutionData, BeforeExecutionData, VmLocalStateData,
};
use zksync_multivm::zk_evm_latest::u256_to_address_unchecked;
use zksync_multivm::zk_evm_latest::vm_state::PrimitiveValue;
use zksync_multivm::zk_evm_latest::zkevm_opcode_defs::{
    FarCallABI, FarCallOpcode, FatPointer, Opcode, CALL_IMPLICIT_CALLDATA_FAT_PTR_REGISTER,
};
use zksync_multivm::{
    interface::tracer::VmExecutionStopReason, tracers::dynamic::vm_1_5_2::DynTracer,
    IntoOldVmTracer,
};

use zksync_multivm::interface::storage::{StoragePtr, WriteStorage};
use zksync_multivm::tracers::old::OldTracers;
use zksync_multivm::vm_latest::{
    constants::BOOTLOADER_HEAP_PAGE, BootloaderState, HistoryMode, SimpleMemory, VmTracer,
    ZkSyncVmState,
};
use zksync_types::{Address, U256};

#[derive(Debug, Clone)]
pub struct ConsoleLogTracer;

impl<S, H: HistoryMode> DynTracer<S, SimpleMemory<H>> for ConsoleLogTracer {
    fn after_execution(
        &mut self,
        state: VmLocalStateData<'_>,
        data: AfterExecutionData,
        memory: &SimpleMemory<H>,
        _storage: StoragePtr<S>,
    ) {
        if data.opcode.inner.variant.opcode != Opcode::FarCall(FarCallOpcode::Normal) {
            return;
        }
        // TODO move to the const
        let console_log_addr = Address::from_slice(
            hex::decode("c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0")
                .unwrap()
                .as_slice(),
        );

        let current = state.vm_local_state.callstack.current;
        if current.this_address != console_log_addr {
            return;
        }

        let calldata = if current.code_page.0 == 0 || current.ergs_remaining == 0 {
            vec![]
        } else {
            let packed_abi =
                state.vm_local_state.registers[CALL_IMPLICIT_CALLDATA_FAT_PTR_REGISTER as usize];
            assert!(packed_abi.is_pointer);
            let far_call_abi = FarCallABI::from_u256(packed_abi.value);
            memory.read_unaligned_bytes(
                far_call_abi.memory_quasi_fat_pointer.memory_page as usize,
                far_call_abi.memory_quasi_fat_pointer.start as usize,
                far_call_abi.memory_quasi_fat_pointer.length as usize,
            )
        };

        if let Ok(val) = String::from_utf8(calldata.clone()) {
            println!("Console log: {}", val);
        } else {
            println!("Console log: {}", hex::encode(calldata));
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmTracer<S, H> for ConsoleLogTracer {}

//
// The rest of the file contains stub tracer implementations for older VM versions.
// Reasoning: `BootloaderDebugTracer` needs to implement `MultiVmTracer` to be compatible with era
// abstractions such as `BatchExecutor` and `BatchExecutorFactory`.
//

impl<S, H: zksync_multivm::vm_1_4_1::HistoryMode>
    zksync_multivm::tracers::dynamic::vm_1_4_1::DynTracer<
        S,
        zksync_multivm::vm_1_4_1::SimpleMemory<H>,
    > for ConsoleLogTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_1_4_1::HistoryMode>
    zksync_multivm::vm_1_4_1::VmTracer<S, H> for ConsoleLogTracer
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
    > for ConsoleLogTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_1_4_2::HistoryMode>
    zksync_multivm::vm_1_4_2::VmTracer<S, H> for ConsoleLogTracer
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
    > for ConsoleLogTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_boojum_integration::HistoryMode>
    zksync_multivm::vm_boojum_integration::VmTracer<S, H> for ConsoleLogTracer
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
    > for ConsoleLogTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_refunds_enhancement::HistoryMode>
    zksync_multivm::vm_refunds_enhancement::VmTracer<S, H> for ConsoleLogTracer
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
    > for ConsoleLogTracer
{
}

impl<H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::ExecutionEndTracer<H> for ConsoleLogTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::ExecutionProcessing<S, H> for ConsoleLogTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::VmTracer<S, H> for ConsoleLogTracer
{
}

impl IntoOldVmTracer for ConsoleLogTracer {
    fn old_tracer(&self) -> OldTracers {
        todo!()
    }
}
