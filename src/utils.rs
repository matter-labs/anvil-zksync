use std::pin::Pin;

use futures::Future;
use vm::{
    utils::BLOCK_GAS_LIMIT,
    vm_with_bootloader::{
        derive_base_fee_and_gas_per_pubdata, init_vm_inner, BlockContext, BlockContextMode,
        BootloaderJobType, TxExecutionMode, BLOCK_OVERHEAD_GAS, BLOCK_OVERHEAD_PUBDATA,
        BOOTLOADER_TX_ENCODING_SPACE,
    },
    HistoryEnabled, OracleTools,
};
use zksync_basic_types::{H256, U256};
use zksync_state::StorageView;
use zksync_state::WriteStorage;
use zksync_types::{
    api::Block, zk_evm::zkevm_opcode_defs::system_params::MAX_TX_ERGS_LIMIT, MAX_TXS_IN_BLOCK,
};
use zksync_utils::{ceil_div_u256, u256_to_h256};

use crate::{
    fork::{ForkSource, ForkStorage},
    node::{compute_hash, InMemoryNodeInner},
};

pub(crate) trait IntoBoxedFuture: Sized + Send + 'static {
    fn into_boxed_future(self) -> Pin<Box<dyn Future<Output = Self> + Send>> {
        Box::pin(async { self })
    }
}

impl<T, U> IntoBoxedFuture for Result<T, U>
where
    T: Send + 'static,
    U: Send + 'static,
{
}

