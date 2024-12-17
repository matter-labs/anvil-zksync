use anyhow::Context;
use chrono::{DateTime, Utc};
use futures::Future;
use jsonrpc_core::{Error, ErrorCode};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::{convert::TryInto, fmt, pin::Pin};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};
use zksync_multivm::interface::{Call, CallType, ExecutionResult, VmExecutionResultAndLogs};
use zksync_types::{
    api::{BlockNumber, DebugCall, DebugCallType},
    l2::L2Tx,
    web3::Bytes,
    CONTRACT_DEPLOYER_ADDRESS, H256, U256, U64,
};
use zksync_utils::bytes_to_be_words;
use zksync_web3_decl::error::Web3Error;

pub(crate) trait IntoBoxedFuture: Sized + Send + 'static {
    fn into_boxed_future(self) -> Pin<Box<dyn Future<Output = Self> + Send>> {
        Box::pin(async { self })
    }
}

impl<T, U> IntoBoxedFuture for Result<T, U>
where
    T: Send + 'static,
    U: Send + 'static,
{
}

/// Takes long integers and returns them in human friendly format with "_".
/// For example: 12_334_093
pub fn to_human_size(input: U256) -> String {
    let input = format!("{:?}", input);
    let tmp: Vec<_> = input
        .chars()
        .rev()
        .enumerate()
        .flat_map(|(index, val)| {
            if index > 0 && index % 3 == 0 {
                vec!['_', val]
            } else {
                vec![val]
            }
        })
        .collect();
    tmp.iter().rev().collect()
}

pub fn bytecode_to_factory_dep(bytecode: Vec<u8>) -> Result<(U256, Vec<U256>), anyhow::Error> {
    zksync_utils::bytecode::validate_bytecode(&bytecode).context("Invalid bytecode")?;
    let bytecode_hash = zksync_utils::bytecode::hash_bytecode(&bytecode);
    let bytecode_hash = U256::from_big_endian(bytecode_hash.as_bytes());

    let bytecode_words = bytes_to_be_words(bytecode);

    Ok((bytecode_hash, bytecode_words))
}

/// Returns the actual [U64] block number from [BlockNumber].
///
/// # Arguments
///
/// * `block_number` - [BlockNumber] for a block.
/// * `latest_block_number` - A [U64] representing the latest block number.
///
/// # Returns
///
/// A [U64] representing the input block number.
pub fn to_real_block_number(block_number: BlockNumber, latest_block_number: U64) -> U64 {
    match block_number {
        BlockNumber::Finalized
        | BlockNumber::Pending
        | BlockNumber::Committed
        | BlockNumber::L1Committed
        | BlockNumber::Latest => latest_block_number,
        BlockNumber::Earliest => U64::zero(),
        BlockNumber::Number(n) => n,
    }
}

/// Returns a [jsonrpc_core::Error] indicating that the method is not implemented.
pub fn not_implemented<T: Send + 'static>(
    method_name: &str,
) -> jsonrpc_core::BoxFuture<Result<T, jsonrpc_core::Error>> {
    tracing::warn!("Method {} is not implemented", method_name);
    Err(jsonrpc_core::Error {
        data: None,
        code: jsonrpc_core::ErrorCode::MethodNotFound,
        message: format!("Method {} is not implemented", method_name),
    })
    .into_boxed_future()
}

/// Creates a [DebugCall] from a [L2Tx], [VmExecutionResultAndLogs] and a list of [Call]s.
pub fn create_debug_output(
    l2_tx: &L2Tx,
    result: &VmExecutionResultAndLogs,
    traces: Vec<Call>,
) -> Result<DebugCall, Web3Error> {
    let calltype = if l2_tx
        .recipient_account()
        .map(|addr| addr == CONTRACT_DEPLOYER_ADDRESS)
        .unwrap_or_default()
    {
        DebugCallType::Create
    } else {
        DebugCallType::Call
    };
    match &result.result {
        ExecutionResult::Success { output } => Ok(DebugCall {
            gas_used: result.statistics.gas_used.into(),
            output: output.clone().into(),
            r#type: calltype,
            from: l2_tx.initiator_account(),
            to: l2_tx.recipient_account().unwrap_or_default(),
            gas: l2_tx.common_data.fee.gas_limit,
            value: l2_tx.execute.value,
            input: l2_tx.execute.calldata().into(),
            error: None,
            revert_reason: None,
            calls: traces.into_iter().map(call_to_debug_call).collect(),
        }),
        ExecutionResult::Revert { output } => Ok(DebugCall {
            gas_used: result.statistics.gas_used.into(),
            output: output.encoded_data().into(),
            r#type: calltype,
            from: l2_tx.initiator_account(),
            to: l2_tx.recipient_account().unwrap_or_default(),
            gas: l2_tx.common_data.fee.gas_limit,
            value: l2_tx.execute.value,
            input: l2_tx.execute.calldata().into(),
            error: None,
            revert_reason: Some(output.to_string()),
            calls: traces.into_iter().map(call_to_debug_call).collect(),
        }),
        ExecutionResult::Halt { reason } => Err(Web3Error::SubmitTransactionError(
            reason.to_string(),
            vec![],
        )),
    }
}

