//! In-memory node, that supports forking other networks.
use crate::{
    bootloader_debug::{BootloaderDebug, BootloaderDebugTracer},
    console_log::ConsoleLogHandler,
    deps::InMemoryStorage,
    filters::EthFilters,
    fork::{ForkDetails, ForkSource, ForkStorage},
    formatter,
    observability::Observability,
    system_contracts::{self, Options, SystemContracts},
    utils::{
        adjust_l1_gas_price_for_tx, bytecode_to_factory_dep, create_debug_output, to_human_size,
    },
};
use clap::Parser;
use colored::Colorize;
use core::fmt::Display;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use std::{
    cmp::{self},
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::{Arc, RwLock},
};

use multivm::interface::{
    ExecutionResult, L1BatchEnv, L2BlockEnv, SystemEnv, TxExecutionMode, VmExecutionMode,
    VmExecutionResultAndLogs,
};
use multivm::vm_virtual_blocks::{
    constants::{BLOCK_GAS_LIMIT, BLOCK_OVERHEAD_PUBDATA, MAX_PUBDATA_PER_BLOCK},
    utils::{
        fee::derive_base_fee_and_gas_per_pubdata,
        l2_blocks::load_last_l2_block,
        overhead::{derive_overhead, OverheadCoeficients},
    },
    CallTracer, HistoryDisabled, Vm, VmTracer,
};
use zksync_basic_types::{
    web3::signing::keccak256, Address, Bytes, L1BatchNumber, MiniblockNumber, H160, H256, U256, U64,
};
use zksync_contracts::BaseSystemContracts;
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_state::{ReadStorage, StoragePtr, StorageView, WriteStorage};
use zksync_types::{
    api::{Block, DebugCall, Log, TransactionReceipt, TransactionVariant},
    block::legacy_miniblock_hash,
    fee::Fee,
    get_nonce_key,
    l2::L2Tx,
    l2::TransactionType,
    utils::{decompose_full_nonce, nonces_to_full_nonce, storage_key_for_eth_balance},
    vm_trace::Call,
    PackedEthSignature, StorageKey, StorageLogQueryType, StorageValue, Transaction,
    ACCOUNT_CODE_STORAGE_ADDRESS, EIP_712_TX_TYPE, MAX_GAS_PER_PUBDATA_BYTE, MAX_L2_TX_GAS_LIMIT,
};
use zksync_utils::{
    bytecode::{compress_bytecode, hash_bytecode},
    h256_to_account_address, h256_to_u256, u256_to_h256,
};
use zksync_web3_decl::error::Web3Error;

/// Max possible size of an ABI encoded tx (in bytes).
pub const MAX_TX_SIZE: usize = 1_000_000;
/// Timestamp of the first block (if not running in fork mode).
pub const NON_FORK_FIRST_BLOCK_TIMESTAMP: u64 = 1_000;
/// Network ID we use for the test node.
pub const TEST_NODE_NETWORK_ID: u32 = 260;
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
/// The maximum number of previous blocks to store the state for.
pub const MAX_PREVIOUS_STATES: u16 = 128;
/// The zks protocol version.
pub const PROTOCOL_VERSION: &str = "zks/1";

pub fn compute_hash(block_number: u64, tx_hash: H256) -> H256 {
    let digest = [&block_number.to_be_bytes()[..], tx_hash.as_bytes()].concat();
    H256(keccak256(&digest))
}

pub fn create_empty_block<TX>(block_number: u64, timestamp: u64, batch: u32) -> Block<TX> {
    let hash = compute_hash(block_number, H256::zero());
    Block {
        hash,
        number: U64::from(block_number),
        timestamp: U256::from(timestamp),
        l1_batch_number: Some(U64::from(batch)),
        transactions: vec![],
        gas_used: U256::from(0),
        gas_limit: U256::from(BLOCK_GAS_LIMIT),
        ..Default::default()
    }
}

/// Information about the executed transaction.
#[derive(Debug, Clone)]
pub struct TxExecutionInfo {
    pub tx: L2Tx,
    // Batch number where transaction was executed.
    pub batch_number: u32,
    pub miniblock_number: u64,
    pub result: VmExecutionResultAndLogs,
}

#[derive(Debug, Default, clap::Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowCalls {
    #[default]
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

#[derive(Debug, Default, Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowStorageLogs {
    #[default]
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

#[derive(Debug, Default, Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowVMDetails {
    #[default]
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

#[derive(Debug, Default, Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowGasDetails {
    #[default]
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

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub info: TxExecutionInfo,
    pub receipt: TransactionReceipt,
    pub debug: DebugCall,
}

impl TransactionResult {
    /// Returns the debug information for the transaction.
    /// If `only_top` is true - will only return the top level call.
    pub fn debug_info(&self, only_top: bool) -> DebugCall {
        let calls = if only_top {
            vec![]
        } else {
            self.debug.calls.clone()
        };
        DebugCall {
            calls,
            ..self.debug.clone()
        }
    }
}

/// Helper struct for InMemoryNode.
/// S - is the Source of the Fork.
#[derive(Clone)]
pub struct InMemoryNodeInner<S> {
    /// The latest timestamp that was already generated.
    /// Next block will be current_timestamp + 1
    pub current_timestamp: u64,
    /// The latest batch number that was already generated.
    /// Next block will be current_batch + 1
    pub current_batch: u32,
    /// The latest miniblock number that was already generated.
    /// Next transaction will go to the block current_miniblock + 1
    pub current_miniblock: u64,
    /// The latest miniblock hash.
    pub current_miniblock_hash: H256,
    pub l1_gas_price: u64,
    // Map from transaction to details about the exeuction
    pub tx_results: HashMap<H256, TransactionResult>,
    // Map from block hash to information about the block.
    pub blocks: HashMap<H256, Block<TransactionVariant>>,
    // Map from block number to a block hash.
    pub block_hashes: HashMap<u64, H256>,
    // Map from filter_id to the eth filter
    pub filters: EthFilters,
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
    pub impersonated_accounts: HashSet<Address>,
    pub rich_accounts: HashSet<H160>,
    /// Keeps track of historical states indexed via block hash. Limited to [MAX_PREVIOUS_STATES].
    pub previous_states: IndexMap<H256, HashMap<StorageKey, StorageValue>>,
    /// An optional handle to the observability stack
    pub observability: Option<Observability>,
}

