use clap::ValueEnum;
use serde::Deserialize;
use zksync_basic_types::h256_to_u256;
use zksync_contracts::{
    read_bootloader_code, read_sys_contract_bytecode, BaseSystemContracts,
    BaseSystemContractsHashes, ContractLanguage, SystemContractCode,
};
use zksync_multivm::{interface::TxExecutionMode, utils::bytecode::bytes_to_be_words};
// use zksync_utils::{bytes_to_be_words};
use zksync_types::bytecode::BytecodeHash;

use crate::deps::system_contracts::bytecode_from_slice;

#[derive(Deserialize, Default, Debug, Copy, Clone, PartialEq, ValueEnum)]
pub enum Options {
    // Use the compiled-in contracts
    #[default]
    BuiltIn,
    // Load the contracts bytecode at runtime from ZKSYNC_HOME
    Local,
    // Don't verify the signatures and return transaction result on calls (used only for testing - for example Forge).
    BuiltInWithoutSecurity,
}

/// Holds the system contracts (and bootloader) that are used by the in-memory node.
#[derive(Debug, Clone)]
pub struct SystemContracts {
    baseline_contracts: BaseSystemContracts,
    playground_contracts: BaseSystemContracts,
    fee_estimate_contracts: BaseSystemContracts,
    baseline_impersonating_contracts: BaseSystemContracts,
    fee_estimate_impersonating_contracts: BaseSystemContracts,
}

impl Default for SystemContracts {
    /// Creates SystemContracts that use compiled-in contracts.
    fn default() -> Self {
        SystemContracts::from_options(&Options::BuiltIn, false)
    }
}

impl SystemContracts {
    /// Creates the SystemContracts that use the complied contracts from ZKSYNC_HOME path.
    /// These are loaded at binary runtime.
    pub fn from_options(options: &Options, use_evm_emulator: bool) -> Self {
        Self {
            baseline_contracts: baseline_contracts(options, use_evm_emulator),
            playground_contracts: playground(options, use_evm_emulator),
            fee_estimate_contracts: fee_estimate_contracts(options, use_evm_emulator),
            baseline_impersonating_contracts: baseline_impersonating_contracts(
                options,
                use_evm_emulator,
            ),
            fee_estimate_impersonating_contracts: fee_estimate_impersonating_contracts(
                options,
                use_evm_emulator,
            ),
        }
    }

    pub fn contracts_for_l2_call(&self) -> &BaseSystemContracts {
        self.contracts(TxExecutionMode::EthCall, false)
    }

    pub fn contracts_for_fee_estimate(&self, impersonating: bool) -> &BaseSystemContracts {
        self.contracts(TxExecutionMode::EstimateFee, impersonating)
    }

    pub fn contracts(
        &self,
        execution_mode: TxExecutionMode,
        impersonating: bool,
    ) -> &BaseSystemContracts {
        match (execution_mode, impersonating) {
            // 'real' contracts, that do all the checks.
            (TxExecutionMode::VerifyExecute, false) => &self.baseline_contracts,
            // Ignore invalid signatures. These requests are often coming unsigned, and they keep changing the
            // gas limit - so the signatures are often not matching.
            (TxExecutionMode::EstimateFee, false) => &self.fee_estimate_contracts,
            // Read-only call - don't check signatures, have a lower (fixed) gas limit.
            (TxExecutionMode::EthCall, false) => &self.playground_contracts,
            // Without account validation and sender related checks.
            (TxExecutionMode::VerifyExecute, true) => &self.baseline_impersonating_contracts,
            (TxExecutionMode::EstimateFee, true) => &self.fee_estimate_impersonating_contracts,
            (TxExecutionMode::EthCall, true) => {
                panic!("Account impersonating with eth_call is not supported")
            }
        }
    }

    pub fn base_system_contracts_hashes(&self) -> BaseSystemContractsHashes {
        self.baseline_contracts.hashes()
    }
}

