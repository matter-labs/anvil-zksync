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

    /// Fixed amount of gas for each transaction.
    pub intrinsic_overhead: U256,

    /// Closing a block has a non-trivial cost for the operator (they have to run the prover, and commit results to L1).
    /// That's why we have to judge how much a given transaction is contributing the operator closer to sealing the block.

    /// Cost of the whole block for the operator.
    pub total_overhead_for_block: U256,

    /// The maximum amount that operator could have requested.
    pub required_overhead: U256,

    /// How much did operator request for the block.
    pub operator_overhead: U256,

    /// The amount of the overhead that circuits / gas are responsible for.
    pub overhead_for_circuits: U256,
    /// The amount of the overhead that transaction length it responsible for.
    pub overhead_for_length: U256,
    /// The amount of the overhead that simply using a slot of the block is responsible for.
    pub overhead_for_slot: U256,
}

fn load_debug_slot<H: HistoryMode>(vm: &VmInstance<H>, slot: usize) -> U256 {
    vm.state
        .memory
        .memory
        .inner()
        .read_slot(BOOTLOADER_HEAP_PAGE as usize, DEBUG_START_SLOT + slot)
        .value
}

impl BootloaderDebug {
    pub fn load_from_memory<H: HistoryMode>(vm: &VmInstance<H>) -> eyre::Result<Self> {
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
                intrinsic_overhead: load_debug_slot(vm, 10),
                operator_overhead: load_debug_slot(vm, 11),
                required_overhead: load_debug_slot(vm, 12),
                total_overhead_for_block: load_debug_slot(vm, 13),
                overhead_for_circuits: load_debug_slot(vm, 14),
                overhead_for_length: load_debug_slot(vm, 15),
                overhead_for_slot: load_debug_slot(vm, 16),
            })
        }
    }
}
