use std::{alloc::Global, collections::HashMap};

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
        ExecutionResult, PushTransactionResult, TxExecutionMode, VmExecutionResultAndLogs,
        VmInterface, VmInterfaceHistoryEnabled, VmRevertReason,
    },
    tracers::TracerDispatcher,
    vm_latest::HistoryEnabled,
    HistoryMode,
};
use zksync_types::{
    block::{pack_block_info, unpack_block_info},
    web3::keccak256,
    AccountTreeId, Address, StorageKey, Transaction, H160, H256, SYSTEM_CONTEXT_ADDRESS,
    SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
};
use zksync_utils::{address_to_h256, h256_to_u256, u256_to_h256};

use crate::deps::InMemoryStorage;

use super::vm_interface::TestNodeVMInterface;

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

// TODO: check endinanness
pub fn h160_to_b160(data: &H160) -> B160 {
    B160::from_be_bytes(data.as_fixed_bytes().clone())
}

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
    println!("Tree size is: {}", tree.cold_storage.len());
    println!("Preimage size is: {}", preimage_source.inner.len());

    (tree, preimage_source)
}

pub fn add_elem_to_tree(tree: &mut InMemoryTree, k: &StorageKey, v: &H256) {
    let kk = derive_flat_storage_key(&h160_to_b160(k.address()), &h256_to_bytes32(k.key()));
    let vv = h256_to_bytes32(v);

    tree.storage_tree.insert(&kk, &vv);
    tree.cold_storage.insert(kk, vv);
}

