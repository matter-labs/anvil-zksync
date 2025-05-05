use jsonrpsee::types::{ErrorCode, ErrorObjectOwned};
use zksync_error::{
    anvil_zksync::{node::AnvilNodeError, rpc::RpcError, state::StateLoaderError},
    core::web3::Web3Error,
    CustomErrorMessage as _, ICustomError as _, IError as _,
};
use zksync_types::{L1BatchNumber, L2BlockNumber};
use zksync_web3_decl::error::Web3Error as LegacyWeb3Error;

// TODO This datatype may be erased once we port Web3Error from ZKsync to
// zksync-error
#[derive(Debug, thiserror::Error)]
pub enum JsonRpcAdapter {
    LegacyWeb3(#[from] LegacyWeb3Error),
    Node(#[from] AnvilNodeError),
    Web3(#[from] Web3Error),
    LoadState(#[from] StateLoaderError),
    Rpc(#[from] RpcError),
    Anyhow(#[from] anyhow::Error),
}

impl std::fmt::Display for JsonRpcAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{self:#?}"))
    }
}

impl From<JsonRpcAdapter> for ErrorObjectOwned {
    fn from(val: JsonRpcAdapter) -> Self {
        let rpc_error = match val {
            JsonRpcAdapter::LegacyWeb3(web3_error) => RpcError::Web3Error {
                inner: Box::new(legacy_web3_adapter(web3_error)),
            },
            JsonRpcAdapter::Node(anvil_node) => RpcError::NodeError {
                inner: Box::new(anvil_node),
            },
            JsonRpcAdapter::Web3(web3) => RpcError::Web3Error {
                inner: Box::new(web3),
            },
            JsonRpcAdapter::LoadState(state_loader) => RpcError::LoadStateError {
                inner: Box::new(state_loader),
            },
            JsonRpcAdapter::Rpc(rpc) => rpc,
            JsonRpcAdapter::Anyhow(error) => RpcError::GenericError {
                message: error.to_string(),
            },
        };
        into_json_rpc(rpc_error)
    }
}

fn legacy_web3_adapter(error: LegacyWeb3Error) -> Web3Error {
    match error {
        LegacyWeb3Error::NoBlock => Web3Error::NoBlock,
        LegacyWeb3Error::PrunedBlock(L2BlockNumber(first_retained_block)) => {
            Web3Error::PrunedBlock {
                first_retained_block,
            }
        }
        LegacyWeb3Error::PrunedL1Batch(L1BatchNumber(first_retained_batch)) => {
            Web3Error::PrunedL1Batch {
                first_retained_batch,
            }
        }
        LegacyWeb3Error::ProxyError(enriched_client_error) => Web3Error::ProxyError {
            details: enriched_client_error.to_string(),
        },
        LegacyWeb3Error::SubmitTransactionError(error, data) => {
            Web3Error::SubmitTransactionError { error, data }
        }
        LegacyWeb3Error::SerializationError(serialization_transaction_error) => {
            Web3Error::SerializationError {
                details: serialization_transaction_error.to_string(),
            }
        }
        LegacyWeb3Error::TooManyTopics => Web3Error::TooManyTopics,
        LegacyWeb3Error::FilterNotFound => Web3Error::FilterNotFound,
        LegacyWeb3Error::LogsLimitExceeded(limit, from_block, to_block) => {
            Web3Error::LogsLimitExceeded {
                limit: limit as u32,
                from_block,
                to_block,
            }
        }
        LegacyWeb3Error::InvalidFilterBlockHash => Web3Error::InvalidFilterBlockHash,
        LegacyWeb3Error::MethodNotImplemented => Web3Error::MethodNotImplemented,
        LegacyWeb3Error::TreeApiUnavailable => Web3Error::TreeApiUnavailable,
        LegacyWeb3Error::InternalError(error) => Web3Error::GenericError {
            message: error.to_string(),
        },
        LegacyWeb3Error::ServerShuttingDown => Web3Error::ServerShuttingDown,
    }
}

fn web3_into_json_rpc(error: &Web3Error) -> ErrorObjectOwned {
    let message = error.get_message();
    let data = error.to_unified();
    let code = code_remap::web3(error.into());
    ErrorObjectOwned::owned(code.code(), message, Some(data))
}

pub fn into_json_rpc(error: RpcError) -> ErrorObjectOwned {
    match &error {
        RpcError::LoadStateError { inner } => ErrorObjectOwned::owned(
            inner.to_unified().get_identifier().encode() as i32,
            inner.get_message(),
            Some(inner),
        ),
        RpcError::UnsupportedMethod { .. } => ErrorObjectOwned::owned(
            ErrorCode::MethodNotFound.code(),
            // May substitute the line for the following if we don't care about
            // returning exactly the MethodNotFound code from JSON RPC standard.
            // error.to_unified().get_identifier().encode() as i32,
            error.get_message(),
            Some(error.to_unified()),
        ),
        RpcError::GenericError { .. } => ErrorObjectOwned::owned(
            // May substitute the line for the following if we don't care about
            // returning exactly the MethodNotFound code from JSON RPC standard.
            // error.to_unified().get_identifier().encode() as i32,
            ErrorCode::InternalError.code(),
            error.get_message(),
            Some(error.to_unified()),
        ),
        RpcError::NodeError { .. } => ErrorObjectOwned::owned(
            // May substitute the line for the following if we don't care about
            // returning exactly the MethodNotFound code from JSON RPC standard.
            // error.to_unified().get_identifier().encode() as i32,
            ErrorCode::InternalError.code(),
            error.get_message(),
            Some(error.to_unified()),
        ),
        RpcError::Web3Error { inner } => web3_into_json_rpc(inner),
        other => todo!("Unsupported RPC error type: {other:#?}. This is a bug; please, report it."),
    }
}

pub fn rpc_unsupported<T>(method_name: &str) -> jsonrpsee::core::RpcResult<T> {
    Err(JsonRpcAdapter::Rpc(RpcError::UnsupportedMethod {
        method_name: method_name.to_owned(),
    })
    .into())
}

mod code_remap {
    use jsonrpsee::types::ErrorCode as RpcErrorCode;
    use zksync_error::core::web3::ErrorCode as Web3ErrorCode;

    pub(super) fn web3(error: Web3ErrorCode) -> RpcErrorCode {
        // May erase this method if we don't care about mapping error codes to
        // jsonrpc standard whenever possible.
        match error {
            Web3ErrorCode::MethodNotImplemented => RpcErrorCode::MethodNotFound,
            Web3ErrorCode::GenericError => RpcErrorCode::InternalError,
            Web3ErrorCode::NoBlock
            | Web3ErrorCode::PrunedBlock
            | Web3ErrorCode::PrunedL1Batch
            | Web3ErrorCode::ProxyError
            | Web3ErrorCode::TooManyTopics
            | Web3ErrorCode::FilterNotFound
            | Web3ErrorCode::LogsLimitExceeded
            | Web3ErrorCode::InvalidFilterBlockHash
            | Web3ErrorCode::TreeApiUnavailable => RpcErrorCode::InvalidParams,
            Web3ErrorCode::SubmitTransactionError | Web3ErrorCode::SerializationError => {
                RpcErrorCode::ServerError(3)
            }
            Web3ErrorCode::ServerShuttingDown => RpcErrorCode::ServerIsBusy,
        }
    }
}
