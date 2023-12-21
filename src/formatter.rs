//! Helper methods to display transaction data in more human readable way.
use crate::{node::ShowCalls, resolver};

use colored::Colorize;

use serde::Deserialize;
use std::collections::HashMap;
use std::str;

use crate::fork::block_on;
use zksync_basic_types::H160;

use multivm::interface::VmExecutionResultAndLogs;
use zksync_types::{vm_trace::Call, StorageLogQuery, StorageLogQueryType, VmEvent};

use lazy_static::lazy_static;

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

fn address_to_human_readable(address: H160) -> Option<String> {
    KNOWN_ADDRESSES
        .get(&address)
        .map(|known_address| match known_address.contract_type {
            ContractType::System => known_address.name.to_string(),
            ContractType::Precompile => format!("{}", known_address.name.dimmed()),
            ContractType::Popular => format!("{}", known_address.name.green()),
            ContractType::Unknown => known_address.name.to_string(),
        })
}

/// Pretty-prints event object
/// if skip_resolve is false, will try to contact openchain to resolve the topic hashes.
pub fn print_event(event: &VmEvent, resolve_hashes: bool) {
    let event = event.clone();
    block_on(async move {
        let mut tt: Vec<String> = vec![];
        if !resolve_hashes {
            tt = event
                .indexed_topics
                .iter()
                .map(|t| format!("{:#x}", t))
                .collect();
        } else {
            for topic in event.indexed_topics {
                let selector = resolver::decode_event_selector(&format!("{:#x}", topic))
                    .await
                    .unwrap();
                tt.push(selector.unwrap_or(format!("{:#x}", topic)));
            }
        }

        tracing::info!(
            "{}",
            address_to_human_readable(event.address)
                .map(|x| format!("{:42}", x.blue()))
                .unwrap_or(format!("{:42}", format!("{:?}", event.address).blue()))
        );

        tracing::info!("{}", "  Topics:".truecolor(128, 128, 128));
        for indexed_topic in &tt {
            tracing::info!("    {}", indexed_topic);
        }

        if event.value.is_empty() {
            tracing::info!("{}", "  Data: EMPTY".truecolor(128, 128, 128));
        } else {
            match str::from_utf8(&event.value) {
                Ok(v) => {
                    tracing::info!(
                        "{} {}",
                        "  Data (String):".truecolor(128, 128, 128),
                        v.to_string()
                    );
                }
                Err(_) => {
                    let hex_str = hex::encode(&event.value);
                    let display_str = if hex_str.len() > 200 {
                        format!("{}...", &hex_str[..200])
                    } else {
                        hex_str.to_string()
                    };

                    tracing::info!(
                        "{} 0x{}",
                        "  Data (Hex):".truecolor(128, 128, 128),
                        display_str
                    );
                }
            };
        }

        tracing::info!("");
    });
}

/// Pretty-prints contents of a 'call' - including subcalls.
/// If skip_resolve is false, will try to contact openchain to resolve the ABI names.
pub fn print_call(call: &Call, padding: usize, show_calls: &ShowCalls, resolve_hashes: bool) {
    let contract_type = KNOWN_ADDRESSES
        .get(&call.to)
        .cloned()
        .map(|known_address| known_address.contract_type)
        .unwrap_or(ContractType::Unknown);

    let should_print = match (&contract_type, &show_calls) {
        (_, ShowCalls::All) => true,
        (_, ShowCalls::None) => false,
        // now we're left only with 'user' and 'system'
        (ContractType::Unknown, _) => true,
        (ContractType::Popular, _) => true,
        (ContractType::Precompile, _) => false,
        // Now we're left with System
        (ContractType::System, ShowCalls::User) => false,
        (ContractType::System, ShowCalls::System) => true,
    };
    if should_print {
        let function_signature = if call.input.len() >= 4 {
            let sig = call.input.as_slice()[..4]
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<_>>()
                .join("");

            if contract_type == ContractType::Precompile || !resolve_hashes {
                format!("{:>16}", sig)
            } else {
                block_on(async move {
                    let fetch = resolver::decode_function_selector(&sig).await.unwrap();
                    fetch.unwrap_or(format!("{:>16}", format!("0x{}", sig).dimmed()))
                })
            }
        } else {
            format!(
                "0x{}",
                call.input
                    .as_slice()
                    .iter()
                    .map(|byte| format!("{:02x}", byte))
                    .collect::<Vec<_>>()
                    .join("")
            )
        };

        let pretty_print = format!(
            "{}{:?} {} {} {} {} {}",
            " ".repeat(padding),
            call.r#type,
            address_to_human_readable(call.to)
                .map(|x| format!("{:<52}", x))
                .unwrap_or(format!("{:<52}", format!("{:?}", call.to).bold())),
            function_signature,
            call.revert_reason
                .as_ref()
                .map(|s| format!("Revert: {}", s))
                .unwrap_or_default(),
            call.error
                .as_ref()
                .map(|s| format!("Error: {}", s))
                .unwrap_or_default(),
            call.gas
        );

        if call.revert_reason.as_ref().is_some() || call.error.as_ref().is_some() {
            tracing::info!("{}", pretty_print.on_red());
        } else {
            tracing::info!("{}", pretty_print);
        }
    }
    for subcall in &call.calls {
        print_call(subcall, padding + 2, show_calls, resolve_hashes);
    }
}

