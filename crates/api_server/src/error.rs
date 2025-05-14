use jsonrpsee::{
    core::Serialize,
    types::{ErrorCode, ErrorObject, ErrorObjectOwned},
};
use zksync_error::{
    anvil_zksync::{node::AnvilNodeError, state::StateLoaderError},
    ICustomError, IError as _, ZksyncError,
};
use zksync_web3_decl::error::Web3Error;

use jsonrpsee::types::ErrorCode as RpcErrorCode;

fn to_rpc<I: Serialize + ICustomError<ZksyncError, ZksyncError>>(
    code: Option<i32>,
    error: I,
) -> ErrorObjectOwned {
    ErrorObject::owned(
        code.unwrap_or(error.to_unified().get_identifier().encode() as i32),
        error.to_unified().get_message(),
        Some(error),
    )
}

pub trait RpcErrorAdapter {
    fn into(error: Self) -> ErrorObjectOwned;
}

impl RpcErrorAdapter for StateLoaderError {
    fn into(error: Self) -> ErrorObjectOwned {
        to_rpc(None, error)
    }
}

impl RpcErrorAdapter for AnvilNodeError {
    fn into(error: Self) -> ErrorObjectOwned {
        to_rpc(Some(ErrorCode::InternalError.code()), error)
    }
}

impl RpcErrorAdapter for Web3Error {
    fn into(error: Self) -> ErrorObjectOwned {
        let web3_error = &error;
        let code: RpcErrorCode = match web3_error {
            Web3Error::InternalError(_) => RpcErrorCode::InternalError,
            Web3Error::MethodNotImplemented => RpcErrorCode::MethodNotFound,
            Web3Error::NoBlock
            | Web3Error::PrunedBlock(_)
            | Web3Error::PrunedL1Batch(_)
            | Web3Error::ProxyError(_)
            | Web3Error::TooManyTopics
            | Web3Error::FilterNotFound
            | Web3Error::LogsLimitExceeded(_, _, _)
            | Web3Error::InvalidFilterBlockHash
            | Web3Error::TreeApiUnavailable => RpcErrorCode::InvalidParams,
            Web3Error::SubmitTransactionError(_, _) | Web3Error::SerializationError(_) => {
                RpcErrorCode::ServerError(3)
            }
            Web3Error::ServerShuttingDown => RpcErrorCode::ServerIsBusy,
        };

        let data = match &error {
            Web3Error::SubmitTransactionError(_, data) => Some(format!("0x{}", hex::encode(data))),
            _ => None,
        };

        let message = match web3_error {
            Web3Error::InternalError(e) => e.to_string(),
            _ => web3_error.to_string(),
        };

        ErrorObject::owned(code.code(), message, data)
    }
}

impl RpcErrorAdapter for anyhow::Error {
    fn into(error: Self) -> ErrorObjectOwned {
        ErrorObjectOwned::owned(
            ErrorCode::InternalError.code(),
            error.to_string(),
            None::<()>,
        )
    }
}

pub(crate) fn rpc_invalid_params(msg: String) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(ErrorCode::InvalidParams.code(), msg, None::<()>)
}

pub(crate) fn rpc_unsupported<T>(method_name: &str) -> jsonrpsee::core::RpcResult<T> {
    Err(ErrorObject::owned(
        ErrorCode::MethodNotFound.code(),
        format!("Method not found: {}", method_name),
        None::<()>,
    ))
}
