use std::path::{Path, PathBuf};

use crate::deps::system_contracts::load_builtin_contract;
use crate::node::ImpersonationManager;
use anvil_zksync_config::types::{SystemContractsOptions, ZKOSConfig};
use zksync_contracts::{
    read_sys_contract_bytecode, BaseSystemContracts, BaseSystemContractsHashes, ContractLanguage,
    SystemContractCode, SystemContractsRepo,
};
use zksync_multivm::interface::TxExecutionMode;
use zksync_types::bytecode::BytecodeHash;
use zksync_types::{Address, ProtocolVersionId};

/// Builder for SystemContracts
#[derive(Debug, Default)]
pub struct SystemContractsBuilder {
    system_contracts_options: Option<SystemContractsOptions>,
    system_contracts_path: Option<PathBuf>,
    protocol_version: Option<ProtocolVersionId>,
    use_evm_emulator: bool,
    use_zkos: bool,
}

impl SystemContractsBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the system contracts options (e.g. Local, BuiltIn, BuiltInWithoutSecurity)
    pub fn system_contracts_options(mut self, opts: SystemContractsOptions) -> Self {
        self.system_contracts_options = Some(opts);
        self
    }

    /// Set the system contracts path
    pub fn system_contracts_path(mut self, path: Option<PathBuf>) -> Self {
        self.system_contracts_path = path;
        self
    }

    /// Set the protocol version
    pub fn protocol_version(mut self, version: ProtocolVersionId) -> Self {
        self.protocol_version = Some(version);
        self
    }

    /// Enable or disable the EVM emulator
    pub fn use_evm_emulator(mut self, flag: bool) -> Self {
        self.use_evm_emulator = flag;
        self
    }

    /// Enable or disable ZKOS
    pub fn use_zkos(mut self, flag: bool) -> Self {
        self.use_zkos = flag;
        self
    }

    /// Build the SystemContracts instance.
    ///
    /// This method will panic if the `system_contracts_options` is not provided.
    /// For the protocol version, if none is provided, the latest version is used.
    pub fn build(self) -> SystemContracts {
        let options = self
            .system_contracts_options
            .expect("SystemContractsOptions must be provided");
        let protocol_version = self
            .protocol_version
            .unwrap_or_else(ProtocolVersionId::latest);

        tracing::debug!(
            %protocol_version, use_evm_emulator = self.use_evm_emulator, use_zkos = self.use_zkos,
            "Building SystemContracts"
        );

        SystemContracts::from_options(
            options,
            self.system_contracts_path,
            protocol_version,
            self.use_evm_emulator,
            self.use_zkos,
        )
    }
}

/// Holds the system contracts (and bootloader) that are used by the in-memory node.
#[derive(Debug, Clone)]
pub struct SystemContracts {
    pub protocol_version: ProtocolVersionId,
    baseline_contracts: BaseSystemContracts,
    playground_contracts: BaseSystemContracts,
    fee_estimate_contracts: BaseSystemContracts,
    baseline_impersonating_contracts: BaseSystemContracts,
    fee_estimate_impersonating_contracts: BaseSystemContracts,
    use_evm_emulator: bool,
    // For now, store the zkos switch flag here.
    // Long term, we should probably refactor this code, and add another struct ('System')
    // that would hold separate things for ZKOS and for EraVM. (but that's too early for now).
    pub zkos_config: ZKOSConfig,
}

impl SystemContracts {
    /// Creates a builder for SystemContracts
    pub fn builder() -> SystemContractsBuilder {
        SystemContractsBuilder::new()
    }

