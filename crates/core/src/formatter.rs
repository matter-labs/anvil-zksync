//! Helper methods to display transaction data in more human readable way.
use crate::bootloader_debug::BootloaderDebug;
use crate::resolver;
use crate::utils::block_on;
use crate::utils::{calculate_eth_cost, to_human_size};
use anvil_zksync_config::utils::format_gwei;
use anvil_zksync_types::ShowCalls;
use colored::Colorize;
use futures::future::join_all;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::{collections::HashMap, str};
use zksync_multivm::interface::{Call, VmEvent, VmExecutionResultAndLogs};
use zksync_types::{
    fee_model::FeeModelConfigV2, Address, StorageLogWithPreviousValue, Transaction, H160, H256,
    U256,
};
use zksync_error::documentation::Documented;
use zksync_error::error::{CustomErrorMessage, NamedError};
use zksync_error_description::ErrorDocumentation;
use zksync_types::l2::L2Tx;
use alloy::hex::ToHexExt;

// @dev elected to have GasDetails struct as we can do more with it in the future
// We can provide more detailed understanding of gas errors and gas usage
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct GasDetails {
    total_gas_limit: U256,
    intrinsic_gas: U256,
    gas_for_validation: U256,
    gas_spent_on_compute: U256,
    gas_used: U256,
    bytes_published: u64,
    spent_on_pubdata: u64,
    gas_spent_on_bytecode_preparation: U256,
    refund_computed: U256,
    refund_by_operator: U256,
    required_overhead: U256,
    operator_overhead: U256,
    intrinsic_overhead: U256,
    overhead_for_length: U256,
    overhead_for_slot: U256,
    gas_per_pubdata: U256,
    total_gas_limit_from_user: U256,
    gas_spent_on_execution: U256,
    gas_limit_after_intrinsic: U256,
    gas_after_validation: U256,
    reserved_gas: U256,
}

/// Computes the gas details for the transaction to be displayed.
pub fn compute_gas_details(
    bootloader_debug: &BootloaderDebug,
    spent_on_pubdata: u64,
) -> GasDetails {
    let total_gas_limit = bootloader_debug
        .total_gas_limit_from_user
        .saturating_sub(bootloader_debug.reserved_gas);
    let intrinsic_gas = total_gas_limit - bootloader_debug.gas_limit_after_intrinsic;
    let gas_for_validation =
        bootloader_debug.gas_limit_after_intrinsic - bootloader_debug.gas_after_validation;
    let gas_spent_on_compute = bootloader_debug.gas_spent_on_execution
        - bootloader_debug.gas_spent_on_bytecode_preparation;
    let gas_used = intrinsic_gas
        + gas_for_validation
        + bootloader_debug.gas_spent_on_bytecode_preparation
        + gas_spent_on_compute;

    let bytes_published = spent_on_pubdata / bootloader_debug.gas_per_pubdata.as_u64();

    GasDetails {
        total_gas_limit,
        intrinsic_gas,
        gas_for_validation,
        gas_spent_on_compute,
        gas_used,
        bytes_published,
        spent_on_pubdata,
        gas_spent_on_bytecode_preparation: bootloader_debug.gas_spent_on_bytecode_preparation,
        refund_computed: bootloader_debug.refund_computed,
        refund_by_operator: bootloader_debug.refund_by_operator,
        required_overhead: bootloader_debug.required_overhead,
        operator_overhead: bootloader_debug.operator_overhead,
        intrinsic_overhead: bootloader_debug.intrinsic_overhead,
        overhead_for_length: bootloader_debug.overhead_for_length,
        overhead_for_slot: bootloader_debug.overhead_for_slot,
        gas_per_pubdata: bootloader_debug.gas_per_pubdata,
        total_gas_limit_from_user: bootloader_debug.total_gas_limit_from_user,
        gas_spent_on_execution: bootloader_debug.gas_spent_on_execution,
        gas_limit_after_intrinsic: bootloader_debug.gas_limit_after_intrinsic,
        gas_after_validation: bootloader_debug.gas_after_validation,
        reserved_gas: bootloader_debug.reserved_gas,
    }
}

