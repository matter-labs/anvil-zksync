use std::collections::HashMap;

use crate::formatter::{self, PubdataBytesInfo};

use super::ShowStorageLogs;
use multivm::vm_latest::VmExecutionResultAndLogs;
use zksync_basic_types::AccountTreeId;
use zksync_types::{
    utils::storage_key_for_eth_balance,
    writes::{
        compression::compress_with_best_strategy, BYTES_PER_DERIVED_KEY,
        BYTES_PER_ENUMERATION_INDEX,
    },
    StorageKey, StorageLogQuery, StorageLogQueryType, BOOTLOADER_ADDRESS, SYSTEM_CONTEXT_ADDRESS,
};
use zksync_utils::u256_to_h256;

fn is_storage_key_free(key: &StorageKey) -> bool {
    key.address() == &SYSTEM_CONTEXT_ADDRESS
        || *key == storage_key_for_eth_balance(&BOOTLOADER_ADDRESS)
}

fn compute_and_update_pubdata_cost(
    cost_paid: &mut HashMap<StorageKey, u32>,
    log_query: &StorageLogQuery,
) -> PubdataBytesInfo {
    let storage_key = StorageKey::new(
        AccountTreeId::new(log_query.log_query.address),
        u256_to_h256(log_query.log_query.key),
    );

    if is_storage_key_free(&storage_key) {
        PubdataBytesInfo::FreeSlot
    } else {
        // how many bytes it takes after compression.
        let compressed_value_size = compress_with_best_strategy(
            log_query.log_query.read_value,
            log_query.log_query.written_value,
        )
        .len() as u32;

        let final_pubdata_cost = if log_query.log_type == StorageLogQueryType::InitialWrite {
            (BYTES_PER_DERIVED_KEY as u32) + compressed_value_size
        } else {
            (BYTES_PER_ENUMERATION_INDEX as u32) + compressed_value_size
        };

        let result = match cost_paid.get(&storage_key).copied() {
            Some(already_paid) => {
                let to_pay = final_pubdata_cost.saturating_sub(already_paid);
                if to_pay > 0 {
                    PubdataBytesInfo::AdditionalPayment(to_pay, final_pubdata_cost)
                } else {
                    PubdataBytesInfo::PaidAlready
                }
            }
            None => PubdataBytesInfo::Paid(final_pubdata_cost),
        };
        cost_paid.insert(storage_key, final_pubdata_cost);
        result
    }
}

pub fn print_storage_logs_details(
    show_storage_logs: &ShowStorageLogs,
    result: &VmExecutionResultAndLogs,
) {
    tracing::info!("");
    tracing::info!("┌──────────────────┐");
    tracing::info!("│   STORAGE LOGS   │");
    tracing::info!("└──────────────────┘");

    let mut cost_paid = HashMap::<StorageKey, u32>::default();

    for log_query in &result.logs.storage_logs {
        let pubdata_bytes_info = if matches!(
            log_query.log_type,
            StorageLogQueryType::RepeatedWrite | StorageLogQueryType::InitialWrite
        ) {
            Some(compute_and_update_pubdata_cost(&mut cost_paid, log_query))
        } else {
            None
        };

        match show_storage_logs {
            ShowStorageLogs::Write => {
                if matches!(
                    log_query.log_type,
                    StorageLogQueryType::RepeatedWrite | StorageLogQueryType::InitialWrite
                ) {
                    formatter::print_logs(log_query, pubdata_bytes_info);
                }
            }
            ShowStorageLogs::Paid => {
                // Show only the logs that incur any cost.
                if pubdata_bytes_info
                    .as_ref()
                    .map(|x| x.does_cost())
                    .unwrap_or_default()
                {
                    formatter::print_logs(log_query, pubdata_bytes_info);
                }
            }
            ShowStorageLogs::Read => {
                if log_query.log_type == StorageLogQueryType::Read {
                    formatter::print_logs(log_query, pubdata_bytes_info);
                }
            }
            ShowStorageLogs::All => {
                formatter::print_logs(log_query, pubdata_bytes_info);
            }

            _ => {}
        }
    }
}
