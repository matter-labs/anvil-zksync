use anvil_zksync_common::address_map::{ContractType, KNOWN_ADDRESSES};
use colored::Colorize;
use zksync_types::H160;

/// Converts a raw Ethereum address to a human-readable format.
/// 
/// If the address is known (such as a system contract, precompile, or popular contract),
/// this function returns a formatted string with the name and address. Otherwise, returns None.
///
/// # Arguments
///
/// * `address` - The Ethereum address to format
///
/// # Returns
///
/// * `Option<String>` - A formatted string or None if the address is not known
pub fn address_to_human_readable(address: H160) -> Option<String> {
    format_known_address(address)
}

/// Formats a known address with appropriate styling based on its contract type.
///
/// # Arguments
///
/// * `address` - The Ethereum address to format
///
/// # Returns
///
/// * `Option<String>` - A colored string representation of the address if known, None otherwise
fn format_known_address(address: H160) -> Option<String> {
    KNOWN_ADDRESSES.get(&address).map(|known_address| {
        let name = match known_address.contract_type {
            ContractType::System => known_address.name.bold().bright_blue(),
            ContractType::Precompile => known_address.name.bold().magenta(),
            ContractType::Popular => known_address.name.bold().bright_green(),
            ContractType::Unknown => known_address.name.dimmed(),
        }
        .to_string();

        let formatted_address = format!("{address:#x}").dimmed();
        format!("{name}{at}{formatted_address}", at = "@".dimmed())
    })
}
