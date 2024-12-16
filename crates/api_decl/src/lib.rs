mod namespaces;

pub use namespaces::{AnvilNamespaceServer, ConfigNamespaceServer, EvmNamespaceServer};

// Re-export available namespaces from zksync-era
pub use zksync_web3_decl::namespaces::{
    DebugNamespaceServer, NetNamespaceServer, Web3NamespaceServer,
};
