//! This module encapsulates mutable parts of the system and provides read-only views on various
//! components of the system's state (e.g. time, storage, blocks). It is still possible to mutate
//! the state outside of this module but only through [`InMemoryNodeInner`]'s public high-level
//! methods.
//!
//! The idea behind this is being able to read current time to answer API requests while a lock on
//! [`InMemoryNodeInner`] is being held for block production. At the same time it is impossible to
//! advance the time without holding a lock to [`InMemoryNodeInner`].
//!
//! FIXME: The above is not 100% true yet (there are some internal parts of InMemoryNodeInner that
//!        are available outside of this module)
pub mod blockchain;
pub mod fork;
mod fork_storage;
mod in_memory_inner;
pub mod node_executor;
pub mod storage;
pub mod time;
mod vm_runner;

pub use fork_storage::{SerializableForkStorage, SerializableStorage};
pub use in_memory_inner::InMemoryNodeInner;

use crate::filters::EthFilters;
use crate::node::blockchain::Blockchain;
use crate::node::inner::storage::ReadStorageDyn;
use crate::node::inner::vm_runner::VmRunner;
use crate::node::keys::StorageKeyLayout;
use crate::node::{ImpersonationManager, TestNodeFeeInputProvider};
use crate::system_contracts::SystemContracts;
use anvil_zksync_config::constants::NON_FORK_FIRST_BLOCK_TIMESTAMP;
use anvil_zksync_config::TestNodeConfig;
use blockchain::ReadBlockchain;
use fork::{Fork, ForkClient, ForkSource};
use fork_storage::ForkStorage;
use std::sync::Arc;
use time::{ReadTime, Time};
use tokio::sync::RwLock;

impl InMemoryNodeInner {
    // TODO: Bake in Arc<RwLock<_>> into the struct itself
    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        fork_client_opt: Option<ForkClient>,
        fee_input_provider: TestNodeFeeInputProvider,
        filters: Arc<RwLock<EthFilters>>,
        config: TestNodeConfig,
        impersonation: ImpersonationManager,
        system_contracts: SystemContracts,
        storage_key_layout: StorageKeyLayout,
        generate_system_logs: bool,
    ) -> (
        Arc<RwLock<Self>>,
        Box<dyn ReadStorageDyn>,
        Box<dyn ReadBlockchain>,
        Box<dyn ReadTime>,
        Box<dyn ForkSource>,
        VmRunner,
    ) {
        // TODO: We wouldn't have to clone cache config here if there was a proper per-component
        //       config separation.
        let fork = Fork::new(fork_client_opt, config.cache_config.clone());
        let fork_details = fork.details();
        let time = Time::new(
            fork_details
                .as_ref()
                .map(|fd| fd.block_timestamp)
                .unwrap_or(NON_FORK_FIRST_BLOCK_TIMESTAMP),
        );
        let blockchain = Blockchain::new(
            system_contracts.protocol_version,
            fork_details.as_ref(),
            config.genesis.as_ref(),
            config.genesis_timestamp,
        );
        // TODO: Create read-only/mutable versions of `ForkStorage` like `blockchain` and `time` above
        let fork_storage = ForkStorage::new(
            fork.clone(),
            config.system_contracts_options,
            system_contracts.protocol_version,
            config.chain_id,
            config.system_contracts_path.as_deref(),
        );
        let vm_runner = VmRunner::new(
            time.clone(),
            fork_storage.clone(),
            system_contracts.clone(),
            generate_system_logs,
            config.is_bytecode_compression_enforced(),
        );

        let node_inner = InMemoryNodeInner::new(
            blockchain.clone(),
            time.clone(),
            fork_storage.clone(),
            fork.clone(),
            fee_input_provider.clone(),
            filters,
            config.clone(),
            impersonation.clone(),
            system_contracts.clone(),
            storage_key_layout,
        );

        (
            Arc::new(RwLock::new(node_inner)),
            Box::new(fork_storage),
            Box::new(blockchain),
            Box::new(time),
            Box::new(fork),
            vm_runner,
        )
    }
}
