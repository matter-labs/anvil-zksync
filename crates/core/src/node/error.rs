use crate::resolver::decode_function_selector;
use alloy::hex::ToHexExt;
use async_trait::async_trait;
use zksync_error::anvil_zks::halt::HaltError;
use zksync_error::anvil_zks::revert::RevertError;
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

async fn handle_vm_revert_reason(reason: &VmRevertReason, default_msg: &str) -> (String, String) {
    match reason {
        VmRevertReason::General { msg, data } => (msg.to_string(), data.encode_hex()),
        VmRevertReason::InnerTxError => ("Inner transaction error".to_string(), String::new()),
        VmRevertReason::VmError => ("VM Error".to_string(), String::new()),
        VmRevertReason::Unknown {
            function_selector,
            data,
        } => {
            if function_selector.is_empty() {
                // Function selector is empty, so we return empty strings
                (String::new(), String::new())
            } else {
                let hex_selector = function_selector.encode_hex();

                match decode_function_selector(&hex_selector).await {
                    // Successfully decoded something like "InsufficientFunds(uint256,uint256)"
                    Ok(Some(decoded_name)) => (decoded_name, data.encode_hex()),
                    // Decoding returned None => unknown signature
                    Ok(None) => (
                        format!("Error with function selector: 0x{hex_selector}"),
                        data.encode_hex(),
                    ),
                    Err(e) => (
                        format!(
                            "Error with function selector: 0x{hex_selector}. Decode failure: {e}"
                        ),
                        data.encode_hex(),
                    ),
                }
            }
        }
        _ => (default_msg.to_string(), String::new()),
    }
}

#[async_trait]
pub trait ToRevertReason {
    async fn to_revert_reason(self) -> RevertError;
}

#[async_trait]
impl ToRevertReason for VmRevertReason {
    async fn to_revert_reason(self) -> RevertError {
        let default_msg = "Unknown revert reason";
        let (message, data) = handle_vm_revert_reason(&self, default_msg).await;

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

#[async_trait]
pub trait ToHaltError {
    async fn to_halt_error(self) -> HaltError;
}

#[async_trait]
impl ToHaltError for Halt {
    async fn to_halt_error(self) -> HaltError {
        match self {
            Halt::ValidationFailed(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(&vm_revert_reason, "Validation Failed").await;
                HaltError::ValidationFailed { msg: message, data }
            }
            Halt::PaymasterValidationFailed(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(&vm_revert_reason, "Paymaster Validation Failed").await;
                HaltError::PaymasterValidationFailed { msg: message, data }
            }
            Halt::PrePaymasterPreparationFailed(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(&vm_revert_reason, "Pre-Paymaster Preparation Failed")
                        .await;
                HaltError::PrePaymasterPreparationFailed { msg: message, data }
            }
            Halt::PayForTxFailed(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(&vm_revert_reason, "PayForTx Failed").await;
                HaltError::PayForTxFailed { msg: message, data }
            }
            Halt::FailedToMarkFactoryDependencies(vm_revert_reason) => {
                let (message, data) = handle_vm_revert_reason(
                    &vm_revert_reason,
                    "Failed to Mark Factory Dependencies",
                )
                .await;
                HaltError::FailedToMarkFactoryDependencies { msg: message, data }
            }
            Halt::FailedToChargeFee(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(&vm_revert_reason, "Failed to Charge Fee").await;
                HaltError::FailedToChargeFee { msg: message, data }
            }
            Halt::Unknown(vm_revert_reason) => {
                let (message, data) =
                    handle_vm_revert_reason(&vm_revert_reason, "Unknown Error").await;
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
