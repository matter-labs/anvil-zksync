use zksync_multivm::interface::storage::{StoragePtr, WriteStorage};
use zksync_multivm::interface::VmExecutionResultAndLogs;
use zksync_multivm::vm_latest::Vm;
use zksync_multivm::HistoryMode;
use zksync_types::Transaction;

pub trait TestNodeVMInterface {
    fn execute_tx(&mut self, tx: Transaction) -> VmExecutionResultAndLogs;
}

impl<S, H> TestNodeVMInterface for Vm<S, H>
where
    S: WriteStorage,
    H: HistoryMode,
{
    fn execute_tx(&mut self, tx: Transaction) -> VmExecutionResultAndLogs {
        todo!()
    }
}