/// Amount of pubdata that given write has cost.
pub enum PubdataBytesInfo {
    // This slot is free
    FreeSlot,
    // This slot costs this much.
    Paid(u32),
    // This happens when we already paid a litte for this slot in the past.
    // This slots costs additional X, the total cost is Y.
    AdditionalPayment(u32, u32),
    // We already paid for this slot in this transaction.
    PaidAlready,
}

impl std::fmt::Display for PubdataBytesInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PubdataBytesInfo::FreeSlot => write!(f, "free slot"),
            PubdataBytesInfo::Paid(cost) => write!(f, "{:?} bytes", cost),
            PubdataBytesInfo::AdditionalPayment(additional_cost, total_cost) => write!(
                f,
                "{:?} addditional bytes, {:?} total cost",
                additional_cost, total_cost
            ),
            PubdataBytesInfo::PaidAlready => write!(f, "already paid"),
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

pub fn print_logs(log_query: &StorageLogQuery, pubdata_bytes: Option<PubdataBytesInfo>) {
    let separator = "─".repeat(82);
    tracing::info!("{:<15} {:?}", "Type:", log_query.log_type);
    tracing::info!(
        "{:<15} {}",
        "Address:",
        address_to_human_readable(log_query.log_query.address)
            .unwrap_or(format!("{}", log_query.log_query.address))
    );
    tracing::info!("{:<15} {:#066x}", "Key:", log_query.log_query.key);

    tracing::info!(
        "{:<15} {:#066x}",
        "Read Value:",
        log_query.log_query.read_value
    );

    if log_query.log_type != StorageLogQueryType::Read {
        tracing::info!(
            "{:<15} {:#066x}",
            "Written Value:",
            log_query.log_query.written_value
        );
    }
    if let Some(pubdata_bytes) = pubdata_bytes {
        tracing::info!("{:<15} {:}", "Pubdata bytes:", pubdata_bytes);
    }
    tracing::info!("{}", separator);
}

pub fn print_vm_details(result: &VmExecutionResultAndLogs) {
    tracing::info!("");
    tracing::info!("┌──────────────────────────┐");
    tracing::info!("│   VM EXECUTION RESULTS   │");
    tracing::info!("└──────────────────────────┘");

    tracing::info!("Cycles Used:          {}", result.statistics.cycles_used);
    tracing::info!(
        "Computation Gas Used: {}",
        result.statistics.computational_gas_used
    );
    tracing::info!("Contracts Used:       {}", result.statistics.contracts_used);
    match &result.result {
        multivm::interface::ExecutionResult::Success { .. } => {}
        multivm::interface::ExecutionResult::Revert { output } => {
            tracing::info!("");
            tracing::info!(
                "{}",
                format!(
                    "\n[!] Revert Reason:    {}",
                    output.to_user_friendly_string()
                )
                .on_red()
            );
        }
        multivm::interface::ExecutionResult::Halt { reason } => {
            tracing::info!("");
            tracing::info!("{}", format!("\n[!] Halt Reason:    {}", reason).on_red());
        }
    }

    tracing::info!("════════════════════════════");
}
