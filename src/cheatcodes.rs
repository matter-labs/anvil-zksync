use crate::{
    node::{BlockContext, InMemoryNodeInner},
    utils::bytecode_to_factory_dep,
};
use ethers::{abi::AbiDecode, prelude::abigen};
use itertools::Itertools;
use multivm::zk_evm_1_3_3::tracing::AfterExecutionData;
use multivm::zk_evm_1_3_3::vm_state::PrimitiveValue;
use multivm::zk_evm_1_3_3::zkevm_opcode_defs::RET_IMPLICIT_RETURNDATA_PARAMS_REGISTER;
use multivm::{
    interface::dyn_tracers::vm_1_3_3::DynTracer,
    interface::{tracer::TracerExecutionStatus, L1BatchEnv},
    vm_refunds_enhancement::{HistoryMode, SimpleMemory, VmTracer},
    zk_evm_1_3_3::{
        tracing::VmLocalStateData,
        zkevm_opcode_defs::all::Opcode,
        zkevm_opcode_defs::{FatPointer, CALL_IMPLICIT_CALLDATA_FAT_PTR_REGISTER},
    },
};
use std::{
    fmt::Debug,
    sync::{Arc, Mutex, RwLock},
};
use zksync_basic_types::{AccountTreeId, Address, H160, H256, U256};
use zksync_state::{StoragePtr, WriteStorage};
use zksync_types::{
    block::{pack_block_info, unpack_block_info},
    get_code_key, get_nonce_key,
    utils::{decompose_full_nonce, nonces_to_full_nonce, storage_key_for_eth_balance},
    StorageKey, Timestamp,
};
use zksync_utils::{h256_to_u256, u256_to_h256};

// address(uint160(uint256(keccak256('hevm cheat code'))))
const CHEATCODE_ADDRESS: H160 = H160([
    113, 9, 112, 158, 207, 169, 26, 128, 98, 111, 243, 152, 157, 104, 246, 127, 91, 29, 209, 45,
]);

const INTERNAL_CONTRACT_ADDRESSES: [H160; 20] = [
    zksync_types::BOOTLOADER_ADDRESS,
    zksync_types::ACCOUNT_CODE_STORAGE_ADDRESS,
    zksync_types::NONCE_HOLDER_ADDRESS,
    zksync_types::KNOWN_CODES_STORAGE_ADDRESS,
    zksync_types::IMMUTABLE_SIMULATOR_STORAGE_ADDRESS,
    zksync_types::CONTRACT_DEPLOYER_ADDRESS,
    zksync_types::CONTRACT_FORCE_DEPLOYER_ADDRESS,
    zksync_types::L1_MESSENGER_ADDRESS,
    zksync_types::MSG_VALUE_SIMULATOR_ADDRESS,
    zksync_types::KECCAK256_PRECOMPILE_ADDRESS,
    zksync_types::L2_ETH_TOKEN_ADDRESS,
    zksync_types::SYSTEM_CONTEXT_ADDRESS,
    zksync_types::BOOTLOADER_UTILITIES_ADDRESS,
    zksync_types::EVENT_WRITER_ADDRESS,
    zksync_types::COMPRESSOR_ADDRESS,
    zksync_types::COMPLEX_UPGRADER_ADDRESS,
    zksync_types::ECRECOVER_PRECOMPILE_ADDRESS,
    zksync_types::SHA256_PRECOMPILE_ADDRESS,
    zksync_types::MINT_AND_BURN_ADDRESS,
    H160::zero(),
];

#[derive(Clone, Debug, Default)]
pub struct CheatcodeTracer<F> {
    node_ctx: F,
    returndata: Option<Vec<U256>>,
    return_ptr: Option<FatPointer>,
    near_calls: usize,
    start_prank_opts: Option<StartPrankOpts>,
}

#[derive(Clone, Debug)]
pub struct StartPrankOpts {
    sender: Address,
    origin: Option<Address>,
}

pub trait NodeCtx {
    fn set_time(&mut self, time: u64);
    fn store_factory_dep(&mut self, hash: H256, bytecode: Vec<u8>);
}

abigen!(
    CheatcodeContract,
    r#"[
        function deal(address who, uint256 newBalance)
        function etch(address who, bytes calldata code)
        function getNonce(address account)
        function roll(uint256 blockNumber)
        function setNonce(address account, uint64 nonce)
        function startPrank(address sender)
        function startPrank(address sender, address origin)
        function stopPrank()
        function warp(uint256 timestamp)
    ]"#
);

