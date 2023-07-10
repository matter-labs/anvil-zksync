//! In-memory node, that supports forking other networks.
use crate::{
    console_log::ConsoleLogHandler,
    deps::{system_contracts::bytecode_from_slice, ReadStorage},
    fork::{ForkDetails, ForkStorage},
    formatter, ShowCalls,
};

use colored::Colorize;
use std::{
    collections::HashMap,
    convert::TryInto,
    sync::{Arc, RwLock},
};
use zksync_state::storage_view::StorageView;

use zksync_basic_types::{AccountTreeId, Bytes, H160, H256, U256, U64};
use zksync_contracts::{
    read_playground_block_bootloader_bytecode, read_sys_contract_bytecode, BaseSystemContracts,
    ContractLanguage, SystemContractCode,
};
use zksync_core::api_server::web3::backend_jsonrpc::namespaces::eth::EthNamespaceT;
use zksync_types::{
    api::{Log, TransactionReceipt, TransactionVariant},
    get_code_key, get_nonce_key,
    l2::L2Tx,
    transaction_request::{l2_tx_from_call_req, TransactionRequest},
    tx::tx_execution_info::TxExecutionStatus,
    utils::{storage_key_for_eth_balance, storage_key_for_standard_token_balance},
    vm_trace::VmTrace,
    zk_evm::block_properties::BlockProperties,
    StorageKey, StorageLogQueryType, Transaction, ACCOUNT_CODE_STORAGE_ADDRESS,
    L2_ETH_TOKEN_ADDRESS,
};
use zksync_utils::{
    bytecode::hash_bytecode, bytes_to_be_words, h256_to_account_address, h256_to_u256, h256_to_u64,
    u256_to_h256,
};

use vm::{
    utils::{BLOCK_GAS_LIMIT, ETH_CALL_GAS_LIMIT},
    vm::VmTxExecutionResult,
    vm_with_bootloader::{
        init_vm_inner, push_transaction_to_bootloader_memory, BlockContext, BlockContextMode,
        BootloaderJobType, TxExecutionMode,
    },
    HistoryEnabled, OracleTools,
};
use zksync_web3_decl::types::{Filter, FilterChanges};

pub const MAX_TX_SIZE: usize = 1000000;
// Timestamp of the first block (if not running in fork mode).
pub const NON_FORK_FIRST_BLOCK_TIMESTAMP: u64 = 1000;
/// Network ID we use for the test node.
pub const TEST_NODE_NETWORK_ID: u16 = 260;

/// Basic information about the generated block (which is block l1 batch and miniblock).
/// Currently, this test node supports exactly one transaction per block.
pub struct BlockInfo {
    pub batch_number: u32,
    pub block_timestamp: u64,
    /// Transaction included in this block.
    pub tx_hash: H256,
}

/// Information about the executed transaction.
pub struct TxExecutionInfo {
    pub tx: L2Tx,
    // Batch number where transaction was executed.
    pub batch_number: u32,
    pub miniblock_number: u64,
    pub result: VmTxExecutionResult,
}

/// Helper struct for InMemoryNode.
pub struct InMemoryNodeInner {
    /// Timestamp, batch number and miniblock number that will be used by the next block.
    pub current_timestamp: u64,
    pub current_batch: u32,
    pub current_miniblock: u64,
    pub l1_gas_price: u64,
    // Map from transaction to details about the exeuction
    pub tx_results: HashMap<H256, TxExecutionInfo>,
    // Map from batch number to information about the block.
    pub blocks: HashMap<u32, BlockInfo>,
    // Underlying storage
    pub fork_storage: ForkStorage,
    // Debug level information.
    pub show_calls: ShowCalls,
    // If true - will contact openchain to resolve the ABI to function names.
    pub resolve_hashes: bool,
    pub console_log_handler: ConsoleLogHandler,
    pub dev_use_local_contracts: bool,
    pub baseline_contracts: BaseSystemContracts,
    pub playground_contracts: BaseSystemContracts,
}

