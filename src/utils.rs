use std::collections::HashMap;
use std::convert::TryInto;
use std::pin::Pin;

use chrono::{DateTime, Utc};
use futures::Future;
use multivm::interface::{ExecutionResult, VmExecutionResultAndLogs, VmInterface};
use multivm::vm_latest::HistoryDisabled;
use multivm::vm_latest::{utils::fee::derive_base_fee_and_gas_per_pubdata, Vm};
use zksync_basic_types::{H256, U256, U64};
use zksync_state::WriteStorage;
use zksync_types::api::{BlockNumber, DebugCall, DebugCallType};
use zksync_types::l2::L2Tx;
use zksync_types::vm_trace::Call;
use zksync_types::CONTRACT_DEPLOYER_ADDRESS;
use zksync_utils::u256_to_h256;
use zksync_utils::{bytecode::hash_bytecode, bytes_to_be_words};
use zksync_web3_decl::error::Web3Error;

use crate::deps::storage_view::StorageView;
use crate::node::create_empty_block;
use crate::{fork::ForkSource, node::InMemoryNodeInner};

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

/// Adjusts the L1 gas price for a transaction based on the current pubdata price and the fair L2 gas price.
/// If the current pubdata price is small enough, returns the original L1 gas price.
/// Otherwise, calculates a new L1 gas price based on the fair L2 gas price and the transaction gas per pubdata limit.
///
/// # Arguments
///
/// * `l1_gas_price` - The original L1 gas price.
/// * `fair_l2_gas_price` - The fair L2 gas price.
/// * `tx_gas_per_pubdata_limit` - The transaction gas per pubdata limit.
///
/// # Returns
///
/// The adjusted L1 gas price.
pub fn adjust_l1_gas_price_for_tx(
    l1_gas_price: u64,
    fair_l2_gas_price: u64,
    tx_gas_per_pubdata_limit: U256,
) -> u64 {
    let (_, current_pubdata_price) =
        derive_base_fee_and_gas_per_pubdata(l1_gas_price, fair_l2_gas_price);
    if U256::from(current_pubdata_price) <= tx_gas_per_pubdata_limit {
        // The current pubdata price is small enough
        l1_gas_price
    } else {
        let l1_gas_price = U256::from(fair_l2_gas_price)
            * (tx_gas_per_pubdata_limit - U256::from(1u32))
            / U256::from(17);

        l1_gas_price.as_u64()
    }
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

pub fn bytecode_to_factory_dep(bytecode: Vec<u8>) -> (U256, Vec<U256>) {
    let bytecode_hash = hash_bytecode(&bytecode);
    let bytecode_hash = U256::from_big_endian(bytecode_hash.as_bytes());

    let bytecode_words = bytes_to_be_words(bytecode);

    (bytecode_hash, bytecode_words)
}

/// Creates and inserts a given number of empty blocks into the node, with a given interval between them.
/// The blocks will be empty (contain no transactions).
/// Currently this is quite slow - as we invoke the VM for each operation, in the future we might want to optimise it
/// by adding a way to set state via some system contract call.
pub fn mine_empty_blocks<S: std::fmt::Debug + ForkSource>(
    node: &mut InMemoryNodeInner<S>,
    num_blocks: u64,
    interval_ms: u64,
) {
    // build and insert new blocks
    for i in 0..num_blocks {
        // roll the vm
        let (keys, bytecodes, block_ctx) = {
            let storage = StorageView::new(&node.fork_storage).into_rc_ptr();

            // system_contract.contracts_for_l2_call() will give playground contracts
            // we need these to use the unsafeOverrideBlock method in SystemContext.sol
            let bootloader_code = node.system_contracts.contracts_for_l2_call();
            let (batch_env, mut block_ctx) = node.create_l1_batch_env(storage.clone());
            // override the next block's timestamp to match up with interval for subsequent blocks
            if i != 0 {
                block_ctx.timestamp = node.current_timestamp.saturating_add(interval_ms);
            }

            // init vm
            let system_env = node.create_system_env(
                bootloader_code.clone(),
                multivm::interface::TxExecutionMode::VerifyExecute,
            );

            let mut vm: Vm<_, HistoryDisabled> = Vm::new(batch_env, system_env, storage.clone());

            vm.execute(multivm::interface::VmExecutionMode::Bootloader);

            let bytecodes: HashMap<U256, Vec<U256>> = vm
                .get_last_tx_compressed_bytecodes()
                .iter()
                .map(|b| bytecode_to_factory_dep(b.original.clone()))
                .collect();
            let modified_keys = storage.borrow().modified_storage_keys().clone();
            (modified_keys, bytecodes, block_ctx)
        };

        for (key, value) in keys.iter() {
            node.fork_storage.set_value(*key, *value);
        }

        // Write all the factory deps.
        for (hash, code) in bytecodes.iter() {
            node.fork_storage.store_factory_dep(
                u256_to_h256(*hash),
                code.iter()
                    .flat_map(|entry| {
                        let mut bytes = vec![0u8; 32];
                        entry.to_big_endian(&mut bytes);
                        bytes.to_vec()
                    })
                    .collect(),
            )
        }

        let block = create_empty_block(
            block_ctx.miniblock,
            block_ctx.timestamp,
            block_ctx.batch,
            None,
        );

        node.block_hashes.insert(block.number.as_u64(), block.hash);
        node.blocks.insert(block.hash, block);

        // leave node state ready for next interaction
        node.current_batch = block_ctx.batch;
        node.current_miniblock = block_ctx.miniblock;
        node.current_timestamp = block_ctx.timestamp;
    }
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
    let calltype = if l2_tx.recipient_account() == CONTRACT_DEPLOYER_ADDRESS {
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
            to: l2_tx.recipient_account(),
            gas: l2_tx.common_data.fee.gas_limit,
            value: l2_tx.execute.value,
            input: l2_tx.execute.calldata().into(),
            error: None,
            revert_reason: None,
            calls: traces.into_iter().map(Into::into).collect(),
        }),
        ExecutionResult::Revert { output } => Ok(DebugCall {
            gas_used: result.statistics.gas_used.into(),
            output: Default::default(),
            r#type: calltype,
            from: l2_tx.initiator_account(),
            to: l2_tx.recipient_account(),
            gas: l2_tx.common_data.fee.gas_limit,
            value: l2_tx.execute.value,
            input: l2_tx.execute.calldata().into(),
            error: None,
            revert_reason: Some(output.to_string()),
            calls: traces.into_iter().map(Into::into).collect(),
        }),
        ExecutionResult::Halt { reason } => Err(Web3Error::SubmitTransactionError(
            reason.to_string(),
            vec![],
        )),
    }
}

