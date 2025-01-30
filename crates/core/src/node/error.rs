use zksync_error::anvil::halt::HaltError;
use zksync_error::anvil::revert::RevertError;
use zksync_multivm::interface::{Halt, VmRevertReason};
use alloy::hex::ToHexExt;

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

pub trait ToRevertReason {
    fn to_revert_reason(self) -> RevertError;
}

impl ToRevertReason for VmRevertReason {
    fn to_revert_reason(self) -> RevertError {
        match self {
            VmRevertReason::General { msg, data } => RevertError::General {
                msg,
                data,
            },
            VmRevertReason::InnerTxError => RevertError::InnerTxError,
            VmRevertReason::VmError => RevertError::VmError,
            VmRevertReason::Unknown { function_selector, data } => RevertError::Unknown {
                function_selector: function_selector.encode_hex(),
                data: data.encode_hex(),
            },
            _ => RevertError::Unknown {
                    function_selector: "Unknown revert reason".to_string(),
                    data: String::new(),
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
            Halt::ValidationFailed(VmRevertReason::General { msg, data }) => {
                HaltError::ValidationFailed {
                    msg,
                    data: data.encode_hex(),
                }
            }
            Halt::PaymasterValidationFailed(VmRevertReason::General { msg, data }) => {
                HaltError::PaymasterValidationFailed {
                    msg,
                    data: data.encode_hex(),
                }
            }
            Halt::PrePaymasterPreparationFailed(VmRevertReason::General { msg, data }) => {
                HaltError::PrePaymasterPreparationFailed {
                    msg,
                    data: data.encode_hex(),
                }
            }
            Halt::PayForTxFailed(VmRevertReason::General { msg, data }) => {
                HaltError::PayForTxFailed {
                    msg,
                    data: data.encode_hex(),
                }
            }
            Halt::FailedToMarkFactoryDependencies(VmRevertReason::General { msg, data }) => {
                HaltError::FailedToMarkFactoryDependencies {
                    msg,
                    data: data.encode_hex(),
                }
            }
            Halt::FailedToChargeFee(VmRevertReason::General { msg, data }) => {
                HaltError::FailedToChargeFee {
                    msg,
                    data: data.encode_hex(),
                }
            }
            Halt::Unknown(VmRevertReason::General { msg, data }) => HaltError::Unknown {
                msg,
                data: data.encode_hex(),
            },
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
            _ => HaltError::Unknown {
                msg: "Unknown halt reason".to_string(),
                data: String::new(),
            },
        }
    }
}
