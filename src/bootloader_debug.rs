use vm::{HistoryMode, VmInstance};
use zksync_basic_types::U256;
use zksync_types::zk_evm::zkevm_opcode_defs::BOOTLOADER_HEAP_PAGE;

const MAX_MEMORY_BYTES: usize = usize::pow(2, 24);

const MAX_TRANSACTIONS: usize = 1024;
const RESULTS_BYTES_OFFSET: usize = MAX_MEMORY_BYTES - MAX_TRANSACTIONS * 32;

const VM_HOOKS_PARAMS: usize = 2;

const VM_HOOKS_START: usize = RESULTS_BYTES_OFFSET - (VM_HOOKS_PARAMS + 1) * 32;

const DEBUG_SLOTS: usize = 32;
const DEBUG_START_BYTE: usize = VM_HOOKS_START - DEBUG_SLOTS * 32;

const DEBUG_START_SLOT: usize = DEBUG_START_BYTE / 32;

/// Struct that represents the additional debug information that we can get from bootloader.
/// Bootloader puts them in a special memory region after each transaction, and we can load them with this struct.
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
}

fn load_debug_slot<'a, H: HistoryMode>(vm: &Box<VmInstance<'a, H>>, slot: usize) -> U256 {
    vm.state
        .memory
        .memory
        .inner()
        .read_slot(BOOTLOADER_HEAP_PAGE as usize, DEBUG_START_SLOT + slot)
        .value
}

impl BootloaderDebug {
    pub fn load_from_memory<'a, H: HistoryMode>(vm: &Box<VmInstance<'a, H>>) -> eyre::Result<Self> {
        if load_debug_slot(vm, 0) != U256::from(1337) {
            eyre::bail!("Debug slot has wrong value. Probably bootloader slot mapping has changed.")
        } else {
            Ok(BootloaderDebug {
                total_gas_limit_from_user: load_debug_slot(vm, 1),
                reserved_gas: load_debug_slot(vm, 2),
                gas_per_pubdata: load_debug_slot(vm, 3),
                gas_limit_after_intrinsic: load_debug_slot(vm, 4),
                gas_after_validation: load_debug_slot(vm, 5),
                gas_spent_on_execution: load_debug_slot(vm, 6),
                gas_spent_on_bytecode_preparation: load_debug_slot(vm, 7),
                refund_computed: load_debug_slot(vm, 8),
                refund_by_operator: load_debug_slot(vm, 9),
            })
        }
    }
}