impl InMemoryNodeInner {
    fn create_block_context(&self) -> BlockContext {
        BlockContext {
            block_number: self.current_batch,
            block_timestamp: self.current_timestamp,
            l1_gas_price: self.l1_gas_price,
            fair_l2_gas_price: 250_000_000, // 0.25 gwei
            operator_address: H160::zero(),
        }
    }
    fn create_block_properties(contracts: &BaseSystemContracts) -> BlockProperties {
        BlockProperties {
            default_aa_code_hash: h256_to_u256(contracts.default_aa.hash),
            zkporter_is_available: false,
        }
    }
}

fn not_implemented<T: Send + 'static>(method_name: &str) -> Result<T, jsonrpc_core::Error> {
    println!("Method {} is not implemented", method_name);
    Err(jsonrpc_core::Error {
        data: None,
        code: jsonrpc_core::ErrorCode::MethodNotFound,
        message: format!("Method {} is not implemented", method_name),
    })
}

/// In-memory node, that can be used for local & unit testing.
/// It also supports the option of forking testnet/mainnet.
/// All contents are removed when object is destroyed.
pub struct InMemoryNode {
    inner: Arc<RwLock<InMemoryNodeInner>>,
}

fn bsc_load_with_bootloader(
    bootloader_bytecode: Vec<u8>,
    use_local_contracts: bool,
) -> BaseSystemContracts {
    let hash = hash_bytecode(&bootloader_bytecode);

    let bootloader = SystemContractCode {
        code: bytes_to_be_words(bootloader_bytecode),
        hash,
    };

    let bytecode = if use_local_contracts {
        read_sys_contract_bytecode("", "DefaultAccount", ContractLanguage::Sol)
    } else {
        bytecode_from_slice(
            "DefaultAccount",
            include_bytes!("deps/contracts/DefaultAccount.json"),
        )
    };
    let hash = hash_bytecode(&bytecode);

    let default_aa = SystemContractCode {
        code: bytes_to_be_words(bytecode),
        hash,
    };

    BaseSystemContracts {
        bootloader,
        default_aa,
    }
}

/// BaseSystemContracts with playground bootloader -  used for handling 'eth_calls'.
pub fn playground(use_local_contracts: bool) -> BaseSystemContracts {
    let bootloader_bytecode = if use_local_contracts {
        read_playground_block_bootloader_bytecode()
    } else {
        include_bytes!("deps/contracts/playground_block.yul.zbin").to_vec()
    };
    bsc_load_with_bootloader(bootloader_bytecode, use_local_contracts)
}

pub fn baseline_contracts(use_local_contracts: bool) -> BaseSystemContracts {
    let bootloader_bytecode = if use_local_contracts {
        read_playground_block_bootloader_bytecode()
    } else {
        include_bytes!("deps/contracts/proved_block.yul.zbin").to_vec()
    };
    bsc_load_with_bootloader(bootloader_bytecode, use_local_contracts)
}

fn contract_address_from_tx_result(execution_result: &VmTxExecutionResult) -> Option<H160> {
    for query in execution_result.result.logs.storage_logs.iter().rev() {
        if query.log_type == StorageLogQueryType::InitialWrite
            && query.log_query.address == ACCOUNT_CODE_STORAGE_ADDRESS
        {
            return Some(h256_to_account_address(&u256_to_h256(query.log_query.key)));
        }
    }
    None
}

