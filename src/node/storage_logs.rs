use crate::formatter;

use super::ShowStorageLogs;
use multivm::vm_latest::VmExecutionResultAndLogs;
use zksync_types::StorageLogQueryType;

pub fn print_storage_logs_details(
    show_storage_logs: &ShowStorageLogs,
    result: &VmExecutionResultAndLogs,
) {
    tracing::info!("");
    tracing::info!("┌──────────────────┐");
    tracing::info!("│   STORAGE LOGS   │");
    tracing::info!("└──────────────────┘");
    for log_query in &result.logs.storage_logs {
        match show_storage_logs {
            ShowStorageLogs::Write => {
                if matches!(
                    log_query.log_type,
                    StorageLogQueryType::RepeatedWrite | StorageLogQueryType::InitialWrite
                ) {
                    formatter::print_logs(log_query);
                }
            }
            ShowStorageLogs::Read => {
                if log_query.log_type == StorageLogQueryType::Read {
                    formatter::print_logs(log_query);
                }
            }
            ShowStorageLogs::All => {
                formatter::print_logs(log_query);
            }
            _ => {}
        }
    }
}