pub fn execute_tx_in_zkos<W: WriteStorage>(
    tx: &Transaction,
    tree: &InMemoryTree,
    preimage_source: &InMemoryPreimageSource,
    storage: &mut StoragePtr<W>,
    simulate_only: bool,
) -> VmExecutionResultAndLogs {
    let batch_context = basic_system::basic_system::BasicBlockMetadataFromOracle {
        eip1559_basefee: ruint::aliases::U256::from(if simulate_only { 0u64 } else { 1000u64 }),
        ergs_price: ruint::aliases::U256::from(1u64),
        // FIXME
        block_number: 1,
        timestamp: 42,
        gas_per_pubdata: ruint::aliases::U256::from(1u64),
    };

    println!("Tree size is: {}", tree.cold_storage.len());

    let aa1 = match &tx.common_data {
        zksync_types::ExecuteTransactionCommon::L1(_) => todo!(),
        zksync_types::ExecuteTransactionCommon::L2(l2_tx_common_data) => l2_tx_common_data,
        zksync_types::ExecuteTransactionCommon::ProtocolUpgrade(_) => todo!(),
    };

    let storage_commitment = StorageCommitment {
        root: *tree.storage_tree.root(),
        next_free_slot: tree.storage_tree.next_free_slot,
    };

    let tx_raw = match tx.tx_format() {
        zksync_types::l2::TransactionType::LegacyTransaction => {
            let mut tx_raw: Vec<u8> = vec![];
            tx_raw.append(&mut vec![0u8; 32]);
            append_address(&mut tx_raw, &aa1.initiator_address);
            append_address(
                &mut tx_raw,
                &tx.execute.contract_address.unwrap_or(H160::zero()),
            );

            let mut gas_limit = aa1.fee.gas_limit;
            // HACK
            if simulate_only {
                gas_limit = gas_limit.saturating_sub(3_000_000.into())
            }

            println!("=== Gas limit: {}", gas_limit);
            append_u256(&mut tx_raw, &gas_limit);
            append_u256(&mut tx_raw, &aa1.fee.gas_per_pubdata_limit);

            let fee_per_gas = aa1.fee.max_fee_per_gas;

            append_u256(&mut tx_raw, &fee_per_gas);
            // hack for legacy tx.
            append_u256(&mut tx_raw, &fee_per_gas);

            // paymaster
            append_u64(&mut tx_raw, 0);

            append_u64(&mut tx_raw, aa1.nonce.0.into());

            append_u256(&mut tx_raw, &tx.execute.value);

            let mut reserved = [0u64; 4];

            // Should check chain_id
            reserved[0] = 1;

            if tx.execute.contract_address.is_none() {
                reserved[1] = 1;
            }

            for i in 0..4 {
                // reserved
                append_u64(&mut tx_raw, reserved[i]);
            }

            let signature_u256 = aa1.signature.len().div_ceil(32) as u64;

            let execute_calldata_words = tx.execute.calldata.len().div_ceil(32) as u64;
            dbg!(execute_calldata_words);

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
            current_offset += 1;

            // len - data.
            append_usize(&mut tx_raw, tx.execute.calldata.len());
            tx_raw.append(&mut pad_to_word(&tx.execute.calldata));

            // len - signature.
            append_usize(&mut tx_raw, aa1.signature.len());
            tx_raw.append(&mut pad_to_word(&aa1.signature));

            // factory deps
            append_u64(&mut tx_raw, 0);
            // paymater
            append_u64(&mut tx_raw, 0);
            // reserved
            append_u64(&mut tx_raw, 0);
            tx_raw
        }
        zksync_types::l2::TransactionType::EIP2930Transaction => todo!(),
        zksync_types::l2::TransactionType::EIP1559Transaction => todo!(),
        zksync_types::l2::TransactionType::EIP712Transaction => todo!(),
        zksync_types::l2::TransactionType::PriorityOpTransaction => todo!(),
        zksync_types::l2::TransactionType::ProtocolUpgradeTransaction => todo!(),
    };

    let (output, new_known_factory_deps) = if simulate_only {
        (
            forward_system::run::simulate_tx(
                tx_raw,
                storage_commitment,
                batch_context,
                tree.clone(),
                preimage_source.clone(),
            )
            .unwrap(),
            None,
        )
    } else {
        let tx_source = TxListSource {
            // transactions: vec![encoded_iwasm_tx].into(),
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

        // apply storage writes..
        for write in batch_output.storage_writes {
            //let ab = write.key.as_u8_array_ref();
            //let ac = H256::from(ab);

            let ab = StorageKey::new(
                AccountTreeId::new(Address::from_slice(&write.account.to_be_bytes_vec())),
                H256::from(write.account_key.as_u8_array_ref()),
            );
            dbg!(&ab);
            storage_ptr.set_value(ab, H256::from(write.value.as_u8_array_ref()));
        }
        let mut f_deps = HashMap::new();

        println!(
            "Adding {} preimages:",
            batch_output.published_preimages.len()
        );

        for factory_dep in batch_output.published_preimages {
            println!("    {:?}", factory_dep.0);
            f_deps.insert(bytes32_to_h256(factory_dep.0), factory_dep.1);
        }

        (batch_output.tx_results[0].clone(), Some(f_deps))
    };

    let tx_output = match output.as_ref() {
        Ok(tx_output) => {
            match &tx_output.execution_result {
                forward_system::run::ExecutionResult::Success(output) => match &output {
                    forward_system::run::ExecutionOutput::Call(data) => data,
                    forward_system::run::ExecutionOutput::Create(data, address) => {
                        dbg!(address);
                        // TODO - pass it to the output somehow.
                        println!("Deployed to {:?}", address);
                        data
                    }
                },
                _ => panic!("TX failed"),
            }
        }
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
                new_known_factory_deps: None,
            }
        }
    };

    VmExecutionResultAndLogs {
        result: ExecutionResult::Success {
            output: tx_output.clone(),
        },
        logs: Default::default(),
        statistics: Default::default(),
        refunds: Default::default(),
        new_known_factory_deps,
    }
}