impl InMemoryNode {
    pub fn new(
        fork: Option<ForkDetails>,
        show_calls: ShowCalls,
        resolve_hashes: bool,
        dev_use_local_contracts: bool,
    ) -> Self {
        InMemoryNode {
            inner: Arc::new(RwLock::new(InMemoryNodeInner {
                current_timestamp: fork
                    .as_ref()
                    .map(|f| f.block_timestamp + 1)
                    .unwrap_or(NON_FORK_FIRST_BLOCK_TIMESTAMP),
                current_batch: fork.as_ref().map(|f| f.l1_block.0 + 1).unwrap_or(1),
                current_miniblock: fork.as_ref().map(|f| f.l2_miniblock + 1).unwrap_or(1),
                l1_gas_price: fork
                    .as_ref()
                    .map(|f| f.l1_gas_price)
                    .unwrap_or(50_000_000_000),
                tx_results: Default::default(),
                blocks: Default::default(),
                fork_storage: ForkStorage::new(fork, dev_use_local_contracts),
                show_calls,
                resolve_hashes,
                console_log_handler: ConsoleLogHandler::default(),
                dev_use_local_contracts,
                playground_contracts: playground(dev_use_local_contracts),
                baseline_contracts: baseline_contracts(dev_use_local_contracts),
            })),
        }
    }

    /// Applies multiple transactions - but still one per L1 batch.
    pub fn apply_txs(&self, txs: Vec<L2Tx>) {
        println!("Running {:?} transactions (one per batch)", txs.len());

        for tx in txs {
            self.run_l2_tx(tx, TxExecutionMode::VerifyExecute);
        }
    }

    /// Adds a lot of tokens to a given account.
    pub fn set_rich_account(&self, address: H160) {
        let key = storage_key_for_eth_balance(&address);
        let mut inner = self.inner.write().unwrap();
        let keys = {
            let mut storage_view = StorageView::new(&inner.fork_storage);
            storage_view.set_value(&key, u256_to_h256(U256::from(10u64.pow(19))));
            storage_view.get_modified_storage_keys().clone()
        };

        for (key, value) in keys.iter() {
            inner.fork_storage.set_value(*key, *value);
        }
    }

    /// Runs L2 'eth call' method - that doesn't commit to a block.
    fn run_l2_call(&self, l2_tx: L2Tx) -> Vec<u8> {
        let execution_mode = TxExecutionMode::EthCall {
            missed_storage_invocation_limit: 1000000,
        };

        let inner = self.inner.write().unwrap();
        let block_context = inner.create_block_context();
        let bootloader_code = &inner.playground_contracts;

        let block_properties = InMemoryNodeInner::create_block_properties(bootloader_code);

        let mut storage_view = StorageView::new(&inner.fork_storage);

        let mut oracle_tools = OracleTools::new(&mut storage_view, HistoryEnabled);

        // init vm
        let mut vm = init_vm_inner(
            &mut oracle_tools,
            BlockContextMode::NewBlock(block_context.into(), Default::default()),
            &block_properties,
            BLOCK_GAS_LIMIT,
            bootloader_code,
            execution_mode,
        );

        let tx: Transaction = l2_tx.into();

        push_transaction_to_bootloader_memory(&mut vm, &tx, execution_mode, None);

        let vm_block_result =
            vm.execute_till_block_end_with_call_tracer(BootloaderJobType::TransactionExecution);

        if let Some(revert_reason) = &vm_block_result.full_result.revert_reason {
            println!("Call {} {:?}", "FAILED".red(), revert_reason.revert_reason);
        } else {
            println!("Call {}", "SUCCESS".green());
        }
        if let VmTrace::CallTrace(call_trace) = &vm_block_result.full_result.trace {
            println!("=== Console Logs: ");
            for call in call_trace {
                inner.console_log_handler.handle_call_recurive(call);
            }

            println!("=== Call traces:");
            for call in call_trace {
                formatter::print_call(call, 0, &inner.show_calls, inner.resolve_hashes);
            }
        }

        match vm_block_result.full_result.revert_reason {
            Some(result) => result.original_data,
            None => vm_block_result
                .full_result
                .return_data
                .into_iter()
                .flat_map(|val| {
                    let bytes: [u8; 32] = val.into();
                    bytes.to_vec()
                })
                .collect::<Vec<_>>(),
        }
    }