impl<F: NodeCtx, S: WriteStorage, H: HistoryMode> DynTracer<S, SimpleMemory<H>>
    for CheatcodeTracer<F>
{
    fn after_execution(
        &mut self,
        state: VmLocalStateData<'_>,
        data: AfterExecutionData,
        memory: &SimpleMemory<H>,
        storage: StoragePtr<S>,
    ) {
        if self.returndata.is_some() {
            if let Opcode::Ret(_call) = data.opcode.variant.opcode {
                if self.near_calls == 0 {
                    let ptr = state.vm_local_state.registers
                        [RET_IMPLICIT_RETURNDATA_PARAMS_REGISTER as usize];
                    let fat_data_pointer = FatPointer::from_u256(ptr.value);
                    self.return_ptr = Some(fat_data_pointer);
                } else {
                    self.near_calls = self.near_calls.saturating_sub(1);
                }
            }
        }

        if let Opcode::NearCall(_call) = data.opcode.variant.opcode {
            if self.returndata.is_some() {
                self.near_calls += 1;
            }
        }
        if let Opcode::FarCall(_call) = data.opcode.variant.opcode {
            let current = state.vm_local_state.callstack.current;
            if current.code_address != CHEATCODE_ADDRESS {
                return;
            }
            if current.code_page.0 == 0 || current.ergs_remaining == 0 {
                tracing::error!("cheatcode triggered, but no calldata or ergs available");
                return;
            }
            tracing::info!("near call: cheatcode triggered");
            let calldata = {
                let ptr = state.vm_local_state.registers
                    [CALL_IMPLICIT_CALLDATA_FAT_PTR_REGISTER as usize];
                assert!(ptr.is_pointer);
                let fat_data_pointer = FatPointer::from_u256(ptr.value);
                memory.read_unaligned_bytes(
                    fat_data_pointer.memory_page as usize,
                    fat_data_pointer.start as usize,
                    fat_data_pointer.length as usize,
                )
            };

            // try to dispatch the cheatcode
            if let Ok(call) = CheatcodeContractCalls::decode(calldata.clone()) {
                self.dispatch_cheatcode(state, data, memory, storage, call);
            } else {
                tracing::error!(
                    "Failed to decode cheatcode calldata (near call): {}",
                    hex::encode(calldata),
                );
            }
        }
    }
}

impl<F: NodeCtx + Send, S: WriteStorage, H: HistoryMode> VmTracer<S, H> for CheatcodeTracer<F> {
    fn finish_cycle(
        &mut self,
        state: &mut multivm::vm_refunds_enhancement::ZkSyncVmState<S, H>,
        _bootloader_state: &mut multivm::vm_refunds_enhancement::BootloaderState,
    ) -> TracerExecutionStatus {
        if let Some(mut fat_pointer) = self.return_ptr.take() {
            let timestamp = Timestamp(state.local_state.timestamp);

            let elements = self.returndata.take().unwrap();
            fat_pointer.length = (elements.len() as u32) * 32;
            state.local_state.registers[RET_IMPLICIT_RETURNDATA_PARAMS_REGISTER as usize] =
                PrimitiveValue {
                    value: fat_pointer.to_u256(),
                    is_pointer: true,
                };
            state.memory.populate_page(
                fat_pointer.memory_page as usize,
                elements.into_iter().enumerate().collect_vec(),
                timestamp,
            );
        }

        if let Some(start_prank_call) = &self.start_prank_opts {
            let this_address = state.local_state.callstack.current.this_address;
            if !INTERNAL_CONTRACT_ADDRESSES.contains(&this_address) {
                state.local_state.callstack.current.msg_sender = start_prank_call.sender;
            }
        }

        TracerExecutionStatus::Continue
    }
}

impl<F: NodeCtx> CheatcodeTracer<F> {
    pub fn new(node_ctx: F) -> Self {
        Self {
            node_ctx,
            start_prank_opts: None,
            returndata: None,
            return_ptr: None,
            near_calls: 0,
        }
    }

