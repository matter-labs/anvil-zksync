use std::pin::Pin;

use futures::Future;
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
