//! Interfaces that use zkos for VM execution.
//! This is still experimental code.
use std::{alloc::Global, collections::HashMap, vec};

use basic_system::basic_system::simple_growable_storage::TestingTree;
use forward_system::run::{
    test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource},
    PreimageType, StorageCommitment,
};
use ruint::aliases::B160;
use system_hooks::addresses_constants::{
    NOMINAL_TOKEN_BALANCE_STORAGE_ADDRESS, NONCE_HOLDER_HOOK_ADDRESS,
};
use zk_ee::{common_structs::derive_flat_storage_key, utils::Bytes32};
use zksync_multivm::{
    interface::{
        storage::{StoragePtr, WriteStorage},
        ExecutionResult, InspectExecutionMode, L1BatchEnv, PushTransactionResult, Refunds,
        SystemEnv, TxExecutionMode, VmExecutionLogs, VmExecutionResultAndLogs, VmInterface,
        VmInterfaceHistoryEnabled, VmRevertReason,
    },
    vm_latest::TracerPointer,
    HistoryMode,
};
use zksync_types::{
    address_to_h256, web3::keccak256, AccountTreeId, Address, ExecuteTransactionCommon, StorageKey,
    StorageLog, StorageLogWithPreviousValue, Transaction, H160, H256, U256,
};

use crate::deps::InMemoryStorage;

// Helper methods for different convertions.
pub fn bytes32_to_h256(data: Bytes32) -> H256 {
    H256::from(data.as_u8_array_ref())
}

pub fn h256_to_bytes32(data: &H256) -> Bytes32 {
    Bytes32::from(data.as_fixed_bytes().clone())
}

pub fn b160_to_h160(data: B160) -> H160 {
    H160::from_slice(&data.to_be_bytes_vec())
}

pub fn pad_to_word(input: &Vec<u8>) -> Vec<u8> {
    let mut data = input.clone();
    let remainder = input.len().div_ceil(32) * 32 - input.len();
    for _ in 0..remainder {
        data.push(0u8);
    }
    data
}

pub fn h160_to_b160(data: &H160) -> B160 {
    B160::from_be_bytes(data.as_fixed_bytes().clone())
}

// Helper methods to add data to the Vec<u8> in the format expected by ZKOS.
pub fn append_address(data: &mut Vec<u8>, address: &H160) {
    let mut pp = vec![0u8; 32];
    let ap1 = address.as_fixed_bytes();
    for i in 0..20 {
        pp[i + 12] = ap1[i];
    }
    data.append(&mut pp);
}

pub fn append_u256(data: &mut Vec<u8>, payload: &zksync_types::U256) {
    let mut pp = [0u8; 32];
    payload.to_big_endian(&mut pp);

    data.append(&mut pp.to_vec());
}
pub fn append_u64(data: &mut Vec<u8>, payload: u64) {
    let mut pp = [0u8; 32];
    let pp1 = payload.to_be_bytes();
    for i in 0..8 {
        pp[24 + i] = pp1[i];
    }
    data.append(&mut pp.to_vec());
}

pub fn append_usize(data: &mut Vec<u8>, payload: usize) {
    let mut pp = [0u8; 32];
    let pp1 = payload.to_be_bytes();
    for i in 0..8 {
        pp[24 + i] = pp1[i];
    }
    data.append(&mut pp.to_vec());
}

/// Iterates over raw storage and creates a tree from it.
pub fn create_tree_from_full_state(
    raw_storage: &InMemoryStorage,
) -> (InMemoryTree, InMemoryPreimageSource) {
    let original_state = &raw_storage.state;
    let mut tree = InMemoryTree {
        storage_tree: TestingTree::new_in(Global),
        cold_storage: HashMap::new(),
    };
    let mut preimage_source = InMemoryPreimageSource {
        inner: HashMap::new(),
    };

    for entry in original_state {
        let kk = derive_flat_storage_key(
            &h160_to_b160(entry.0.address()),
            &h256_to_bytes32(entry.0.key()),
        );
        let vv = h256_to_bytes32(entry.1);

        tree.storage_tree.insert(&kk, &vv);
        tree.cold_storage.insert(kk, vv);
    }

    for entry in &raw_storage.factory_deps {
        preimage_source.inner.insert(
            (
                PreimageType::Bytecode(zk_ee::system::ExecutionEnvironmentType::EVM),
                h256_to_bytes32(entry.0),
            ),
            entry.1.clone(),
        );
    }
    (tree, preimage_source)
}