type L2TxResult = (
    HashMap<StorageKey, H256>,
    VmExecutionResultAndLogs,
    Vec<Call>,
    Block<TransactionVariant>,
    HashMap<U256, Vec<U256>>,
    BlockContext,
);

impl<S: std::fmt::Debug + ForkSource> InMemoryNodeInner<S> {
    pub fn create_l1_batch_env<ST: ReadStorage>(
        &self,
        storage: StoragePtr<ST>,
    ) -> (L1BatchEnv, BlockContext) {
        let last_l2_block_hash = if let Some(last_l2_block) = load_last_l2_block(storage) {
            last_l2_block.hash
        } else {
            // This is the scenario of either the first L2 block ever or
            // the first block after the upgrade for support of L2 blocks.
            legacy_miniblock_hash(MiniblockNumber(self.current_miniblock as u32))
        };
        let block_ctx = BlockContext::from_current(
            self.current_batch,
            self.current_miniblock,
            self.current_timestamp,
        );
        let block_ctx = block_ctx.new_batch();
        let batch_env = L1BatchEnv {
            // TODO: set the previous batch hash properly (take from fork, when forking, and from local storage, when this is not the first block).
            previous_batch_hash: None,
            number: L1BatchNumber::from(block_ctx.batch),
            timestamp: block_ctx.timestamp,
            l1_gas_price: self.l1_gas_price,
            fair_l2_gas_price: L2_GAS_PRICE,
            fee_account: H160::zero(),
            enforced_base_fee: None,
            first_l2_block: L2BlockEnv {
                // the 'current_miniblock' contains the block that was already produced.
                // So the next one should be one higher.
                number: block_ctx.miniblock as u32,
                timestamp: block_ctx.timestamp,
                prev_block_hash: last_l2_block_hash,
                // This is only used during zksyncEra block timestamp/number transition.
                // In case of starting a new network, it doesn't matter.
                // In theory , when forking mainnet, we should match this value
                // to the value that was set in the node at that time - but AFAIK
                // we don't have any API for this - so this might result in slightly
                // incorrect replays of transacions during the migration period, that
                // depend on block number or timestamp.
                max_virtual_blocks_to_create: 1,
            },
        };

        (batch_env, block_ctx)
    }

