use anvil_zksync_common::sh_err;
use chrono::{DateTime, Utc};
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use tokio::runtime::Builder;
use tokio::sync::{RwLock, RwLockReadGuard};
use zksync_multivm::interface::{Call, CallType, ExecutionResult, VmExecutionResultAndLogs};
use zksync_types::{
    CONTRACT_DEPLOYER_ADDRESS, Transaction, U64, U256,
    api::{BlockNumber, DebugCall, DebugCallType},
    web3::Bytes,
};
use zksync_web3_decl::error::Web3Error;

/// Takes long integers and returns them in human friendly format with "_".
/// For example: 12_334_093
pub fn to_human_size(input: U256) -> String {
    let input = format!("{input:?}");
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
        BlockNumber::FastFinalized
        | BlockNumber::Finalized
        | BlockNumber::Pending
        | BlockNumber::Committed
        | BlockNumber::L1Committed
        | BlockNumber::Latest => latest_block_number,
        BlockNumber::Earliest => U64::zero(),
        BlockNumber::Number(n) => n,
    }
}

/// Creates a [DebugCall] from a [L2Tx], [VmExecutionResultAndLogs] and a list of [Call]s.
pub fn create_debug_output(
    tx: &Transaction,
    result: &VmExecutionResultAndLogs,
    traces: Vec<Call>,
) -> Result<DebugCall, Web3Error> {
    let calltype = if tx
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
            from: tx.initiator_account(),
            to: tx.recipient_account().unwrap_or_default(),
            gas: tx.gas_limit(),
            value: tx.execute.value,
            input: tx.execute.calldata().into(),
            error: None,
            revert_reason: None,
            calls: traces.into_iter().map(call_to_debug_call).collect(),
        }),
        ExecutionResult::Revert { output } => Ok(DebugCall {
            gas_used: result.statistics.gas_used.into(),
            output: output.encoded_data().into(),
            r#type: calltype,
            from: tx.initiator_account(),
            to: tx.recipient_account().unwrap_or_default(),
            gas: tx.gas_limit(),
            value: tx.execute.value,
            input: tx.execute.calldata().into(),
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

pub fn internal_error(method_name: &'static str, error: impl fmt::Display) -> Web3Error {
    sh_err!("Internal error in method {method_name}: {error}");
    Web3Error::InternalError(anyhow::Error::msg(error.to_string()))
}

pub fn block_on<F: Future + Send + 'static>(future: F) -> F::Output
where
    F::Output: Send,
{
    std::thread::spawn(move || {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime creation failed");
        runtime.block_on(future)
    })
    .join()
    .unwrap()
}

/// A special version of `Arc<RwLock<T>>` that can only be read from.
#[derive(Debug)]
pub struct ArcRLock<T>(Arc<RwLock<T>>);

impl<T> Clone for ArcRLock<T> {
    fn clone(&self) -> Self {
        ArcRLock(self.0.clone())
    }
}

impl<T> ArcRLock<T> {
    /// Wrap writeable `Arc<RwLock<T>>` into a read-only `ArcRLock<T>`.
    pub fn wrap(inner: Arc<RwLock<T>>) -> Self {
        Self(inner)
    }

    /// Locks this `ArcRLock` with shared read access, causing the current task
    /// to yield until the lock has been acquired.
    pub async fn read(&self) -> RwLockReadGuard<T> {
        self.0.read().await
    }
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
