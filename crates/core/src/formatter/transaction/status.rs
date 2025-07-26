use std::fmt::Display;

use zksync_multivm::interface::ExecutionResult;

///
/// Kind of outcomes of transaction execution.
///
pub enum TransactionStatus {
    Success,
    Failure,
    Halt,
}

impl TransactionStatus {
    pub fn emoji(&self) -> &str {
        match self {
            Self::Success => "✅",
            Self::Failure => "❌",
            Self::Halt => "⏸️",
        }
    }
}
impl From<&zksync_multivm::interface::ExecutionResult> for TransactionStatus {
    fn from(value: &ExecutionResult) -> Self {
        match value {
            ExecutionResult::Success { .. } => Self::Success,
            ExecutionResult::Revert { .. } => Self::Failure,
            ExecutionResult::Halt { .. } => Self::Halt,
        }
    }
}
impl Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TransactionStatus::Success => "SUCCESS",
            TransactionStatus::Failure => "FAILED",
            TransactionStatus::Halt => "HALTED",
        })
    }
}