    fn dispatch_cheatcode<S: WriteStorage, H: HistoryMode>(
        &mut self,
        _state: VmLocalStateData<'_>,
        _data: AfterExecutionData,
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
            Etch(EtchCall { who, code }) => {
                tracing::info!("Setting address code for {who:?}");
                let code_key = get_code_key(&who);
                let (hash, code) = bytecode_to_factory_dep(code.0.into());
                let hash = u256_to_h256(hash);
                self.node_ctx.store_factory_dep(
                    hash,
                    code.iter()
                        .flat_map(|entry| {
                            let mut bytes = vec![0u8; 32];
                            entry.to_big_endian(&mut bytes);
                            bytes.to_vec()
                        })
                        .collect(),
                );
                storage.borrow_mut().set_value(code_key, hash);
            }
            Roll(RollCall { block_number }) => {
                tracing::info!("Setting block number to {}", block_number);

                let key = StorageKey::new(
                    AccountTreeId::new(zksync_types::SYSTEM_CONTEXT_ADDRESS),
                    zksync_types::CURRENT_VIRTUAL_BLOCK_INFO_POSITION,
                );
                let mut storage = storage.borrow_mut();
                let (_, block_timestamp) =
                    unpack_block_info(h256_to_u256(storage.read_value(&key)));
                storage.set_value(
                    key,
                    u256_to_h256(pack_block_info(block_number.as_u64(), block_timestamp)),
                );
            }
            GetNonce(GetNonceCall { account }) => {
                tracing::info!("Getting nonce for {account:?}");
                let mut storage = storage.borrow_mut();
                let nonce_key = get_nonce_key(&account);
                let full_nonce = storage.read_value(&nonce_key);
                let (account_nonce, _) = decompose_full_nonce(h256_to_u256(full_nonce));
                tracing::info!(
                    "👷 Nonces for account {:?} are {}",
                    account,
                    account_nonce.as_u64()
                );
                tracing::info!("👷 Setting returndata",);
                self.returndata = Some(vec![account_nonce.into()]);
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
            StartPrank(StartPrankCall { sender }) => {
                tracing::info!("Starting prank to {sender:?}");
                self.start_prank_opts = Some(StartPrankOpts {
                    sender,
                    origin: None,
                });
            }
            StartPrankWithOrigin(StartPrankWithOriginCall { sender, origin }) => {
                tracing::info!("Starting prank to {sender:?} with origin {origin:?}");

                let key = StorageKey::new(
                    AccountTreeId::new(zksync_types::SYSTEM_CONTEXT_ADDRESS),
                    zksync_types::SYSTEM_CONTEXT_TX_ORIGIN_POSITION,
                );
                let mut storage = storage.borrow_mut();
                let original_tx_origin = storage.read_value(&key);
                storage.set_value(key, origin.into());

                self.start_prank_opts = Some(StartPrankOpts {
                    sender,
                    origin: Some(original_tx_origin.into()),
                });
            }
            StopPrank(StopPrankCall) => {
                tracing::info!("Stopping prank");

                if let Some(origin) = self.start_prank_opts.as_ref().map(|v| v.origin).flatten() {
                    let key = StorageKey::new(
                        AccountTreeId::new(zksync_types::SYSTEM_CONTEXT_ADDRESS),
                        zksync_types::SYSTEM_CONTEXT_TX_ORIGIN_POSITION,
                    );
                    let mut storage = storage.borrow_mut();
                    storage.set_value(key, origin.into());
                }

                self.start_prank_opts = None;
            }
            Warp(WarpCall { timestamp }) => {
                tracing::info!("Setting block timestamp {}", timestamp);
                self.node_ctx.set_time(timestamp.as_u64());

                let key = StorageKey::new(
                    AccountTreeId::new(zksync_types::SYSTEM_CONTEXT_ADDRESS),
                    zksync_types::CURRENT_VIRTUAL_BLOCK_INFO_POSITION,
                );
                let mut storage = storage.borrow_mut();
                let (block_number, _) = unpack_block_info(h256_to_u256(storage.read_value(&key)));
                storage.set_value(
                    key,
                    u256_to_h256(pack_block_info(block_number, timestamp.as_u64())),
                );
            }
        };
    }
}

pub struct CheatcodeNodeContext<T> {
    pub in_memory_node_inner: Arc<RwLock<InMemoryNodeInner<T>>>,
    pub batch_env: Arc<Mutex<L1BatchEnv>>,
    pub block_ctx: Arc<Mutex<BlockContext>>,
}

