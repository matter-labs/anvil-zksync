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
mod in_memory_inner;
pub mod node_executor;
pub mod time;

pub use in_memory_inner::{InMemoryNodeInner, TxExecutionOutput};

use crate::filters::EthFilters;
use crate::node::blockchain::Blockchain;
use crate::node::{ImpersonationManager, TestNodeFeeInputProvider};
use crate::system_contracts::SystemContracts;
use anvil_zksync_config::constants::NON_FORK_FIRST_BLOCK_TIMESTAMP;
use anvil_zksync_config::TestNodeConfig;
use blockchain::ReadBlockchain;
use fork::{ForkDetails, ForkStorage};
use std::sync::Arc;
use time::{ReadTime, Time};
use tokio::sync::RwLock;

impl InMemoryNodeInner {
    // TODO: Bake in Arc<RwLock<_>> into the struct itself
    #[allow(clippy::type_complexity)]
    pub fn init(
        fork: Option<ForkDetails>,
        fee_input_provider: TestNodeFeeInputProvider,
        filters: Arc<RwLock<EthFilters>>,
        config: TestNodeConfig,
        impersonation: ImpersonationManager,
        system_contracts: SystemContracts,
    ) -> (
        Arc<RwLock<Self>>,
        ForkStorage,
        Arc<dyn ReadBlockchain>,
        Arc<dyn ReadTime>,
    ) {
        let time = Time::new(
            fork.as_ref()
                .map(|f| f.block_timestamp)
                .unwrap_or(NON_FORK_FIRST_BLOCK_TIMESTAMP),
        );
        let blockchain = Blockchain::new(
            fork.as_ref(),
            config.genesis.as_ref(),
            config.genesis_timestamp,
        );
        // TODO: Create read-only/mutable versions of `ForkStorage` like `blockchain` and `time` above
        let fork_storage = ForkStorage::new(
            fork,
            &config.system_contracts_options,
            config.use_evm_emulator,
            config.chain_id,
        );

        let node_inner = InMemoryNodeInner::new(
            blockchain.clone(),
            time.clone(),
            fork_storage.clone(),
            fee_input_provider.clone(),
            filters,
            config.clone(),
            impersonation.clone(),
            system_contracts.clone(),
        );

        (
            Arc::new(RwLock::new(node_inner)),
            fork_storage,
            Arc::new(blockchain),
            Arc::new(time),
        )
    }
}