pub fn add_elem_to_tree(tree: &mut InMemoryTree, k: &StorageKey, v: &H256) {
    let kk = derive_flat_storage_key(&h160_to_b160(k.address()), &h256_to_bytes32(k.key()));
    let vv = h256_to_bytes32(v);

    tree.storage_tree.insert(&kk, &vv);
    tree.cold_storage.insert(kk, vv);
}

// Serialize Transaction to ZKOS format.
// Should match the code in basic_bootloader/src/bootloader/transaction/mod.rs
pub fn transaction_to_zkos_vec(tx: &Transaction) -> Vec<u8> {
    let mut tx_raw: Vec<u8> = vec![];
    let tx_type_id = match tx.tx_format() {
        zksync_types::l2::TransactionType::LegacyTransaction => 0u8,
        zksync_types::l2::TransactionType::EIP2930Transaction => 1u8,
        zksync_types::l2::TransactionType::EIP1559Transaction => 2u8,
        zksync_types::l2::TransactionType::EIP712Transaction => todo!(),
        zksync_types::l2::TransactionType::PriorityOpTransaction => todo!(),
        zksync_types::l2::TransactionType::ProtocolUpgradeTransaction => todo!(),
    };
    let common_data = match &tx.common_data {
        zksync_types::ExecuteTransactionCommon::L1(_) => todo!(),
        zksync_types::ExecuteTransactionCommon::L2(l2_tx_common_data) => l2_tx_common_data,
        zksync_types::ExecuteTransactionCommon::ProtocolUpgrade(_) => todo!(),
    };
    // tx_type
    tx_raw.append(&mut vec![0u8; 31]);
    tx_raw.append(&mut vec![tx_type_id; 1]);

    // from
    append_address(&mut tx_raw, &common_data.initiator_address);
    // to
    append_address(
        &mut tx_raw,
        &tx.execute.contract_address.unwrap_or(H160::zero()),
    );

    let gas_limit = common_data.fee.gas_limit;
    // gas limit
    append_u256(&mut tx_raw, &gas_limit);
    // gas per pubdata limit
    append_u256(&mut tx_raw, &common_data.fee.gas_per_pubdata_limit);

    let fee_per_gas = common_data.fee.max_fee_per_gas;

    // max fee per gas
    append_u256(&mut tx_raw, &fee_per_gas);
    // max priority fee per gas.
    // hack for legacy tx (verify!!)
    append_u256(&mut tx_raw, &common_data.fee.max_priority_fee_per_gas);

    // paymaster
    append_u64(&mut tx_raw, 0);

    // nonce
    append_u64(&mut tx_raw, common_data.nonce.0.into());

    append_u256(&mut tx_raw, &tx.execute.value);

    let mut reserved = [0u64; 4];

    // Should check chain_id
    if tx_type_id == 0 {
        reserved[0] = 1;
    }

    if tx.execute.contract_address.is_none() {
        reserved[1] = 1;
    }

    for i in 0..4 {
        // reserved
        append_u64(&mut tx_raw, reserved[i]);
    }

    let signature_u256 = common_data.signature.len().div_ceil(32) as u64;

    let execute_calldata_words = tx.execute.calldata.len().div_ceil(32) as u64;

    let mut current_offset = 19;

    // data offset
    append_u64(&mut tx_raw, current_offset * 32);
    // lent
    current_offset += 1 + execute_calldata_words;
    // signature offset (stupid -- this doesn't include the padding!!)
    append_u64(&mut tx_raw, current_offset * 32);
    current_offset += 1 + signature_u256;

    // factory deps
    append_u64(&mut tx_raw, current_offset * 32);
    current_offset += 1;
    // paymater
    append_u64(&mut tx_raw, current_offset * 32);
    current_offset += 1;
    // reserved
    append_u64(&mut tx_raw, current_offset * 32);

    // len - data.
    append_usize(&mut tx_raw, tx.execute.calldata.len());
    tx_raw.append(&mut pad_to_word(&tx.execute.calldata));

    // len - signature.
    append_usize(&mut tx_raw, common_data.signature.len());
    tx_raw.append(&mut pad_to_word(&common_data.signature));

    // factory deps
    append_u64(&mut tx_raw, 0);
    // paymater
    append_u64(&mut tx_raw, 0);
    // reserved
    append_u64(&mut tx_raw, 0);
    tx_raw
}