/// Responsible for formatting the data in a structured log.
pub struct Formatter {
    sibling_stack: Vec<bool>,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    /// Creates a new formatter with an empty sibling stack.
    pub fn new() -> Self {
        Formatter {
            sibling_stack: Vec::new(),
        }
    }
    /// Logs a section with a title, applies a scoped function, and manages sibling hierarchy.
    pub fn section<F>(&mut self, title: &str, is_last_sibling: bool, f: F)
    where
        F: FnOnce(&mut Self),
    {
        self.format_log(is_last_sibling, title);
        self.enter_scope(is_last_sibling);
        f(self);
        self.exit_scope();
    }
    /// Logs a key-value item as part of the formatted output.
    pub fn item(&mut self, is_last_sibling: bool, key: &str, value: &str) {
        self.format_log(
            is_last_sibling,
            &format!("{}: {}", key.bold(), value.dimmed()),
        );
    }
    /// Enters a new scope for nested logging, tracking sibling relationships.
    pub fn enter_scope(&mut self, has_more_siblings: bool) {
        self.sibling_stack.push(has_more_siblings);
    }
    /// Exits the current logging scope, removing the last sibling marker.
    pub fn exit_scope(&mut self) {
        self.sibling_stack.pop();
    }
    /// Logs a formatted message with a hierarchical prefix.
    pub fn format_log(&self, is_last_sibling: bool, message: &str) {
        let prefix = build_prefix(&self.sibling_stack, is_last_sibling);
        tracing::info!("{}{}", prefix, message);
    }
    /// Logs a formatted error message with a hierarchical prefix.
    pub fn format_error(&self, is_last_sibling: bool, message: &str) {
        let prefix = build_prefix(&self.sibling_stack, is_last_sibling);
        tracing::info!("{}", format!("{}{}", prefix, message).red());
    }
    /// Prints gas details for the transaction in a structured log.
    pub fn print_gas_details(
        &mut self,
        gas_details: &GasDetails,
        fee_model_config: &FeeModelConfigV2,
    ) {
        let GasDetails {
            total_gas_limit,
            intrinsic_gas,
            gas_for_validation,
            gas_spent_on_compute,
            gas_used,
            bytes_published,
            spent_on_pubdata,
            gas_spent_on_bytecode_preparation,
            refund_computed,
            refund_by_operator,
            required_overhead: _required_overhead,
            operator_overhead,
            intrinsic_overhead,
            overhead_for_length,
            overhead_for_slot,
            gas_per_pubdata,
            total_gas_limit_from_user,
            ..
        } = *gas_details;

        self.section("[Gas Details]", true, |gas_details_section| {
            let mut total_items = 0;
            let mut warnings = Vec::new();

            // Prepare warnings
            if refund_computed != refund_by_operator {
                warnings.push(format!(
                    "WARNING: Refund by VM: {}, but operator refunded: {}",
                    to_human_size(refund_computed),
                    to_human_size(refund_by_operator)
                ));
            }

            if total_gas_limit_from_user != total_gas_limit {
                warnings.push(format!(
                    "WARNING: User provided more gas ({}), but system had a lower max limit.",
                    to_human_size(total_gas_limit_from_user)
                ));
            }

            // Calculate total items under [Gas Details]
            total_items += 1; // Gas Summary
            total_items += warnings.len(); // Warnings
            total_items += 1; // Execution Gas Breakdown
            total_items += 1; // Transaction Setup Cost Breakdown
            total_items += 1; // L1 Publishing Costs
            total_items += 1; // Block Contribution

            let mut item_index = 0;

            // Gas Summary
            let is_last_sibling = item_index == total_items - 1;
            gas_details_section.section("Gas Summary", is_last_sibling, |gas_summary_section| {
                let items = vec![
                    ("Limit", to_human_size(total_gas_limit)),
                    ("Used", to_human_size(gas_used)),
                    ("Refunded", to_human_size(refund_by_operator)),
                    ("Paid:", to_human_size(total_gas_limit - refund_by_operator)),
                ];

                let num_items = items.len();
                for (i, (key, value)) in items.into_iter().enumerate() {
                    let is_last_item = i == num_items - 1;
                    gas_summary_section.item(is_last_item, key, &value);
                }
            });
            item_index += 1;

            // warnings
            for warning in warnings {
                let is_last_sibling = item_index == total_items - 1;
                gas_details_section.format_error(is_last_sibling, &warning);
                item_index += 1;
            }

            // Execution Gas Breakdown
            let is_last_sibling = item_index == total_items - 1;
            gas_details_section.section(
                "Execution Gas Breakdown",
                is_last_sibling,
                |execution_breakdown_section| {
                    let gas_breakdown_items = vec![
                        (
                            "Transaction Setup",
                            intrinsic_gas,
                            intrinsic_gas * 100 / gas_used,
                        ),
                        (
                            "Bytecode Preparation",
                            gas_spent_on_bytecode_preparation,
                            gas_spent_on_bytecode_preparation * 100 / gas_used,
                        ),
                        (
                            "Account Validation",
                            gas_for_validation,
                            gas_for_validation * 100 / gas_used,
                        ),
                        (
                            "Computations (Opcodes)",
                            gas_spent_on_compute,
                            gas_spent_on_compute * 100 / gas_used,
                        ),
                    ];

                    let num_items = gas_breakdown_items.len();
                    for (i, (description, amount, percentage)) in
                        gas_breakdown_items.iter().enumerate()
                    {
                        let is_last_item = i == num_items - 1;
                        execution_breakdown_section.item(
                            is_last_item,
                            description,
                            &format!("{} gas ({:>2}%)", to_human_size(*amount), percentage),
                        );
                    }
                },
            );
            item_index += 1;

            // Transaction Setup Cost Breakdown
            let is_last_sibling = item_index == total_items - 1;
            gas_details_section.section(
                "Transaction Setup Cost Breakdown",
                is_last_sibling,
                |transaction_setup_section| {
                    let items = vec![
                        (
                            "Total Setup Cost",
                            format!("{} gas", to_human_size(intrinsic_gas)),
                        ),
                        (
                            "Fixed Cost",
                            format!(
                                "{} gas ({:>2}%)",
                                to_human_size(intrinsic_overhead),
                                intrinsic_overhead * 100 / intrinsic_gas
                            ),
                        ),
                        (
                            "Operator Cost",
                            format!(
                                "{} gas ({:>2}%)",
                                to_human_size(operator_overhead),
                                operator_overhead * 100 / intrinsic_gas
                            ),
                        ),
                    ];

                    let num_items = items.len();
                    for (i, (key, value)) in items.into_iter().enumerate() {
                        let is_last_item = i == num_items - 1;
                        transaction_setup_section.item(is_last_item, key, &value);
                    }
                },
            );
            item_index += 1;

            // L1 Publishing Costs
            let is_last_sibling = item_index == total_items - 1;
            gas_details_section.section(
                "L1 Publishing Costs",
                is_last_sibling,
                |l1_publishing_section| {
                    let items = vec![
                        (
                            "Published",
                            format!("{} bytes", to_human_size(bytes_published.into())),
                        ),
                        (
                            "Cost per Byte",
                            format!("{} gas", to_human_size(gas_per_pubdata)),
                        ),
                        (
                            "Total Gas Cost",
                            format!("{} gas", to_human_size(spent_on_pubdata.into())),
                        ),
                    ];

                    let num_items = items.len();
                    for (i, (key, value)) in items.into_iter().enumerate() {
                        let is_last_item = i == num_items - 1;
                        l1_publishing_section.item(is_last_item, key, &value);
                    }
                },
            );
            item_index += 1;

            // Block Contribution
            let is_last_sibling = item_index == total_items - 1;
            gas_details_section.section("Block Contribution", is_last_sibling, |block_section| {
                let full_block_cost = gas_per_pubdata * fee_model_config.batch_overhead_l1_gas;

                let items = vec![
                    (
                        "Length Overhead",
                        format!("{} gas", to_human_size(overhead_for_length)),
                    ),
                    (
                        "Slot Overhead",
                        format!("{} gas", to_human_size(overhead_for_slot)),
                    ),
                    (
                        "Full Block Cost",
                        format!("~{} L2 gas", to_human_size(full_block_cost)),
                    ),
                ];

                let num_items = items.len();
                for (i, (key, value)) in items.into_iter().enumerate() {
                    let is_last_item = i == num_items - 1;
                    block_section.item(is_last_item, key, &value);
                }
            });
        });
    }
    /// Prints the events of a contract in a structured log.
    pub fn print_event(&mut self, event: &VmEvent, resolve_hashes: bool, is_last_sibling: bool) {
        let event = event.clone();

        let resolved_topics = resolve_topics(&event.indexed_topics, resolve_hashes);
        let topics: Vec<String> = event
            .indexed_topics
            .iter()
            .zip(resolved_topics.iter())
            .map(|(original, resolved)| {
                if resolved.is_empty() {
                    format!("{:#x}", original)
                } else {
                    resolved.clone()
                }
            })
            .collect();

        let contract_display = address_to_human_readable(event.address)
            .map(|x| format!("{:42}", x.blue()))
            .unwrap_or_else(|| format!("{:42}", format!("{:?}", event.address).blue()));

        self.section(
            &format!("Event [{}]", contract_display),
            is_last_sibling,
            |event_section| {
                event_section.section("Topics", false, |topics_section| {
                    let num_topics = topics.len();
                    if num_topics == 0 {
                        topics_section.item(true, "Topics", "EMPTY");
                    } else {
                        for (i, topic) in topics.iter().enumerate() {
                            let is_last_topic = i == num_topics - 1;
                            topics_section.item(is_last_topic, &format!("Topic[{}]", i), topic);
                        }
                    }
                });

                event_section.item(true, "Data", &format_data(&event.value));
            },
        );
    }
    /// Prints the call stack of either the system or user calls in a structured log.
    #[allow(clippy::too_many_arguments)]
    pub fn print_call(
        &mut self,
        initiator: Address,
        contract_address: Option<H160>,
        call: &Call,
        is_last_sibling: bool,
        show_calls: ShowCalls,
        show_outputs: bool,
        resolve_hashes: bool,
    ) {
        let contract_type = KNOWN_ADDRESSES
            .get(&call.to)
            .cloned()
            .map(|known_address| known_address.contract_type)
            .unwrap_or(ContractType::Unknown);

        let should_print = match (&contract_type, &show_calls) {
            (_, ShowCalls::All) => true,
            (_, ShowCalls::None) => false,
            (ContractType::Unknown, _) => true,
            (ContractType::Popular, _) => true,
            (ContractType::Precompile, _) => false,
            (ContractType::System, ShowCalls::User) => false,
            (ContractType::System, ShowCalls::System) => true,
        };

        // Collect subcalls that should be printed (e.g. handle filtering)
        let subcalls_to_print: Vec<&Call> = call
            .calls
            .iter()
            .filter(|subcall| {
                let subcall_contract_type = KNOWN_ADDRESSES
                    .get(&subcall.to)
                    .cloned()
                    .map(|known_address| known_address.contract_type)
                    .unwrap_or(ContractType::Unknown);

                match (&subcall_contract_type, &show_calls) {
                    (_, ShowCalls::All) => true,
                    (_, ShowCalls::None) => false,
                    (ContractType::Unknown, _) => true,
                    (ContractType::Popular, _) => true,
                    (ContractType::Precompile, _) => false,
                    (ContractType::System, ShowCalls::User) => false,
                    (ContractType::System, ShowCalls::System) => true,
                }
            })
            .collect();

        if should_print {
            let call_type_display = format!("{:?}", call.r#type).blue();
            let remaining_gas_display = to_human_size(call.gas.into()).yellow();
            let gas_used_display = format!("({})", to_human_size(call.gas_used.into())).bold();

            let contract_display = format_address_human_readable(
                call.to,
                initiator,
                contract_address,
                format!("{:?}", call.r#type).as_str(),
            )
            .unwrap_or_else(|| format!("{:}", format!("{:?}", call.to).bold()));

            // Get function signature
            let function_signature = if call.input.len() >= 4 {
                let sig = hex::encode(&call.input[0..4]);
                if contract_type == ContractType::Precompile || !resolve_hashes {
                    format!("0x{}", sig)
                } else {
                    block_on(async move {
                        match resolver::decode_function_selector(&sig).await {
                            Ok(Some(name)) => name,
                            Ok(None) | Err(_) => format!("0x{}", sig),
                        }
                    })
                }
            } else {
                "unknown".to_string()
            };

            let function_display = function_signature.cyan().bold();

            let line = format!(
                "{} [{}] {}::{} {}",
                call_type_display,
                remaining_gas_display,
                contract_display,
                function_display,
                gas_used_display
            );

            // Handle errors and outputs within a new indentation scope
            // TODO: can make this more informative by adding "Suggested action" for errors
            self.section(&line, is_last_sibling, |call_section| {
                if call.revert_reason.is_some() || call.error.is_some() {
                    if let Some(ref reason) = call.revert_reason {
                        call_section.format_error(true, &format!("🔴 Revert reason: {}", reason));
                    }
                    if let Some(ref error) = call.error {
                        call_section.format_error(true, &format!("🔴 Error: {}", error));
                    }
                }

                if show_outputs && !call.output.is_empty() {
                    let output_display = call
                        .output
                        .as_slice()
                        .iter()
                        .map(|byte| format!("{:02x}", byte))
                        .collect::<Vec<_>>()
                        .join("");
                    call_section.format_log(true, &format!("Output: {}", output_display.dimmed()));
                }

                // Process subcalls that should be printed
                let num_subcalls = subcalls_to_print.len();
                for (i, subcall) in subcalls_to_print.iter().enumerate() {
                    let is_last_subcall = i == num_subcalls - 1;
                    call_section.print_call(
                        initiator,
                        contract_address,
                        subcall,
                        is_last_subcall,
                        show_calls,
                        show_outputs,
                        resolve_hashes,
                    );
                }
            });
        } else {
            // Call is not printed; process subcalls at the same indentation level
            let num_subcalls = subcalls_to_print.len();
            for (i, subcall) in subcalls_to_print.iter().enumerate() {
                let is_last_subcall = is_last_sibling && (i == num_subcalls - 1);
                self.print_call(
                    initiator,
                    contract_address,
                    subcall,
                    is_last_subcall,
                    show_calls,
                    show_outputs,
                    resolve_hashes,
                );
            }
        }
    }
    /// Prints the storage logs of the system in a structured log.
    pub fn print_storage_logs(
        &mut self,
        log_query: &StorageLogWithPreviousValue,
        pubdata_bytes: Option<PubdataBytesInfo>,
        log_index: usize,
        is_last: bool,
    ) {
        self.section(&format!("Log #{}", log_index), is_last, |log_section| {
            let mut items = vec![
                ("Kind", format!("{:?}", log_query.log.kind)),
                (
                    "Address",
                    address_to_human_readable(*log_query.log.key.address())
                        .unwrap_or_else(|| format!("{:?}", log_query.log.key.address())),
                ),
                ("Key", format!("{:#066x}", log_query.log.key.key())),
                ("Read Value", format!("{:#066x}", log_query.previous_value)),
            ];

            if log_query.log.is_write() {
                items.push(("Written Value", format!("{:#066x}", log_query.log.value)));
            }

            let pubdata_bytes_str = pubdata_bytes
                .map(|p| format!("{}", p))
                .unwrap_or_else(|| "None".to_string());
            items.push(("Pubdata Bytes", pubdata_bytes_str));

            let num_items = items.len();
            for (i, (key, value)) in items.iter().enumerate() {
                let is_last_item = i == num_items - 1;
                log_section.item(is_last_item, key, value);
            }
        });
    }
    /// Prints the VM execution results in a structured log.
    pub fn print_vm_details(&mut self, result: &VmExecutionResultAndLogs) {
        tracing::info!("");

        self.section("[VM Execution Results]", true, |section| {
            let stats = [
                (
                    "Cycles Used",
                    to_human_size(result.statistics.cycles_used.into()),
                ),
                (
                    "Computation Gas Used",
                    to_human_size(result.statistics.computational_gas_used.into()),
                ),
                (
                    "Contracts Used",
                    to_human_size(result.statistics.contracts_used.into()),
                ),
            ];

            for (key, value) in stats.iter() {
                section.item(false, key, value);
            }

            // Handle execution outcome
            match &result.result {
                zksync_multivm::interface::ExecutionResult::Success { .. } => {
                    section.item(true, "Execution Outcome", "Success");
                }
                zksync_multivm::interface::ExecutionResult::Revert { output } => {
                    section.item(false, "Execution Outcome", "Failure");
                    section.format_error(
                        true,
                        &format!("Revert Reason: {}", output.to_user_friendly_string()),
                    );
                }
                zksync_multivm::interface::ExecutionResult::Halt { reason } => {
                    section.item(false, "Execution Outcome", "Failure");
                    section.format_error(true, &format!("Halt Reason: {}", reason));
                }
            }
        });
    }
}
// Builds the branched prefix for the structured logs.
fn build_prefix(sibling_stack: &[bool], is_last_sibling: bool) -> String {
    let mut prefix = String::new();
    if !sibling_stack.is_empty() {
        for &is_last in sibling_stack {
            if !is_last {
                prefix.push_str("│   ");
            } else {
                prefix.push_str("    ");
            }
        }
        let branch = if is_last_sibling {
            "└─ "
        } else {
            "├─ "
        };
        prefix.push_str(branch);
    }
    prefix
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum ContractType {
    System,
    Precompile,
    Popular,
    Unknown,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KnownAddress {
    address: H160,
    name: String,
    contract_type: ContractType,
}

lazy_static! {
    /// Loads the known contact addresses from the JSON file.
    static ref KNOWN_ADDRESSES: HashMap<H160, KnownAddress> = {
        let json_value = serde_json::from_slice(include_bytes!("data/address_map.json")).unwrap();
        let pairs: Vec<KnownAddress> = serde_json::from_value(json_value).unwrap();

        pairs
            .into_iter()
            .map(|entry| (entry.address, entry))
            .collect()
    };
}

fn format_known_address(address: H160) -> Option<String> {
    KNOWN_ADDRESSES.get(&address).map(|known_address| {
        let name = match known_address.contract_type {
            ContractType::System => known_address.name.bold().bright_blue().to_string(),
            ContractType::Precompile => known_address.name.bold().magenta().to_string(),
            ContractType::Popular => known_address.name.bold().bright_green().to_string(),
            ContractType::Unknown => known_address.name.dimmed().to_string(),
        };

        let formatted_address = format!("{:#x}", address).dimmed();
        format!("{}{}{}", name, "@".dimmed(), formatted_address)
    })
}

fn format_address(name: &str, address: H160, color: impl FnOnce(&str) -> String) -> String {
    let name_colored = color(name);
    let formatted_address = format!("{:#x}", address).dimmed();
    format!("{}{}{}", name_colored, "@".dimmed(), formatted_address)
}

fn address_to_human_readable(address: H160) -> Option<String> {
    format_known_address(address)
}

fn format_address_human_readable(
    address: H160,
    initiator: H160,
    contract_address: Option<H160>,
    call_type: &str,
) -> Option<String> {
    // Exclude ContractDeployer and Create2Factory addresses
    let excluded_addresses = [
        H160::from_slice(&hex::decode("0000000000000000000000000000000000008006").unwrap()),
        H160::from_slice(&hex::decode("0000000000000000000000000000000000010000").unwrap()),
    ];

    let is_initiator = address == initiator;
    let is_contract = Some(address) == contract_address && !excluded_addresses.contains(&address);

    if is_initiator {
        return Some(format_address("initiator", address, |s| {
            s.bold().green().to_string()
        }));
    }
    if call_type == "Create" {
        return Some(format_address("deployed", address, |s| {
            s.bold().bright_green().to_string()
        }));
    }
    if is_contract {
        return Some(format_address("contract", address, |s| {
            s.bold().bright_green().to_string()
        }));
    }

    format_known_address(address)
}

fn format_data(value: &[u8]) -> String {
    if value.is_empty() {
        "EMPTY".to_string()
    } else {
        match std::str::from_utf8(value) {
            Ok(v) => format!("{}", v.truecolor(128, 128, 128)),
            Err(_) => {
                let hex_str = hex::encode(value);
                if hex_str.len() > 200 {
                    format!("0x{}...", &hex_str[..200])
                } else {
                    format!("0x{}", hex_str)
                }
            }
        }
    }
}
// Separated from print_events. Consider the same for print_calls.
fn resolve_topics(topics: &[H256], resolve_hashes: bool) -> Vec<String> {
    let topics = topics.to_owned();
    block_on(async move {
        let futures = topics.into_iter().map(|topic| async move {
            if resolve_hashes {
                match resolver::decode_event_selector(&format!("{:#x}", topic)).await {
                    Ok(Some(resolved)) => resolved,
                    Ok(None) | Err(_) => format!("{:#x}", topic),
                }
            } else {
                format!("{:#x}", topic)
            }
        });

        join_all(futures).await
    })
}

/// Amount of pubdata that given write has cost.
/// Used when displaying Storage Logs.
pub enum PubdataBytesInfo {
    // This slot is free
    FreeSlot,
    // This slot costs this much.
    Paid(u32),
    // This happens when we already paid a little for this slot in the past.
    // This slots costs additional X, the total cost is Y.
    AdditionalPayment(u32, u32),
    // We already paid for this slot in this transaction.
    PaidAlready,
}

impl std::fmt::Display for PubdataBytesInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PubdataBytesInfo::FreeSlot => write!(f, "Free Slot (no cost)"),
            PubdataBytesInfo::Paid(cost) => {
                write!(f, "Paid: {} bytes", to_human_size((*cost).into()))
            }
            PubdataBytesInfo::AdditionalPayment(additional_cost, total_cost) => write!(
                f,
                "Additional Payment: {} bytes (Total: {} bytes)",
                to_human_size((*additional_cost).into()),
                to_human_size((*total_cost).into())
            ),
            PubdataBytesInfo::PaidAlready => write!(f, "Already Paid (no additional cost)"),
        }
    }
}

impl PubdataBytesInfo {
    // Whether the slot incurs any cost
    pub fn does_cost(&self) -> bool {
        match self {
            PubdataBytesInfo::FreeSlot => false,
            PubdataBytesInfo::Paid(_) => true,
            PubdataBytesInfo::AdditionalPayment(_, _) => true,
            PubdataBytesInfo::PaidAlready => false,
        }
    }
}

// @dev Separate from Formatter as it does not make use of structured log format.
/// Print the transaction summary.
pub fn print_transaction_summary(
    l2_gas_price: u64,
    tx: &Transaction,
    tx_result: &VmExecutionResultAndLogs,
    status: &str,
) {
    // Calculate used and refunded gas
    let used_gas = tx.gas_limit() - tx_result.refunds.gas_refunded;
    let paid_in_eth = calculate_eth_cost(l2_gas_price, used_gas.as_u64());

    let refunded_gas = tx_result.refunds.gas_refunded;

    // Calculate refunded gas in ETH
    let refunded_in_eth = calculate_eth_cost(l2_gas_price, refunded_gas);

    let emoji = match status {
        "SUCCESS" => "✅",
        "FAILED" => "❌",
        "HALTED" => "⏸️",
        _ => "⚠️",
    };

    tracing::info!("{}  [{}] Hash: {:?}", emoji, status, tx.hash());
    tracing::info!("Initiator: {:?}", tx.initiator_account());
    tracing::info!("Payer: {:?}", tx.payer());
    tracing::info!(
        "Gas Limit: {} | Used: {} | Refunded: {}",
        to_human_size(tx.gas_limit()),
        to_human_size(used_gas),
        to_human_size(tx_result.refunds.gas_refunded.into())
    );
    tracing::info!(
        "Paid: {:.10} ETH ({} gas * {})",
        paid_in_eth,
        used_gas,
        format_gwei(l2_gas_price.into())
    );
    tracing::info!("Refunded: {:.10} ETH", refunded_in_eth);
}

/// Prints halt and revert execution errors.
pub fn print_execution_error<E>(error: &E, tx: Option<&L2Tx>)
where
    E: NamedError + CustomErrorMessage + Documented<Documentation = &'static ErrorDocumentation>,
{
    let error_msg = error.get_message();
    let doc = match error.get_documentation() {
        Ok(opt) => opt,
        Err(e) => {
            eprintln!("Failed to get error documentation: {}", e);
            None
        }
    };

    // Print error header
    println!("{}: {}", "error".red().bold(), error_msg.red());
    println!("    |");
    println!(
        "    = {} {}",
        "error:".bright_red(),
        doc.as_ref()
            .map_or("An unknown error occurred", |d| d.summary.as_str())
    );

    if let Some(tx) = tx {
        println!("    | ");
        println!("    | {}", "Transaction details:".cyan());
        println!(
            "    |   Transaction Type: {:?}",
            tx.common_data.transaction_type
        );
        println!("    |   Nonce: {}", tx.nonce());
        if let Some(contract_address) = &tx.recipient_account() {
            println!("    |   To: {:?}", contract_address);
        }
        println!("    |   From: {:?}", tx.initiator_account());
        if let Some(input_data) = &tx.common_data.input {
            println!("    |   Input Data: {:?}", input_data);
            println!("    |   Hash: {}", tx.hash());
        }
        println!("    |   Gas Limit: {}", tx.common_data.fee.gas_limit);
        println!(
            "    |   Gas Price: {}",
            format_gwei(tx.common_data.fee.max_fee_per_gas)
        );
        println!(
            "    |   Gas Per Pubdata Limit: {}",
            tx.common_data.fee.gas_per_pubdata_limit
        );

        // Log paymaster details if available
        let paymaster_address = tx.common_data.paymaster_params.paymaster;
        let paymaster_input = &tx.common_data.paymaster_params.paymaster_input;
        if paymaster_address != Address::zero() || !paymaster_input.is_empty() {
            println!("    | {}", "Paymaster details:".cyan());
            println!("    |   Paymaster Address: {:?}", paymaster_address);
            println!(
                "    |   Paymaster Input: {}",
                if paymaster_input.is_empty() {
                    "None".to_string()
                } else {
                    format!("{:?}", paymaster_input.encode_hex())
                }
            );
        }
    }

    // Print likely causes if available
    if let Some(doc) = doc {
        if !doc.likely_causes.is_empty() {
            println!("    | ");
            println!("    | {}", "Likely causes:".cyan());
            for cause in &doc.likely_causes {
                println!("    |   - {}", cause.cause);
            }

            // Collect fixes from all causes
            let all_fixes: Vec<&String> = doc
                .likely_causes
                .iter()
                .flat_map(|cause| &cause.fixes)
                .collect();

            if !all_fixes.is_empty() {
                println!("    | ");
                println!("    | {}", "Possible fixes:".green().bold());
                for fix in &all_fixes {
                    println!("    |   - {}", fix);
                }
            }

            // Collect references
            let all_references: Vec<&String> = doc
                .likely_causes
                .iter()
                .flat_map(|cause| &cause.references)
                .collect();

            if !all_references.is_empty() {
                println!(
                    "\n{}",
                    "For more information about this error, visit:"
                        .cyan()
                        .bold()
                );
                for reference in &all_references {
                    println!("  - {}", reference.underline());
                }
            }
        }

        println!("    |");
        println!("{} {}", "note:".blue(), doc.description);
    }

    println!(
        "{} transaction execution halted due to the above error\n",
        "error:".red()
    );
}