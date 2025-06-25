use alloy::hex::ToHexExt;
use anvil_zksync_traces::identifier::SignaturesIdentifier;
use async_trait::async_trait;
use zksync_error::anvil_zksync::halt::HaltError;
use zksync_error::anvil_zksync::revert::RevertError;
use zksync_multivm::interface::{Halt, VmRevertReason};

async fn handle_vm_revert_reason(reason: &VmRevertReason) -> (String, &[u8]) {
    match reason {
        VmRevertReason::General { msg, data } => (msg.to_string(), data),
        VmRevertReason::InnerTxError => ("Inner transaction error".to_string(), &[]),
        VmRevertReason::VmError => ("VM Error".to_string(), &[]),
        VmRevertReason::Unknown {
            function_selector,
            data,
        } => {
            if function_selector.is_empty() {
                ("Error: no function selector available".to_string(), &[])
            } else {
                let hex_selector = function_selector.encode_hex();
                match SignaturesIdentifier::global()
                    .identify_function(function_selector)
                    .await
                {
                    Some(decoded_name) => (decoded_name.name, data),
                    None => (
                        format!("Error with function selector: 0x{hex_selector}"),
                        data,
                    ),
                }
            }
        }
        _ => ("".to_string(), &[]),
    }
}

/// Extracts the VmRevertReason from a Halt, if available.
fn revert_reason(halt: &Halt) -> Option<&VmRevertReason> {
    match halt {
        Halt::ValidationFailed(reason)
        | Halt::PaymasterValidationFailed(reason)
        | Halt::PrePaymasterPreparationFailed(reason)
        | Halt::PayForTxFailed(reason)
        | Halt::FailedToMarkFactoryDependencies(reason)
        | Halt::FailedToChargeFee(reason)
        | Halt::Unknown(reason) => Some(reason),
        _ => None,
    }
}

#[async_trait]
pub trait ToRevertReason {
    async fn to_revert_reason(self) -> RevertError;
}

#[async_trait]
impl ToRevertReason for VmRevertReason {
    async fn to_revert_reason(self) -> RevertError {
        let (message, data) = handle_vm_revert_reason(&self).await;

        match self {
            VmRevertReason::General { .. } => RevertError::General {
                msg: message,
                data: data.encode_hex().into(),
            },
            VmRevertReason::InnerTxError => RevertError::InnerTxError,
            VmRevertReason::VmError => RevertError::VmError,
            VmRevertReason::Unknown { .. } => RevertError::Unknown {
                function_selector: message,
                data: data.encode_hex(),
            },
            _ => RevertError::Unknown {
                function_selector: message,
                data: data.encode_hex(),
            },
        }
    }
}

#[async_trait]
pub trait ToHaltError {
    async fn to_halt_error(self) -> HaltError;
}

#[async_trait]
impl ToHaltError for Halt {
    async fn to_halt_error(self) -> HaltError {
        let (msg_opt, data_opt) = if let Some(reason) = revert_reason(&self) {
            let (msg, data) = handle_vm_revert_reason(reason).await;
            (Some(msg), Some(data.encode_hex()))
        } else {
            (None, None)
        };

        let msg = msg_opt.unwrap_or_else(|| "Unknown revert reason".into());
        let data = data_opt.unwrap_or_default();

        match self {
            Halt::ValidationFailed(_) => HaltError::ValidationFailed { msg, data },
            Halt::PaymasterValidationFailed(_) => {
                HaltError::PaymasterValidationFailed { msg, data }
            }
            Halt::PrePaymasterPreparationFailed(_) => {
                HaltError::PrePaymasterPreparationFailed { msg, data }
            }
            Halt::PayForTxFailed(_) => HaltError::PayForTxFailed { msg, data },
            Halt::FailedToMarkFactoryDependencies(_) => {
                HaltError::FailedToMarkFactoryDependencies { msg, data }
            }
            Halt::FailedToChargeFee(_) => HaltError::FailedToChargeFee { msg, data },
            Halt::Unknown(_) => HaltError::Unknown { msg, data },
            // Other variants that do not include a VmRevertReason:
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
