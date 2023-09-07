//! In-memory node, that supports forking other networks.
use crate::{
    bootloader_debug::BootloaderDebug,
    console_log::ConsoleLogHandler,
    fork::{ForkDetails, ForkSource, ForkStorage},
    formatter,
    system_contracts::{self, SystemContracts},
    utils::{
        adjust_l1_gas_price_for_tx, derive_gas_estimation_overhead, to_human_size, IntoBoxedFuture,
    },
};
use clap::Parser;
use colored::Colorize;
use core::fmt::Display;
use futures::FutureExt;
use jsonrpc_core::BoxFuture;
use std::{
    cmp::{self},
    collections::HashMap,
    str::FromStr,
    sync::{Arc, RwLock},
};

use vm::{
    utils::{BLOCK_GAS_LIMIT, ETH_CALL_GAS_LIMIT},
    vm::VmTxExecutionResult,
    vm_with_bootloader::{
        derive_base_fee_and_gas_per_pubdata, init_vm_inner, push_transaction_to_bootloader_memory,
        BlockContext, BlockContextMode, BootloaderJobType, DerivedBlockContext, TxExecutionMode,
        BLOCK_OVERHEAD_PUBDATA,
    },
    HistoryDisabled, HistoryEnabled, HistoryMode, OracleTools, TxRevertReason, VmBlockResult,
    VmInstance,
};
use zksync_basic_types::{AccountTreeId, Bytes, H160, H256, U256, U64};
use zksync_contracts::BaseSystemContracts;
use zksync_core::api_server::web3::backend_jsonrpc::{
    error::into_jsrpc_error, namespaces::eth::EthNamespaceT,
};
use zksync_state::{ReadStorage, StorageView, WriteStorage};
use zksync_types::{
    api::{Log, TransactionReceipt, TransactionVariant},
    fee::Fee,
    get_code_key, get_nonce_key,
    l2::L2Tx,
    transaction_request::TransactionRequest,
    tx::tx_execution_info::TxExecutionStatus,
    utils::{
        decompose_full_nonce, nonces_to_full_nonce, storage_key_for_eth_balance,
        storage_key_for_standard_token_balance,
    },
    vm_trace::VmTrace,
    zk_evm::{
        block_properties::BlockProperties, zkevm_opcode_defs::system_params::MAX_PUBDATA_PER_BLOCK,
    },
    StorageKey, StorageLogQueryType, Transaction, ACCOUNT_CODE_STORAGE_ADDRESS,
    L2_ETH_TOKEN_ADDRESS, MAX_GAS_PER_PUBDATA_BYTE, MAX_L2_TX_GAS_LIMIT,
};
use zksync_utils::{
    bytecode::{compress_bytecode, hash_bytecode},
    h256_to_account_address, h256_to_u256, h256_to_u64, u256_to_h256,
};
use zksync_web3_decl::{
    error::Web3Error,
    types::{FeeHistory, Filter, FilterChanges},
};

/// Max possible size of an ABI encoded tx (in bytes).
pub const MAX_TX_SIZE: usize = 1_000_000;
/// Timestamp of the first block (if not running in fork mode).
pub const NON_FORK_FIRST_BLOCK_TIMESTAMP: u64 = 1_000;
/// Network ID we use for the test node.
pub const TEST_NODE_NETWORK_ID: u16 = 260;
/// L1 Gas Price.
pub const L1_GAS_PRICE: u64 = 50_000_000_000;
/// L2 Gas Price (0.25 gwei).
pub const L2_GAS_PRICE: u64 = 250_000_000;
/// L1 Gas Price Scale Factor for gas estimation.
pub const ESTIMATE_GAS_L1_GAS_PRICE_SCALE_FACTOR: f64 = 1.2;
/// The max possible number of gas that `eth_estimateGas` is allowed to overestimate.
pub const ESTIMATE_GAS_PUBLISH_BYTE_OVERHEAD: u32 = 100;
/// Acceptable gas overestimation limit.
pub const ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION: u32 = 1_000;
/// The factor by which to scale the gasLimit.
pub const ESTIMATE_GAS_SCALE_FACTOR: f32 = 1.3;

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

#[derive(Debug, clap::Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowCalls {
    None,
    User,
    System,
    All,
}

impl FromStr for ShowCalls {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "none" => Ok(ShowCalls::None),
            "user" => Ok(ShowCalls::User),
            "system" => Ok(ShowCalls::System),
            "all" => Ok(ShowCalls::All),
            _ => Err(format!(
                "Unknown ShowCalls value {} - expected one of none|user|system|all.",
                s
            )),
        }
    }
}

impl Display for ShowCalls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowStorageLogs {
    None,
    Read,
    Write,
    All,
}

impl FromStr for ShowStorageLogs {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "none" => Ok(ShowStorageLogs::None),
            "read" => Ok(ShowStorageLogs::Read),
            "write" => Ok(ShowStorageLogs::Write),
            "all" => Ok(ShowStorageLogs::All),
            _ => Err(format!(
                "Unknown ShowStorageLogs value {} - expected one of none|read|write|all.",
                s
            )),
        }
    }
}

impl Display for ShowStorageLogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowVMDetails {
    None,
    All,
}

impl FromStr for ShowVMDetails {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "none" => Ok(ShowVMDetails::None),
            "all" => Ok(ShowVMDetails::All),
            _ => Err(format!(
                "Unknown ShowVMDetails value {} - expected one of none|all.",
                s
            )),
        }
    }
}

impl Display for ShowVMDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowGasDetails {
    None,
    All,
}

impl FromStr for ShowGasDetails {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "none" => Ok(ShowGasDetails::None),
            "all" => Ok(ShowGasDetails::All),
            _ => Err(format!(
                "Unknown ShowGasDetails value {} - expected one of none|all.",
                s
            )),
        }
    }
}

impl Display for ShowGasDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

/// Helper struct for InMemoryNode.
/// S - is the Source of the Fork.
pub struct InMemoryNodeInner<S> {
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
    pub fork_storage: ForkStorage<S>,
    // Debug level information.
    pub show_calls: ShowCalls,
    // Displays storage logs.
    pub show_storage_logs: ShowStorageLogs,
    // Displays VM details.
    pub show_vm_details: ShowVMDetails,
    // Gas details information.
    pub show_gas_details: ShowGasDetails,
    // If true - will contact openchain to resolve the ABI to function names.
    pub resolve_hashes: bool,
    pub console_log_handler: ConsoleLogHandler,
    pub system_contracts: SystemContracts,
}