    pub fn create_system_env(
        &self,
        base_system_contracts: BaseSystemContracts,
        execution_mode: TxExecutionMode,
    ) -> SystemEnv {
        SystemEnv {
            zk_porter_available: false,
            // TODO: when forking, we could consider taking the protocol version id from the fork itself.
            version: zksync_types::ProtocolVersionId::latest(),
            base_system_smart_contracts: base_system_contracts,
            gas_limit: BLOCK_GAS_LIMIT,
            execution_mode,
            default_validation_computational_gas_limit: BLOCK_GAS_LIMIT,
            chain_id: self.fork_storage.chain_id,
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
        let mut request_with_gas_per_pubdata_overridden = req;

        if let Some(ref mut eip712_meta) = request_with_gas_per_pubdata_overridden.eip712_meta {
            if eip712_meta.gas_per_pubdata == U256::zero() {
                eip712_meta.gas_per_pubdata = MAX_GAS_PER_PUBDATA_BYTE.into();
            }
        }

        let is_eip712 = request_with_gas_per_pubdata_overridden
            .eip712_meta
            .is_some();

        let mut l2_tx =
            match L2Tx::from_request(request_with_gas_per_pubdata_overridden.into(), MAX_TX_SIZE) {
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

        // The user may not include the proper transaction type during the estimation of
        // the gas fee. However, it is needed for the bootloader checks to pass properly.
        if is_eip712 {
            l2_tx.common_data.transaction_type = TransactionType::EIP712Transaction;
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

        let storage = storage_view.to_rc_ptr();

        let execution_mode = TxExecutionMode::EstimateFee;
        let (mut batch_env, _) = self.create_l1_batch_env(storage.clone());
        batch_env.l1_gas_price = l1_gas_price;
        let system_env = self.create_system_env(
            self.system_contracts.contracts_for_fee_estimate().clone(),
            execution_mode,
        );

        // We are using binary search to find the minimal values of gas_limit under which the transaction succeeds
        let mut lower_bound = 0;
        let mut upper_bound = MAX_L2_TX_GAS_LIMIT as u32;
        let mut attempt_count = 1;

        tracing::trace!("Starting gas estimation loop");
        while lower_bound + ESTIMATE_GAS_ACCEPTABLE_OVERESTIMATION < upper_bound {
            let mid = (lower_bound + upper_bound) / 2;
            tracing::trace!(
                "Attempt {} (lower_bound: {}, upper_bound: {}, mid: {})",
                attempt_count,
                lower_bound,
                upper_bound,
                mid
            );
            let try_gas_limit = gas_for_bytecodes_pubdata + mid;

            let estimate_gas_result = InMemoryNodeInner::estimate_gas_step(
                l2_tx.clone(),
                gas_per_pubdata_byte,
                try_gas_limit,
                l1_gas_price,
                batch_env.clone(),
                system_env.clone(),
                &self.fork_storage,
            );

            if estimate_gas_result.result.is_failed() {
                tracing::trace!("Attempt {} FAILED", attempt_count);
                lower_bound = mid + 1;
            } else {
                tracing::trace!("Attempt {} SUCCEEDED", attempt_count);
                upper_bound = mid;
            }
            attempt_count += 1;
        }

        tracing::trace!("Gas Estimation Values:");
        tracing::trace!("  Final upper_bound: {}", upper_bound);
        tracing::trace!("  ESTIMATE_GAS_SCALE_FACTOR: {}", ESTIMATE_GAS_SCALE_FACTOR);
        tracing::trace!("  MAX_L2_TX_GAS_LIMIT: {}", MAX_L2_TX_GAS_LIMIT);
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
            batch_env,
            system_env,
            &self.fork_storage,
        );

        let coefficients = OverheadCoeficients::from_tx_type(EIP_712_TX_TYPE);
        let overhead: u32 = derive_overhead(
            suggested_gas_limit,
            gas_per_pubdata_byte as u32,
            tx.encoding_len(),
            coefficients,
        );

        match estimate_gas_result.result {
            ExecutionResult::Revert { output } => {
                tracing::info!("{}", format!("Unable to estimate gas for the request with our suggested gas limit of {}. The transaction is most likely unexecutable. Breakdown of estimation:", suggested_gas_limit + overhead).red());
                tracing::info!(
                    "{}",
                    format!(
                        "\tEstimated transaction body gas cost: {}",
                        tx_body_gas_limit
                    )
                    .red()
                );
                tracing::info!(
                    "{}",
                    format!("\tGas for pubdata: {}", gas_for_bytecodes_pubdata).red()
                );
                tracing::info!("{}", format!("\tOverhead: {}", overhead).red());
                let message = output.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );
                let data = output.encoded_data();
                tracing::info!("{}", pretty_message.on_red());
                Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                    pretty_message,
                    data,
                )))
            }
            ExecutionResult::Halt { reason } => {
                tracing::info!("{}", format!("Unable to estimate gas for the request with our suggested gas limit of {}. The transaction is most likely unexecutable. Breakdown of estimation:", suggested_gas_limit + overhead).red());
                tracing::info!(
                    "{}",
                    format!(
                        "\tEstimated transaction body gas cost: {}",
                        tx_body_gas_limit
                    )
                    .red()
                );
                tracing::info!(
                    "{}",
                    format!("\tGas for pubdata: {}", gas_for_bytecodes_pubdata).red()
                );
                tracing::info!("{}", format!("\tOverhead: {}", overhead).red());
                let message = reason.to_string();
                let pretty_message = format!(
                    "execution reverted{}{}",
                    if message.is_empty() { "" } else { ": " },
                    message
                );

                tracing::info!("{}", pretty_message.on_red());
                Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                    pretty_message,
                    vec![],
                )))
            }
            ExecutionResult::Success { .. } => {
                let full_gas_limit = match tx_body_gas_limit
                    .overflowing_add(gas_for_bytecodes_pubdata + overhead)
                {
                    (value, false) => value,
                    (_, true) => {
                        tracing::info!("{}", "Overflow when calculating gas estimation. We've exceeded the block gas limit by summing the following values:".red());
                        tracing::info!(
                            "{}",
                            format!(
                                "\tEstimated transaction body gas cost: {}",
                                tx_body_gas_limit
                            )
                            .red()
                        );
                        tracing::info!(
                            "{}",
                            format!("\tGas for pubdata: {}", gas_for_bytecodes_pubdata).red()
                        );
                        tracing::info!("{}", format!("\tOverhead: {}", overhead).red());
                        return Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                            "exceeds block gas limit".into(),
                            Default::default(),
                        )));
                    }
                };

                tracing::trace!("Gas Estimation Results");
                tracing::trace!("  tx_body_gas_limit: {}", tx_body_gas_limit);
                tracing::trace!("  gas_for_bytecodes_pubdata: {}", gas_for_bytecodes_pubdata);
                tracing::trace!("  overhead: {}", overhead);
                tracing::trace!("  full_gas_limit: {}", full_gas_limit);
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
        mut batch_env: L1BatchEnv,
        system_env: SystemEnv,
        fork_storage: &ForkStorage<S>,
    ) -> VmExecutionResultAndLogs {
        let tx: Transaction = l2_tx.clone().into();
        let l1_gas_price =
            adjust_l1_gas_price_for_tx(l1_gas_price, L2_GAS_PRICE, tx.gas_per_pubdata_byte_limit());

        let coefficients = OverheadCoeficients::from_tx_type(EIP_712_TX_TYPE);
        // Set gas_limit for transaction
        let gas_limit_with_overhead = tx_gas_limit
            + derive_overhead(
                tx_gas_limit,
                gas_per_pubdata_byte as u32,
                tx.encoding_len(),
                coefficients,
            );
        l2_tx.common_data.fee.gas_limit = gas_limit_with_overhead.into();

        let storage = StorageView::new(fork_storage).to_rc_ptr();

        // The nonce needs to be updated
        let nonce = l2_tx.nonce();
        let nonce_key = get_nonce_key(&l2_tx.initiator_account());
        let full_nonce = storage.borrow_mut().read_value(&nonce_key);
        let (_, deployment_nonce) = decompose_full_nonce(h256_to_u256(full_nonce));
        let enforced_full_nonce = nonces_to_full_nonce(U256::from(nonce.0), deployment_nonce);
        storage
            .borrow_mut()
            .set_value(nonce_key, u256_to_h256(enforced_full_nonce));

        // We need to explicitly put enough balance into the account of the users
        let payer = l2_tx.payer();
        let balance_key = storage_key_for_eth_balance(&payer);
        let mut current_balance = h256_to_u256(storage.borrow_mut().read_value(&balance_key));
        let added_balance = l2_tx.common_data.fee.gas_limit * l2_tx.common_data.fee.max_fee_per_gas;
        current_balance += added_balance;
        storage
            .borrow_mut()
            .set_value(balance_key, u256_to_h256(current_balance));

        batch_env.l1_gas_price = l1_gas_price;

        let mut vm = Vm::new(batch_env, system_env, storage, HistoryDisabled);

        let tx: Transaction = l2_tx.into();
        vm.push_transaction(tx);

        vm.execute(VmExecutionMode::OneTx)
    }

    /// Sets the `impersonated_account` field of the node.
    /// This field is used to override the `tx.initiator_account` field of the transaction in the `run_l2_tx` method.
    pub fn set_impersonated_account(&mut self, address: Address) -> bool {
        self.impersonated_accounts.insert(address)
    }

    /// Clears the `impersonated_account` field of the node.
    pub fn stop_impersonating_account(&mut self, address: Address) -> bool {
        self.impersonated_accounts.remove(&address)
    }

    /// Archives the current state for later queries.
    pub fn archive_state(&mut self) -> Result<(), String> {
        if self.previous_states.len() > MAX_PREVIOUS_STATES as usize {
            if let Some(entry) = self.previous_states.shift_remove_index(0) {
                tracing::debug!("removing archived state for previous block {:#x}", entry.0);
            }
        }
        tracing::debug!(
            "archiving state for {:#x} #{}",
            self.current_miniblock_hash,
            self.current_miniblock
        );
        self.previous_states.insert(
            self.current_miniblock_hash,
            self.fork_storage
                .inner
                .read()
                .map_err(|err| err.to_string())?
                .raw_storage
                .state
                .clone(),
        );

        Ok(())
    }

    /// Creates a [Snapshot] of the current state of the node.
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let storage = self
            .fork_storage
            .inner
            .read()
            .map_err(|err| format!("failed acquiring read lock on storage: {:?}", err))?;

        Ok(Snapshot {
            current_timestamp: self.current_timestamp,
            current_batch: self.current_batch,
            current_miniblock: self.current_miniblock,
            current_miniblock_hash: self.current_miniblock_hash,
            l1_gas_price: self.l1_gas_price,
            tx_results: self.tx_results.clone(),
            blocks: self.blocks.clone(),
            block_hashes: self.block_hashes.clone(),
            filters: self.filters.clone(),
            impersonated_accounts: self.impersonated_accounts.clone(),
            rich_accounts: self.rich_accounts.clone(),
            previous_states: self.previous_states.clone(),
            raw_storage: storage.raw_storage.clone(),
            value_read_cache: storage.value_read_cache.clone(),
            factory_dep_cache: storage.factory_dep_cache.clone(),
        })
    }

    /// Restores a previously created [Snapshot] of the node.
    pub fn restore_snapshot(&mut self, snapshot: Snapshot) -> Result<(), String> {
        let mut storage = self
            .fork_storage
            .inner
            .write()
            .map_err(|err| format!("failed acquiring write lock on storage: {:?}", err))?;

        self.current_timestamp = snapshot.current_timestamp;
        self.current_batch = snapshot.current_batch;
        self.current_miniblock = snapshot.current_miniblock;
        self.current_miniblock_hash = snapshot.current_miniblock_hash;
        self.l1_gas_price = snapshot.l1_gas_price;
        self.tx_results = snapshot.tx_results;
        self.blocks = snapshot.blocks;
        self.block_hashes = snapshot.block_hashes;
        self.filters = snapshot.filters;
        self.impersonated_accounts = snapshot.impersonated_accounts;
        self.rich_accounts = snapshot.rich_accounts;
        self.previous_states = snapshot.previous_states;
        storage.raw_storage = snapshot.raw_storage;
        storage.value_read_cache = snapshot.value_read_cache;
        storage.factory_dep_cache = snapshot.factory_dep_cache;

        Ok(())
    }
}

