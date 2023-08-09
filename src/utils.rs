use std::pin::Pin;

use futures::Future;
use vm::vm_with_bootloader::{
    derive_base_fee_and_gas_per_pubdata, BLOCK_OVERHEAD_GAS, BLOCK_OVERHEAD_PUBDATA,
    BOOTLOADER_TX_ENCODING_SPACE,
};
use zksync_basic_types::U256;
use zksync_types::{zk_evm::zkevm_opcode_defs::system_params::MAX_TX_ERGS_LIMIT, MAX_TXS_IN_BLOCK};
use zksync_utils::ceil_div_u256;

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

    vec![
        (0.1 * overhead_for_single_instance_circuits.as_u32() as f64).floor() as u32,
        overhead_for_length.as_u32(),
        tx_slot_overhead.as_u32(),
    ]
    .into_iter()
    .max()
    .unwrap()
}

pub fn block_overhead_gas(gas_per_pubdata_byte: u32) -> u32 {
    BLOCK_OVERHEAD_GAS + BLOCK_OVERHEAD_PUBDATA * gas_per_pubdata_byte
}

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