    fn run_l2_tx_inner(
        &self,
        l2_tx: L2Tx,
        execution_mode: TxExecutionMode,
    ) -> (
        HashMap<StorageKey, H256>,
        VmTxExecutionResult,
        BlockInfo,
        HashMap<U256, Vec<U256>>,
    ) {
        let inner = self.inner.write().unwrap();

        let mut storage_view = StorageView::new(&inner.fork_storage);

        let mut oracle_tools = OracleTools::new(&mut storage_view, HistoryEnabled);

        let bootloader_code = match execution_mode {
            TxExecutionMode::VerifyExecute => &inner.baseline_contracts,
            _ => &inner.playground_contracts,
        };

        let block_context = inner.create_block_context();
        let block_properties = InMemoryNodeInner::create_block_properties(&bootloader_code);

        let block = BlockInfo {
            batch_number: block_context.block_number,
            block_timestamp: block_context.block_timestamp,
            tx_hash: l2_tx.hash(),
        };

        // init vm
        let mut vm = init_vm_inner(
            &mut oracle_tools,
            BlockContextMode::NewBlock(block_context.into(), Default::default()),
            &block_properties,
            BLOCK_GAS_LIMIT,
            bootloader_code,
            execution_mode,
        );

        let tx: Transaction = l2_tx.into();

        push_transaction_to_bootloader_memory(&mut vm, &tx, execution_mode, None);

        let tx_result = vm.execute_next_tx(u32::MAX, true).unwrap();

        match tx_result.status {
            TxExecutionStatus::Success => println!("Transaction: {}", "SUCCESS".green()),
            TxExecutionStatus::Failure => println!("Transaction: {}", "FAILED".red()),
        }

        println!(
            "Initiator: {:?} Payer: {:?}",
            tx.initiator_account(),
            tx.payer()
        );

        println!(
            "{} Limit: {:?} used: {:?} refunded: {:?}",
            "Gas".bold(),
            tx.gas_limit(),
            tx.gas_limit() - tx_result.gas_refunded,
            tx_result.gas_refunded
        );
        println!("\n==== Console logs: ");

        for call in &tx_result.call_traces {
            inner.console_log_handler.handle_call_recurive(call);
        }

        println!(
            "\n==== {} Use --show-calls flag to display more info.",
            format!("{:?} call traces. ", tx_result.call_traces.len()).bold()
        );

        if inner.show_calls != ShowCalls::None {
            for call in &tx_result.call_traces {
                formatter::print_call(call, 0, &inner.show_calls, inner.resolve_hashes);
            }
        }

        println!(
            "\n==== {}",
            format!("{} events", tx_result.result.logs.events.len()).bold()
        );
        for event in &tx_result.result.logs.events {
            formatter::print_event(event, inner.resolve_hashes);
        }

        println!("\n\n");
        vm.execute_till_block_end(BootloaderJobType::BlockPostprocessing);

        let bytecodes = vm
            .state
            .decommittment_processor
            .known_bytecodes
            .inner()
            .clone();

        let modified_keys = storage_view.get_modified_storage_keys().clone();
        (modified_keys, tx_result, block, bytecodes)
    }

