//! This file hold testing helpers for other unit tests.
//!
//! There is MockServer that can help simulate a forked network.
//!

#![cfg(test)]

use crate::node::TxExecutionInfo;
use anvil_zksync_types::L2TxBuilder;
use zksync_types::api::{DebugCall, DebugCallType, Log};
use zksync_types::bytecode::BytecodeHash;
use zksync_types::fee::Fee;
use zksync_types::l2::L2Tx;
use zksync_types::{
    Address, ExecuteTransactionCommon, K256PrivateKey, L2ChainId, Nonce, Transaction, H160, H256,
    U256, U64,
};

#[derive(Debug, Clone)]
pub struct TransactionBuilder {
    from_account_private_key: K256PrivateKey,
    gas_limit: U256,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self {
            from_account_private_key: K256PrivateKey::from_bytes(H256::random()).unwrap(),
            gas_limit: U256::from(4_000_000),
            max_fee_per_gas: U256::from(50_000_000),
            max_priority_fee_per_gas: U256::from(50_000_000),
        }
    }
}

impl TransactionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn deploy_contract(
        private_key: &K256PrivateKey,
        bytecode: Vec<u8>,
        calldata: Option<Vec<u8>>,
        nonce: Nonce,
    ) -> L2Tx {
        use alloy::dyn_abi::{DynSolValue, JsonAbiExt};
        use alloy::json_abi::{Function, Param, StateMutability};

        let salt = [0u8; 32];
        let bytecode_hash = BytecodeHash::for_bytecode(&bytecode).value().0;
        let call_data = calldata.unwrap_or_default();

        let create = Function {
            name: "create".to_string(),
            inputs: vec![
                Param {
                    name: "_salt".to_string(),
                    ty: "bytes32".to_string(),
                    components: vec![],
                    internal_type: None,
                },
                Param {
                    name: "_bytecodeHash".to_string(),
                    ty: "bytes32".to_string(),
                    components: vec![],
                    internal_type: None,
                },
                Param {
                    name: "_input".to_string(),
                    ty: "bytes".to_string(),
                    components: vec![],
                    internal_type: None,
                },
            ],
            outputs: vec![Param {
                name: "".to_string(),
                ty: "address".to_string(),
                components: vec![],
                internal_type: None,
            }],
            state_mutability: StateMutability::Payable,
        };

        let data = create
            .abi_encode_input(&[
                DynSolValue::FixedBytes(salt.into(), salt.len()),
                DynSolValue::FixedBytes(
                    bytecode_hash[..].try_into().expect("invalid hash length"),
                    bytecode_hash.len(),
                ),
                DynSolValue::Bytes(call_data),
            ])
            .expect("failed to encode function data");

        L2Tx::new_signed(
            Some(zksync_types::CONTRACT_DEPLOYER_ADDRESS),
            data.to_vec(),
            nonce,
            Fee {
                gas_limit: U256::from(400_000_000),
                max_fee_per_gas: U256::from(50_000_000),
                max_priority_fee_per_gas: U256::from(50_000_000),
                gas_per_pubdata_limit: U256::from(50000),
            },
            U256::from(0),
            zksync_types::L2ChainId::from(260),
            private_key,
            vec![bytecode],
            Default::default(),
        )
        .expect("failed signing tx")
    }

    pub fn set_gas_limit(&mut self, gas_limit: U256) -> &mut Self {
        self.gas_limit = gas_limit;
        self
    }

    pub fn set_max_fee_per_gas(&mut self, max_fee_per_gas: U256) -> &mut Self {
        self.max_fee_per_gas = max_fee_per_gas;
        self
    }

    pub fn set_max_priority_fee_per_gas(&mut self, max_priority_fee_per_gas: U256) -> &mut Self {
        self.max_priority_fee_per_gas = max_priority_fee_per_gas;
        self
    }

    pub fn build(&mut self) -> L2Tx {
        L2Tx::new_signed(
            Some(Address::random()),
            vec![],
            Nonce(0),
            Fee {
                gas_limit: self.gas_limit,
                max_fee_per_gas: self.max_fee_per_gas,
                max_priority_fee_per_gas: self.max_priority_fee_per_gas,
                gas_per_pubdata_limit: U256::from(50000),
            },
            U256::from(1),
            L2ChainId::from(260),
            &self.from_account_private_key,
            vec![],
            Default::default(),
        )
        .unwrap()
    }

    pub fn impersonate(&mut self, to_impersonate: Address) -> L2Tx {
        L2TxBuilder::new(
            to_impersonate,
            Nonce(0),
            self.gas_limit,
            self.max_fee_per_gas,
            260.into(),
        )
        .with_to(Address::random())
        .with_max_priority_fee_per_gas(self.max_priority_fee_per_gas)
        .build_impersonated()
    }
}

/// Builds transaction logs
#[derive(Debug, Default, Clone)]
pub struct LogBuilder {
    block_number: U64,
    block_timestamp: U64,
    address: Option<H160>,
    topics: Option<Vec<H256>>,
}