type L2TxResult = (
    HashMap<StorageKey, H256>,
    VmTxExecutionResult,
    BlockInfo,
    HashMap<U256, Vec<U256>>,
);

impl<S: std::fmt::Debug + ForkSource> InMemoryNodeInner<S> {
    fn create_block_context(&self) -> BlockContext {
        BlockContext {
            block_number: self.current_batch,
            block_timestamp: self.current_timestamp,
            l1_gas_price: self.l1_gas_price,
            fair_l2_gas_price: L2_GAS_PRICE,
            operator_address: H160::zero(),
        }
    }

    fn create_block_properties(contracts: &BaseSystemContracts) -> BlockProperties {
        BlockProperties {
            default_aa_code_hash: h256_to_u256(contracts.default_aa.hash),
            zkporter_is_available: false,
        }
    }

    /// Estimates the gas required for a given call request.
    ///
    /// # Arguments
    ///
    /// * `req` - A `CallRequest` struct representing the call request to estimate gas for.
    ///
    /// # Returns
    ///
    /// A `Result` with a `Fee` representing the estimated gas related data.
    pub fn estimate_gas_impl(
        &self,
        req: zksync_types::transaction_request::CallRequest,
    ) -> jsonrpc_core::Result<Fee> {
        let mut l2_tx = match L2Tx::from_request(req.into(), MAX_TX_SIZE) {
            Ok(tx) => tx,
            Err(e) => {
                let error = Web3Error::SerializationError(e);
                return Err(into_jsrpc_error(error));
            }
        };

        let tx: Transaction = l2_tx.clone().into();
        let fair_l2_gas_price = L2_GAS_PRICE;

        // Calculate Adjusted L1 Price
        let l1_gas_price = {
            let current_l1_gas_price =
                ((self.l1_gas_price as f64) * ESTIMATE_GAS_L1_GAS_PRICE_SCALE_FACTOR) as u64;

            // In order for execution to pass smoothly, we need to ensure that block's required gasPerPubdata will be
            // <= to the one in the transaction itself.
            adjust_l1_gas_price_for_tx(
                current_l1_gas_price,
                L2_GAS_PRICE,
                tx.gas_per_pubdata_byte_limit(),
            )
        };

        let (base_fee, gas_per_pubdata_byte) =
            derive_base_fee_and_gas_per_pubdata(l1_gas_price, fair_l2_gas_price);

        // Properly format signature
        if l2_tx.common_data.signature.is_empty() {
            l2_tx.common_data.signature = vec![0u8; 65];
            l2_tx.common_data.signature[64] = 27;
        }

        l2_tx.common_data.fee.gas_per_pubdata_limit = MAX_GAS_PER_PUBDATA_BYTE.into();
        l2_tx.common_data.fee.max_fee_per_gas = base_fee.into();
        l2_tx.common_data.fee.max_priority_fee_per_gas = base_fee.into();

        let mut storage_view = StorageView::new(&self.fork_storage);

        // Calculate gas_for_bytecodes_pubdata
        let pubdata_for_factory_deps = l2_tx
            .execute
            .factory_deps
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|bytecode| {
                if storage_view.is_bytecode_known(&hash_bytecode(bytecode)) {
                    return 0;
                }

                let length = if let Ok(compressed) = compress_bytecode(bytecode) {
                    compressed.len()
                } else {
                    bytecode.len()
                };
                length as u32 + ESTIMATE_GAS_PUBLISH_BYTE_OVERHEAD
            })
            .sum::<u32>();

