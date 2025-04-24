use alloy::primitives::utils::format_ether;
use alloy::primitives::utils::format_units;
use zksync_types::U256;

/// Formats a `U256` value as Ether without capping decimal points.
pub fn format_eth(value: U256) -> String {
    let value = alloy::primitives::U256::from_limbs(value.0);
    let var_name = format!("{} ETH", format_ether(value));
    var_name
}
/// Formats a `U256` value as Gwei without capping decimal points.
pub fn format_gwei(value: U256) -> String {
    let value = alloy::primitives::U256::from_limbs(value.0);
    format!("{:.8} gwei", format_units(value, "gwei").unwrap())
}