/// Creates a restorable snapshot for the [InMemoryNodeInner]. The snapshot contains all the necessary
/// data required to restore the [InMemoryNodeInner] state to a previous point in time.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub(crate) current_timestamp: u64,
    pub(crate) current_batch: u32,
    pub(crate) current_miniblock: u64,
    pub(crate) current_miniblock_hash: H256,
    pub(crate) l1_gas_price: u64,
    pub(crate) tx_results: HashMap<H256, TransactionResult>,
    pub(crate) blocks: HashMap<H256, Block<TransactionVariant>>,
    pub(crate) block_hashes: HashMap<u64, H256>,
    pub(crate) filters: EthFilters,
    pub(crate) impersonated_accounts: HashSet<Address>,
    pub(crate) rich_accounts: HashSet<H160>,
    pub(crate) previous_states: IndexMap<H256, HashMap<StorageKey, StorageValue>>,
    pub(crate) raw_storage: InMemoryStorage,
    pub(crate) value_read_cache: HashMap<StorageKey, H256>,
    pub(crate) factory_dep_cache: HashMap<H256, Option<Vec<u8>>>,
}

/// Defines the configuration parameters for the [InMemoryNode].
#[derive(Default, Debug, Clone)]
pub struct InMemoryNodeConfig {
    pub show_calls: ShowCalls,
    pub show_storage_logs: ShowStorageLogs,
    pub show_vm_details: ShowVMDetails,
    pub show_gas_details: ShowGasDetails,
    pub resolve_hashes: bool,
    pub system_contracts_options: system_contracts::Options,
}

/// In-memory node, that can be used for local & unit testing.
/// It also supports the option of forking testnet/mainnet.
/// All contents are removed when object is destroyed.
#[derive(Clone)]
pub struct InMemoryNode<S: Clone> {
    /// A thread safe reference to the [InMemoryNodeInner].
    inner: Arc<RwLock<InMemoryNodeInner<S>>>,
    /// List of snapshots of the [InMemoryNodeInner]. This is bounded at runtime by [MAX_SNAPSHOTS].
    pub(crate) snapshots: Arc<RwLock<Vec<Snapshot>>>,
}

fn contract_address_from_tx_result(execution_result: &VmExecutionResultAndLogs) -> Option<H160> {
    for query in execution_result.logs.storage_logs.iter().rev() {
        if query.log_type == StorageLogQueryType::InitialWrite
            && query.log_query.address == ACCOUNT_CODE_STORAGE_ADDRESS
        {
            return Some(h256_to_account_address(&u256_to_h256(query.log_query.key)));
        }
    }
    None
}

impl<S: ForkSource + std::fmt::Debug + Clone> Default for InMemoryNode<S> {
    fn default() -> Self {
        InMemoryNode::new(None, None, InMemoryNodeConfig::default())
    }
}