        if pubdata_for_factory_deps > MAX_PUBDATA_PER_BLOCK {
            return Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                "exceeds limit for published pubdata".into(),
                Default::default(),
            )));
        }

        let gas_for_bytecodes_pubdata: u32 =
            pubdata_for_factory_deps * (gas_per_pubdata_byte as u32);

        let block_context = self.create_block_context();

        // We are using binary search to find the minimal values of gas_limit under which the transaction succeeds
        let mut lower_bound = 0;
        let mut upper_bound = MAX_L2_TX_GAS_LIMIT as u32;

        while lower_bound + ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION < upper_bound {
            let mid = (lower_bound + upper_bound) / 2;
            let try_gas_limit = gas_for_bytecodes_pubdata + mid;

            let estimate_gas_result = InMemoryNodeInner::estimate_gas_step(
                l2_tx.clone(),
                gas_per_pubdata_byte,
                try_gas_limit,
                l1_gas_price,
                base_fee,
                block_context,
                &self.fork_storage,
                self.system_contracts.contracts_for_fee_estimate(),
            );

            if estimate_gas_result.is_err() {
                lower_bound = mid + 1;
            } else {
                upper_bound = mid;
            }
        }

        let tx_body_gas_limit = cmp::min(
            MAX_L2_TX_GAS_LIMIT as u32,
            (upper_bound as f32 * ESTIMATE_GAS_SCALE_FACTOR) as u32,
        );
        let suggested_gas_limit = tx_body_gas_limit + gas_for_bytecodes_pubdata;

        let estimate_gas_result = InMemoryNodeInner::estimate_gas_step(
            l2_tx.clone(),
            gas_per_pubdata_byte,
            suggested_gas_limit,
            l1_gas_price,
            base_fee,
            block_context,
            &self.fork_storage,
            self.system_contracts.contracts_for_fee_estimate(),
        );

        let overhead: u32 = derive_gas_estimation_overhead(
            suggested_gas_limit,
            gas_per_pubdata_byte as u32,
            tx.encoding_len(),
        );

        match estimate_gas_result {
            Err(tx_revert_reason) => {
                log::info!("{}", format!("Unable to estimate gas for the request with our suggested gas limit of {}. The transaction is most likely unexecutable. Breakdown of estimation:", suggested_gas_limit + overhead).red());
                log::info!(
                    "{}",
                    format!(
                        "\tEstimated transaction body gas cost: {}",
                        tx_body_gas_limit
                    )
                    .red()
                );
                log::info!(
                    "{}",
                    format!("\tGas for pubdata: {}", gas_for_bytecodes_pubdata).red()
                );
                log::info!("{}", format!("\tOverhead: {}", overhead).red());
                let message = tx_revert_reason.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );
                let data = match tx_revert_reason {
                    TxRevertReason::EthCall(vm_revert_reason) => vm_revert_reason.encoded_data(),
                    TxRevertReason::TxReverted(vm_revert_reason) => vm_revert_reason.encoded_data(),
                    _ => vec![],
                };
                log::info!("{}", pretty_message.on_red());
                Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                    pretty_message,
                    data,
                )))
            }
            Ok(_) => {
                let full_gas_limit = match tx_body_gas_limit
                    .overflowing_add(gas_for_bytecodes_pubdata + overhead)
                {
                    (value, false) => value,
                    (_, true) => {
                        log::info!("{}", "Overflow when calculating gas estimation. We've exceeded the block gas limit by summing the following values:".red());
                        log::info!(
                            "{}",
                            format!(
                                "\tEstimated transaction body gas cost: {}",
                                tx_body_gas_limit
                            )
                            .red()
                        );
                        log::info!(
                            "{}",
                            format!("\tGas for pubdata: {}", gas_for_bytecodes_pubdata).red()
                        );
                        log::info!("{}", format!("\tOverhead: {}", overhead).red());
                        return Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                            "exceeds block gas limit".into(),
                            Default::default(),
                        )));
                    }
                };

                let fee = Fee {
                    max_fee_per_gas: base_fee.into(),
                    max_priority_fee_per_gas: 0u32.into(),
                    gas_limit: full_gas_limit.into(),
                    gas_per_pubdata_limit: gas_per_pubdata_byte.into(),
                };
                Ok(fee)
            }
        }
    }

    /// Runs fee estimation against a sandbox vm with the given gas_limit.
    #[allow(clippy::too_many_arguments)]
    fn estimate_gas_step(
        mut l2_tx: L2Tx,
        gas_per_pubdata_byte: u64,
        tx_gas_limit: u32,
        l1_gas_price: u64,
        base_fee: u64,
        mut block_context: BlockContext,
        fork_storage: &ForkStorage<S>,
        bootloader_code: &BaseSystemContracts,
    ) -> Result<VmBlockResult, TxRevertReason> {
        let tx: Transaction = l2_tx.clone().into();
        let l1_gas_price =
            adjust_l1_gas_price_for_tx(l1_gas_price, L2_GAS_PRICE, tx.gas_per_pubdata_byte_limit());

        // Set gas_limit for transaction
        let gas_limit_with_overhead = tx_gas_limit
            + derive_gas_estimation_overhead(
                tx_gas_limit,
                gas_per_pubdata_byte as u32,
                tx.encoding_len(),
            );
        l2_tx.common_data.fee.gas_limit = gas_limit_with_overhead.into();

        let mut storage_view = StorageView::new(fork_storage);

        // The nonce needs to be updated
        let nonce = l2_tx.nonce();
        let nonce_key = get_nonce_key(&l2_tx.initiator_account());
        let full_nonce = storage_view.read_value(&nonce_key);
        let (_, deployment_nonce) = decompose_full_nonce(h256_to_u256(full_nonce));
        let enforced_full_nonce = nonces_to_full_nonce(U256::from(nonce.0), deployment_nonce);
        storage_view.set_value(nonce_key, u256_to_h256(enforced_full_nonce));

        // We need to explicitly put enough balance into the account of the users
        let payer = l2_tx.payer();
        let balance_key = storage_key_for_eth_balance(&payer);
        let mut current_balance = h256_to_u256(storage_view.read_value(&balance_key));
        let added_balance = l2_tx.common_data.fee.gas_limit * l2_tx.common_data.fee.max_fee_per_gas;
        current_balance += added_balance;
        storage_view.set_value(balance_key, u256_to_h256(current_balance));

        let mut oracle_tools = OracleTools::new(&mut storage_view, HistoryDisabled);

        block_context.l1_gas_price = l1_gas_price;
        let derived_block_context = DerivedBlockContext {
            context: block_context,
            base_fee,
        };

        let block_properties = InMemoryNodeInner::<S>::create_block_properties(bootloader_code);

        let execution_mode = TxExecutionMode::EstimateFee {
            missed_storage_invocation_limit: 1000000,
        };

        // init vm
        let mut vm = init_vm_inner(
            &mut oracle_tools,
            BlockContextMode::OverrideCurrent(derived_block_context),
            &block_properties,
            BLOCK_GAS_LIMIT,
            bootloader_code,
            execution_mode,
        );

        let tx: Transaction = l2_tx.into();

        push_transaction_to_bootloader_memory(&mut vm, &tx, execution_mode, None);

        let vm_block_result = vm.execute_till_block_end(BootloaderJobType::TransactionExecution);

        match vm_block_result.full_result.revert_reason {
            None => Ok(vm_block_result),
            Some(revert) => Err(revert.revert_reason),
        }
    }
}

fn not_implemented<T: Send + 'static>(
    method_name: &str,
) -> jsonrpc_core::BoxFuture<Result<T, jsonrpc_core::Error>> {
    log::info!("Method {} is not implemented", method_name);
    Err(jsonrpc_core::Error {
        data: None,
        code: jsonrpc_core::ErrorCode::MethodNotFound,
        message: format!("Method {} is not implemented", method_name),
    })
    .into_boxed_future()
}