/// Creates BaseSystemContracts object with a specific bootloader.
fn bsc_load_with_bootloader(
    bootloader_bytecode: Vec<u8>,
    options: &Options,
    use_evm_emulator: bool,
) -> BaseSystemContracts {
    let hash = BytecodeHash::for_bytecode(&bootloader_bytecode).value();

    let bootloader = SystemContractCode {
        code: (bootloader_bytecode),
        hash,
    };

    let aa_bytecode = match options {
        Options::BuiltIn => bytecode_from_slice(
            "DefaultAccount",
            include_bytes!("deps/contracts/DefaultAccount.json"),
        ),
        Options::Local => read_sys_contract_bytecode("", "DefaultAccount", ContractLanguage::Sol),
        Options::BuiltInWithoutSecurity => bytecode_from_slice(
            "DefaultAccountNoSecurity",
            include_bytes!("deps/contracts/DefaultAccountNoSecurity.json"),
        ),
    };

    let aa_hash = BytecodeHash::for_bytecode(&aa_bytecode).value();
    let default_aa = SystemContractCode {
        code: (aa_bytecode),
        hash: aa_hash,
    };

    let evm_emulator = if use_evm_emulator {
        let evm_emulator_bytecode = match options {
            Options::Local => read_sys_contract_bytecode("", "EvmEmulator", ContractLanguage::Yul),
            Options::BuiltIn | Options::BuiltInWithoutSecurity => {
                panic!("no built-in EVM emulator yet")
            }
        };
        let evm_emulator_hash = BytecodeHash::for_bytecode(&evm_emulator_bytecode).value();
        Some(SystemContractCode {
            code: (evm_emulator_bytecode),
            hash: evm_emulator_hash,
        })
    } else {
        None
    };

    BaseSystemContracts {
        bootloader,
        default_aa,
        evm_emulator,
    }
}

/// BaseSystemContracts with playground bootloader -  used for handling 'eth_calls'.
fn playground(options: &Options, use_evm_emulator: bool) -> BaseSystemContracts {
    let bootloader_bytecode = match options {
        Options::BuiltIn | Options::BuiltInWithoutSecurity => {
            include_bytes!("deps/contracts/playground_batch.yul.zbin").to_vec()
        }
        Options::Local => read_bootloader_code("playground_batch"),
    };

    bsc_load_with_bootloader(bootloader_bytecode, options, use_evm_emulator)
}

/// Returns the system contracts for fee estimation.
///
/// # Returns
///
/// A `BaseSystemContracts` struct containing the system contracts used for handling 'eth_estimateGas'.
/// It sets ENSURE_RETURNED_MAGIC to 0 and BOOTLOADER_TYPE to 'playground_block'
fn fee_estimate_contracts(options: &Options, use_evm_emulator: bool) -> BaseSystemContracts {
    let bootloader_bytecode = match options {
        Options::BuiltIn | Options::BuiltInWithoutSecurity => {
            include_bytes!("deps/contracts/fee_estimate.yul.zbin").to_vec()
        }
        Options::Local => read_bootloader_code("fee_estimate"),
    };

    bsc_load_with_bootloader(bootloader_bytecode, options, use_evm_emulator)
}

fn fee_estimate_impersonating_contracts(
    options: &Options,
    use_evm_emulator: bool,
) -> BaseSystemContracts {
    let bootloader_bytecode = match options {
        Options::BuiltIn | Options::BuiltInWithoutSecurity => {
            include_bytes!("deps/contracts/fee_estimate_impersonating.yul.zbin").to_vec()
        }
        // Account impersonating is not supported with the local contracts
        Options::Local => read_bootloader_code("fee_estimate"),
    };

    bsc_load_with_bootloader(bootloader_bytecode, options, use_evm_emulator)
}

fn baseline_contracts(options: &Options, use_evm_emulator: bool) -> BaseSystemContracts {
    let bootloader_bytecode = match options {
        Options::BuiltIn | Options::BuiltInWithoutSecurity => {
            include_bytes!("deps/contracts/proved_batch.yul.zbin").to_vec()
        }
        Options::Local => read_bootloader_code("proved_batch"),
    };
    bsc_load_with_bootloader(bootloader_bytecode, options, use_evm_emulator)
}

fn baseline_impersonating_contracts(
    options: &Options,
    use_evm_emulator: bool,
) -> BaseSystemContracts {
    let bootloader_bytecode = match options {
        Options::BuiltIn | Options::BuiltInWithoutSecurity => {
            include_bytes!("deps/contracts/proved_batch_impersonating.yul.zbin").to_vec()
        }
        // Account impersonating is not supported with the local contracts
        Options::Local => read_bootloader_code("proved_batch"),
    };
    bsc_load_with_bootloader(bootloader_bytecode, options, use_evm_emulator)
}
