use std::sync::{Arc, RwLock};
use zksync_multivm::{
    interface::tracer::VmExecutionStopReason, tracers::dynamic::vm_1_5_0::DynTracer,
    IntoOldVmTracer,
};

use zksync_multivm::interface::storage::WriteStorage;
use zksync_multivm::tracers::old::OldTracers;
use zksync_multivm::vm_latest::{
    constants::BOOTLOADER_HEAP_PAGE, BootloaderState, HistoryMode, SimpleMemory, VmTracer,
    ZkSyncVmState,
};
use zksync_types::U256;

/// Magic value that we put in bootloader.yul at the beginning of the debug section - to detect that
/// debugger was enabled.
const DEBUG_START_SENTINEL: u64 = 1337;

// Taken from bootloader.yul (MAX_MEM_SIZE)
const MAX_MEMORY_BYTES: usize = 63_800_000;

// Taken from Systemconfig.json
const MAX_TRANSACTIONS: usize = 10000;

const RESULTS_BYTES_OFFSET: usize = MAX_MEMORY_BYTES - MAX_TRANSACTIONS * 32;

const VM_HOOKS_PARAMS: usize = 3;

const VM_HOOKS_START: usize = RESULTS_BYTES_OFFSET - (VM_HOOKS_PARAMS + 1) * 32;

const DEBUG_SLOTS: usize = 32;
const DEBUG_START_BYTE: usize = VM_HOOKS_START - DEBUG_SLOTS * 32;

const DEBUG_START_SLOT: usize = DEBUG_START_BYTE / 32;

/// Struct that represents the additional debug information that we can get from bootloader.
/// Bootloader puts them in a special memory region after each transaction, and we can load them with this struct.

#[derive(Debug, Clone)]
pub struct BootloaderDebug {
    /// Amount of gas that user attached to the transaction.
    pub total_gas_limit_from_user: U256,
    /// If provided more gas than the system can support. (this 'reserved gas' will not be used and simply refunded at the end).
    pub reserved_gas: U256,
    /// Amount of gas that user has to pay for each pubdata byte.
    pub gas_per_pubdata: U256,
    /// Amount of gas left after intrinsic (block creation) fees.
    pub gas_limit_after_intrinsic: U256,
    /// Amount of gas left after account validation.
    pub gas_after_validation: U256,
    /// Amount of gas spent on actual function execution.
    pub gas_spent_on_execution: U256,

    /// Gas spent on factory dependencies and bytecode preparation.
    pub gas_spent_on_bytecode_preparation: U256,

    /// Amount of refund computed by the system.
    pub refund_computed: U256,
    /// Amount of refund provided by the operator (it might be larger than refund computed - for example due to pubdata compression).
    pub refund_by_operator: U256,

    /// Fixed amount of gas for each transaction.
    pub intrinsic_overhead: U256,

    // Closing a block has a non-trivial cost for the operator (they have to run the prover, and commit results to L1).
    // That's why we have to judge how much a given transaction is contributing the operator closer to sealing the block.
    /// The maximum amount that operator could have requested.
    pub required_overhead: U256,

    /// How much did operator request for the block.
    pub operator_overhead: U256,

    /// The amount of the overhead that transaction length it responsible for.
    pub overhead_for_length: U256,
    /// The amount of the overhead that simply using a slot of the block is responsible for.
    pub overhead_for_slot: U256,
}

/// The role of this tracer is to read the memory slots directly from bootloader memory at
/// the end of VM execution - and put them into BootloaderDebug object.
#[derive(Debug, Clone)]
pub struct BootloaderDebugTracer {
    pub result: Arc<RwLock<Result<BootloaderDebug, String>>>,
}

impl BootloaderDebugTracer {
    pub fn new(result: Arc<RwLock<Result<BootloaderDebug, String>>>) -> Self {
        Self { result }
    }
}

impl<S, H: HistoryMode> DynTracer<S, SimpleMemory<H>> for BootloaderDebugTracer {}