impl<S: ForkSource + std::fmt::Debug + Clone> InMemoryNode<S> {
    pub fn new(
        fork: Option<ForkDetails<S>>,
        observability: Option<Observability>,
        config: InMemoryNodeConfig,
    ) -> Self {
        let inner = if let Some(f) = &fork {
            let mut block_hashes = HashMap::<u64, H256>::new();
            block_hashes.insert(f.l2_block.number.as_u64(), f.l2_block.hash);
            let mut blocks = HashMap::<H256, Block<TransactionVariant>>::new();
            blocks.insert(f.l2_block.hash, f.l2_block.clone());

            InMemoryNodeInner {
                current_timestamp: f.block_timestamp,
                current_batch: f.l1_block.0,
                current_miniblock: f.l2_miniblock,
                current_miniblock_hash: f.l2_miniblock_hash,
                l1_gas_price: f.l1_gas_price,
                tx_results: Default::default(),
                blocks,
                block_hashes,
                filters: Default::default(),
                fork_storage: ForkStorage::new(fork, &config.system_contracts_options),
                show_calls: config.show_calls,
                show_storage_logs: config.show_storage_logs,
                show_vm_details: config.show_vm_details,
                show_gas_details: config.show_gas_details,
                resolve_hashes: config.resolve_hashes,
                console_log_handler: ConsoleLogHandler::default(),
                system_contracts: SystemContracts::from_options(&config.system_contracts_options),
                impersonated_accounts: Default::default(),
                rich_accounts: HashSet::new(),
                previous_states: Default::default(),
                observability,
            }
        } else {
            let mut block_hashes = HashMap::<u64, H256>::new();
            block_hashes.insert(0, H256::zero());
            let mut blocks = HashMap::<H256, Block<TransactionVariant>>::new();
            blocks.insert(
                H256::zero(),
                create_empty_block(0, NON_FORK_FIRST_BLOCK_TIMESTAMP, 0),
            );

            InMemoryNodeInner {
                current_timestamp: NON_FORK_FIRST_BLOCK_TIMESTAMP,
                current_batch: 0,
                current_miniblock: 0,
                current_miniblock_hash: H256::zero(),
                l1_gas_price: L1_GAS_PRICE,
                tx_results: Default::default(),
                blocks,
                block_hashes,
                filters: Default::default(),
                fork_storage: ForkStorage::new(fork, &config.system_contracts_options),
                show_calls: config.show_calls,
                show_storage_logs: config.show_storage_logs,
                show_vm_details: config.show_vm_details,
                show_gas_details: config.show_gas_details,
                resolve_hashes: config.resolve_hashes,
                console_log_handler: ConsoleLogHandler::default(),
                system_contracts: SystemContracts::from_options(&config.system_contracts_options),
                impersonated_accounts: Default::default(),
                rich_accounts: HashSet::new(),
                previous_states: Default::default(),
                observability,
            }
        };

        InMemoryNode {
            inner: Arc::new(RwLock::new(inner)),
            snapshots: Default::default(),
        }
    }

    pub fn get_inner(&self) -> Arc<RwLock<InMemoryNodeInner<S>>> {
        self.inner.clone()
    }

    /// Applies multiple transactions - but still one per L1 batch.
    pub fn apply_txs(&self, txs: Vec<L2Tx>) -> Result<(), String> {
        tracing::info!("Running {:?} transactions (one per batch)", txs.len());

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
                tracing::info!("Failed to acquire write lock: {}", e);
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
        inner.rich_accounts.insert(address);
    }

    /// Runs L2 'eth call' method - that doesn't commit to a block.
    pub fn run_l2_call(&self, mut l2_tx: L2Tx) -> Result<ExecutionResult, String> {
        let execution_mode = TxExecutionMode::EthCall;

        let inner = self
            .inner
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        let storage = StorageView::new(&inner.fork_storage).to_rc_ptr();

        let bootloader_code = inner.system_contracts.contracts_for_l2_call();

        // init vm

        let (batch_env, _) = inner.create_l1_batch_env(storage.clone());
        let system_env = inner.create_system_env(bootloader_code.clone(), execution_mode);

        let mut vm = Vm::new(batch_env, system_env, storage, HistoryDisabled);

        // We must inject *some* signature (otherwise bootloader code fails to generate hash).
        if l2_tx.common_data.signature.is_empty() {
            l2_tx.common_data.signature = PackedEthSignature::default().serialize_packed().into();
        }

        let tx: Transaction = l2_tx.into();
        vm.push_transaction(tx);

        let call_tracer_result = Arc::new(OnceCell::default());

        let custom_tracers =
            vec![
                Box::new(CallTracer::new(call_tracer_result.clone(), HistoryDisabled))
                    as Box<dyn VmTracer<StorageView<&ForkStorage<S>>, HistoryDisabled>>,
            ];

        let tx_result = vm.inspect(custom_tracers, VmExecutionMode::OneTx);

        let call_traces = Arc::try_unwrap(call_tracer_result)
            .unwrap()
            .take()
            .unwrap_or_default();

        match &tx_result.result {
            ExecutionResult::Success { output } => {
                tracing::info!("Call: {}", "SUCCESS".green());
                let output_bytes = zksync_basic_types::Bytes::from(output.clone());
                tracing::info!("Output: {}", serde_json::to_string(&output_bytes).unwrap());
            }
            ExecutionResult::Revert { output } => {
                tracing::info!("Call: {}: {}", "FAILED".red(), output);
            }
            ExecutionResult::Halt { reason } => {
                tracing::info!("Call: {} {}", "HALTED".red(), reason)
            }
        };

        tracing::info!("=== Console Logs: ");
        for call in &call_traces {
            inner.console_log_handler.handle_call_recursive(call);
        }

        tracing::info!("=== Call traces:");
        for call in &call_traces {
            formatter::print_call(call, 0, &inner.show_calls, inner.resolve_hashes);
        }

        Ok(tx_result.result)
    }