fn call_to_debug_call(value: Call) -> DebugCall {
    let calls = value.calls.into_iter().map(call_to_debug_call).collect();
    let debug_type = match value.r#type {
        CallType::Call(_) => DebugCallType::Call,
        CallType::Create => DebugCallType::Create,
        CallType::NearCall => unreachable!("We have to filter our near calls before"),
    };
    DebugCall {
        r#type: debug_type,
        from: value.from,
        to: value.to,
        gas: U256::from(value.gas),
        gas_used: U256::from(value.gas_used),
        value: value.value,
        output: Bytes::from(value.output.clone()),
        input: Bytes::from(value.input.clone()),
        error: value.error.clone(),
        revert_reason: value.revert_reason,
        calls,
    }
}

/// Converts a timestamp in milliseconds since epoch to a [DateTime] in UTC.
pub fn utc_datetime_from_epoch_ms(millis: u64) -> DateTime<Utc> {
    let secs = millis / 1000;
    let nanos = (millis % 1000) * 1_000_000;
    // expect() is ok- nanos can't be >2M
    DateTime::<Utc>::from_timestamp(secs as i64, nanos as u32).expect("valid timestamp")
}

pub fn report_into_jsrpc_error(error: eyre::Report) -> Error {
    into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
        error.to_string(),
    )))
}

pub fn into_jsrpc_error(err: Web3Error) -> Error {
    Error {
        code: match err {
            Web3Error::InternalError(_) | Web3Error::MethodNotImplemented => {
                ErrorCode::InternalError
            }
            Web3Error::NoBlock
            | Web3Error::PrunedBlock(_)
            | Web3Error::PrunedL1Batch(_)
            | Web3Error::ProxyError(_)
            | Web3Error::TooManyTopics
            | Web3Error::FilterNotFound
            | Web3Error::LogsLimitExceeded(_, _, _)
            | Web3Error::InvalidFilterBlockHash
            | Web3Error::TreeApiUnavailable => ErrorCode::InvalidParams,
            Web3Error::SubmitTransactionError(_, _) | Web3Error::SerializationError(_) => {
                ErrorCode::ServerError(3)
            }
        },
        message: match &err {
            Web3Error::SubmitTransactionError(_, _) => err.to_string(),
            Web3Error::InternalError(err) => {
                if let Some(TransparentError(message)) = err.downcast_ref() {
                    message.clone()
                } else {
                    err.to_string()
                }
            }
            _ => err.to_string(),
        },
        data: match err {
            Web3Error::SubmitTransactionError(_, data) => {
                Some(format!("0x{}", hex::encode(data)).into())
            }
            _ => None,
        },
    }
}

/// Error that can be converted to a [`Web3Error`] and has transparent JSON-RPC error message (unlike `anyhow::Error` conversions).
#[derive(Debug)]
pub(crate) struct TransparentError(pub String);

impl fmt::Display for TransparentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TransparentError {}

impl From<TransparentError> for Web3Error {
    fn from(err: TransparentError) -> Self {
        Self::InternalError(err.into())
    }
}

pub fn into_jsrpc_error_message(msg: String) -> Error {
    Error {
        code: ErrorCode::InternalError,
        message: msg,
        data: None,
    }
}

pub fn internal_error(method_name: &'static str, error: impl fmt::Display) -> Web3Error {
    tracing::error!("Internal error in method {method_name}: {error}");
    Web3Error::InternalError(anyhow::Error::msg(error.to_string()))
}

