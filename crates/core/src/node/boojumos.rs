//! Interfaces that use boojumos for VM execution.
//! This is still experimental code.
use std::{alloc::Global, collections::HashMap, vec};

use anvil_zksync_config::types::BoojumConfig;
use rig::utils::evm_bytecode_into_account_properties;
use ruint::aliases::B160;
use zk_ee::{common_structs::derive_flat_storage_key, utils::Bytes32};
use zk_os_basic_system::system_implementation::flat_storage_model::{
    address_into_special_storage_key, AccountProperties, TestingTree,
    ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use zk_os_forward_system::run::{
    test_impl::{InMemoryPreimageSource, InMemoryTree, NoopTxCallback, TxListSource},
    StorageCommitment,
};
use zk_os_system_hooks::addresses_constants::{
    ACCOUNT_CODE_STORAGE_STORAGE_ADDRESS, BASE_TOKEN_ADDRESS,
};
use zksync_multivm::{
    interface::{
        storage::{StoragePtr, WriteStorage},
        ExecutionResult, InspectExecutionMode, L1BatchEnv, PushTransactionResult, Refunds,
        SystemEnv, TxExecutionMode, VmExecutionLogs, VmExecutionResultAndLogs, VmInterface,
        VmInterfaceHistoryEnabled, VmRevertReason,
    },
    tracers::TracerDispatcher,
    vm_latest::TracerPointer,
    HistoryMode,
};

use zksync_multivm::MultiVmTracerPointer;
use zksync_types::{
    address_to_h256, get_code_key, u256_to_h256, web3::keccak256, AccountTreeId, Address,
    ExecuteTransactionCommon, StorageKey, StorageLog, StorageLogWithPreviousValue, Transaction,
    H160, H256, U256,
};

use crate::deps::InMemoryStorage;
use once_cell::sync::Lazy;
use std::sync::Mutex;

pub const BOOJUM_CALL_GAS_LIMIT: u64 = 100_000_000;

static BATCH_WITNESS: Lazy<Mutex<HashMap<u32, Vec<u8>>>> = Lazy::new(|| {
    let m = HashMap::new();
    Mutex::new(m)
});

// TODO: we're using some unused spot.
// As nonces are normally kept inside account properties, and here we only 'simulate' putting them in some
// location, so that the rest of the anvil can work.
// But if we tried reading stuff from 0x8003, where nonces are normally located - we might be reading some garbage.
pub const FAKE_NONCE_ADDRESS: B160 = B160::from_limbs([0x8153_u64, 0, 0]);

pub fn set_batch_witness(key: u32, value: Vec<u8>) {
    let mut map = BATCH_WITNESS.lock().unwrap();
    map.insert(key, value);
}

pub fn boojumos_get_batch_witness(key: &u32) -> Option<Vec<u8>> {
    let map = BATCH_WITNESS.lock().unwrap();
    map.get(key).cloned()
}

// Helper methods for different convertions.
pub fn bytes32_to_h256(data: Bytes32) -> H256 {
    H256::from(data.as_u8_array_ref())
}

pub fn h256_to_bytes32(data: &H256) -> Bytes32 {
    Bytes32::from(*data.as_fixed_bytes())
}

pub fn b160_to_h160(data: B160) -> H160 {
    H160::from_slice(&data.to_be_bytes_vec())
}

pub fn pad_to_word(input: &[u8]) -> Vec<u8> {
    let mut data = input.to_owned();
    let remainder = input.len().div_ceil(32) * 32 - input.len();
    data.extend(std::iter::repeat_n(0u8, remainder));
    data
}

pub fn h160_to_b160(data: &H160) -> B160 {
    B160::from_be_bytes(*data.as_fixed_bytes())
}

// Helper methods to add data to the Vec<u8> in the format expected by ZKOS.
pub fn append_address(data: &mut Vec<u8>, address: &H160) {
    let mut pp = vec![0u8; 32];
    let ap1 = address.as_fixed_bytes();
    pp[12..(20 + 12)].copy_from_slice(ap1);
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
    pp[24..(8 + 24)].copy_from_slice(&pp1);
    data.append(&mut pp.to_vec());
}

pub fn append_usize(data: &mut Vec<u8>, payload: usize) {
    let mut pp = [0u8; 32];
    let pp1 = payload.to_be_bytes();
    pp[24..(8 + 24)].copy_from_slice(&pp1);
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
    for entry in &raw_storage.factory_deps {
        preimage_source
            .inner
            .insert(h256_to_bytes32(entry.0), entry.1.clone());
    }

    for entry in original_state {
        let kk = derive_flat_storage_key(
            &h160_to_b160(entry.0.address()),
            &h256_to_bytes32(entry.0.key()),
        );
        let vv = h256_to_bytes32(entry.1);

        let aa = h256_to_bytes32(entry.0.key());
        let cc = B160::try_from_be_slice(&aa.as_u8_array()[12..]).unwrap();

        if entry.0.address() == &b160_to_h160(FAKE_NONCE_ADDRESS) {
            set_account_properties(
                &mut tree,
                &mut preimage_source,
                cc,
                None,
                Some(entry.1.to_low_u64_be()),
                None,
            );
        }

        if entry.0.address() == &b160_to_h160(BASE_TOKEN_ADDRESS) {
            set_account_properties(
                &mut tree,
                &mut preimage_source,
                cc,
                Some(ruint::aliases::U256::from_be_slice(entry.1.as_bytes())),
                None,
                None,
            );
        }

        if entry.0.address() == &b160_to_h160(ACCOUNT_CODE_STORAGE_STORAGE_ADDRESS) {
            println!("Setting bytecode for {:?}", entry.0.key());
            if entry.0.key().is_zero() {
                continue;
            }
            if entry.1.is_zero() {
                continue;
            }

            let bytecode_hash = entry.1;
            let bytecode = raw_storage
                .factory_deps
                .get(bytecode_hash)
                .unwrap_or_else(|| panic!("Cannot find bytecode for {:?}", bytecode_hash));
            set_account_properties(
                &mut tree,
                &mut preimage_source,
                cc,
                None,
                None,
                Some(bytecode.clone()),
            );
        }

        tree.storage_tree.insert(&kk, &vv);
        tree.cold_storage.insert(kk, vv);
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
    // TODO: hack for legacy tx (verify!!)
    append_u256(&mut tx_raw, &common_data.fee.max_priority_fee_per_gas);

    // paymaster
    append_u64(&mut tx_raw, 0);

    // nonce
    append_u64(&mut tx_raw, common_data.nonce.0.into());

    append_u256(&mut tx_raw, &tx.execute.value);

    let mut reserved = [0u64; 4];

    // TODO: should check chain_id
    if tx_type_id == 0 {
        reserved[0] = 1;
    }

    if tx.execute.contract_address.is_none() {
        reserved[1] = 1;
    }

    for &value in reserved.iter().take(4) {
        append_u64(&mut tx_raw, value);
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
    // paymaster
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
    // paymaster
    append_u64(&mut tx_raw, 0);
    // reserved
    append_u64(&mut tx_raw, 0);
    tx_raw
}

#[allow(clippy::too_many_arguments)]
pub fn execute_tx_in_zkos<W: WriteStorage>(
    tx: &Transaction,
    tree: &InMemoryTree,
    preimage_source: &InMemoryPreimageSource,
    storage: &mut StoragePtr<W>,
    simulate_only: bool,
    batch_env: &L1BatchEnv,
    chain_id: u64,
    // if zkos_path is passed, it will also compute witness.
    zkos_path: Option<String>,
) -> (VmExecutionResultAndLogs, Option<Vec<u8>>) {
    let batch_context = zk_ee::system::metadata::BlockMetadataFromOracle {
        // TODO: get fee from batch_env.
        eip1559_basefee: ruint::aliases::U256::from(if simulate_only { 0u64 } else { 1000u64 }),
        block_number: batch_env.number.0 as u64,
        timestamp: batch_env.timestamp,
        gas_per_pubdata: ruint::aliases::U256::from(1u64),
        chain_id,
        // TODO: set proper values
        block_hashes: Default::default(),
        coinbase: B160::ZERO,
        // TODO: investigate
        native_price: ruint::aliases::U256::from(10u64),
        gas_limit: 100_000_000,
    };

    let storage_commitment = StorageCommitment {
        root: *tree.storage_tree.root(),
        next_free_slot: tree.storage_tree.next_free_slot,
    };

    let mut tx = tx.clone();
    if simulate_only {
        // Currently zkos doesn't do validation when running a simulated transaction.
        // This results in lower gas estimation - which might cause issues for the user.
        // So here, we're manually adjusting the gas limit to account for the expected validation cost.
        const ZKOS_EXPECTED_VALIDATION_COST: u64 = 6_000;
        let new_gas_limit = tx
            .gas_limit()
            .saturating_sub(U256::from(ZKOS_EXPECTED_VALIDATION_COST));
        match &mut tx.common_data {
            ExecuteTransactionCommon::L1(data) => data.gas_limit = new_gas_limit,
            ExecuteTransactionCommon::L2(data) => data.fee.gas_limit = new_gas_limit,
            ExecuteTransactionCommon::ProtocolUpgrade(data) => data.gas_limit = new_gas_limit,
        };

        // FIXME: currently zkos requires gas price to be >0 (otherwise it fails with out of resources during validation)
        if tx.max_fee_per_gas() == 0.into() {
            let new_gas_price = 100.into();
            if let ExecuteTransactionCommon::L2(data) = &mut tx.common_data {
                data.fee.max_fee_per_gas = new_gas_price;
                data.fee.max_priority_fee_per_gas = new_gas_price;
            };
        }
    }

    let tx_raw = transaction_to_zkos_vec(&tx);

    let mut witness = None;

    let (output, dynamic_factory_deps, storage_logs) = if simulate_only {
        (
            zk_os_forward_system::run::simulate_tx(
                tx_raw,
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
        let noop = NoopTxCallback {};
        let batch_output = zk_os_forward_system::run::run_batch(
            batch_context,
            // TODO: FIXME
            tree.clone(),
            preimage_source.clone(),
            tx_source.clone(),
            noop,
        )
        .unwrap();

        if let Some(zkos_path) = zkos_path {
            let result = zksync_os_api::run_batch_generate_witness(
                batch_context,
                tree.clone(),
                preimage_source.clone(),
                tx_source,
                storage_commitment,
                &zkos_path,
            );

            witness = Some(result.iter().flat_map(|x| x.to_be_bytes()).collect());
        }

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
                // TODO: FIXME - should distinguish between initial write and repeated write.
                kind: zksync_types::StorageLogKind::InitialWrite,
                key: storage_key,
                value: storage_value,
            };
            storage_logs.push(StorageLogWithPreviousValue {
                log: storage_log,
                previous_value: prev_value,
            });
            if write.account == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
                // also update a balance & nonce.

                // find preimage in batch output.published_preimages
                let dst_address = &write.account_key.as_u8_ref()[12..];
                let dst_address = zksync_types::Address::from_slice(dst_address);
                let mut found = false;

                for preimage in batch_output.published_preimages.iter() {
                    if preimage.0 == write.value {
                        found = true;
                        let account_properties =
                            AccountProperties::decode(&preimage.1.clone().try_into().unwrap());
                        let balance = account_properties.balance;
                        let nonce = account_properties.nonce;
                        let bytecode_hash = account_properties.bytecode_hash;

                        println!(
                            "For account :{:?} nonce is {:?} balance is {:?}, bytecode_hash is {:?} preimage hash is {:?}",
                            dst_address, nonce, balance, bytecode_hash, preimage.0
                        );

                        for (key, value) in [
                            (
                                boojumos_get_nonce_key(&dst_address),
                                u256_to_h256(U256::from(nonce)),
                            ),
                            (
                                zkos_storage_key_for_standard_token_balance(
                                    AccountTreeId::new(b160_to_h160(BASE_TOKEN_ADDRESS)),
                                    &dst_address,
                                ),
                                H256::from_slice(&balance.to_be_bytes_vec()),
                            ),
                            (
                                get_code_key(&dst_address),
                                H256::from(bytecode_hash.as_u8_array_ref()),
                            ),
                        ] {
                            let storage_log = StorageLog {
                                // TODO: FIXME - should distinguish between initial write and repeated write.
                                kind: zksync_types::StorageLogKind::InitialWrite,
                                key,
                                value,
                            };
                            // Create storage write, only if actually something has changed.
                            let prev_value = storage_ptr.read_value(&key);
                            if prev_value != value {
                                let prev_value = storage_ptr.set_value(key, value);

                                storage_logs.push(StorageLogWithPreviousValue {
                                    log: storage_log,
                                    previous_value: prev_value,
                                })
                            };
                        }
                    }
                }
                if !found {
                    panic!("Failed to find preimage for account_properties");
                }
            }
        }
        let mut f_deps = HashMap::new();

        for factory_dep in batch_output.published_preimages {
            f_deps.insert(bytes32_to_h256(factory_dep.0), factory_dep.1);
        }

        (batch_output.tx_results[0].clone(), f_deps, storage_logs)
    };

    let (tx_output, gas_refunded) = match output.as_ref() {
        Ok(tx_output) => match &tx_output.execution_result {
            zk_os_forward_system::run::ExecutionResult::Success(output) => match &output {
                zk_os_forward_system::run::ExecutionOutput::Call(data) => {
                    (data, tx_output.gas_refunded)
                }
                zk_os_forward_system::run::ExecutionOutput::Create(data, _) => {
                    (data, tx_output.gas_refunded)
                }
            },
            zk_os_forward_system::run::ExecutionResult::Revert(data) => {
                return (
                    VmExecutionResultAndLogs {
                        result: ExecutionResult::Revert {
                            output: VmRevertReason::General {
                                msg: "Transaction reverted".to_string(),
                                data: data.clone(),
                            },
                        },
                        logs: VmExecutionLogs {
                            // TODO: check if we should return storage_logs on revert.
                            storage_logs,
                            events: Default::default(),
                            user_l2_to_l1_logs: Default::default(),
                            system_l2_to_l1_logs: Default::default(),
                            total_log_queries_count: Default::default(),
                        },
                        statistics: Default::default(),
                        refunds: Refunds {
                            gas_refunded: tx_output.gas_refunded,
                            operator_suggested_refund: tx_output.gas_refunded,
                        },
                        dynamic_factory_deps,
                    },
                    witness,
                );
            }
        },
        Err(invalid_tx) => {
            return (
                VmExecutionResultAndLogs {
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
                },
                witness,
            )
        }
    };

    (
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
        },
        witness,
    )
}

pub fn boojumos_get_nonce_key(account: &Address) -> StorageKey {
    let nonce_manager = AccountTreeId::new(b160_to_h160(FAKE_NONCE_ADDRESS));

    // The `minNonce` (used as nonce for EOAs) is stored in a mapping inside the `NONCE_HOLDER` system contract
    let key = address_to_h256(account);

    StorageKey::new(nonce_manager, key)
}

pub fn boojumos_key_for_eth_balance(address: &Address) -> H256 {
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

fn zkos_storage_key_for_standard_token_balance(
    token_contract: AccountTreeId,
    address: &Address,
) -> StorageKey {
    // We have different implementation of the standard ERC20 contract and native
    // eth contract. The key for the balance is different for each.
    let key = if token_contract.address() == &b160_to_h160(BASE_TOKEN_ADDRESS) {
        boojumos_key_for_eth_balance(address)
    } else {
        zkos_key_for_erc20_balance(address)
    };

    StorageKey::new(token_contract, key)
}

pub fn boojumos_storage_key_for_eth_balance(address: &Address) -> StorageKey {
    zkos_storage_key_for_standard_token_balance(
        AccountTreeId::new(b160_to_h160(BASE_TOKEN_ADDRESS)),
        address,
    )
}

#[derive(Debug)]
pub struct BoojumOsVM<S: WriteStorage, H: HistoryMode> {
    pub storage: StoragePtr<S>,
    pub tree: InMemoryTree,
    preimage: InMemoryPreimageSource,
    transactions: Vec<Transaction>,
    system_env: SystemEnv,
    batch_env: L1BatchEnv,
    config: BoojumConfig,
    witness: Option<Vec<u8>>,
    _phantom: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> BoojumOsVM<S, H> {
    pub fn new(
        batch_env: L1BatchEnv,
        system_env: SystemEnv,
        storage: StoragePtr<S>,
        raw_storage: &InMemoryStorage,
        config: &BoojumConfig,
    ) -> Self {
        let (tree, preimage) = { create_tree_from_full_state(raw_storage) };
        BoojumOsVM {
            storage,
            tree,
            preimage,
            transactions: vec![],
            system_env,
            batch_env,
            witness: None,
            config: config.clone(),
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

pub struct BoojumOsTracerDispatcher<S: WriteStorage, H: HistoryMode> {
    _tracers: Vec<S>,
    _marker: std::marker::PhantomData<H>,
}

impl<S: WriteStorage, H: HistoryMode> Default for BoojumOsTracerDispatcher<S, H> {
    fn default() -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> From<Vec<TracerPointer<S, H>>>
    for BoojumOsTracerDispatcher<S, H>
{
    fn from(_value: Vec<TracerPointer<S, H>>) -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> From<Vec<MultiVmTracerPointer<S, H>>>
    for BoojumOsTracerDispatcher<S, H>
{
    fn from(_value: Vec<MultiVmTracerPointer<S, H>>) -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> From<TracerDispatcher<S, H>>
    for BoojumOsTracerDispatcher<S, H>
{
    fn from(_value: TracerDispatcher<S, H>) -> Self {
        Self {
            _tracers: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmInterface for BoojumOsVM<S, H> {
    type TracerDispatcher = BoojumOsTracerDispatcher<S, H>;

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
            // This is called at the end of seal block.
            // Now is the moment to collect the witness and store it.

            // TODO: add support for multiple transactions.

            if let Some(witness) = self.witness.clone() {
                set_batch_witness(self.batch_env.number.0, witness);
            }

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
        let (result, witness) = execute_tx_in_zkos(
            &tx,
            &self.tree,
            &self.preimage,
            &mut self.storage,
            simulate_only,
            &self.batch_env,
            self.system_env.chain_id.as_u64(),
            self.config.boojum_bin_path.clone(),
        );

        self.witness = witness;
        result
    }

    fn start_new_l2_block(&mut self, _l2_block_env: zksync_multivm::interface::L2BlockEnv) {
        // no-op
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

impl<S: WriteStorage, H: HistoryMode> VmInterfaceHistoryEnabled for BoojumOsVM<S, H> {
    fn make_snapshot(&mut self) {}

    fn rollback_to_the_latest_snapshot(&mut self) {
        panic!("Not implemented for boojumos");
    }

    fn pop_snapshot_no_rollback(&mut self) {}
}

// Stuff copied from RIG TODO: consider merging with RIG.

fn get_account_properties(
    state_tree: &InMemoryTree,
    preimage_source: &mut InMemoryPreimageSource,
    address: &B160,
) -> AccountProperties {
    use zk_os_forward_system::run::PreimageSource;
    let key = address_into_special_storage_key(address);
    let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);
    match state_tree.cold_storage.get(&flat_key) {
        None => AccountProperties::default(),
        Some(account_hash) => {
            if account_hash.is_zero() {
                // Empty (default) account
                AccountProperties::default()
            } else {
                // Get from preimage:
                let encoded = preimage_source
                    .get_preimage(*account_hash)
                    .unwrap_or_else(|| {
                        panic!("Missing preimage for {:?} and {:?}", address, account_hash)
                    });

                AccountProperties::decode(&encoded.try_into().unwrap())
            }
        }
    }
}

pub fn set_account_properties(
    state_tree: &mut InMemoryTree,
    preimage_source: &mut InMemoryPreimageSource,

    address: B160,
    balance: Option<ruint::aliases::U256>,
    nonce: Option<u64>,
    bytecode: Option<Vec<u8>>,
) {
    let mut account_properties = get_account_properties(state_tree, preimage_source, &address);
    if let Some(bytecode) = bytecode {
        account_properties = evm_bytecode_into_account_properties(&bytecode);
        // Save bytecode preimage
        preimage_source
            .inner
            .insert(account_properties.bytecode_hash, bytecode);
    }
    if let Some(nominal_token_balance) = balance {
        account_properties.balance = nominal_token_balance;
    }
    if let Some(nonce) = nonce {
        account_properties.nonce = nonce;
    }

    let encoding = account_properties.encoding();
    let properties_hash = account_properties.compute_hash();

    let key = address_into_special_storage_key(&address);
    let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);

    // Save preimage
    preimage_source
        .inner
        .insert(properties_hash, encoding.to_vec());
    state_tree.cold_storage.insert(flat_key, properties_hash);
    state_tree.storage_tree.insert(&flat_key, &properties_hash);
}