    fn display_detailed_gas_info(
        &self,
        bootloader_debug_result: Option<&eyre::Result<BootloaderDebug, String>>,
        spent_on_pubdata: u32,
    ) -> eyre::Result<(), String> {
        if let Some(bootloader_result) = bootloader_debug_result {
            let bootloader_debug = bootloader_result.clone()?;

            tracing::info!("┌─────────────────────────┐");
            tracing::info!("│       GAS DETAILS       │");
            tracing::info!("└─────────────────────────┘");

            // Total amount of gas (should match tx.gas_limit).
            let total_gas_limit = bootloader_debug
                .total_gas_limit_from_user
                .saturating_sub(bootloader_debug.reserved_gas);

            let intrinsic_gas = total_gas_limit - bootloader_debug.gas_limit_after_intrinsic;
            let gas_for_validation =
                bootloader_debug.gas_limit_after_intrinsic - bootloader_debug.gas_after_validation;

            let gas_spent_on_compute = bootloader_debug.gas_spent_on_execution
                - bootloader_debug.gas_spent_on_bytecode_preparation;

            let gas_used = intrinsic_gas
                + gas_for_validation
                + bootloader_debug.gas_spent_on_bytecode_preparation
                + gas_spent_on_compute;

            tracing::info!(
                "Gas - Limit: {} | Used: {} | Refunded: {}",
                to_human_size(total_gas_limit),
                to_human_size(gas_used),
                to_human_size(bootloader_debug.refund_by_operator)
            );

            if bootloader_debug.total_gas_limit_from_user != total_gas_limit {
                tracing::info!(
                    "{}",
                    format!(
                "  WARNING: user actually provided more gas {}, but system had a lower max limit.",
                to_human_size(bootloader_debug.total_gas_limit_from_user)
            )
                    .yellow()
                );
            }
            if bootloader_debug.refund_computed != bootloader_debug.refund_by_operator {
                tracing::info!(
                    "{}",
                    format!(
                        "  WARNING: Refund by VM: {}, but operator refunded more: {}",
                        to_human_size(bootloader_debug.refund_computed),
                        to_human_size(bootloader_debug.refund_by_operator)
                    )
                    .yellow()
                );
            }

            if bootloader_debug.refund_computed + gas_used != total_gas_limit {
                tracing::info!(
                    "{}",
                    format!(
                        "  WARNING: Gas totals don't match. {} != {} , delta: {}",
                        to_human_size(bootloader_debug.refund_computed + gas_used),
                        to_human_size(total_gas_limit),
                        to_human_size(
                            total_gas_limit.abs_diff(bootloader_debug.refund_computed + gas_used)
                        )
                    )
                    .yellow()
                );
            }

            let bytes_published = spent_on_pubdata / bootloader_debug.gas_per_pubdata.as_u32();

            tracing::info!(
                "During execution published {} bytes to L1, @{} each - in total {} gas",
                to_human_size(bytes_published.into()),
                to_human_size(bootloader_debug.gas_per_pubdata),
                to_human_size(spent_on_pubdata.into())
            );

            tracing::info!("Out of {} gas used, we spent:", to_human_size(gas_used));
            tracing::info!(
                "  {:>15} gas ({:>2}%) for transaction setup",
                to_human_size(intrinsic_gas),
                to_human_size(intrinsic_gas * 100 / gas_used)
            );
            tracing::info!(
                "  {:>15} gas ({:>2}%) for bytecode preparation (decompression etc)",
                to_human_size(bootloader_debug.gas_spent_on_bytecode_preparation),
                to_human_size(bootloader_debug.gas_spent_on_bytecode_preparation * 100 / gas_used)
            );
            tracing::info!(
                "  {:>15} gas ({:>2}%) for account validation",
                to_human_size(gas_for_validation),
                to_human_size(gas_for_validation * 100 / gas_used)
            );
            tracing::info!(
                "  {:>15} gas ({:>2}%) for computations (opcodes)",
                to_human_size(gas_spent_on_compute),
                to_human_size(gas_spent_on_compute * 100 / gas_used)
            );

            tracing::info!("");
            tracing::info!("");
            tracing::info!(
                "{}",
                "=== Transaction setup cost breakdown ===".to_owned().bold(),
            );

            tracing::info!("Total cost: {}", to_human_size(intrinsic_gas).bold());
            tracing::info!(
                "  {:>15} gas ({:>2}%) fixed cost",
                to_human_size(bootloader_debug.intrinsic_overhead),
                to_human_size(bootloader_debug.intrinsic_overhead * 100 / intrinsic_gas)
            );
            tracing::info!(
                "  {:>15} gas ({:>2}%) operator cost",
                to_human_size(bootloader_debug.operator_overhead),
                to_human_size(bootloader_debug.operator_overhead * 100 / intrinsic_gas)
            );

            tracing::info!("");
            tracing::info!(
                "  FYI: operator could have charged up to: {}, so you got {}% discount",
                to_human_size(bootloader_debug.required_overhead),
                to_human_size(
                    (bootloader_debug.required_overhead - bootloader_debug.operator_overhead) * 100
                        / bootloader_debug.required_overhead
                )
            );

            let publish_block_l1_bytes = BLOCK_OVERHEAD_PUBDATA;
            tracing::info!(
            "Publishing full block costs the operator up to: {}, where {} is due to {} bytes published to L1",
            to_human_size(bootloader_debug.total_overhead_for_block),
            to_human_size(bootloader_debug.gas_per_pubdata * publish_block_l1_bytes),
            to_human_size(publish_block_l1_bytes.into())
        );
            tracing::info!("Your transaction has contributed to filling up the block in the following way (we take the max contribution as the cost):");
            tracing::info!(
                "  Circuits overhead:{:>15} ({}% of the full block: {})",
                to_human_size(bootloader_debug.overhead_for_circuits),
                to_human_size(
                    bootloader_debug.overhead_for_circuits * 100
                        / bootloader_debug.total_overhead_for_block
                ),
                to_human_size(bootloader_debug.total_overhead_for_block)
            );
            tracing::info!(
                "  Length overhead:  {:>15}",
                to_human_size(bootloader_debug.overhead_for_length)
            );
            tracing::info!(
                "  Slot overhead:    {:>15}",
                to_human_size(bootloader_debug.overhead_for_slot)
            );
            Ok(())
        } else {
            Err("Booloader tracer didn't finish.".to_owned())
        }
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

        let storage = StorageView::new(&inner.fork_storage).to_rc_ptr();

        let (batch_env, block_ctx) = inner.create_l1_batch_env(storage.clone());

        // if we are impersonating an account, we need to use non-verifying system contracts
        let nonverifying_contracts;
        let bootloader_code = {
            if inner
                .impersonated_accounts
                .contains(&l2_tx.common_data.initiator_address)
            {
                tracing::info!(
                    "🕵️ Executing tx from impersonated account {:?}",
                    l2_tx.common_data.initiator_address
                );
                nonverifying_contracts =
                    SystemContracts::from_options(&Options::BuiltInWithoutSecurity);
                nonverifying_contracts.contracts(execution_mode)
            } else {
                inner.system_contracts.contracts(execution_mode)
            }
        };
        let system_env = inner.create_system_env(bootloader_code.clone(), execution_mode);

        let mut vm = Vm::new(
            batch_env.clone(),
            system_env,
            storage.clone(),
            HistoryDisabled,
        );

        let tx: Transaction = l2_tx.clone().into();

        vm.push_transaction(tx.clone());

        let call_tracer_result = Arc::new(OnceCell::default());
        let bootloader_debug_result = Arc::new(OnceCell::default());

        let custom_tracers = vec![
            Box::new(CallTracer::new(call_tracer_result.clone(), HistoryDisabled))
                as Box<dyn VmTracer<StorageView<&ForkStorage<S>>, HistoryDisabled>>,
            Box::new(BootloaderDebugTracer {
                result: bootloader_debug_result.clone(),
            }) as Box<dyn VmTracer<StorageView<&ForkStorage<S>>, HistoryDisabled>>,
        ];

        let tx_result = vm.inspect(custom_tracers, VmExecutionMode::OneTx);

        let call_traces = call_tracer_result.get().unwrap();

        let spent_on_pubdata =
            tx_result.statistics.gas_used - tx_result.statistics.computational_gas_used;

        tracing::info!("┌─────────────────────────┐");
        tracing::info!("│   TRANSACTION SUMMARY   │");
        tracing::info!("└─────────────────────────┘");

        match &tx_result.result {
            ExecutionResult::Success { .. } => tracing::info!("Transaction: {}", "SUCCESS".green()),
            ExecutionResult::Revert { .. } => tracing::info!("Transaction: {}", "FAILED".red()),
            ExecutionResult::Halt { .. } => tracing::info!("Transaction: {}", "HALTED".red()),
        }

        tracing::info!("Initiator: {:?}", tx.initiator_account());
        tracing::info!("Payer: {:?}", tx.payer());
        tracing::info!(
            "Gas - Limit: {} | Used: {} | Refunded: {}",
            to_human_size(tx.gas_limit()),
            to_human_size(tx.gas_limit() - tx_result.refunds.gas_refunded),
            to_human_size(tx_result.refunds.gas_refunded.into())
        );

        match inner.show_gas_details {
            ShowGasDetails::None => tracing::info!(
                "Use --show-gas-details flag or call config_setShowGasDetails to display more info"
            ),
            ShowGasDetails::All => {
                if self
                    .display_detailed_gas_info(bootloader_debug_result.get(), spent_on_pubdata)
                    .is_err()
                {
                    tracing::info!(
                        "{}",
                        "!!! FAILED TO GET DETAILED GAS INFO !!!".to_owned().red()
                    );
                }
            }
        }

        if inner.show_storage_logs != ShowStorageLogs::None {
            tracing::info!("");
            tracing::info!("┌──────────────────┐");
            tracing::info!("│   STORAGE LOGS   │");
            tracing::info!("└──────────────────┘");
        }

        for log_query in &tx_result.logs.storage_logs {
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
            formatter::print_vm_details(&tx_result);
        }

        tracing::info!("");
        tracing::info!("==== Console logs: ");
        for call in call_traces {
            inner.console_log_handler.handle_call_recursive(call);
        }
        tracing::info!("");
        let call_traces_count = if !call_traces.is_empty() {
            // All calls/sub-calls are stored within the first call trace
            call_traces[0].calls.len()
        } else {
            0
        };
        tracing::info!(
            "==== {} Use --show-calls flag or call config_setShowCalls to display more info.",
            format!("{:?} call traces. ", call_traces_count).bold()
        );

        if inner.show_calls != ShowCalls::None {
            for call in call_traces {
                formatter::print_call(call, 0, &inner.show_calls, inner.resolve_hashes);
            }
        }
        tracing::info!("");
        tracing::info!(
            "==== {}",
            format!("{} events", tx_result.logs.events.len()).bold()
        );
        for event in &tx_result.logs.events {
            formatter::print_event(event, inner.resolve_hashes);
        }

        // The computed block hash here will be different than that in production.
        let hash = compute_hash(block_ctx.miniblock, l2_tx.hash());

        let mut transaction = zksync_types::api::Transaction::from(l2_tx);
        let block_hash = inner
            .block_hashes
            .get(&inner.current_miniblock)
            .ok_or(format!(
                "Block hash not found for block: {}",
                inner.current_miniblock
            ))?;
        transaction.block_hash = Some(*block_hash);
        transaction.block_number = Some(U64::from(inner.current_miniblock));

        let block = Block {
            hash,
            number: U64::from(block_ctx.miniblock),
            timestamp: U256::from(batch_env.timestamp),
            l1_batch_number: Some(U64::from(batch_env.number.0)),
            transactions: vec![TransactionVariant::Full(transaction)],
            gas_used: U256::from(tx_result.statistics.gas_used),
            gas_limit: U256::from(BLOCK_GAS_LIMIT),
            ..Default::default()
        };

        tracing::info!("");
        tracing::info!("");

        let bytecodes = vm
            .get_last_tx_compressed_bytecodes()
            .iter()
            .map(|b| bytecode_to_factory_dep(b.original.clone()))
            .collect();

        vm.execute(VmExecutionMode::Bootloader);

        let modified_keys = storage.borrow().modified_storage_keys().clone();
        Ok((
            modified_keys,
            tx_result,
            call_traces.clone(),
            block,
            bytecodes,
            block_ctx,
        ))
    }

