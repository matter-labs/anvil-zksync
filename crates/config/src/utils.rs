use zksync_types::U256;

// TODO: look to remove in favour of alloy-primitives utils
/// Formats a `U256` value as Ether without capping decimal points.
pub fn format_eth(value: U256) -> String {
    let wei_per_eth = U256::from(10).pow(U256::from(18));
    let whole_eth = value / wei_per_eth;
    let remainder_wei = value % wei_per_eth;
    let fractional_eth = remainder_wei.as_u128() as f64 / 1e18;

    format!("{} ETH", whole_eth.as_u128() as f64 + fractional_eth)
}
// TODO: look to remove in favour of alloy-primitives utils
/// Formats a `U256` value as Gwei without capping decimal points.
pub fn format_gwei(value: U256) -> String {
    let gwei_value = value / U256::exp10(9);
    let fractional = value % U256::exp10(9);

    let fractional_part = fractional.as_u128() as f64 / 1e9;
    let full_gwei = gwei_value.as_u128() as f64 + fractional_part;

    format!("{:.8} gwei", full_gwei)
}