impl<T> CheatcodeNodeContext<T> {
    pub fn new(
        in_memory_node_inner: Arc<RwLock<InMemoryNodeInner<T>>>,
        batch_env: Arc<Mutex<L1BatchEnv>>,
        block_ctx: Arc<Mutex<BlockContext>>,
    ) -> Self {
        Self {
            in_memory_node_inner,
            batch_env,
            block_ctx,
        }
    }
}

impl<T> NodeCtx for CheatcodeNodeContext<T> {
    fn set_time(&mut self, time: u64) {
        self.in_memory_node_inner.write().unwrap().current_timestamp = time;
        self.batch_env.lock().unwrap().timestamp = time + 1;
        self.block_ctx.lock().unwrap().timestamp = time + 1;
    }

    fn store_factory_dep(&mut self, hash: H256, bytecode: Vec<u8>) {
        self.in_memory_node_inner
            .write()
            .unwrap()
            .fork_storage
            .store_factory_dep(hash, bytecode)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::{
        deps::system_contracts::bytecode_from_slice,
        http_fork_source::HttpForkSource,
        node::{InMemoryNode, TransactionResult},
        testing::{self, LogBuilder, TransactionBuilder},
    };
    use ethers::abi::{short_signature, AbiEncode, HumanReadableParser, ParamType, Token};
    use zksync_basic_types::{Address, L2ChainId, Nonce, H160, H256, U256};
    use zksync_core::api_server::web3::backend_jsonrpc::namespaces::eth::EthNamespaceT;
    use zksync_types::{
        api::{Block, CallTracerConfig, SupportedTracers, TransactionReceipt},
        fee::Fee,
        l2::L2Tx,
        transaction_request::CallRequestBuilder,
        utils::deployed_address_create,
    };

    #[test]
    fn test_cheatcode_address() {
        assert_eq!(
            CHEATCODE_ADDRESS,
            H160::from_str("0x7109709ECfa91a80626fF3989D68f67F5b1DD12D").unwrap()
        );
    }

    fn deploy_test_contract(node: &InMemoryNode<HttpForkSource>) -> Address {
        let private_key = H256::repeat_byte(0xee);
        let from_account = zksync_types::PackedEthSignature::address_from_private_key(&private_key)
            .expect("failed generating address");
        node.set_rich_account(from_account);

        let bytecode = bytecode_from_slice(
            "Secondary",
            include_bytes!("deps/test-contracts/TestCheatcodes.json"),
        );
        let deployed_address = deployed_address_create(from_account, U256::zero());
        testing::deploy_contract(
            &node,
            H256::repeat_byte(0x1),
            private_key,
            bytecode,
            None,
            Nonce(0),
        );
        deployed_address
    }
    use crate::namespaces::ConfigurationApiNamespaceT;

    #[tokio::test]
    async fn test_cheatcode_contract() {
        let node = InMemoryNode::<HttpForkSource>::default();
        node.config_set_show_calls("all".to_string()).unwrap();

        let test_contract_address = deploy_test_contract(&node);
        println!("test contract address: {:?}", test_contract_address);
        let private_key = H256::repeat_byte(0xee);
        let from_account = zksync_types::PackedEthSignature::address_from_private_key(&private_key)
            .expect("failed generating address");
        node.set_rich_account(from_account);
        // let recipient_address = Address::random();
        // let func = HumanReadableParser::parse_function("testDeal(address)").unwrap();
        // let calldata = func
        //     .encode_input(&[Token::Address(recipient_address)])
        //     .unwrap();

        let calldata = short_signature("deal()", &[]);
        let mut l2tx = L2Tx::new_signed(
            test_contract_address,
            calldata.into(),
            Nonce(1),
            Fee {
                gas_limit: U256::from(1_000_000),
                max_fee_per_gas: U256::from(250_000_000),
                max_priority_fee_per_gas: U256::from(250_000_000),
                gas_per_pubdata_limit: U256::from(20000),
            },
            U256::from(1),
            L2ChainId::from(260),
            &private_key,
            None,
            Default::default(),
        )
        .unwrap();
        l2tx.set_input(vec![], H256::repeat_byte(0x1));
        node.apply_txs(vec![l2tx]).unwrap();

        let receipt = node
            .get_transaction_receipt(H256::repeat_byte(0x1))
            .await
            .unwrap()
            .unwrap();

        // check that the transaction was successful
        assert_eq!(receipt.status.unwrap(), 1.into());
    }
}