    /// Creates the SystemContracts that use the complied contracts from ZKSYNC_HOME path.
    /// These are loaded at binary runtime.
    pub fn from_options(
        options: SystemContractsOptions,
        system_contracts_path: Option<PathBuf>,
        protocol_version: ProtocolVersionId,
        use_evm_emulator: bool,
        zkos_config: ZKOSConfig,
    ) -> Self {
        tracing::info!(
            %protocol_version,
            use_evm_emulator,
            use_zkos,
            "initializing system contracts"
        );
        let path = system_contracts_path.unwrap_or_else(|| SystemContractsRepo::default().root);
        Self {
            protocol_version,
            baseline_contracts: baseline_contracts(
                options,
                protocol_version,
                use_evm_emulator,
                &path,
            ),
            playground_contracts: playground(options, protocol_version, use_evm_emulator, &path),
            fee_estimate_contracts: fee_estimate_contracts(
                options,
                protocol_version,
                use_evm_emulator,
                &path,
            ),
            baseline_impersonating_contracts: baseline_impersonating_contracts(
                options,
                protocol_version,
                use_evm_emulator,
                &path,
            ),
            fee_estimate_impersonating_contracts: fee_estimate_impersonating_contracts(
                options,
                protocol_version,
                use_evm_emulator,
                &path,
            ),
            use_evm_emulator,
            zkos_config,
        }
    }

    /// Whether it accepts the transactions that have 'null' as target.
    /// This is used only when EVM emulator is enabled, or we're running in zkos mode.
    pub fn allow_no_target(&self) -> bool {
        self.zkos_config.use_zkos || self.use_evm_emulator
    }