    /// Runs L2 transaction and commits it to a new block.
    fn run_l2_tx(&self, l2_tx: L2Tx, execution_mode: TxExecutionMode) {
        let tx_hash = l2_tx.hash();
        println!("\nExecuting {}", format!("{:?}", tx_hash).bold());

        let (keys, result, block, bytecodes) = self.run_l2_tx_inner(l2_tx.clone(), execution_mode);

        // Write all the mutated keys (storage slots).
        let mut inner = self.inner.write().unwrap();
        for (key, value) in keys.iter() {
            inner.fork_storage.set_value(*key, *value);
        }

        // Write all the factory deps.
        for (hash, code) in bytecodes.iter() {
            inner.fork_storage.store_factory_dep(
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
        let current_miniblock = inner.current_miniblock;
        inner.tx_results.insert(
            tx_hash,
            TxExecutionInfo {
                tx: l2_tx,
                batch_number: block.batch_number,
                miniblock_number: current_miniblock,
                result,
            },
        );
        inner.blocks.insert(block.batch_number, block);
        {
            inner.current_timestamp += 1;
            inner.current_batch += 1;
            inner.current_miniblock += 1;
        }
    }
}

impl EthNamespaceT for InMemoryNode {
    fn chain_id(&self) -> jsonrpc_core::Result<zksync_basic_types::U64> {
        let inner = self.inner.read().unwrap();
        Ok(U64::from(inner.fork_storage.chain_id.0 as u64))
    }

    fn call(
        &self,
        req: zksync_types::transaction_request::CallRequest,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> jsonrpc_core::Result<zksync_basic_types::Bytes> {
        let mut tx = l2_tx_from_call_req(req, MAX_TX_SIZE).unwrap();
        tx.common_data.fee.gas_limit = ETH_CALL_GAS_LIMIT.into();
        let result = self.run_l2_call(tx);

        Ok(result.into())
    }

    fn get_balance(
        &self,
        address: zksync_basic_types::Address,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> jsonrpc_core::Result<U256> {
        let balance_key = storage_key_for_standard_token_balance(
            AccountTreeId::new(L2_ETH_TOKEN_ADDRESS),
            &address,
        );

        let balance = self
            .inner
            .write()
            .unwrap()
            .fork_storage
            .read_value(&balance_key);

        Ok(h256_to_u256(balance))
    }

    fn get_block_by_number(
        &self,
        block_number: zksync_types::api::BlockNumber,
        _full_transactions: bool,
    ) -> jsonrpc_core::Result<Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>>
    {
        // Currently we support only the 'most recent' block.
        let reader = self.inner.read().unwrap();
        match block_number {
            zksync_types::api::BlockNumber::Committed
            | zksync_types::api::BlockNumber::Finalized
            | zksync_types::api::BlockNumber::Latest => {}
            zksync_types::api::BlockNumber::Earliest => return not_implemented("get_block_by_number__Earliest"),
            | zksync_types::api::BlockNumber::Pending => return not_implemented("get_block_by_number__Pending"),
            zksync_types::api::BlockNumber::Number(ask_number) => {
                if ask_number != U64::from(reader.current_miniblock) {
                    let not_implemented_format = format!("get_block_by_number__{}", ask_number);
                    return not_implemented(&not_implemented_format)
                }
            },
        }

        let txn: Vec<TransactionVariant> = vec![];

        let block = zksync_types::api::Block {
            transactions: txn,
            hash: Default::default(),
            parent_hash: Default::default(),
            uncles_hash: Default::default(),
            author: Default::default(),
            state_root: Default::default(),
            transactions_root: Default::default(),
            receipts_root: Default::default(),
            number: U64::from(reader.current_miniblock),
            l1_batch_number: Some(U64::from(reader.current_batch)),
            gas_used: Default::default(),
            gas_limit: U256::from(ETH_CALL_GAS_LIMIT),
            base_fee_per_gas: Default::default(),
            extra_data: Default::default(),
            logs_bloom: Default::default(),
            timestamp: Default::default(),
            l1_batch_timestamp: Default::default(),
            difficulty: Default::default(),
            total_difficulty: Default::default(),
            seal_fields: Default::default(),
            uncles: Default::default(),
            size: Default::default(),
            mix_hash: Default::default(),
            nonce: Default::default(),
        };

        Ok(Some(block))
    }

    fn get_code(
        &self,
        address: zksync_basic_types::Address,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> jsonrpc_core::Result<zksync_basic_types::Bytes> {
        let code_key = get_code_key(&address);

        let code_hash = self
            .inner
            .write()
            .unwrap()
            .fork_storage
            .read_value(&code_key);

        Ok(Bytes::from(code_hash.as_bytes()))
    }

    fn get_transaction_count(
        &self,
        address: zksync_basic_types::Address,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> jsonrpc_core::Result<U256> {
        let nonce_key = get_nonce_key(&address);

        let result = self
            .inner
            .write()
            .unwrap()
            .fork_storage
            .read_value(&nonce_key);
        Ok(h256_to_u64(result).into())
    }

    fn get_transaction_receipt(
        &self,
        hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::Result<Option<zksync_types::api::TransactionReceipt>> {
        let reader = self.inner.read().unwrap();
        let tx_result = reader.tx_results.get(&hash);

        let receipt = tx_result.map(|info| {
            let status = if info.result.status == TxExecutionStatus::Success {
                U64::from(1)
            } else {
                U64::from(0)
            };

            let logs: Vec<Log> = info
                .result
                .result
                .logs
                .events
                .iter()
                .map(|log| {
                    let address = log.address;
                    let topics = &log.indexed_topics;
                    let data = log.value.clone();
                    let block_hash = Some(hash);
                    let block_number = Some(U64::from(info.miniblock_number));
                    let l1_batch_number = Some(U64::from(info.batch_number as u64));
                    let transaction_hash = Some(hash);
                    let transaction_index = Some(U64::from(1).into());
                    let log_index = Some(U256::default());
                    let transaction_log_index = Some(U256::default());
                    let log_type = None;
                    let removed = None;

                    Log {
                        address,
                        topics: topics.to_vec(),
                        data: zksync_types::Bytes(data),
                        block_hash,
                        block_number,
                        l1_batch_number,
                        transaction_hash,
                        transaction_index,
                        log_index,
                        transaction_log_index,
                        log_type,
                        removed,
                    }
                })
                .collect();

            TransactionReceipt {
                transaction_hash: hash,
                transaction_index: U64::from(1),
                block_hash: Some(hash),
                block_number: Some(U64::from(info.miniblock_number)),
                l1_batch_tx_index: None,
                l1_batch_number: Some(U64::from(info.batch_number as u64)),
                from: Default::default(),
                to: Some(info.tx.execute.contract_address),
                cumulative_gas_used: Default::default(),
                gas_used: Some(info.tx.common_data.fee.gas_limit - info.result.gas_refunded),
                contract_address: contract_address_from_tx_result(&info.result),
                logs: logs,
                l2_to_l1_logs: vec![],
                status: Some(status),
                root: None,
                logs_bloom: Default::default(),
                transaction_type: None,
                effective_gas_price: Some(500.into()),
            }
        });

        Ok(receipt)
    }

    fn send_raw_transaction(
        &self,
        tx_bytes: zksync_basic_types::Bytes,
    ) -> jsonrpc_core::Result<zksync_basic_types::H256> {
        let chain_id = {
            let reader = self.inner.read().unwrap();
            reader.fork_storage.chain_id
        };

        let (tx_req, hash) =
            TransactionRequest::from_bytes(&tx_bytes.0, chain_id.0, MAX_TX_SIZE).unwrap();

        let mut l2_tx: L2Tx = tx_req.try_into().unwrap();
        l2_tx.set_input(tx_bytes.0, hash);
        assert_eq!(hash, l2_tx.hash());

        self.run_l2_tx(l2_tx, TxExecutionMode::VerifyExecute);

        Ok(hash)
    }

    // Methods below are not currently implemented.

    fn get_block_number(&self) -> jsonrpc_core::Result<zksync_basic_types::U64> {
        let reader = self.inner.read().unwrap();
        Ok(U64::from(reader.current_miniblock))
    }

    fn estimate_gas(
        &self,
        _req: zksync_types::transaction_request::CallRequest,
        _block: Option<zksync_types::api::BlockNumber>,
    ) -> jsonrpc_core::Result<U256> {
        let gas_used = U256::from(ETH_CALL_GAS_LIMIT);
        Ok(gas_used)
    }

    fn gas_price(&self) -> jsonrpc_core::Result<U256> {
        let fair_l2_gas_price: u64 = 250_000_000; // 0.25 gwei
        Ok(U256::from(fair_l2_gas_price))
    }

    fn new_filter(&self, _filter: Filter) -> jsonrpc_core::Result<U256> {
        not_implemented("new_filter")
    }

    fn new_block_filter(&self) -> jsonrpc_core::Result<U256> {
        not_implemented("new_block_filter")
    }

    fn uninstall_filter(&self, _idx: U256) -> jsonrpc_core::Result<bool> {
        not_implemented("uninstall_filter")
    }

    fn new_pending_transaction_filter(&self) -> jsonrpc_core::Result<U256> {
        not_implemented("new_pending_transaction_filter")
    }

    fn get_logs(&self, _filter: Filter) -> jsonrpc_core::Result<Vec<zksync_types::api::Log>> {
        not_implemented("get_logs")
    }

    fn get_filter_logs(&self, _filter_index: U256) -> jsonrpc_core::Result<FilterChanges> {
        not_implemented("get_filter_logs")
    }

    fn get_filter_changes(&self, _filter_index: U256) -> jsonrpc_core::Result<FilterChanges> {
        not_implemented("get_filter_changes")
    }

    fn get_block_by_hash(
        &self,
        _hash: zksync_basic_types::H256,
        _full_transactions: bool,
    ) -> jsonrpc_core::Result<Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>>
    {
        not_implemented("get_block_by_hash")
    }

    fn get_block_transaction_count_by_number(
        &self,
        _block_number: zksync_types::api::BlockNumber,
    ) -> jsonrpc_core::Result<Option<U256>> {
        not_implemented("get_block_transaction_count_by_number")
    }

    fn get_block_transaction_count_by_hash(
        &self,
        _block_hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::Result<Option<U256>> {
        not_implemented("get_block_transaction_count_by_hash")
    }

    fn get_storage(
        &self,
        _address: zksync_basic_types::Address,
        _idx: U256,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> jsonrpc_core::Result<zksync_basic_types::H256> {
        not_implemented("get_storage")
    }

    fn get_transaction_by_hash(
        &self,
        _hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::Result<Option<zksync_types::api::Transaction>> {
        not_implemented("get_transaction_by_hash")
    }

    fn get_transaction_by_block_hash_and_index(
        &self,
        _block_hash: zksync_basic_types::H256,
        _index: zksync_basic_types::web3::types::Index,
    ) -> jsonrpc_core::Result<Option<zksync_types::api::Transaction>> {
        not_implemented("get_transaction_by_block_hash_and_index")
    }

    fn get_transaction_by_block_number_and_index(
        &self,
        _block_number: zksync_types::api::BlockNumber,
        _index: zksync_basic_types::web3::types::Index,
    ) -> jsonrpc_core::Result<Option<zksync_types::api::Transaction>> {
        not_implemented("get_transaction_by_block_number_and_index")
    }

    fn protocol_version(&self) -> jsonrpc_core::Result<String> {
        not_implemented("protocol_version")
    }

    fn syncing(&self) -> jsonrpc_core::Result<zksync_basic_types::web3::types::SyncState> {
        not_implemented("syncing")
    }

    fn accounts(&self) -> jsonrpc_core::Result<Vec<zksync_basic_types::Address>> {
        not_implemented("accounts")
    }

    fn coinbase(&self) -> jsonrpc_core::Result<zksync_basic_types::Address> {
        not_implemented("coinbase")
    }

    fn compilers(&self) -> jsonrpc_core::Result<Vec<String>> {
        not_implemented("compilers")
    }

    fn hashrate(&self) -> jsonrpc_core::Result<U256> {
        not_implemented("hashrate")
    }

    fn get_uncle_count_by_block_hash(
        &self,
        _hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::Result<Option<U256>> {
        not_implemented("get_uncle_count_by_block_hash")
    }

    fn get_uncle_count_by_block_number(
        &self,
        _number: zksync_types::api::BlockNumber,
    ) -> jsonrpc_core::Result<Option<U256>> {
        not_implemented("get_uncle_count_by_block_number")
    }

    fn mining(&self) -> jsonrpc_core::Result<bool> {
        not_implemented("mining")
    }

    fn send_transaction(
        &self,
        _transaction_request: zksync_types::web3::types::TransactionRequest,
    ) -> jsonrpc_core::Result<zksync_basic_types::H256> {
        not_implemented("send_transaction")
    }
}
