//!
//! Error handling utilities for the JSON-RPC API server.
//!

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

/// An utility method to convert an error from zksync-error format to a JSON-RPC error object.
///
/// # Arguments
///
/// * `code` - Optional error code to use. If None, the error code is derived from the error's identifier.
/// * `error` - The error to convert.
///
/// # Returns
///
/// A JSON-RPC error object.
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

/// Trait for converting custom errors into JSON-RPC error objects.
///
/// This trait allows different error types to be converted into a format
/// suitable for JSON-RPC responses.
pub trait RpcErrorAdapter {
    /// Converts the error into a JSON-RPC error object.
    ///
    /// # Arguments
    ///
    /// * `error` - The error to convert.
    ///
    /// # Returns
    ///
    /// A JSON-RPC error object.
    fn into(error: Self) -> ErrorObjectOwned;
}

/// Maps state loader errors to appropriate JSON-RPC error codes based on the error type.
impl RpcErrorAdapter for StateLoaderError {
    fn into(error: Self) -> ErrorObjectOwned {
        let error_code = match error {
            StateLoaderError::LoadingStateOverExistingState
            | StateLoaderError::LoadEmptyState
            | StateLoaderError::StateDecompression { .. }
            | StateLoaderError::StateDeserialization { .. }
            | StateLoaderError::UnknownStateVersion { .. }
            | StateLoaderError::StateFileAccess { .. } => ErrorCode::InvalidParams,
            StateLoaderError::GenericError { .. } => ErrorCode::InternalError,
            _ => ErrorCode::InternalError,
        };
        to_rpc(Some(error_code.code()), error)
    }
}

/// All Anvil node errors are treated as internal errors.
impl RpcErrorAdapter for AnvilNodeError {
    fn into(error: Self) -> ErrorObjectOwned {
        to_rpc(Some(ErrorCode::InternalError.code()), error)
    }
}

/// Maps Web3 errors to appropriate JSON-RPC error codes based on the error type.
impl RpcErrorAdapter for Web3Error {
    fn into(error: Self) -> ErrorObjectOwned {
        let web3_error = &error;
        // Map the Web3Error to an appropriate RPC error code
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

        // For transaction submission errors, include the transaction data as part of the error
        let data = match &error {
            Web3Error::SubmitTransactionError(_, data) => Some(format!("0x{}", hex::encode(data))),
            _ => None,
        };

        // Format the error message
        let message = match web3_error {
            Web3Error::InternalError(e) => e.to_string(),
            _ => web3_error.to_string(),
        };

        ErrorObject::owned(code.code(), message, data)
    }
}

/// All anyhow errors are treated as internal errors.
impl RpcErrorAdapter for anyhow::Error {
    fn into(error: Self) -> ErrorObjectOwned {
        ErrorObjectOwned::owned(
            ErrorCode::InternalError.code(),
            error.to_string(),
            None::<()>,
        )
    }
}

/// Creates a JSON-RPC error object for invalid parameters.
///
/// # Arguments
///
/// * `msg` - The error message.
///
/// # Returns
///
/// A JSON-RPC error object with the InvalidParams error code.
pub(crate) fn rpc_invalid_params(msg: String) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(ErrorCode::InvalidParams.code(), msg, None::<()>)
}

/// Creates a JSON-RPC error result for unsupported methods.
///
/// # Arguments
///
/// * `method_name` - The name of the unsupported method.
///
/// # Returns
///
/// A JSON-RPC error result with the MethodNotFound error code.
pub(crate) fn rpc_unsupported<T>(method_name: &str) -> jsonrpsee::core::RpcResult<T> {
    Err(ErrorObject::owned(
        ErrorCode::MethodNotFound.code(),
        format!("Method not found: {}", method_name),
        None::<()>,
    ))
}