// pub fn addresss_from_private_key(private_key: &K256PrivateKey) {
//     let private_key = H256::from_slice(&private_key.0);
//     let address = KeyPair::from_secret(private_key)?.address();
//     Ok(Address::from(address.0))
// }

/// Converts `h256` value as BE into the u64
pub fn h256_to_u64(value: H256) -> u64 {
    let be_u64_bytes: [u8; 8] = value[24..].try_into().unwrap();
    u64::from_be_bytes(be_u64_bytes)
}

/// Helper type to be able to parse both `u64` and `U256` depending on the user input
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Numeric {
    /// A [U256] value.
    U256(U256),
    /// A `u64` value.
    Num(u64),
}

impl From<u64> for Numeric {
    fn from(value: u64) -> Self {
        Numeric::Num(value)
    }
}

impl TryFrom<Numeric> for u64 {
    type Error = anyhow::Error;

    fn try_from(value: Numeric) -> Result<Self, Self::Error> {
        match value {
            Numeric::U256(n) => {
                if n >= U256::from(u64::MAX) {
                    return Err(anyhow::anyhow!("Number is too big"));
                }
                Ok(n.as_u64())
            }
            Numeric::Num(n) => Ok(n),
        }
    }
}

/// Calculates the cost of a transaction in ETH.
pub fn calculate_eth_cost(gas_price_in_wei_per_gas: u64, gas_used: u64) -> f64 {
    // Convert gas price from wei to gwei
    let gas_price_in_gwei = gas_price_in_wei_per_gas as f64 / 1e9;

    // Calculate total cost in gwei
    let total_cost_in_gwei = gas_price_in_gwei * gas_used as f64;

    // Convert total cost from gwei to ETH
    total_cost_in_gwei / 1e9
}

/// Writes the given serializable object as JSON to the specified file path using pretty printing.
/// Returns an error if the file cannot be created or if serialization/writing fails.
pub fn write_json_file<T: Serialize>(path: &Path, obj: &T) -> anyhow::Result<()> {
    let file = File::create(path)
        .with_context(|| format!("Failed to create file '{}'", path.display()))?;
    let mut writer = BufWriter::new(file);
    // Note: intentionally using pretty printing for better readability.
    serde_json::to_writer_pretty(&mut writer, obj)
        .with_context(|| format!("Failed to write JSON to '{}'", path.display()))?;
    writer
        .flush()
        .with_context(|| format!("Failed to flush writer for '{}'", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use zksync_types::U256;

    use super::*;

    #[test]
    fn test_utc_datetime_from_epoch_ms() {
        let actual = utc_datetime_from_epoch_ms(1623931200000);
        assert_eq!(DateTime::from_timestamp(1623931200, 0).unwrap(), actual);
    }

    #[test]
    fn test_human_sizes() {
        assert_eq!("123", to_human_size(U256::from(123u64)));
        assert_eq!("1_234", to_human_size(U256::from(1234u64)));
        assert_eq!("12_345", to_human_size(U256::from(12345u64)));
        assert_eq!("0", to_human_size(U256::from(0)));
        assert_eq!("1", to_human_size(U256::from(1)));
        assert_eq!("50_000_000", to_human_size(U256::from(50000000u64)));
    }

    #[test]
    fn test_to_real_block_number_finalized() {
        let actual = to_real_block_number(BlockNumber::Finalized, U64::from(10));
        assert_eq!(U64::from(10), actual);
    }

    #[test]
    fn test_to_real_block_number_pending() {
        let actual = to_real_block_number(BlockNumber::Pending, U64::from(10));
        assert_eq!(U64::from(10), actual);
    }

    #[test]
    fn test_to_real_block_number_committed() {
        let actual = to_real_block_number(BlockNumber::Committed, U64::from(10));
        assert_eq!(U64::from(10), actual);
    }

    #[test]
    fn test_to_real_block_number_latest() {
        let actual = to_real_block_number(BlockNumber::Latest, U64::from(10));
        assert_eq!(U64::from(10), actual);
    }

    #[test]
    fn test_to_real_block_number_earliest() {
        let actual = to_real_block_number(BlockNumber::Earliest, U64::from(10));
        assert_eq!(U64::zero(), actual);
    }

    #[test]
    fn test_to_real_block_number_number() {
        let actual = to_real_block_number(BlockNumber::Number(U64::from(5)), U64::from(10));
        assert_eq!(U64::from(5), actual);
    }
}
