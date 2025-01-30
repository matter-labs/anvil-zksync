use alloy::hex::ToHexExt;
use zksync_error::anvil::halt::HaltError;
use zksync_error::anvil::revert::RevertError;
use zksync_multivm::interface::{Halt, VmRevertReason};

#[derive(thiserror::Error, Debug)]
pub enum LoadStateError {
    #[error("loading state into a node with existing state is not allowed (please create an issue if you have a valid use case)")]
    HasExistingState,
    #[error("loading empty state (no blocks) is not allowed")]
    EmptyState,
    #[error("failed to decompress state: {0}")]
    FailedDecompress(std::io::Error),
    #[error("failed to deserialize state: {0}")]
    FailedDeserialize(serde_json::Error),
    #[error("unknown state version `{0}`")]
    UnknownStateVersion(u8),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

fn handle_vm_revert_reason(reason: VmRevertReason, default_msg: &str) -> (String, String) {
    match reason {
        VmRevertReason::General { msg, data } => (msg, data.encode_hex()),
        VmRevertReason::InnerTxError => ("Inner transaction error".to_string(), String::new()),
        VmRevertReason::VmError => ("VM Error".to_string(), String::new()),
        VmRevertReason::Unknown {
            function_selector,
            data,
        } => {
            let formatted_selector = format!("0x{}", function_selector.encode_hex());
            (
                format!(
                    "Account validation error: Unknown revert reason for function: {}. Data: {}",
                    formatted_selector,
                    data.encode_hex()
                ),
                data.encode_hex(),
            )
        }
        _ => (default_msg.to_string(), String::new()),
    }
}

pub trait ToRevertReason {
    fn to_revert_reason(self) -> RevertError;
}

impl ToRevertReason for VmRevertReason {
    fn to_revert_reason(self) -> RevertError {
        let default_msg = "Unknown revert reason";
        let (message, data) = handle_vm_revert_reason(self.clone(), default_msg);

        match self {
            VmRevertReason::General { .. } => RevertError::General {
                msg: message,
                data: data.into(),
            },
            VmRevertReason::InnerTxError => RevertError::InnerTxError,
            VmRevertReason::VmError => RevertError::VmError,
            VmRevertReason::Unknown { .. } => RevertError::Unknown {
                function_selector: message.encode_hex(),
                data,
            },
            _ => RevertError::Unknown {
                function_selector: message.encode_hex(),
                data,
            },
        }
    }
}

pub trait ToHaltError {
    fn to_halt_error(self) -> HaltError;
}

impl ToHaltError for Halt {
    fn to_halt_error(self) -> HaltError {
        match self {
            Halt::ValidationFailed(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(vm_revert_reason, "Validation Failed");
                HaltError::ValidationFailed { msg: message, data }
            }
            Halt::PaymasterValidationFailed(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(vm_revert_reason, "Paymaster Validation Failed");
                HaltError::PaymasterValidationFailed { msg: message, data }
            }
            Halt::PrePaymasterPreparationFailed(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(vm_revert_reason, "Pre-Paymaster Preparation Failed");
                HaltError::PrePaymasterPreparationFailed { msg: message, data }
            }
            Halt::PayForTxFailed(vm_revert_reason) => {
                let (message, data) = handle_vm_revert_reason(vm_revert_reason, "PayForTx Failed");
                HaltError::PayForTxFailed { msg: message, data }
            }
            Halt::FailedToMarkFactoryDependencies(vm_revert_reason) => {
                let (message, data) = handle_vm_revert_reason(
                    vm_revert_reason,
                    "Failed to Mark Factory Dependencies",
                );
                HaltError::FailedToMarkFactoryDependencies { msg: message, data }
            }
            Halt::FailedToChargeFee(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(vm_revert_reason, "Failed to Charge Fee");
                HaltError::FailedToChargeFee { msg: message, data }
            }
            Halt::Unknown(vm_revert_reason) => {
                let (message, data) = handle_vm_revert_reason(vm_revert_reason, "Unknown Error");
                HaltError::Unknown { msg: message, data }
            }
            Halt::UnexpectedVMBehavior(msg) => HaltError::UnexpectedVMBehavior { problem: msg },
            Halt::FailedToSetL2Block(msg) => HaltError::FailedToSetL2Block { msg },
            Halt::FailedToAppendTransactionToL2Block(msg) => {
                HaltError::FailedToAppendTransactionToL2Block { msg }
            }
            Halt::TracerCustom(msg) => HaltError::TracerCustom { msg },
            Halt::FromIsNotAnAccount => HaltError::FromIsNotAnAccount,
            Halt::InnerTxError => HaltError::InnerTxError,
            Halt::BootloaderOutOfGas => HaltError::BootloaderOutOfGas,
            Halt::ValidationOutOfGas => HaltError::ValidationOutOfGas,
            Halt::TooBigGasLimit => HaltError::TooBigGasLimit,
            Halt::NotEnoughGasProvided => HaltError::NotEnoughGasProvided,
            Halt::MissingInvocationLimitReached => HaltError::MissingInvocationLimitReached,
            Halt::VMPanic => HaltError::VMPanic,
            Halt::FailedToPublishCompressedBytecodes => {
                HaltError::FailedToPublishCompressedBytecodes
            }
            Halt::FailedBlockTimestampAssertion => HaltError::FailedBlockTimestampAssertion,
        }
    }
}
