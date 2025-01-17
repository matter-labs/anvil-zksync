mod namespaces;

pub use namespaces::{
    AnvilNamespaceServer, ConfigNamespaceServer, EthTestNamespaceServer, EvmNamespaceServer,
    ZKOSNamespaceServer,
};

// Re-export available namespaces from zksync-era
pub use zksync_web3_decl::namespaces::{
    DebugNamespaceServer, EthNamespaceServer, NetNamespaceServer, Web3NamespaceServer,
    ZksNamespaceServer,
};