impl LogBuilder {
    /// Create a new instance of [LogBuilder]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the log's block number
    pub fn set_block(&mut self, number: U64) -> &mut Self {
        self.block_number = number;
        self
    }

    /// Sets the log address
    pub fn set_address(&mut self, address: H160) -> &mut Self {
        self.address = Some(address);
        self
    }

    /// Sets the log topics
    pub fn set_topics(&mut self, topics: Vec<H256>) -> &mut Self {
        self.topics = Some(topics);
        self
    }

    /// Builds the [Log] object
    pub fn build(&mut self) -> Log {
        Log {
            address: self.address.unwrap_or_default(),
            topics: self.topics.clone().unwrap_or_default(),
            data: Default::default(),
            block_hash: Some(H256::zero()),
            block_number: Some(self.block_number),
            l1_batch_number: Default::default(),
            transaction_hash: Default::default(),
            transaction_index: Default::default(),
            log_index: Default::default(),
            transaction_log_index: Default::default(),
            log_type: Default::default(),
            removed: Some(false),
            block_timestamp: Some(self.block_timestamp),
        }
    }
}

/// Simple storage solidity contract that stores and retrieves two numbers
///
/// contract Storage {
///   uint256 number1 = 1024;
///   uint256 number2 = 115792089237316195423570985008687907853269984665640564039457584007913129639935; // uint256::max
///
///   function retrieve1() public view returns (uint256) {
///     return number1;
///   }
///
///   function retrieve2() public view returns (uint256) {
///     return number2;
///   }
///
///   function transact_retrieve1() public returns (uint256) {
///     return number1;
///   }
/// }
pub const STORAGE_CONTRACT_BYTECODE: &str    = "0000008003000039000000400030043f0000000102200190000000150000c13d00000000020100190000000d02200198000000290000613d000000000101043b000000e0011002700000000e0210009c000000220000613d0000000f0210009c000000220000613d000000100110009c000000290000c13d0000000001000416000000000101004b000000290000c13d0000000101000039000000000101041a000000260000013d0000000001000416000000000101004b000000290000c13d0000040001000039000000000010041b000000010100008a0000000102000039000000000012041b0000002001000039000001000010044300000120000004430000000c010000410000002c0001042e0000000001000416000000000101004b000000290000c13d000000000100041a000000800010043f00000011010000410000002c0001042e00000000010000190000002d000104300000002b000004320000002c0001042e0000002d0001043000000000000000000000000000000000000000020000000000000000000000000000004000000100000000000000000000000000000000000000000000000000fffffffc00000000000000000000000000000000000000000000000000000000000000000000000000000000bbf5533500000000000000000000000000000000000000000000000000000000ae2e2cce000000000000000000000000000000000000000000000000000000002711432d0000000000000000000000000000000000000020000000800000000000000000ccac83652a1e8701e76052e8662f8e7889170c68883ae295c1c984f22be3560f";

/// Returns a default instance for a successful [TxExecutionInfo]
pub fn default_tx_execution_info() -> TxExecutionInfo {
    TxExecutionInfo {
        tx: Transaction {
            common_data: ExecuteTransactionCommon::L2(Default::default()),
            execute: Default::default(),
            received_timestamp_ms: 0,
            raw_bytes: None,
        },
        batch_number: Default::default(),
        miniblock_number: Default::default(),
    }
}

/// Returns a default instance for a successful [DebugCall]
pub fn default_tx_debug_info() -> DebugCall {
    DebugCall {
        r#type: DebugCallType::Call,
        from: Address::zero(),
        to: Address::zero(),
        gas: U256::zero(),
        gas_used: U256::zero(),
        value: U256::zero(),
        output: Default::default(),
        input: Default::default(),
        error: None,
        revert_reason: None,
        calls: vec![DebugCall {
            r#type: DebugCallType::Call,
            from: Address::zero(),
            to: Address::zero(),
            gas: U256::zero(),
            gas_used: U256::zero(),
            value: U256::zero(),
            output: Default::default(),
            input: Default::default(),
            error: None,
            revert_reason: None,
            calls: vec![],
        }],
    }
}

mod test {
    use super::*;

    #[test]
    fn test_log_builder_set_block() {
        let log = LogBuilder::new().set_block(U64::from(2)).build();

        assert_eq!(Some(U64::from(2)), log.block_number);
    }

    #[test]
    fn test_log_builder_set_address() {
        let log = LogBuilder::new()
            .set_address(H160::repeat_byte(0x1))
            .build();

        assert_eq!(H160::repeat_byte(0x1), log.address);
    }

    #[test]
    fn test_log_builder_set_topics() {
        let log = LogBuilder::new()
            .set_topics(vec![
                H256::repeat_byte(0x1),
                H256::repeat_byte(0x2),
                H256::repeat_byte(0x3),
                H256::repeat_byte(0x4),
            ])
            .build();

        assert_eq!(
            vec![
                H256::repeat_byte(0x1),
                H256::repeat_byte(0x2),
                H256::repeat_byte(0x3),
                H256::repeat_byte(0x4),
            ],
            log.topics
        );
    }
}