/// Derives the gas estimation overhead based on the given gas limit, gas price per pubdata, and encoded length.
///
/// # Arguments
///
/// * `gas_limit` - A `u32` representing the gas limit.
/// * `gas_price_per_pubdata` - A `u32` representing the gas price per pubdata.
/// * `encoded_len` - A `usize` representing the encoded length.
///
/// # Returns
///
/// A `u32` representing the derived gas estimation overhead.
pub fn derive_gas_estimation_overhead(
    gas_limit: u32,
    gas_price_per_pubdata: u32,
    encoded_len: usize,
) -> u32 {
    // Even if the gas limit is greater than the MAX_TX_ERGS_LIMIT, we assume that everything beyond MAX_TX_ERGS_LIMIT
    // will be spent entirely on publishing bytecodes and so we derive the overhead solely based on the capped value
    let gas_limit = std::cmp::min(MAX_TX_ERGS_LIMIT, gas_limit);

    // Using large U256 type to avoid overflow
    let max_block_overhead = U256::from(block_overhead_gas(gas_price_per_pubdata));
    let gas_limit = U256::from(gas_limit);
    let encoded_len = U256::from(encoded_len);

    // The MAX_TX_ERGS_LIMIT is formed in a way that may fullfills a single-instance circuits
    // if used in full. That is, within MAX_TX_ERGS_LIMIT it is possible to fully saturate all the single-instance
    // circuits.
    let overhead_for_single_instance_circuits =
        ceil_div_u256(gas_limit * max_block_overhead, MAX_TX_ERGS_LIMIT.into());

    // The overhead for occupying the bootloader memory
    let overhead_for_length = ceil_div_u256(
        encoded_len * max_block_overhead,
        BOOTLOADER_TX_ENCODING_SPACE.into(),
    );

    // The overhead for occupying a single tx slot
    let tx_slot_overhead = ceil_div_u256(max_block_overhead, MAX_TXS_IN_BLOCK.into());

    // For L2 transactions we allow a certain default discount with regard to the number of ergs.
    // Multiinstance circuits can in theory be spawned infinite times, while projected future limitations
    // on gas per pubdata allow for roughly 800k gas per L1 batch, so the rough trust "discount" on the proof's part
    // to be paid by the users is 0.1.
    const ERGS_LIMIT_OVERHEAD_COEFFICIENT: f64 = 0.1;

    vec![
        (ERGS_LIMIT_OVERHEAD_COEFFICIENT * overhead_for_single_instance_circuits.as_u32() as f64)
            .floor() as u32,
        overhead_for_length.as_u32(),
        tx_slot_overhead.as_u32(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

/// Calculates the total gas cost of the block overhead, including the gas cost of the public data.
///
/// # Arguments
///
/// * `gas_per_pubdata_byte` - The gas cost per byte of public data.
///
/// # Returns
///
/// The total gas cost of the block overhead, including the gas cost of the public data.
pub fn block_overhead_gas(gas_per_pubdata_byte: u32) -> u32 {
    BLOCK_OVERHEAD_GAS + BLOCK_OVERHEAD_PUBDATA * gas_per_pubdata_byte
}

/// Adjusts the L1 gas price for a transaction based on the current pubdata price and the fair L2 gas price.
/// If the current pubdata price is small enough, returns the original L1 gas price.
/// Otherwise, calculates a new L1 gas price based on the fair L2 gas price and the transaction gas per pubdata limit.
///
/// # Arguments
///
/// * `l1_gas_price` - The original L1 gas price.
/// * `fair_l2_gas_price` - The fair L2 gas price.
/// * `tx_gas_per_pubdata_limit` - The transaction gas per pubdata limit.
///
/// # Returns
///
/// The adjusted L1 gas price.
pub fn adjust_l1_gas_price_for_tx(
    l1_gas_price: u64,
    fair_l2_gas_price: u64,
    tx_gas_per_pubdata_limit: U256,
) -> u64 {
    let (_, current_pubdata_price) =
        derive_base_fee_and_gas_per_pubdata(l1_gas_price, fair_l2_gas_price);
    if U256::from(current_pubdata_price) <= tx_gas_per_pubdata_limit {
        // The current pubdata price is small enough
        l1_gas_price
    } else {
        let l1_gas_price = U256::from(fair_l2_gas_price)
            * (tx_gas_per_pubdata_limit - U256::from(1u32))
            / U256::from(17);

        l1_gas_price.as_u64()
    }
}

/// Takes long integers and returns them in human friendly format with "_".
/// For example: 12_334_093
pub fn to_human_size(input: U256) -> String {
    let input = format!("{:?}", input);
    let tmp: Vec<_> = input
        .chars()
        .rev()
        .enumerate()
        .flat_map(|(index, val)| {
            if index > 0 && index % 3 == 0 {
                vec!['_', val]
            } else {
                vec![val]
            }
        })
        .collect();
    tmp.iter().rev().collect()
}

/// Creates and inserts a given number of empty blocks into the node, with a given interval between them.
/// The blocks will be empty (contain no transactions).
/// The test system contracts will be used to force overwriting the block number and timestamp in VM state,
/// otherwise the VM will reject subsequent blocks as invalid.
pub fn mine_empty_blocks<S: std::fmt::Debug + ForkSource>(
    node: &mut InMemoryNodeInner<S>,
    num_blocks: u64,
    interval_ms: u64,
) {
    // build and insert new blocks
    for _ in 0..num_blocks {
        node.current_miniblock = node.current_miniblock.saturating_add(1);

        let block = Block {
            hash: compute_hash(node.current_miniblock as u32, H256::zero()),
            number: node.current_miniblock.into(),
            timestamp: node.current_timestamp.into(),
            ..Default::default()
        };

        node.block_hashes.insert(node.current_miniblock, block.hash);
        node.blocks.insert(block.hash, block);

        // leave node state ready for next interaction
        node.current_timestamp = node.current_timestamp.saturating_add(interval_ms);
    }

    // roll the vm
    let (keys, bytecodes) = {
        let mut storage_view: StorageView<&ForkStorage<S>> = StorageView::new(&node.fork_storage);
        let mut oracle_tools = OracleTools::new(&mut storage_view, HistoryEnabled);

        // system_contract.contacts_for_l2_call() will give playground contracts
        // we need these to use the unsafeOverrideBlock method in SystemContext.sol
        let bootloader_code = node.system_contracts.contacts_for_l2_call();
        let block_context = BlockContext {
            block_number: node.current_miniblock as u32,
            block_timestamp: node.current_timestamp,
            ..node.create_block_context()
        };
        let block_properties: zksync_types::zk_evm::block_properties::BlockProperties =
            InMemoryNodeInner::<S>::create_block_properties(bootloader_code);

        // init vm
        let mut vm = init_vm_inner(
            &mut oracle_tools,
            BlockContextMode::OverrideCurrent(block_context.into()),
            &block_properties,
            BLOCK_GAS_LIMIT,
            bootloader_code,
            TxExecutionMode::VerifyExecute,
        );

        vm.execute_till_block_end(BootloaderJobType::TransactionExecution);

        let bytecodes = vm
            .state
            .decommittment_processor
            .known_bytecodes
            .inner()
            .clone();

        let modified_keys = storage_view.modified_storage_keys().clone();
        (modified_keys, bytecodes)
    };

    for (key, value) in keys.iter() {
        node.fork_storage.set_value(*key, *value);
    }

    // Write all the factory deps.
    for (hash, code) in bytecodes.iter() {
        node.fork_storage.store_factory_dep(
            u256_to_h256(*hash),
            code.iter()
                .flat_map(|entry| {
                    let mut bytes = vec![0u8; 32];
                    entry.to_big_endian(&mut bytes);
                    bytes.to_vec()
                })
                .collect(),
        )
    }

    // increment batch
    node.current_batch = node.current_batch.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use zksync_basic_types::U256;

    use crate::utils::to_human_size;

    #[test]
    fn test_human_sizes() {
        assert_eq!("123", to_human_size(U256::from(123u64)));
        assert_eq!("1_234", to_human_size(U256::from(1234u64)));
        assert_eq!("12_345", to_human_size(U256::from(12345u64)));
        assert_eq!("0", to_human_size(U256::from(0)));
        assert_eq!("1", to_human_size(U256::from(1)));
        assert_eq!("250_000_000", to_human_size(U256::from(250000000u64)));
    }
}