    pub fn use_zkos(&self) -> bool {
        self.zkos_config.use_zkos
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

    pub fn system_contracts_for_initiator(
        &self,
        impersonation: &ImpersonationManager,
        initiator: &Address,
    ) -> BaseSystemContracts {
        if impersonation.is_impersonating(initiator) {
            tracing::info!("Executing tx from impersonated account {initiator:?}");
            self.contracts(TxExecutionMode::VerifyExecute, true).clone()
        } else {
            self.contracts(TxExecutionMode::VerifyExecute, false)
                .clone()
        }
    }
}

/// Creates BaseSystemContracts object with a specific bootloader.
fn bsc_load_with_bootloader(
    bootloader_bytecode: Vec<u8>,
    options: SystemContractsOptions,
    protocol_version: ProtocolVersionId,
    use_evm_emulator: bool,
    system_contracts_path: &Path,
) -> BaseSystemContracts {
    let repo = system_contracts_repo(system_contracts_path);
    let hash = BytecodeHash::for_bytecode(&bootloader_bytecode);

    let bootloader = SystemContractCode {
        code: bootloader_bytecode,
        hash: hash.value(),
    };

    let aa_bytecode = match options {
        SystemContractsOptions::BuiltIn => {
            load_builtin_contract(protocol_version, "DefaultAccount")
        }
        SystemContractsOptions::Local => {
            repo.read_sys_contract_bytecode("", "DefaultAccount", None, ContractLanguage::Sol)
        }
        SystemContractsOptions::BuiltInWithoutSecurity => {
            load_builtin_contract(protocol_version, "DefaultAccountNoSecurity")
        }
    };

    let aa_hash = BytecodeHash::for_bytecode(&aa_bytecode);
    let default_aa = SystemContractCode {
        code: aa_bytecode,
        hash: aa_hash.value(),
    };

    let evm_emulator = if use_evm_emulator {
        let evm_emulator_bytecode = match options {
            SystemContractsOptions::Local => {
                read_sys_contract_bytecode("", "EvmEmulator", ContractLanguage::Yul)
            }
            SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
                load_builtin_contract(protocol_version, "EvmEmulator")
            }
        };
        let evm_emulator_hash = BytecodeHash::for_bytecode(&evm_emulator_bytecode);
        Some(SystemContractCode {
            code: evm_emulator_bytecode,
            hash: evm_emulator_hash.value(),
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
fn playground(
    options: SystemContractsOptions,
    protocol_version: ProtocolVersionId,
    use_evm_emulator: bool,
    system_contracts_path: &Path,
) -> BaseSystemContracts {
    let repo = system_contracts_repo(system_contracts_path);
    let bootloader_bytecode = match options {
        SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
            load_builtin_contract(protocol_version, "playground_batch")
        }
        SystemContractsOptions::Local => repo.read_sys_contract_bytecode(
            "bootloader",
            "playground_batch",
            Some("Bootloader"),
            ContractLanguage::Yul,
        ),
    };

    bsc_load_with_bootloader(
        bootloader_bytecode,
        options,
        protocol_version,
        use_evm_emulator,
        system_contracts_path,
    )
}

/// Returns the system contracts for fee estimation.
///
/// # Returns
///
/// A `BaseSystemContracts` struct containing the system contracts used for handling 'eth_estimateGas'.
/// It sets ENSURE_RETURNED_MAGIC to 0 and BOOTLOADER_TYPE to 'playground_block'
fn fee_estimate_contracts(
    options: SystemContractsOptions,
    protocol_version: ProtocolVersionId,
    use_evm_emulator: bool,
    system_contracts_path: &Path,
) -> BaseSystemContracts {
    let repo = system_contracts_repo(system_contracts_path);
    let bootloader_bytecode = match options {
        SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
            load_builtin_contract(protocol_version, "fee_estimate")
        }
        SystemContractsOptions::Local => repo.read_sys_contract_bytecode(
            "bootloader",
            "fee_estimate",
            Some("Bootloader"),
            ContractLanguage::Yul,
        ),
    };

    bsc_load_with_bootloader(
        bootloader_bytecode,
        options,
        protocol_version,
        use_evm_emulator,
        system_contracts_path,
    )
}

fn fee_estimate_impersonating_contracts(
    options: SystemContractsOptions,
    protocol_version: ProtocolVersionId,
    use_evm_emulator: bool,
    system_contracts_path: &Path,
) -> BaseSystemContracts {
    let repo = system_contracts_repo(system_contracts_path);
    let bootloader_bytecode = match options {
        SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
            load_builtin_contract(protocol_version, "fee_estimate_impersonating")
        }
        // Account impersonating is not supported with the local contracts
        SystemContractsOptions::Local => repo.read_sys_contract_bytecode(
            "bootloader",
            "fee_estimate",
            Some("Bootloader"),
            ContractLanguage::Yul,
        ),
    };

    bsc_load_with_bootloader(
        bootloader_bytecode,
        options,
        protocol_version,
        use_evm_emulator,
        system_contracts_path,
    )
}

fn baseline_contracts(
    options: SystemContractsOptions,
    protocol_version: ProtocolVersionId,
    use_evm_emulator: bool,
    system_contracts_path: &Path,
) -> BaseSystemContracts {
    let repo = system_contracts_repo(system_contracts_path);
    let bootloader_bytecode = match options {
        SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
            load_builtin_contract(protocol_version, "proved_batch")
        }
        SystemContractsOptions::Local => repo.read_sys_contract_bytecode(
            "bootloader",
            "proved_batch",
            Some("Bootloader"),
            ContractLanguage::Yul,
        ),
    };
    bsc_load_with_bootloader(
        bootloader_bytecode,
        options,
        protocol_version,
        use_evm_emulator,
        system_contracts_path,
    )
}

fn baseline_impersonating_contracts(
    options: SystemContractsOptions,
    protocol_version: ProtocolVersionId,
    use_evm_emulator: bool,
    system_contracts_path: &Path,
) -> BaseSystemContracts {
    let repo = system_contracts_repo(system_contracts_path);
    let bootloader_bytecode = match options {
        SystemContractsOptions::BuiltIn | SystemContractsOptions::BuiltInWithoutSecurity => {
            load_builtin_contract(protocol_version, "proved_batch_impersonating")
        }
        // Account impersonating is not supported with the local contracts
        SystemContractsOptions::Local => repo.read_sys_contract_bytecode(
            "bootloader",
            "proved_batch",
            Some("Bootloader"),
            ContractLanguage::Yul,
        ),
    };
    bsc_load_with_bootloader(
        bootloader_bytecode,
        options,
        protocol_version,
        use_evm_emulator,
        system_contracts_path,
    )
}

fn system_contracts_repo(root: &Path) -> SystemContractsRepo {
    SystemContractsRepo {
        root: root.to_path_buf(),
    }
}
