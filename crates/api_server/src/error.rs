use anvil_zksync_core::node::error::LoadStateError;
use jsonrpsee::types::{ErrorCode, ErrorObjectOwned};

#[derive(thiserror::Error, Debug)]
pub enum RpcError {
    #[error("failed to load state: {0}")]
    LoadState(#[from] LoadStateError),
    #[error("method is unsupported")]
    Unsupported,
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
            RpcError::Other(error) => internal(error.to_string()),
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