    /// Runs L2 transaction and commits it to a new block.
    pub fn run_l2_tx(&self, l2_tx: L2Tx, execution_mode: TxExecutionMode) -> Result<(), String> {
        let tx_hash = l2_tx.hash();
        tracing::info!("");
        tracing::info!("Executing {}", format!("{:?}", tx_hash).bold());

        {
            let mut inner = self
                .inner
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            inner.filters.notify_new_pending_transaction(tx_hash);
        }

        let (keys, result, call_traces, block, bytecodes, block_ctx) =
            self.run_l2_tx_inner(l2_tx.clone(), execution_mode)?;

        if let ExecutionResult::Halt { reason } = result.result {
            // Halt means that something went really bad with the transaction execution (in most cases invalid signature,
            // but it could also be bootloader panic etc).
            // In such case, we should not persist the VM data, and we should pretend that transaction never existed.
            return Err(format!("Transaction HALT: {}", reason));
        }
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

        for (log_idx, event) in result.logs.events.iter().enumerate() {
            inner.filters.notify_new_log(
                &Log {
                    address: event.address,
                    topics: event.indexed_topics.clone(),
                    data: Bytes(event.value.clone()),
                    block_hash: Some(block.hash),
                    block_number: Some(block.number),
                    l1_batch_number: block.l1_batch_number,
                    transaction_hash: Some(tx_hash),
                    transaction_index: Some(U64::zero()),
                    log_index: Some(U256::from(log_idx)),
                    transaction_log_index: Some(U256::from(log_idx)),
                    log_type: None,
                    removed: None,
                },
                block.number,
            );
        }
        let tx_receipt = TransactionReceipt {
            transaction_hash: tx_hash,
            transaction_index: U64::from(0),
            block_hash: Some(block.hash),
            block_number: Some(block.number),
            l1_batch_tx_index: None,
            l1_batch_number: block.l1_batch_number,
            from: l2_tx.initiator_account(),
            to: Some(l2_tx.recipient_account()),
            cumulative_gas_used: Default::default(),
            gas_used: Some(l2_tx.common_data.fee.gas_limit - result.refunds.gas_refunded),
            contract_address: contract_address_from_tx_result(&result),
            logs: result
                .logs
                .events
                .iter()
                .enumerate()
                .map(|(log_idx, log)| Log {
                    address: log.address,
                    topics: log.indexed_topics.clone(),
                    data: Bytes(log.value.clone()),
                    block_hash: Some(block.hash),
                    block_number: Some(block.number),
                    l1_batch_number: block.l1_batch_number,
                    transaction_hash: Some(tx_hash),
                    transaction_index: Some(U64::zero()),
                    log_index: Some(U256::from(log_idx)),
                    transaction_log_index: Some(U256::from(log_idx)),
                    log_type: None,
                    removed: None,
                })
                .collect(),
            l2_to_l1_logs: vec![],
            status: Some(if result.result.is_failed() {
                U64::from(0)
            } else {
                U64::from(1)
            }),
            effective_gas_price: Some(L2_GAS_PRICE.into()),
            ..Default::default()
        };
        let debug = create_debug_output(&l2_tx, &result, call_traces).expect("create debug output"); // OK to unwrap here as Halt is handled above
        inner.tx_results.insert(
            tx_hash,
            TransactionResult {
                info: TxExecutionInfo {
                    tx: l2_tx,
                    batch_number: block.l1_batch_number.unwrap_or_default().as_u32(),
                    miniblock_number: block.number.as_u64(),
                    result,
                },
                receipt: tx_receipt,
                debug,
            },
        );

        // With the introduction of 'l2 blocks' (and virtual blocks),
        // we are adding one l2 block at the end of each batch (to handle things like remaining events etc).
        //  You can look at insert_fictive_l2_block function in VM to see how this fake block is inserted.
        let block_ctx = block_ctx.new_block();
        let empty_block_at_end_of_batch =
            create_empty_block(block_ctx.miniblock, block_ctx.timestamp, block_ctx.batch);

        inner.current_batch = inner.current_batch.saturating_add(1);

        for block in vec![block, empty_block_at_end_of_batch] {
            // archive current state before we produce new batch/blocks
            if let Err(err) = inner.archive_state() {
                tracing::error!(
                    "failed archiving state for block {}: {}",
                    inner.current_miniblock,
                    err
                );
            }

            inner.current_miniblock = inner.current_miniblock.saturating_add(1);
            inner.current_timestamp = inner.current_timestamp.saturating_add(1);

            let actual_l1_batch_number = block
                .l1_batch_number
                .expect("block must have a l1_batch_number");
            if actual_l1_batch_number.as_u32() != inner.current_batch {
                panic!(
                    "expected next block to have batch_number {}, got {}",
                    inner.current_batch,
                    actual_l1_batch_number.as_u32()
                );
            }

            if block.number.as_u64() != inner.current_miniblock {
                panic!(
                    "expected next block to have miniblock {}, got {}",
                    inner.current_miniblock,
                    block.number.as_u64()
                );
            }

            if block.timestamp.as_u64() != inner.current_timestamp {
                panic!(
                    "expected next block to have timestamp {}, got {}",
                    inner.current_timestamp,
                    block.timestamp.as_u64()
                );
            }

            let block_hash = block.hash;
            inner.current_miniblock_hash = block_hash;
            inner.block_hashes.insert(block.number.as_u64(), block.hash);
            inner.blocks.insert(block.hash, block);
            inner.filters.notify_new_block(block_hash);
        }

        Ok(())
    }