/// Converts a timestamp in milliseconds since epoch to a [DateTime] in UTC.
pub fn utc_datetime_from_epoch_ms(millis: u64) -> DateTime<Utc> {
    let secs = millis / 1000;
    let nanos = (millis % 1000) * 1_000_000;
    // expect() is ok- nanos can't be >2M
    DateTime::<Utc>::from_timestamp(secs as i64, nanos as u32).expect("valid timestamp")
}

#[cfg(test)]
mod tests {
    use zksync_basic_types::{H256, U256};

    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode, testing};

    use super::*;

    #[test]
    fn test_utc_datetime_from_epoch_ms() {
        let actual = utc_datetime_from_epoch_ms(1623931200000);
        assert_eq!(
            DateTime::<Utc>::from_naive_utc_and_offset(
                chrono::NaiveDateTime::from_timestamp_opt(1623931200, 0).unwrap(),
                Utc
            ),
            actual
        );
    }

    #[test]
    fn test_human_sizes() {
        assert_eq!("123", to_human_size(U256::from(123u64)));
        assert_eq!("1_234", to_human_size(U256::from(1234u64)));
        assert_eq!("12_345", to_human_size(U256::from(12345u64)));
        assert_eq!("0", to_human_size(U256::from(0)));
        assert_eq!("1", to_human_size(U256::from(1)));
        assert_eq!("250_000_000", to_human_size(U256::from(250000000u64)));
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

    #[test]
    fn test_mine_empty_blocks_mines_the_first_block_immediately() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();

        let starting_block = {
            let reader = inner.read().expect("failed acquiring reader");
            reader
                .block_hashes
                .get(&reader.current_miniblock)
                .and_then(|hash| reader.blocks.get(hash))
                .expect("failed finding block")
                .clone()
        };
        assert_eq!(U64::from(0), starting_block.number);
        assert_eq!(Some(U64::from(0)), starting_block.l1_batch_number);
        assert_eq!(U256::from(1000), starting_block.timestamp);

        {
            let mut writer = inner.write().expect("failed acquiring write lock");
            mine_empty_blocks(&mut writer, 1, 1000);
        }

        let reader = inner.read().expect("failed acquiring reader");
        let mined_block = reader
            .block_hashes
            .get(&1)
            .and_then(|hash| reader.blocks.get(hash))
            .expect("failed finding block");
        assert_eq!(U64::from(1), mined_block.number);
        assert_eq!(Some(U64::from(1)), mined_block.l1_batch_number);
        assert_eq!(U256::from(1001), mined_block.timestamp);
    }

    #[test]
    fn test_mine_empty_blocks_mines_2_blocks_with_interval() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();

        let starting_block = {
            let reader = inner.read().expect("failed acquiring reader");
            reader
                .block_hashes
                .get(&reader.current_miniblock)
                .and_then(|hash| reader.blocks.get(hash))
                .expect("failed finding block")
                .clone()
        };
        assert_eq!(U64::from(0), starting_block.number);
        assert_eq!(Some(U64::from(0)), starting_block.l1_batch_number);
        assert_eq!(U256::from(1000), starting_block.timestamp);

        {
            let mut writer = inner.write().expect("failed acquiring write lock");
            mine_empty_blocks(&mut writer, 2, 1000);
        }

        let reader = inner.read().expect("failed acquiring reader");
        let mined_block_1 = reader
            .block_hashes
            .get(&1)
            .and_then(|hash| reader.blocks.get(hash))
            .expect("failed finding block 1");
        assert_eq!(U64::from(1), mined_block_1.number);
        assert_eq!(Some(U64::from(1)), mined_block_1.l1_batch_number);
        assert_eq!(U256::from(1001), mined_block_1.timestamp);

        let mined_block_2 = reader
            .block_hashes
            .get(&2)
            .and_then(|hash| reader.blocks.get(hash))
            .expect("failed finding block 2");
        assert_eq!(U64::from(2), mined_block_2.number);
        assert_eq!(Some(U64::from(2)), mined_block_2.l1_batch_number);
        assert_eq!(U256::from(2001), mined_block_2.timestamp);
    }

    #[test]
    fn test_mine_empty_blocks_mines_2_blocks_with_interval_and_next_block_immediately() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();

        let starting_block = {
            let reader = inner.read().expect("failed acquiring reader");
            reader
                .block_hashes
                .get(&reader.current_miniblock)
                .and_then(|hash| reader.blocks.get(hash))
                .expect("failed finding block")
                .clone()
        };
        assert_eq!(U64::from(0), starting_block.number);
        assert_eq!(Some(U64::from(0)), starting_block.l1_batch_number);
        assert_eq!(U256::from(1000), starting_block.timestamp);

        {
            let mut writer = inner.write().expect("failed acquiring write lock");
            mine_empty_blocks(&mut writer, 2, 1000);
        }

        {
            let reader = inner.read().expect("failed acquiring reader");
            let mined_block_1 = reader
                .block_hashes
                .get(&1)
                .and_then(|hash| reader.blocks.get(hash))
                .expect("failed finding block 1");
            assert_eq!(U64::from(1), mined_block_1.number);
            assert_eq!(Some(U64::from(1)), mined_block_1.l1_batch_number);
            assert_eq!(U256::from(1001), mined_block_1.timestamp);

            let mined_block_2 = reader
                .block_hashes
                .get(&2)
                .and_then(|hash| reader.blocks.get(hash))
                .expect("failed finding block 2");
            assert_eq!(U64::from(2), mined_block_2.number);
            assert_eq!(Some(U64::from(2)), mined_block_2.l1_batch_number);
            assert_eq!(U256::from(2001), mined_block_2.timestamp);
        }

        {
            testing::apply_tx(&node, H256::repeat_byte(0x1));
            let reader = inner.read().expect("failed acquiring reader");
            let tx_block_3 = reader
                .block_hashes
                .get(&3)
                .and_then(|hash| reader.blocks.get(hash))
                .expect("failed finding block 2");
            assert_eq!(U64::from(3), tx_block_3.number);
            assert_eq!(Some(U64::from(3)), tx_block_3.l1_batch_number);
            assert_eq!(U256::from(2002), tx_block_3.timestamp);
        }
    }
}

/// Converts `h256` value as BE into the u64
pub fn h256_to_u64(value: H256) -> u64 {
    let be_u64_bytes: [u8; 8] = value[24..].try_into().unwrap();
    u64::from_be_bytes(be_u64_bytes)
}