/// In-memory node, that can be used for local & unit testing.
/// It also supports the option of forking testnet/mainnet.
/// All contents are removed when object is destroyed.
pub struct InMemoryNode<S> {
    inner: Arc<RwLock<InMemoryNodeInner<S>>>,
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

impl<S: ForkSource + std::fmt::Debug> Default for InMemoryNode<S> {
    fn default() -> Self {
        InMemoryNode::new(
            None,
            crate::node::ShowCalls::None,
            ShowStorageLogs::None,
            ShowVMDetails::None,
            ShowGasDetails::None,
            false,
            &system_contracts::Options::BuiltIn,
        )
    }
}

impl<S: ForkSource + std::fmt::Debug> InMemoryNode<S> {
    pub fn new(
        fork: Option<ForkDetails<S>>,
        show_calls: ShowCalls,
        show_storage_logs: ShowStorageLogs,
        show_vm_details: ShowVMDetails,
        show_gas_details: ShowGasDetails,
        resolve_hashes: bool,
        system_contracts_options: &system_contracts::Options,
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
                    .unwrap_or(L1_GAS_PRICE),
                tx_results: Default::default(),
                blocks: Default::default(),
                fork_storage: ForkStorage::new(fork, system_contracts_options),
                show_calls,
                show_storage_logs,
                show_vm_details,
                show_gas_details,
                resolve_hashes,
                console_log_handler: ConsoleLogHandler::default(),
                system_contracts: SystemContracts::from_options(system_contracts_options),
            })),
        }
    }

    pub fn get_inner(&self) -> Arc<RwLock<InMemoryNodeInner<S>>> {
        self.inner.clone()
    }

    /// Applies multiple transactions - but still one per L1 batch.
    pub fn apply_txs(&self, txs: Vec<L2Tx>) -> Result<(), String> {
        log::info!("Running {:?} transactions (one per batch)", txs.len());

        for tx in txs {
            self.run_l2_tx(tx, TxExecutionMode::VerifyExecute)?;
        }

        Ok(())
    }

    /// Adds a lot of tokens to a given account.
    pub fn set_rich_account(&self, address: H160) {
        let key = storage_key_for_eth_balance(&address);

        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(e) => {
                log::info!("Failed to acquire write lock: {}", e);
                return;
            }
        };

        let keys = {
            let mut storage_view = StorageView::new(&inner.fork_storage);
            storage_view.set_value(key, u256_to_h256(U256::from(10u128.pow(30))));
            storage_view.modified_storage_keys().clone()
        };

        for (key, value) in keys.iter() {
            inner.fork_storage.set_value(*key, *value);
        }
    }

    /// Runs L2 'eth call' method - that doesn't commit to a block.
    fn run_l2_call(&self, l2_tx: L2Tx) -> Result<VmBlockResult, String> {
        let execution_mode = TxExecutionMode::EthCall {
            missed_storage_invocation_limit: 1000000,
        };

        let inner = self
            .inner
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        let mut storage_view = StorageView::new(&inner.fork_storage);

        let mut oracle_tools = OracleTools::new(&mut storage_view, HistoryEnabled);

        let bootloader_code = &inner.system_contracts.contacts_for_l2_call();

        let block_context = inner.create_block_context();
        let block_properties = InMemoryNodeInner::<S>::create_block_properties(bootloader_code);

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
            log::info!("Call {} {:?}", "FAILED".red(), revert_reason.revert_reason);
        } else {
            log::info!("Call {}", "SUCCESS".green());
        }
        if let VmTrace::CallTrace(call_trace) = &vm_block_result.full_result.trace {
            log::info!("=== Console Logs: ");
            for call in call_trace {
                inner.console_log_handler.handle_call_recurive(call);
            }

            log::info!("=== Call traces:");
            for call in call_trace {
                formatter::print_call(call, 0, &inner.show_calls, inner.resolve_hashes);
            }
        }

        Ok(vm_block_result)
    }

    fn display_detailed_gas_info<H: HistoryMode>(
        &self,
        vm: &VmInstance<H>,
        spent_on_pubdata: u32,
    ) -> eyre::Result<()> {
        let debug = BootloaderDebug::load_from_memory(vm)?;

        log::info!("┌─────────────────────────┐");
        log::info!("│       GAS DETAILS       │");
        log::info!("└─────────────────────────┘");

        // Total amount of gas (should match tx.gas_limit).
        let total_gas_limit = debug
            .total_gas_limit_from_user
            .saturating_sub(debug.reserved_gas);

        let intrinsic_gas = total_gas_limit - debug.gas_limit_after_intrinsic;
        let gas_for_validation = debug.gas_limit_after_intrinsic - debug.gas_after_validation;

        let gas_spent_on_compute =
            debug.gas_spent_on_execution - debug.gas_spent_on_bytecode_preparation;

        let gas_used = intrinsic_gas
            + gas_for_validation
            + debug.gas_spent_on_bytecode_preparation
            + gas_spent_on_compute;

        log::info!(
            "Gas - Limit: {} | Used: {} | Refunded: {}",
            to_human_size(total_gas_limit),
            to_human_size(gas_used),
            to_human_size(debug.refund_by_operator)
        );

        if debug.total_gas_limit_from_user != total_gas_limit {
            log::info!(
                "{}",
                format!(
                "  WARNING: user actually provided more gas {}, but system had a lower max limit.",
                to_human_size(debug.total_gas_limit_from_user)
            )
                .yellow()
            );
        }
        if debug.refund_computed != debug.refund_by_operator {
            log::info!(
                "{}",
                format!(
                    "  WARNING: Refund by VM: {}, but operator refunded more: {}",
                    to_human_size(debug.refund_computed),
                    to_human_size(debug.refund_by_operator)
                )
                .yellow()
            );
        }

        if debug.refund_computed + gas_used != total_gas_limit {
            log::info!(
                "{}",
                format!(
                    "  WARNING: Gas totals don't match. {} != {} , delta: {}",
                    to_human_size(debug.refund_computed + gas_used),
                    to_human_size(total_gas_limit),
                    to_human_size(total_gas_limit.abs_diff(debug.refund_computed + gas_used))
                )
                .yellow()
            );
        }

        let bytes_published = spent_on_pubdata / debug.gas_per_pubdata.as_u32();

        log::info!(
            "During execution published {} bytes to L1, @{} each - in total {} gas",
            to_human_size(bytes_published.into()),
            to_human_size(debug.gas_per_pubdata),
            to_human_size(spent_on_pubdata.into())
        );

        log::info!("Out of {} gas used, we spent:", to_human_size(gas_used));
        log::info!(
            "  {:>15} gas ({:>2}%) for transaction setup",
            to_human_size(intrinsic_gas),
            to_human_size(intrinsic_gas * 100 / gas_used)
        );
        log::info!(
            "  {:>15} gas ({:>2}%) for bytecode preparation (decompression etc)",
            to_human_size(debug.gas_spent_on_bytecode_preparation),
            to_human_size(debug.gas_spent_on_bytecode_preparation * 100 / gas_used)
        );
        log::info!(
            "  {:>15} gas ({:>2}%) for account validation",
            to_human_size(gas_for_validation),
            to_human_size(gas_for_validation * 100 / gas_used)
        );
        log::info!(
            "  {:>15} gas ({:>2}%) for computations (opcodes)",
            to_human_size(gas_spent_on_compute),
            to_human_size(gas_spent_on_compute * 100 / gas_used)
        );

        log::info!(
            "\n\n {}",
            "=== Transaction setup cost breakdown ===".to_owned().bold(),
        );

        log::info!("Total cost: {}", to_human_size(intrinsic_gas).bold());
        log::info!(
            "  {:>15} gas ({:>2}%) fixed cost",
            to_human_size(debug.intrinsic_overhead),
            to_human_size(debug.intrinsic_overhead * 100 / intrinsic_gas)
        );
        log::info!(
            "  {:>15} gas ({:>2}%) operator cost",
            to_human_size(debug.operator_overhead),
            to_human_size(debug.operator_overhead * 100 / intrinsic_gas)
        );

        log::info!(
            "\n  FYI: operator could have charged up to: {}, so you got {}% discount",
            to_human_size(debug.required_overhead),
            to_human_size(
                (debug.required_overhead - debug.operator_overhead) * 100 / debug.required_overhead
            )
        );

        let publish_block_l1_bytes = BLOCK_OVERHEAD_PUBDATA;
        log::info!(
            "Publishing full block costs the operator up to: {}, where {} is due to {} bytes published to L1",
            to_human_size(debug.total_overhead_for_block),
            to_human_size(debug.gas_per_pubdata * publish_block_l1_bytes),
            to_human_size(publish_block_l1_bytes.into())
        );
        log::info!("Your transaction has contributed to filling up the block in the following way (we take the max contribution as the cost):");
        log::info!(
            "  Circuits overhead:{:>15} ({}% of the full block: {})",
            to_human_size(debug.overhead_for_circuits),
            to_human_size(debug.overhead_for_circuits * 100 / debug.total_overhead_for_block),
            to_human_size(debug.total_overhead_for_block)
        );
        log::info!(
            "  Length overhead:  {:>15}",
            to_human_size(debug.overhead_for_length)
        );
        log::info!(
            "  Slot overhead:    {:>15}",
            to_human_size(debug.overhead_for_slot)
        );
        Ok(())
    }

    /// Executes the given L2 transaction and returns all the VM logs.
    pub fn run_l2_tx_inner(
        &self,
        l2_tx: L2Tx,
        execution_mode: TxExecutionMode,
    ) -> Result<L2TxResult, String> {
        let inner = self
            .inner
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        let mut storage_view = StorageView::new(&inner.fork_storage);

        let mut oracle_tools = OracleTools::new(&mut storage_view, HistoryEnabled);

        let bootloader_code = inner.system_contracts.contracts(execution_mode);

        let block_context = inner.create_block_context();
        let block_properties = InMemoryNodeInner::<S>::create_block_properties(bootloader_code);

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
        let spent_on_pubdata_before = vm.state.local_state.spent_pubdata_counter;

        let tx: Transaction = l2_tx.into();
        push_transaction_to_bootloader_memory(&mut vm, &tx, execution_mode, None);
        let tx_result = vm
            .execute_next_tx(u32::MAX, true)
            .map_err(|e| format!("Failed to execute next transaction: {}", e))?;

        let spent_on_pubdata = vm.state.local_state.spent_pubdata_counter - spent_on_pubdata_before;

        log::info!("┌─────────────────────────┐");
        log::info!("│   TRANSACTION SUMMARY   │");
        log::info!("└─────────────────────────┘");

        match tx_result.status {
            TxExecutionStatus::Success => log::info!("Transaction: {}", "SUCCESS".green()),
            TxExecutionStatus::Failure => log::info!("Transaction: {}", "FAILED".red()),
        }

        log::info!(
            "Initiator: {:?}\nPayer: {:?}",
            tx.initiator_account(),
            tx.payer()
        );
        log::info!(
            "Gas - Limit: {} | Used: {} | Refunded: {}",
            to_human_size(tx.gas_limit()),
            to_human_size(tx.gas_limit() - tx_result.gas_refunded),
            to_human_size(tx_result.gas_refunded.into())
        );

        match inner.show_gas_details {
            ShowGasDetails::None => log::info!(
                "Use --show-gas-details flag or call config_setShowGasDetails to display more info"
            ),
            ShowGasDetails::All => {
                if self
                    .display_detailed_gas_info(&vm, spent_on_pubdata)
                    .is_err()
                {
                    log::info!(
                        "{}",
                        "!!! FAILED TO GET DETAILED GAS INFO !!!".to_owned().red()
                    );
                }
            }
        }

        if inner.show_storage_logs != ShowStorageLogs::None {
            log::info!("\n┌──────────────────┐");
            log::info!("│   STORAGE LOGS   │");
            log::info!("└──────────────────┘");
        }

        for log_query in &tx_result.result.logs.storage_logs {
            match inner.show_storage_logs {
                ShowStorageLogs::Write => {
                    if matches!(
                        log_query.log_type,
                        StorageLogQueryType::RepeatedWrite | StorageLogQueryType::InitialWrite
                    ) {
                        formatter::print_logs(log_query);
                    }
                }
                ShowStorageLogs::Read => {
                    if log_query.log_type == StorageLogQueryType::Read {
                        formatter::print_logs(log_query);
                    }
                }
                ShowStorageLogs::All => {
                    formatter::print_logs(log_query);
                }
                _ => {}
            }
        }

        if inner.show_vm_details != ShowVMDetails::None {
            formatter::print_vm_details(&tx_result.result);
        }

        log::info!("\n==== Console logs: ");
        for call in &tx_result.call_traces {
            inner.console_log_handler.handle_call_recurive(call);
        }

        log::info!(
            "\n==== {} Use --show-calls flag or call config_setShowCalls to display more info.",
            format!("{:?} call traces. ", tx_result.call_traces.len()).bold()
        );

        if inner.show_calls != ShowCalls::None {
            for call in &tx_result.call_traces {
                formatter::print_call(call, 0, &inner.show_calls, inner.resolve_hashes);
            }
        }

        log::info!(
            "\n==== {}",
            format!("{} events", tx_result.result.logs.events.len()).bold()
        );
        for event in &tx_result.result.logs.events {
            formatter::print_event(event, inner.resolve_hashes);
        }

        log::info!("\n\n");

        vm.execute_till_block_end(BootloaderJobType::BlockPostprocessing);

        let bytecodes = vm
            .state
            .decommittment_processor
            .known_bytecodes
            .inner()
            .clone();

        let modified_keys = storage_view.modified_storage_keys().clone();
        Ok((modified_keys, tx_result, block, bytecodes))
    }

    /// Runs L2 transaction and commits it to a new block.
    fn run_l2_tx(&self, l2_tx: L2Tx, execution_mode: TxExecutionMode) -> Result<(), String> {
        let tx_hash = l2_tx.hash();
        log::info!("\nExecuting {}", format!("{:?}", tx_hash).bold());
        let (keys, result, block, bytecodes) =
            self.run_l2_tx_inner(l2_tx.clone(), execution_mode)?;
        // Write all the mutated keys (storage slots).
        let mut inner = self
            .inner
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
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

        Ok(())
    }
}

