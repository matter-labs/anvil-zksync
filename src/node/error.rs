use crate::utils::into_jsrpc_error;
use zksync_web3_decl::error::Web3Error;

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

impl From<LoadStateError> for jsonrpc_core::Error {
    fn from(value: LoadStateError) -> Self {
        match value {
            err @ LoadStateError::HasExistingState
            | err @ LoadStateError::EmptyState
            | err @ LoadStateError::FailedDecompress(_)
            | err @ LoadStateError::FailedDeserialize(_)
            | err @ LoadStateError::UnknownStateVersion(_) => {
                jsonrpc_core::Error::invalid_params(err.to_string())
            }
            LoadStateError::Other(err) => {
                tracing::error!("failed loading state: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(err))
            }
        }
    }
}
