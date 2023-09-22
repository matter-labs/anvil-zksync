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
use zksync_basic_types::{H256, U256, U64};
use zksync_state::StorageView;
use zksync_state::WriteStorage;
use zksync_types::{
    api::{Block, BlockNumber},
    zk_evm::zkevm_opcode_defs::system_params::MAX_TX_ERGS_LIMIT,
    MAX_TXS_IN_BLOCK,
};
use zksync_utils::{ceil_div_u256, u256_to_h256};

use crate::{
    fork::{ForkSource, ForkStorage},
    node::{compute_hash, InMemoryNodeInner},
};
use vm::utils::fee::derive_base_fee_and_gas_per_pubdata;

use zksync_basic_types::U256;
use zksync_utils::{bytecode::hash_bytecode, bytes_to_be_words};

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

pub fn bytecode_to_factory_dep(bytecode: Vec<u8>) -> (U256, Vec<U256>) {
    let bytecode_hash = hash_bytecode(&bytecode);
    let bytecode_hash = U256::from_big_endian(bytecode_hash.as_bytes());

    let bytecode_words = bytes_to_be_words(bytecode);

    (bytecode_hash, bytecode_words)
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

/// Returns the actual [U64] block number from [BlockNumber].
///
/// # Arguments
///
/// * `block_number` - [BlockNumber] for a block.
/// * `latest_block_number` - A [U64] representing the latest block number.
///
/// # Returns
///
/// A [U64] representing the input block number.
pub fn to_real_block_number(block_number: BlockNumber, latest_block_number: U64) -> U64 {
    match block_number {
        BlockNumber::Finalized
        | BlockNumber::Pending
        | BlockNumber::Committed
        | BlockNumber::Latest => latest_block_number,
        BlockNumber::Earliest => U64::zero(),
        BlockNumber::Number(n) => n,
    }
}

#[cfg(test)]
mod tests {
    use zksync_basic_types::U256;

    use super::*;

    #[test]
    fn test_human_sizes() {
        assert_eq!("123", to_human_size(U256::from(123u64)));
        assert_eq!("1_234", to_human_size(U256::from(1234u64)));
        assert_eq!("12_345", to_human_size(U256::from(12345u64)));
        assert_eq!("0", to_human_size(U256::from(0)));
        assert_eq!("1", to_human_size(U256::from(1)));
        assert_eq!("250_000_000", to_human_size(U256::from(250000000u64)));
    }

    #[test]
    fn test_to_real_block_number_finalized() {
        let actual = to_real_block_number(BlockNumber::Finalized, U64::from(10));
        assert_eq!(U64::from(10), actual);
    }

    #[test]
    fn test_to_real_block_number_pending() {
        let actual = to_real_block_number(BlockNumber::Pending, U64::from(10));
        assert_eq!(U64::from(10), actual);
    }

    #[test]
    fn test_to_real_block_number_committed() {
        let actual = to_real_block_number(BlockNumber::Committed, U64::from(10));
        assert_eq!(U64::from(10), actual);
    }

    #[test]
    fn test_to_real_block_number_latest() {
        let actual = to_real_block_number(BlockNumber::Latest, U64::from(10));
        assert_eq!(U64::from(10), actual);
    }

    #[test]
    fn test_to_real_block_number_earliest() {
        let actual = to_real_block_number(BlockNumber::Earliest, U64::from(10));
        assert_eq!(U64::zero(), actual);
    }

    #[test]
    fn test_to_real_block_number_number() {
        let actual = to_real_block_number(BlockNumber::Number(U64::from(5)), U64::from(10));
        assert_eq!(U64::from(5), actual);
    }
}