    // Validates L2 transaction
    pub fn validate_tx(&self, tx: &L2Tx) -> Result<(), String> {
        let max_gas = U256::from(u32::MAX);
        if tx.common_data.fee.gas_limit > max_gas
            || tx.common_data.fee.gas_per_pubdata_limit > max_gas
        {
            return Err("exceeds block gas limit".into());
        }

        if tx.common_data.fee.max_fee_per_gas < L2_GAS_PRICE.into() {
            tracing::info!(
                "Submitted Tx is Unexecutable {:?} because of MaxFeePerGasTooLow {}",
                tx.hash(),
                tx.common_data.fee.max_fee_per_gas
            );
            return Err("block base fee higher than max fee per gas".into());
        }

        if tx.common_data.fee.max_fee_per_gas < tx.common_data.fee.max_priority_fee_per_gas {
            tracing::info!(
                "Submitted Tx is Unexecutable {:?} because of MaxPriorityFeeGreaterThanMaxFee {}",
                tx.hash(),
                tx.common_data.fee.max_fee_per_gas
            );
            return Err("max priority fee per gas higher than max fee per gas".into());
        }
        Ok(())
    }
}

/// Keeps track of a block's batch number, miniblock number and timestamp.
/// Useful for keeping track of the current context when creating multiple blocks.
pub struct BlockContext {
    pub batch: u32,
    pub miniblock: u64,
    pub timestamp: u64,
}

impl BlockContext {
    /// Create the current instance that represents the latest block.
    pub fn from_current(batch: u32, miniblock: u64, timestamp: u64) -> Self {
        Self {
            batch,
            miniblock,
            timestamp,
        }
    }

    /// Create the next batch instance that has all parameters incremented by `1`.
    pub fn new_batch(&self) -> Self {
        Self {
            batch: self.batch.saturating_add(1),
            miniblock: self.miniblock.saturating_add(1),
            timestamp: self.timestamp.saturating_add(1),
        }
    }

    /// Create the next batch instance that uses the same batch number, and has all other parameters incremented by `1`.
    pub fn new_block(&self) -> BlockContext {
        Self {
            batch: self.batch,
            miniblock: self.miniblock.saturating_add(1),
            timestamp: self.timestamp.saturating_add(1),
        }
    }
}