pub fn zkos_get_nonce_key(account: &Address) -> StorageKey {
    let nonce_manager = AccountTreeId::new(b160_to_h160(NONCE_HOLDER_HOOK_ADDRESS));

    // The `minNonce` (used as nonce for EOAs) is stored in a mapping inside the `NONCE_HOLDER` system contract
    //let key = get_address_mapping_key(account, H256::zero());
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

pub struct ZKOsVM<S: WriteStorage> {
    pub storage: StoragePtr<S>,
    pub tree: InMemoryTree,
    preimage: InMemoryPreimageSource,
    transactions: Vec<Transaction>,
    execution_mode: TxExecutionMode,
}

impl<S: WriteStorage> ZKOsVM<S> {
    pub fn new(
        storage: StoragePtr<S>,
        raw_storage: &InMemoryStorage,
        execution_mode: TxExecutionMode,
    ) -> Self {
        let (tree, preimage) = { create_tree_from_full_state(raw_storage) };
        ZKOsVM {
            storage,
            tree,
            preimage,
            transactions: vec![],
            execution_mode,
        }
    }
}

impl<S: WriteStorage> TestNodeVMInterface for ZKOsVM<S> {
    fn execute_tx(&mut self, tx: Transaction) -> VmExecutionResultAndLogs {
        {
            let mut storage_ptr = self.storage.borrow_mut();
            let current_l1_batch_info_key = StorageKey::new(
                AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
                SYSTEM_CONTEXT_BLOCK_INFO_POSITION,
            );
            let current_l1_batch_info = storage_ptr.read_value(&current_l1_batch_info_key);
            let (batch_number, batch_timestamp) =
                unpack_block_info(h256_to_u256(current_l1_batch_info));

            dbg!(batch_number);
            dbg!(batch_timestamp);

            // TODO: move this somewhere
            let aa = pack_block_info(batch_number + 1, batch_timestamp + 1);
            storage_ptr.set_value(current_l1_batch_info_key, u256_to_h256(aa));
        }

        let simulate_only = match self.execution_mode {
            TxExecutionMode::VerifyExecute => false,
            TxExecutionMode::EstimateFee => true,
            TxExecutionMode::EthCall => true,
        };
        let tx_result = {
            execute_tx_in_zkos(
                &tx,
                &self.tree,
                &self.preimage,
                &mut self.storage,
                simulate_only,
            )
        };
        tx_result
    }
}

#[derive(Default)]
pub struct ZkOsTracerDispatcher {}

impl<S: WriteStorage> VmInterface for ZKOsVM<S> {
    type TracerDispatcher = ZkOsTracerDispatcher;

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
        dispatcher: &mut Self::TracerDispatcher,
        execution_mode: zksync_multivm::interface::InspectExecutionMode,
    ) -> VmExecutionResultAndLogs {
        let simulate_only = match self.execution_mode {
            TxExecutionMode::VerifyExecute => false,
            TxExecutionMode::EstimateFee => true,
            TxExecutionMode::EthCall => true,
        };

        // FIXME.
        let tx = self.transactions[0].clone();
        execute_tx_in_zkos(
            &tx,
            &self.tree,
            &self.preimage,
            &mut self.storage,
            simulate_only,
        )
    }

    fn start_new_l2_block(&mut self, l2_block_env: zksync_multivm::interface::L2BlockEnv) {
        todo!()
    }

    fn inspect_transaction_with_bytecode_compression(
        &mut self,
        tracer: &mut Self::TracerDispatcher,
        tx: Transaction,
        with_compression: bool,
    ) -> (
        zksync_multivm::interface::BytecodeCompressionResult<'_>,
        VmExecutionResultAndLogs,
    ) {
        todo!()
    }

    fn finish_batch(
        &mut self,
        pubdata_builder: std::rc::Rc<dyn zksync_multivm::interface::pubdata::PubdataBuilder>,
    ) -> zksync_multivm::interface::FinishedL1Batch {
        todo!()
    }
}

impl<S: WriteStorage> VmInterfaceHistoryEnabled for ZKOsVM<S> {
    fn make_snapshot(&mut self) {}

    fn rollback_to_the_latest_snapshot(&mut self) {
        panic!("Not implemented for zkos");
    }

    fn pop_snapshot_no_rollback(&mut self) {}
}