pub fn execute_tx_in_zkos<W: WriteStorage>(
    tx: &Transaction,
    tree: &InMemoryTree,
    preimage_source: &InMemoryPreimageSource,
    storage: &mut StoragePtr<W>,
    simulate_only: bool,
    batch_env: &L1BatchEnv,
    chain_id: u64,
) -> VmExecutionResultAndLogs {
    let batch_context = basic_system::basic_system::BasicBlockMetadataFromOracle {
        // TODO: get fee from batch_env.
        eip1559_basefee: ruint::aliases::U256::from(if simulate_only { 0u64 } else { 1000u64 }),
        ergs_price: ruint::aliases::U256::from(1u64),
        block_number: batch_env.number.0 as u64,
        timestamp: batch_env.timestamp,
        gas_per_pubdata: ruint::aliases::U256::from(1u64),
        chain_id,
    };

    let storage_commitment = StorageCommitment {
        root: *tree.storage_tree.root(),
        next_free_slot: tree.storage_tree.next_free_slot,
    };

    let mut tx = tx.clone();
    if simulate_only {
        // Currently zkos doesn't do validation when running a simulated transaction.
        // This results in lower gas estimation - which might cause issues for the user.
        const ZKOS_EXPECTED_VALIDATION_COST: u64 = 6_000;
        let new_gas_limit = tx
            .gas_limit()
            .saturating_sub(U256::from(ZKOS_EXPECTED_VALIDATION_COST));
        match &mut tx.common_data {
            ExecuteTransactionCommon::L1(data) => data.gas_limit = new_gas_limit,
            ExecuteTransactionCommon::L2(data) => data.fee.gas_limit = new_gas_limit,
            ExecuteTransactionCommon::ProtocolUpgrade(data) => data.gas_limit = new_gas_limit,
        };
    }

    let tx_raw = transaction_to_zkos_vec(&tx);

    let (output, dynamic_factory_deps, storage_logs) = if simulate_only {
        (
            forward_system::run::simulate_tx(
                tx_raw,
                storage_commitment,
                batch_context,
                tree.clone(),
                preimage_source.clone(),
            )
            .unwrap(),
            Default::default(), // dynamic factory deps
            vec![],             // storage logs
        )
    } else {
        let tx_source = TxListSource {
            transactions: vec![tx_raw].into(),
        };
        let batch_output = forward_system::run::run_batch(
            batch_context,
            storage_commitment,
            // FIXME
            tree.clone(),
            preimage_source.clone(),
            tx_source,
        )
        .unwrap();

        let mut storage_ptr = storage.borrow_mut();

        let mut storage_logs = vec![];

        // apply storage writes..
        for write in batch_output.storage_writes {
            let storage_key = StorageKey::new(
                AccountTreeId::new(Address::from_slice(&write.account.to_be_bytes_vec())),
                H256::from(write.account_key.as_u8_array_ref()),
            );
            let storage_value = H256::from(write.value.as_u8_array_ref());
            let prev_value = storage_ptr.set_value(storage_key, storage_value);

            let storage_log = StorageLog {
                // FIXME - should distinguish between initial write and repeated write.
                kind: zksync_types::StorageLogKind::InitialWrite,
                key: storage_key,
                value: storage_value,
            };
            storage_logs.push(StorageLogWithPreviousValue {
                log: storage_log,
                previous_value: prev_value,
            });
        }
        let mut f_deps = HashMap::new();

        for factory_dep in batch_output.published_preimages {
            f_deps.insert(bytes32_to_h256(factory_dep.0), factory_dep.1);
        }

        (batch_output.tx_results[0].clone(), f_deps, storage_logs)
    };

    let (tx_output, gas_refunded) = match output.as_ref() {
        Ok(tx_output) => match &tx_output.execution_result {
            forward_system::run::ExecutionResult::Success(output) => match &output {
                forward_system::run::ExecutionOutput::Call(data) => (data, tx_output.gas_refunded),
                forward_system::run::ExecutionOutput::Create(data, _) => {
                    (data, tx_output.gas_refunded)
                }
            },
            forward_system::run::ExecutionResult::Revert(data) => {
                return VmExecutionResultAndLogs {
                    result: ExecutionResult::Revert {
                        output: VmRevertReason::General {
                            msg: "Transaction reverted".to_string(),
                            data: data.clone(),
                        },
                    },
                    logs: Default::default(),
                    statistics: Default::default(),
                    refunds: Refunds {
                        gas_refunded: tx_output.gas_refunded,
                        operator_suggested_refund: tx_output.gas_refunded,
                    },
                    dynamic_factory_deps: Default::default(),
                }
            }
        },
        Err(invalid_tx) => {
            return VmExecutionResultAndLogs {
                result: ExecutionResult::Revert {
                    output: VmRevertReason::General {
                        msg: format!("{:?}", invalid_tx),
                        data: vec![],
                    },
                },
                logs: Default::default(),
                statistics: Default::default(),
                refunds: Default::default(),
                dynamic_factory_deps: Default::default(),
            }
        }
    };

    VmExecutionResultAndLogs {
        result: ExecutionResult::Success {
            output: tx_output.clone(),
        },
        logs: VmExecutionLogs {
            storage_logs,
            events: Default::default(),
            user_l2_to_l1_logs: Default::default(),
            system_l2_to_l1_logs: Default::default(),
            total_log_queries_count: Default::default(),
        },
        statistics: Default::default(),
        refunds: Refunds {
            gas_refunded,
            operator_suggested_refund: gas_refunded,
        },
        dynamic_factory_deps,
    }
}

