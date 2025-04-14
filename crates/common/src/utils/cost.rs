/// Calculates the cost of a transaction in ETH.
pub fn calculate_eth_cost(gas_price_in_wei_per_gas: u64, gas_used: u64) -> f64 {
    // Convert gas price from wei to gwei
    let gas_price_in_gwei = gas_price_in_wei_per_gas as f64 / 1e9;

    // Calculate total cost in gwei
    let total_cost_in_gwei = gas_price_in_gwei * gas_used as f64;

    // Convert total cost from gwei to ETH
    total_cost_in_gwei / 1e9
}
