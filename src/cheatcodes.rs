use ethers::{abi::AbiDecode, prelude::abigen};
use multivm::{
    vm_1_3_2::zk_evm_1_3_3::{
        tracing::{BeforeExecutionData, VmLocalStateData},
        zkevm_opcode_defs::all::Opcode,
    },
    vm_m6::zk_evm_1_3_1::zkevm_opcode_defs::{FarCallABI, CALL_IMPLICIT_CALLDATA_FAT_PTR_REGISTER},
    vm_virtual_blocks::{
        DynTracer, ExecutionEndTracer, ExecutionProcessing, HistoryMode, SimpleMemory, VmTracer,
    },
};
use zksync_basic_types::H160;
use zksync_state::{StoragePtr, WriteStorage};
use zksync_types::{
    get_nonce_key,
    utils::{decompose_full_nonce, nonces_to_full_nonce, storage_key_for_eth_balance},
};
use zksync_utils::{h256_to_u256, u256_to_h256};

// address(uint160(uint256(keccak256('hevm cheat code'))))
const CHEATCODE_ADDRESS: H160 = H160([
    113, 9, 112, 158, 207, 169, 26, 128, 98, 111, 243, 152, 157, 104, 246, 127, 91, 29, 209, 45,
]);

#[derive(Default)]
pub struct CheatcodeTracer;

abigen!(
    CheatcodeContract,
    r#"[
        function deal(address who, uint256 newBalance) external
        function setNonce(address account, uint64 nonce) external
    ]"#
);

impl<S: WriteStorage, H: HistoryMode> DynTracer<S, H> for CheatcodeTracer {
    fn before_execution(
        &mut self,
        state: VmLocalStateData<'_>,
        data: BeforeExecutionData,
        memory: &SimpleMemory<H>,
        storage: StoragePtr<S>,
    ) {
        if let Opcode::FarCall(_call) = data.opcode.variant.opcode {
            let current = state.vm_local_state.callstack.current;
            if current.this_address != CHEATCODE_ADDRESS {
                return;
            }

            tracing::info!("Cheatcode triggered");
            let calldata = if current.code_page.0 == 0 || current.ergs_remaining == 0 {
                vec![]
            } else {
                let packed_abi = state.vm_local_state.registers
                    [CALL_IMPLICIT_CALLDATA_FAT_PTR_REGISTER as usize];
                assert!(packed_abi.is_pointer);
                let far_call_abi = FarCallABI::from_u256(packed_abi.value);
                memory.read_unaligned_bytes(
                    far_call_abi.memory_quasi_fat_pointer.memory_page as usize,
                    far_call_abi.memory_quasi_fat_pointer.start as usize,
                    far_call_abi.memory_quasi_fat_pointer.length as usize,
                )
            };

            // try to dispatch the cheatcode
            if let Ok(call) = CheatcodeContractCalls::decode(calldata) {
                self.dispatch_cheatcode(state, data, memory, storage, call)
            } else {
                tracing::error!("Failed to decode cheatcode calldata");
            }
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmTracer<S, H> for CheatcodeTracer {}
impl<H: HistoryMode> ExecutionEndTracer<H> for CheatcodeTracer {}
impl<S: WriteStorage, H: HistoryMode> ExecutionProcessing<S, H> for CheatcodeTracer {}

impl CheatcodeTracer {
    fn dispatch_cheatcode<S: WriteStorage, H: HistoryMode>(
        &mut self,
        _state: VmLocalStateData<'_>,
        _data: BeforeExecutionData,
        _memory: &SimpleMemory<H>,
        storage: StoragePtr<S>,
        call: CheatcodeContractCalls,
    ) {
        use CheatcodeContractCalls::*;
        match call {
            Deal(DealCall { who, new_balance }) => {
                tracing::info!("Setting balance for {who:?} to {new_balance}");
                storage
                    .borrow_mut()
                    .set_value(storage_key_for_eth_balance(&who), u256_to_h256(new_balance));
            }
            SetNonce(SetNonceCall { account, nonce }) => {
                tracing::info!("Setting nonce for {account:?} to {nonce}");
                let mut storage = storage.borrow_mut();
                let nonce_key = get_nonce_key(&account);
                let full_nonce = storage.read_value(&nonce_key);
                let (mut account_nonce, mut deployment_nonce) =
                    decompose_full_nonce(h256_to_u256(full_nonce));
                if account_nonce.as_u64() >= nonce {
                    tracing::error!(
                        "SetNonce cheatcode failed: Account nonce is already set to a higher value ({}, requested {})",
                        account_nonce,
                        nonce
                    );
                    return;
                }
                account_nonce = nonce.into();
                if deployment_nonce.as_u64() >= nonce {
                    tracing::error!(
                        "SetNonce cheatcode failed: Deployment nonce is already set to a higher value ({}, requested {})",
                        deployment_nonce,
                        nonce
                    );
                    return;
                }
                deployment_nonce = nonce.into();
                let enforced_full_nonce = nonces_to_full_nonce(account_nonce, deployment_nonce);
                tracing::info!(
                    "👷 Nonces for account {:?} have been set to {}",
                    account,
                    nonce
                );
                storage.set_value(nonce_key, u256_to_h256(enforced_full_nonce));
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_cheatcode_address() {
        assert_eq!(
            CHEATCODE_ADDRESS,
            H160::from_str("0x7109709ECfa91a80626fF3989D68f67F5b1DD12D").unwrap()
        );
    }
}