pub fn zkos_get_nonce_key(account: &Address) -> StorageKey {
    let nonce_manager = AccountTreeId::new(b160_to_h160(NONCE_HOLDER_HOOK_ADDRESS));

    // The `minNonce` (used as nonce for EOAs) is stored in a mapping inside the `NONCE_HOLDER` system contract
    let key = address_to_h256(account);

    StorageKey::new(nonce_manager, key)
}

pub fn zkos_key_for_eth_balance(address: &Address) -> H256 {
    address_to_h256(address)
}

/// Create a `key` part of `StorageKey` to access the balance from ERC20 contract balances
fn zkos_key_for_erc20_balance(address: &Address) -> H256 {
    let address_h256 = address_to_h256(address);

    // 20 bytes address first gets aligned to 32 bytes with index of `balanceOf` storage slot
    // of default ERC20 contract and to then to 64 bytes.
    let slot_index = H256::from_low_u64_be(51);
    let mut bytes = [0_u8; 64];
    bytes[..32].copy_from_slice(address_h256.as_bytes());
    bytes[32..].copy_from_slice(slot_index.as_bytes());
    H256(keccak256(&bytes))
}

pub fn zkos_storage_key_for_standard_token_balance(
    token_contract: AccountTreeId,
    address: &Address,
) -> StorageKey {
    // We have different implementation of the standard ERC20 contract and native
    // eth contract. The key for the balance is different for each.
    let key = if token_contract.address() == &b160_to_h160(NOMINAL_TOKEN_BALANCE_STORAGE_ADDRESS) {
        zkos_key_for_eth_balance(address)
    } else {
        zkos_key_for_erc20_balance(address)
    };

    StorageKey::new(token_contract, key)
}

pub fn zkos_storage_key_for_eth_balance(address: &Address) -> StorageKey {
    zkos_storage_key_for_standard_token_balance(
        AccountTreeId::new(b160_to_h160(NOMINAL_TOKEN_BALANCE_STORAGE_ADDRESS)),
        address,
    )
}

