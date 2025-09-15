// Copy-pasted from the core API sandbox implementation.

use zksync_multivm::interface::storage::{ReadStorage, StorageWithOverrides};
use zksync_types::{
    api::state_override::{BytecodeOverride, OverrideState, StateOverride},
    bytecode::{pad_evm_bytecode, BytecodeHash, BytecodeMarker},
    get_code_key, get_evm_code_hash_key, get_known_code_key, get_nonce_key, h256_to_u256,
    u256_to_h256,
    utils::{decompose_full_nonce, nonces_to_full_nonce, storage_key_for_eth_balance},
    web3, AccountTreeId, StorageKey, H256,
};

pub(super) fn apply_state_override<S: ReadStorage>(
    storage: S,
    state_override: StateOverride,
) -> StorageWithOverrides<S> {
    let mut storage = StorageWithOverrides::new(storage);

    for (account, overrides) in state_override {
        if let Some(balance) = overrides.balance {
            let balance_key = storage_key_for_eth_balance(&account);
            storage.set_value(balance_key, u256_to_h256(balance));
        }

        if let Some(nonce) = overrides.nonce {
            let nonce_key = get_nonce_key(&account);
            let full_nonce = storage.read_value(&nonce_key);
            let (_, deployment_nonce) = decompose_full_nonce(h256_to_u256(full_nonce));
            let new_full_nonce = u256_to_h256(nonces_to_full_nonce(nonce, deployment_nonce));
            storage.set_value(nonce_key, new_full_nonce);
        }

        if let Some(code) = overrides.code {
            let (bytecode_kind, code) = match code {
                BytecodeOverride::Unspecified(code) => (BytecodeMarker::detect(&code.0), code),
                BytecodeOverride::EraVm(code) => (BytecodeMarker::EraVm, code),
                BytecodeOverride::Evm(code) => (BytecodeMarker::Evm, code),
            };
            let code_key = get_code_key(&account);

            let (code_hash, prepared_code) = match bytecode_kind {
                BytecodeMarker::EraVm => (BytecodeHash::for_bytecode(&code.0).value(), code.0),
                BytecodeMarker::Evm => {
                    // For better usability, we allow overriding EVM bytecodes even if EVM contract deployment is not enabled for the chain.
                    let versioned_hash = BytecodeHash::for_raw_evm_bytecode(&code.0).value();
                    let evm_bytecode_hash_key = get_evm_code_hash_key(versioned_hash);
                    storage.set_value(evm_bytecode_hash_key, H256(web3::keccak256(&code.0)));
                    (versioned_hash, pad_evm_bytecode(&code.0))
                }
            };

            storage.set_value(code_key, code_hash);
            let known_code_key = get_known_code_key(&code_hash);
            storage.set_value(known_code_key, H256::from_low_u64_be(1));
            storage.store_factory_dep(code_hash, prepared_code);
        }

        match overrides.state {
            Some(OverrideState::State(state)) => {
                let account = AccountTreeId::new(account);
                for (key, value) in state {
                    storage.set_value(StorageKey::new(account, key), value);
                }
                storage.insert_erased_account(account);
            }
            Some(OverrideState::StateDiff(state_diff)) => {
                let account = AccountTreeId::new(account);
                for (key, value) in state_diff {
                    storage.set_value(StorageKey::new(account, key), value);
                }
            }
            None => { /* do nothing */ }
        }
    }
    storage
}