fn load_debug_slot<H: HistoryMode>(memory: &SimpleMemory<H>, slot: usize) -> U256 {
    memory
        .read_slot(BOOTLOADER_HEAP_PAGE as usize, DEBUG_START_SLOT + slot)
        .value
}

impl<S: WriteStorage, H: HistoryMode> VmTracer<S, H> for BootloaderDebugTracer {
    fn after_vm_execution(
        &mut self,
        state: &mut ZkSyncVmState<S, H>,
        _bootloader_state: &BootloaderState,
        _stop_reason: VmExecutionStopReason,
    ) {
        *self.result.write().unwrap() = BootloaderDebug::load_from_memory(&state.memory);
    }
}

impl BootloaderDebug {
    pub fn load_from_memory<H: HistoryMode>(memory: &SimpleMemory<H>) -> Result<Self, String> {
        if load_debug_slot(memory, 0) != U256::from(DEBUG_START_SENTINEL) {
            Err(
                "Debug slot has wrong value. Probably bootloader slot mapping has changed."
                    .to_owned(),
            )
        } else {
            Ok(BootloaderDebug {
                total_gas_limit_from_user: load_debug_slot(memory, 1),
                reserved_gas: load_debug_slot(memory, 2),
                gas_per_pubdata: load_debug_slot(memory, 3),
                gas_limit_after_intrinsic: load_debug_slot(memory, 4),
                gas_after_validation: load_debug_slot(memory, 5),
                gas_spent_on_execution: load_debug_slot(memory, 6),
                gas_spent_on_bytecode_preparation: load_debug_slot(memory, 7),
                refund_computed: load_debug_slot(memory, 8),
                refund_by_operator: load_debug_slot(memory, 9),
                intrinsic_overhead: load_debug_slot(memory, 10),
                operator_overhead: load_debug_slot(memory, 11),
                required_overhead: load_debug_slot(memory, 12),
                overhead_for_length: load_debug_slot(memory, 13),
                overhead_for_slot: load_debug_slot(memory, 14),
            })
        }
    }
}

//
// The rest of the file contains stub tracer implementations for older VM versions.
// Reasoning: `BootloaderDebugTracer` needs to implement `MultiVmTracer` to be compatible with era
// abstractions such as `BatchExecutor` and `BatchExecutorFactory`.
//

impl<S, H: zksync_multivm::vm_1_4_1::HistoryMode>
    zksync_multivm::tracers::dynamic::vm_1_4_1::DynTracer<
        S,
        zksync_multivm::vm_1_4_1::SimpleMemory<H>,
    > for BootloaderDebugTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_1_4_1::HistoryMode>
    zksync_multivm::vm_1_4_1::VmTracer<S, H> for BootloaderDebugTracer
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
    > for BootloaderDebugTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_1_4_2::HistoryMode>
    zksync_multivm::vm_1_4_2::VmTracer<S, H> for BootloaderDebugTracer
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
    > for BootloaderDebugTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_boojum_integration::HistoryMode>
    zksync_multivm::vm_boojum_integration::VmTracer<S, H> for BootloaderDebugTracer
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
    > for BootloaderDebugTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_refunds_enhancement::HistoryMode>
    zksync_multivm::vm_refunds_enhancement::VmTracer<S, H> for BootloaderDebugTracer
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
    > for BootloaderDebugTracer
{
}

impl<H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::ExecutionEndTracer<H> for BootloaderDebugTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::ExecutionProcessing<S, H> for BootloaderDebugTracer
{
}

impl<S: WriteStorage, H: zksync_multivm::vm_virtual_blocks::HistoryMode>
    zksync_multivm::vm_virtual_blocks::VmTracer<S, H> for BootloaderDebugTracer
{
}

impl IntoOldVmTracer for BootloaderDebugTracer {
    fn old_tracer(&self) -> OldTracers {
        todo!()
    }
}