impl<S: Send + Sync + 'static + ForkSource + std::fmt::Debug> EthNamespaceT for InMemoryNode<S> {
    /// Returns the chain ID of the node.
    fn chain_id(&self) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::U64>> {
        match self.inner.read() {
            Ok(inner) => Ok(U64::from(inner.fork_storage.chain_id.0 as u64)).into_boxed_future(),
            Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)).into_boxed_future(),
        }
    }

    /// Calls the specified function on the L2 contract with the given arguments.
    ///
    /// # Arguments
    ///
    /// * `req` - The call request containing the function name and arguments.
    /// * `_block` - The block ID variant (unused).
    ///
    /// # Returns
    ///
    /// A boxed future containing the result of the function call.
    fn call(
        &self,
        req: zksync_types::transaction_request::CallRequest,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::Bytes>> {
        match L2Tx::from_request(req.into(), MAX_TX_SIZE) {
            Ok(mut tx) => {
                tx.common_data.fee.gas_limit = ETH_CALL_GAS_LIMIT.into();
                let result = self.run_l2_call(tx);

                match result {
                    Ok(vm_block_result) => match vm_block_result.full_result.revert_reason {
                        Some(revert) => {
                            let message = revert.revert_reason.to_string();
                            let pretty_message = format!(
                                "execution reverted{}{}",
                                if message.is_empty() { "" } else { ": " },
                                message
                            );
                            let data = match revert.revert_reason {
                                TxRevertReason::EthCall(vm_revert_reason) => {
                                    vm_revert_reason.encoded_data()
                                }
                                TxRevertReason::TxReverted(vm_revert_reason) => {
                                    vm_revert_reason.encoded_data()
                                }
                                _ => vec![],
                            };
                            log::info!("{}", pretty_message.on_red());
                            Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                                pretty_message,
                                data,
                            )))
                            .into_boxed_future()
                        }
                        None => Ok(vm_block_result
                            .full_result
                            .return_data
                            .into_iter()
                            .flat_map(|val| {
                                let bytes: [u8; 32] = val.into();
                                bytes.to_vec()
                            })
                            .collect::<Vec<_>>()
                            .into())
                        .into_boxed_future(),
                    },
                    Err(e) => {
                        let error = Web3Error::InvalidTransactionData(
                            zksync_types::ethabi::Error::InvalidName(e),
                        );
                        Err(into_jsrpc_error(error)).into_boxed_future()
                    }
                }
            }
            Err(e) => {
                let error = Web3Error::SerializationError(e);
                Err(into_jsrpc_error(error)).into_boxed_future()
            }
        }
    }

    /// Returns the balance of the specified address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to get the balance of.
    /// * `_block` - The block ID variant (optional).
    ///
    /// # Returns
    ///
    /// A `BoxFuture` that resolves to a `Result` containing the balance of the specified address as a `U256` or a `jsonrpc_core::Error` if an error occurred.
    fn get_balance(
        &self,
        address: zksync_basic_types::Address,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> BoxFuture<Result<U256, jsonrpc_core::Error>> {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            let balance_key = storage_key_for_standard_token_balance(
                AccountTreeId::new(L2_ETH_TOKEN_ADDRESS),
                &address,
            );

            match inner.write() {
                Ok(mut inner_guard) => {
                    let balance = inner_guard.fork_storage.read_value(&balance_key);
                    Ok(h256_to_u256(balance))
                }
                Err(_) => {
                    let web3_error = Web3Error::InternalError;
                    Err(into_jsrpc_error(web3_error))
                }
            }
        })
    }

    /// Returns a block by its number.
    ///
    /// # Arguments
    ///
    /// * `block_number` - A `BlockNumber` enum variant representing the block number to retrieve.
    /// * `_full_transactions` - A boolean value indicating whether to retrieve full transactions or not.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an `Option` of `zksync_types::api::Block<zksync_types::api::TransactionVariant>`.
    fn get_block_by_number(
        &self,
        block_number: zksync_types::api::BlockNumber,
        _full_transactions: bool,
    ) -> BoxFuture<
        jsonrpc_core::Result<
            Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>,
        >,
    > {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            let reader = match inner.read() {
                Ok(r) => r,
                Err(_) => return Err(into_jsrpc_error(Web3Error::InternalError)),
            };

            match block_number {
                zksync_types::api::BlockNumber::Earliest => {
                    log::info!(
                        "Method get_block_by_number with BlockNumber::Earliest is not implemented"
                    );
                    return Err(into_jsrpc_error(Web3Error::NotImplemented));
                }
                zksync_types::api::BlockNumber::Pending => {
                    log::info!(
                        "Method get_block_by_number with BlockNumber::Pending is not implemented"
                    );
                    return Err(into_jsrpc_error(Web3Error::NotImplemented));
                }
                zksync_types::api::BlockNumber::Number(ask_number)
                    if ask_number != U64::from(reader.current_miniblock) =>
                {
                    log::info!("Method get_block_by_number with BlockNumber::Number({}) is not implemented", ask_number);
                    return Err(into_jsrpc_error(Web3Error::NotImplemented));
                }
                _ => {}
            }

            let block = zksync_types::api::Block {
                transactions: vec![],
                number: U64::from(reader.current_miniblock),
                l1_batch_number: Some(U64::from(reader.current_batch)),
                gas_limit: U256::from(ETH_CALL_GAS_LIMIT),
                ..Default::default()
            };

            Ok(Some(block))
        })
    }

    /// Returns the code stored at the specified address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to retrieve the code from.
    /// * `_block` - An optional block ID variant.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing the result of the operation, which is a `jsonrpc_core::Result` containing
    /// the code as a `zksync_basic_types::Bytes` object.
    fn get_code(
        &self,
        address: zksync_basic_types::Address,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> BoxFuture<jsonrpc_core::Result<zksync_basic_types::Bytes>> {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            let code_key = get_code_key(&address);

            match inner.write() {
                Ok(mut guard) => {
                    let code_hash = guard.fork_storage.read_value(&code_key);

                    let code = guard
                        .fork_storage
                        .load_factory_dep_internal(code_hash)
                        .unwrap_or_default();

                    Ok(Bytes::from(code))
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    /// Returns the transaction count for a given address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to get the transaction count for.
    /// * `_block` - Optional block ID variant.
    ///
    /// # Returns
    ///
    /// Returns a `BoxFuture` containing the transaction count as a `U256` wrapped in a `jsonrpc_core::Result`.
    fn get_transaction_count(
        &self,
        address: zksync_basic_types::Address,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> BoxFuture<jsonrpc_core::Result<U256>> {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            let nonce_key = get_nonce_key(&address);

            match inner.write() {
                Ok(mut guard) => {
                    let result = guard.fork_storage.read_value(&nonce_key);
                    Ok(h256_to_u64(result).into())
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    /// Retrieves the transaction receipt for a given transaction hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - The hash of the transaction to retrieve the receipt for.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` that resolves to an `Option` of a `TransactionReceipt` or an error.
    fn get_transaction_receipt(
        &self,
        hash: zksync_basic_types::H256,
    ) -> BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::TransactionReceipt>>> {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            let reader = match inner.read() {
                Ok(r) => r,
                Err(_) => return Err(into_jsrpc_error(Web3Error::InternalError)),
            };

            let tx_result = reader.tx_results.get(&hash);

            let receipt = tx_result.map(|info| TransactionReceipt {
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
                logs: info
                    .result
                    .result
                    .logs
                    .events
                    .iter()
                    .map(|log| Log {
                        address: log.address,
                        topics: log.indexed_topics.clone(),
                        data: zksync_types::Bytes(log.value.clone()),
                        block_hash: Some(hash),
                        block_number: Some(U64::from(info.miniblock_number)),
                        l1_batch_number: Some(U64::from(info.batch_number as u64)),
                        transaction_hash: Some(hash),
                        transaction_index: Some(U64::from(1)),
                        log_index: Some(U256::default()),
                        transaction_log_index: Some(U256::default()),
                        log_type: None,
                        removed: None,
                    })
                    .collect(),
                l2_to_l1_logs: vec![],
                status: Some(if info.result.status == TxExecutionStatus::Success {
                    U64::from(1)
                } else {
                    U64::from(0)
                }),
                effective_gas_price: Some(L2_GAS_PRICE.into()),
                ..Default::default()
            });

            Ok(receipt).map_err(|_: jsonrpc_core::Error| into_jsrpc_error(Web3Error::InternalError))
        })
    }

    /// Sends a raw transaction to the L2 network.
    ///
    /// # Arguments
    ///
    /// * `tx_bytes` - The transaction bytes to send.
    ///
    /// # Returns
    ///
    /// A future that resolves to the hash of the transaction if successful, or an error if the transaction is invalid or execution fails.
    fn send_raw_transaction(
        &self,
        tx_bytes: zksync_basic_types::Bytes,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::H256>> {
        let chain_id = match self.inner.read() {
            Ok(reader) => reader.fork_storage.chain_id,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError)).boxed()
            }
        };

        let (tx_req, hash) = match TransactionRequest::from_bytes(&tx_bytes.0, chain_id.0) {
            Ok(result) => result,
            Err(e) => {
                return futures::future::err(into_jsrpc_error(Web3Error::SerializationError(e)))
                    .boxed()
            }
        };

        let mut l2_tx: L2Tx = match L2Tx::from_request(tx_req, MAX_TX_SIZE) {
            Ok(tx) => tx,
            Err(e) => {
                return futures::future::err(into_jsrpc_error(Web3Error::SerializationError(e)))
                    .boxed()
            }
        };

        l2_tx.set_input(tx_bytes.0, hash);
        if hash != l2_tx.hash() {
            return futures::future::err(into_jsrpc_error(Web3Error::InvalidTransactionData(
                zksync_types::ethabi::Error::InvalidData,
            )))
            .boxed();
        };

        match self.run_l2_tx(l2_tx.clone(), TxExecutionMode::VerifyExecute) {
            Ok(_) => Ok(hash).into_boxed_future(),
            Err(e) => {
                let error_message = format!("Execution error: {}", e);
                futures::future::err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                    error_message,
                    l2_tx.hash().as_bytes().to_vec(),
                )))
                .boxed()
            }
        }
    }

    /// Returns a block by its hash. Currently, only hashes for blocks in memory are supported.
    ///
    /// # Arguments
    ///
    /// * `hash` - A `H256` type representing the hash of the block to retrieve.
    /// * `_full_transactions` - A boolean value indicating whether to retrieve full transactions or not.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` that resolves to a `Result` containing an `Option` of a `Block` with its transactions and other details.
    fn get_block_by_hash(
        &self,
        hash: zksync_basic_types::H256,
        _full_transactions: bool,
    ) -> jsonrpc_core::BoxFuture<
        jsonrpc_core::Result<
            Option<zksync_types::api::Block<zksync_types::api::TransactionVariant>>,
        >,
    > {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            // Currently we support only hashes for blocks in memory
            let reader = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;

            let matching_transaction = reader.tx_results.get(&hash);
            if matching_transaction.is_none() {
                return Err(into_jsrpc_error(Web3Error::InvalidTransactionData(
                    zksync_types::ethabi::Error::InvalidData,
                )));
            }

            let matching_block = reader
                .blocks
                .get(&matching_transaction.unwrap().batch_number);
            if matching_block.is_none() {
                return Err(into_jsrpc_error(Web3Error::NoBlock));
            }

            let txn: Vec<TransactionVariant> = vec![];
            let block = zksync_types::api::Block {
                transactions: txn,
                number: U64::from(matching_block.unwrap().batch_number),
                l1_batch_number: Some(U64::from(reader.current_batch)),
                gas_limit: U256::from(ETH_CALL_GAS_LIMIT),
                ..Default::default()
            };

            Ok(Some(block))
        })
    }

    /// Returns a future that resolves to an optional transaction with the given hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A 32-byte hash of the transaction.
    ///
    /// # Returns
    ///
    /// A `jsonrpc_core::BoxFuture` that resolves to a `jsonrpc_core::Result` containing an optional `zksync_types::api::Transaction`.
    fn get_transaction_by_hash(
        &self,
        hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::Transaction>>> {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            let reader = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;
            let tx_result = reader.tx_results.get(&hash);

            Ok(tx_result.and_then(|info| {
                let input_data = info.tx.common_data.input.clone().or(None)?;

                let chain_id = info.tx.extract_chain_id().or(None)?;

                Some(zksync_types::api::Transaction {
                    hash,
                    nonce: U256::from(info.tx.common_data.nonce.0),
                    block_hash: Some(hash),
                    block_number: Some(U64::from(info.miniblock_number)),
                    transaction_index: Some(U64::from(1)),
                    from: Some(info.tx.initiator_account()),
                    to: Some(info.tx.recipient_account()),
                    value: info.tx.execute.value,
                    gas_price: Default::default(),
                    gas: Default::default(),
                    input: input_data.data.into(),
                    v: Some(chain_id.into()),
                    r: Some(U256::zero()),
                    s: Some(U256::zero()),
                    raw: None,
                    transaction_type: {
                        let tx_type = match info.tx.common_data.transaction_type {
                            zksync_types::l2::TransactionType::LegacyTransaction => 0,
                            zksync_types::l2::TransactionType::EIP2930Transaction => 1,
                            zksync_types::l2::TransactionType::EIP1559Transaction => 2,
                            zksync_types::l2::TransactionType::EIP712Transaction => 113,
                            zksync_types::l2::TransactionType::PriorityOpTransaction => 255,
                            zksync_types::l2::TransactionType::ProtocolUpgradeTransaction => 254,
                        };
                        Some(tx_type.into())
                    },
                    access_list: None,
                    max_fee_per_gas: Some(info.tx.common_data.fee.max_fee_per_gas),
                    max_priority_fee_per_gas: Some(
                        info.tx.common_data.fee.max_priority_fee_per_gas,
                    ),
                    chain_id: chain_id.into(),
                    l1_batch_number: Some(U64::from(info.batch_number as u64)),
                    l1_batch_tx_index: None,
                })
            }))
        })
    }

    /// Returns the current block number as a `U64` wrapped in a `BoxFuture`.
    fn get_block_number(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::U64>> {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            let reader = inner
                .read()
                .map_err(|_| into_jsrpc_error(Web3Error::InternalError))?;
            Ok(U64::from(reader.current_miniblock))
        })
    }

    /// Estimates the gas required for a given call request.
    ///
    /// # Arguments
    ///
    /// * `req` - A `CallRequest` struct representing the call request to estimate gas for.
    /// * `_block` - An optional `BlockNumber` struct representing the block number to estimate gas for.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `U256` representing the estimated gas required.
    fn estimate_gas(
        &self,
        req: zksync_types::transaction_request::CallRequest,
        _block: Option<zksync_types::api::BlockNumber>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<U256>> {
        let inner = Arc::clone(&self.inner);
        let reader = match inner.read() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError)).boxed()
            }
        };

        let result: jsonrpc_core::Result<Fee> = reader.estimate_gas_impl(req);
        match result {
            Ok(fee) => Ok(fee.gas_limit).into_boxed_future(),
            Err(err) => return futures::future::err(err).boxed(),
        }
    }

    /// Returns the current gas price in U256 format.
    fn gas_price(&self) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<U256>> {
        let fair_l2_gas_price: u64 = L2_GAS_PRICE;
        Ok(U256::from(fair_l2_gas_price)).into_boxed_future()
    }

    // Methods below are not currently implemented.

    fn new_filter(&self, _filter: Filter) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<U256>> {
        not_implemented("new_filter")
    }

    fn new_block_filter(&self) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<U256>> {
        not_implemented("new_block_filter")
    }

    fn uninstall_filter(&self, _idx: U256) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<bool>> {
        not_implemented("uninstall_filter")
    }

    fn new_pending_transaction_filter(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<U256>> {
        not_implemented("new_pending_transaction_filter")
    }

    fn get_logs(
        &self,
        _filter: Filter,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<zksync_types::api::Log>>> {
        not_implemented("get_logs")
    }

    fn get_filter_logs(
        &self,
        _filter_index: U256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<FilterChanges>> {
        not_implemented("get_filter_logs")
    }

    fn get_filter_changes(
        &self,
        _filter_index: U256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<FilterChanges>> {
        not_implemented("get_filter_changes")
    }

    fn get_block_transaction_count_by_number(
        &self,
        _block_number: zksync_types::api::BlockNumber,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<U256>>> {
        not_implemented("get_block_transaction_count_by_number")
    }

    fn get_block_transaction_count_by_hash(
        &self,
        _block_hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<U256>>> {
        not_implemented("get_block_transaction_count_by_hash")
    }

    fn get_storage(
        &self,
        _address: zksync_basic_types::Address,
        _idx: U256,
        _block: Option<zksync_types::api::BlockIdVariant>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::H256>> {
        not_implemented("get_storage")
    }

    fn get_transaction_by_block_hash_and_index(
        &self,
        _block_hash: zksync_basic_types::H256,
        _index: zksync_basic_types::web3::types::Index,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::Transaction>>> {
        not_implemented("get_transaction_by_block_hash_and_index")
    }

    fn get_transaction_by_block_number_and_index(
        &self,
        _block_number: zksync_types::api::BlockNumber,
        _index: zksync_basic_types::web3::types::Index,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<zksync_types::api::Transaction>>> {
        not_implemented("get_transaction_by_block_number_and_index")
    }

    fn protocol_version(&self) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<String>> {
        not_implemented("protocol_version")
    }

    fn syncing(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::web3::types::SyncState>>
    {
        not_implemented("syncing")
    }

    fn accounts(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<zksync_basic_types::Address>>> {
        not_implemented("accounts")
    }

    fn coinbase(
        &self,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::Address>> {
        not_implemented("coinbase")
    }

    fn compilers(&self) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Vec<String>>> {
        not_implemented("compilers")
    }

    fn hashrate(&self) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<U256>> {
        not_implemented("hashrate")
    }

    fn get_uncle_count_by_block_hash(
        &self,
        _hash: zksync_basic_types::H256,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<U256>>> {
        not_implemented("get_uncle_count_by_block_hash")
    }

    fn get_uncle_count_by_block_number(
        &self,
        _number: zksync_types::api::BlockNumber,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<Option<U256>>> {
        not_implemented("get_uncle_count_by_block_number")
    }

    fn mining(&self) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<bool>> {
        not_implemented("mining")
    }

    fn fee_history(
        &self,
        _block_count: U64,
        _newest_block: zksync_types::api::BlockNumber,
        _reward_percentiles: Vec<f32>,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<FeeHistory>> {
        not_implemented("fee history")
    }
}