pub struct ZKOsVM<S: WriteStorage, H: HistoryMode> {
    pub storage: StoragePtr<S>,
    pub tree: InMemoryTree,
    preimage: InMemoryPreimageSource,
    transactions: Vec<Transaction>,
    system_env: SystemEnv,
    batch_env: L1BatchEnv,
    _phantom: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> ZKOsVM<S, H> {
    pub fn new(
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
        storage: StoragePtr<S>,
        raw_storage: &InMemoryStorage,
    ) -> Self {
        let (tree, preimage) = { create_tree_from_full_state(raw_storage) };
        ZKOsVM {
            storage,
            tree,
            preimage,
            transactions: vec![],
            system_env,
            batch_env,
            _phantom: Default::default(),
        }
    }

    /// If any keys are updated in storage externally, but not reflected in internal tree.
    pub fn update_inconsistent_keys(&mut self, inconsistent_nodes: &[&StorageKey]) {
        for key in inconsistent_nodes {
            let value = self.storage.borrow_mut().read_value(key);
            add_elem_to_tree(&mut self.tree, key, &value);
        }
    }
}

pub struct ZkOsTracerDispatcher<S: WriteStorage, H: HistoryMode> {
    _tracers: Vec<S>,
    _marker: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> Default for ZkOsTracerDispatcher<S, H> {
    fn default() -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> From<Vec<TracerPointer<S, H>>>
    for ZkOsTracerDispatcher<S, H>
{
    fn from(_value: Vec<TracerPointer<S, H>>) -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmInterface for ZKOsVM<S, H> {
    type TracerDispatcher = ZkOsTracerDispatcher<S, H>;

    fn push_transaction(
        &mut self,
        tx: Transaction,
    ) -> zksync_multivm::interface::PushTransactionResult<'_> {
        self.transactions.push(tx);
        PushTransactionResult {
            compressed_bytecodes: Default::default(),
        }
    }

    fn inspect(
        &mut self,
        _dispatcher: &mut Self::TracerDispatcher,
        execution_mode: zksync_multivm::interface::InspectExecutionMode,
    ) -> VmExecutionResultAndLogs {
        if let InspectExecutionMode::Bootloader = execution_mode {
            return VmExecutionResultAndLogs {
                result: ExecutionResult::Success { output: vec![] },
                logs: Default::default(),
                statistics: Default::default(),
                refunds: Default::default(),
                dynamic_factory_deps: Default::default(),
            };
        }
        let simulate_only = match self.system_env.execution_mode {
            TxExecutionMode::VerifyExecute => false,
            TxExecutionMode::EstimateFee => true,
            TxExecutionMode::EthCall => true,
        };

        // For now we only support one transaction.
        assert_eq!(
            1,
            self.transactions.len(),
            "only one tx per batch supported for now"
        );

        // TODO: add support for multiple transactions.
        let tx = self.transactions[0].clone();
        execute_tx_in_zkos(
            &tx,
            &self.tree,
            &self.preimage,
            &mut self.storage,
            simulate_only,
            &self.batch_env,
            self.system_env.chain_id.as_u64(),
        )
    }

    fn start_new_l2_block(&mut self, _l2_block_env: zksync_multivm::interface::L2BlockEnv) {
        todo!()
    }

    fn inspect_transaction_with_bytecode_compression(
        &mut self,
        _tracer: &mut Self::TracerDispatcher,
        _tx: Transaction,
        _with_compression: bool,
    ) -> (
        zksync_multivm::interface::BytecodeCompressionResult<'_>,
        VmExecutionResultAndLogs,
    ) {
        todo!()
    }

    fn finish_batch(
        &mut self,
        _pubdata_builder: std::rc::Rc<dyn zksync_multivm::interface::pubdata::PubdataBuilder>,
    ) -> zksync_multivm::interface::FinishedL1Batch {
        todo!()
    }
}

impl<S: WriteStorage, H: HistoryMode> VmInterfaceHistoryEnabled for ZKOsVM<S, H> {
    fn make_snapshot(&mut self) {}

    fn rollback_to_the_latest_snapshot(&mut self) {
        panic!("Not implemented for zkos");
    }

    fn pop_snapshot_no_rollback(&mut self) {}
}
