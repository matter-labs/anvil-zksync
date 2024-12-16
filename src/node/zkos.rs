use std::{alloc::Global, cell::RefMut, collections::HashMap};

use basic_system::basic_system::simple_growable_storage::TestingTree;
use forward_system::run::{
    test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource},
    PreimageType, StorageCommitment,
};
use ruint::aliases::B160;
use zk_ee::{common_structs::derive_flat_storage_key, utils::Bytes32};
use zksync_multivm::interface::{
    storage::{StoragePtr, WriteStorage},
    ExecutionResult, VmExecutionResultAndLogs, VmRevertReason,
};
use zksync_types::{AccountTreeId, Address, StorageKey, Transaction, H160, H256};

use crate::deps::{storage_view::StorageView, InMemoryStorage};

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
    tree: &mut InMemoryTree,
    preimage_source: &mut InMemoryPreimageSource,
    storage: &mut StoragePtr<W>,
) -> VmExecutionResultAndLogs {
    let batch_context = basic_system::basic_system::BasicBlockMetadataFromOracle {
        eip1559_basefee: ruint::aliases::U256::from(1000u64),
        ergs_price: ruint::aliases::U256::from(1u64),
        block_number: 1,
        timestamp: 42,
    };

    println!("Tree size is: {}", tree.cold_storage.len());

    /*tree.storage_tree
    .insert(&zk_ee::utils::Bytes32::ZERO, &zk_ee::utils::Bytes32::MAX);*/

    let aa1 = match &tx.common_data {
        zksync_types::ExecuteTransactionCommon::L1(l1_tx_common_data) => todo!(),
        zksync_types::ExecuteTransactionCommon::L2(l2_tx_common_data) => l2_tx_common_data,
        zksync_types::ExecuteTransactionCommon::ProtocolUpgrade(
            protocol_upgrade_tx_common_data,
        ) => todo!(),
    };

    /*add_funds_to_address(
        B160::from_be_bytes(aa1.initiator_address.0),
        ruint::aliases::U256::from(1_000_000_000_000_000_u64),
        &mut tree,
    );*/

    let storage_commitment = StorageCommitment {
        root: *tree.storage_tree.root(),
        next_free_slot: tree.storage_tree.next_free_slot,
    };

    //let foo = ethereum_types::H160::repeat_byte(3);
    //ethereum_types::H160::
    // aa1.initiator_address

    /*let bb = Token::Tuple(vec![
        Token::Uint(ethereum_types::U256::zero()),
        Token::Address(foo),
    ])
    .to_vec();*/
    // FIXME: this might be wrong..
    //let aa = tx.raw_bytes.as_ref().unwrap();
    //println!("Tx raw bytes: {:?}", aa);

    let mut tx_raw: Vec<u8> = vec![];
    tx_raw.append(&mut vec![0u8; 32]);
    append_address(&mut tx_raw, &aa1.initiator_address);
    append_address(
        &mut tx_raw,
        &tx.execute.contract_address.unwrap_or(H160::zero()),
    );
    append_u256(&mut tx_raw, &aa1.fee.gas_limit);
    append_u256(&mut tx_raw, &aa1.fee.gas_per_pubdata_limit);

    let mut fee_per_gas = aa1.fee.max_fee_per_gas;
    // HACK - for 'call' calls.
    if fee_per_gas.is_zero() {
        fee_per_gas = 1000.into();
    }
    append_u256(&mut tx_raw, &fee_per_gas);
    //append_u256(&mut tx_raw, &aa1.fee.max_priority_fee_per_gas);
    // hack for legacy tx.
    append_u256(&mut tx_raw, &fee_per_gas);

    // paymaster
    append_u64(&mut tx_raw, 0);

    append_u64(&mut tx_raw, aa1.nonce.0.into());

    append_u256(&mut tx_raw, &tx.execute.value);

    let mut reserved = [0u64; 4];

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

    /*let mut pp = vec![0u8; 32];
    let ap1 = aa1.initiator_address.as_fixed_bytes();
    for i in 0..20 {
        pp[i + 12] = ap1[i];
    }
    tx_raw.append(&mut pp);*/

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

    let tx_output = match batch_output.tx_results[0].as_ref() {
        Ok(tx_output) => {
            match &tx_output.execution_result {
                forward_system::run::output::ExecutionResult::Success(output) => match &output {
                    forward_system::run::output::ExecutionOutput::Call(data) => data,
                    forward_system::run::output::ExecutionOutput::Create(data, address) => {
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
    // TODO - update newknown fcatory deps with batch_output.published_preimages..

    let mut f_deps = HashMap::new();

    println!(
        "Adding {} preimages:",
        batch_output.published_preimages.len()
    );

    for factory_dep in batch_output.published_preimages {
        println!("    {:?}", factory_dep.0);
        f_deps.insert(bytes32_to_h256(factory_dep.0), factory_dep.1);
    }

    VmExecutionResultAndLogs {
        result: ExecutionResult::Success {
            output: tx_output.clone(),
        },
        logs: Default::default(),
        statistics: Default::default(),
        refunds: Default::default(),
        new_known_factory_deps: Some(f_deps),
    }
}
