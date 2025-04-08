use anvil_zksync_core::node::error::LoadStateError;
use jsonrpsee::types::{ErrorCode, ErrorObjectOwned};
use zksync_error::anvil_zksync::node::AnvilNodeError;
use zksync_web3_decl::error::Web3Error;

#[derive(thiserror::Error, Debug)]
pub enum RpcError {
    #[error("failed to load state: {0}")]
    LoadState(#[from] LoadStateError),
    #[error("method is unsupported")]
    Unsupported,
    #[error("{0}")]
    Web3Error(#[from] Web3Error),
    #[error("{0}")]
    NodeError(#[from] AnvilNodeError),
    // TODO: Shouldn't exist once we create a proper error hierarchy
    #[error("internal error: {0}")]
    Other(#[from] anyhow::Error),
}

impl From<RpcError> for ErrorObjectOwned {
    fn from(error: RpcError) -> Self {
        match error {
            RpcError::LoadState(error) => match error {
                err @ LoadStateError::HasExistingState
                | err @ LoadStateError::EmptyState
                | err @ LoadStateError::FailedDecompress(_)
                | err @ LoadStateError::FailedDeserialize(_)
                | err @ LoadStateError::UnknownStateVersion(_) => invalid_params(err.to_string()),
                LoadStateError::Other(error) => internal(error.to_string()),
            },
            RpcError::Unsupported => unsupported(),
            RpcError::Web3Error(error) => into_jsrpc_error(error),
            RpcError::Other(error) => internal(error.to_string()),
            RpcError::NodeError(error) => internal(error.to_string()),
        }
    }
}

fn internal(msg: String) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(ErrorCode::InternalError.code(), msg, None::<()>)
}

fn invalid_params(msg: String) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(ErrorCode::InvalidParams.code(), msg, None::<()>)
}

fn unsupported() -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        ErrorCode::MethodNotFound.code(),
        String::from("Method is unsupported"),
        None::<()>,
    )
}

pub fn into_jsrpc_error(err: Web3Error) -> ErrorObjectOwned {
    let code = match err {
        Web3Error::MethodNotImplemented => ErrorCode::MethodNotFound.code(),
        Web3Error::InternalError(_) => ErrorCode::InternalError.code(),
        Web3Error::NoBlock
        | Web3Error::PrunedBlock(_)
        | Web3Error::PrunedL1Batch(_)
        | Web3Error::ProxyError(_)
        | Web3Error::TooManyTopics
        | Web3Error::FilterNotFound
        | Web3Error::LogsLimitExceeded(_, _, _)
        | Web3Error::InvalidFilterBlockHash
        | Web3Error::TreeApiUnavailable => ErrorCode::InvalidParams.code(),
        Web3Error::SubmitTransactionError(_, _) | Web3Error::SerializationError(_) => {
            ErrorCode::ServerError(3).code()
        }
        Web3Error::ServerShuttingDown => ErrorCode::ServerIsBusy.code(),
    };
    let message = match &err {
        Web3Error::SubmitTransactionError(_, _) => err.to_string(),
        Web3Error::InternalError(err) => err.to_string(),
        _ => err.to_string(),
    };
    let data = match err {
        Web3Error::SubmitTransactionError(_, data) => Some(format!("0x{}", hex::encode(data))),
        _ => None,
    };
    ErrorObjectOwned::owned(code, message, data)
}
