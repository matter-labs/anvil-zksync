use crate::cache::CacheConfig;
use crate::node::{InMemoryNodeConfig, ShowGasDetails, ShowStorageLogs, ShowVMDetails};
use crate::observability::Observability;
use clap::{Parser, Subcommand, ValueEnum};
use fork::{ForkDetails, ForkSource};
use logging_middleware::LoggingMiddleware;
use node::ShowCalls;
use observability::LogLevel;
use tracing_subscriber::filter::LevelFilter;

mod bootloader_debug;
mod cache;
mod console_log;
mod deps;
mod filters;
mod fork;
mod formatter;
mod http_fork_source;
mod logging_middleware;
mod namespaces;
mod node;
pub mod observability;
mod resolver;
mod system_contracts;
mod testing;
mod utils;

use node::InMemoryNode;

use std::fs::File;
use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};

use futures::{
    channel::oneshot,
    future::{self},
    FutureExt,
};
use jsonrpc_core::MetaIoHandler;
use zksync_basic_types::{H160, H256};

use crate::namespaces::{
    ConfigurationApiNamespaceT, DebugNamespaceT, EthNamespaceT, EvmNamespaceT, HardhatNamespaceT,
    NetNamespaceT, ZksNamespaceT,
};

/// List of wallets (address, private key) that we seed with tokens at start.
pub const RICH_WALLETS: [(&str, &str); 10] = [
    (
        "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
        "0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110",
    ),
    (
        "0xa61464658AfeAf65CccaaFD3a512b69A83B77618",
        "0xac1e735be8536c6534bb4f17f06f6afc73b2b5ba84ac2cfb12f7461b20c0bbe3",
    ),
    (
        "0x0D43eB5B8a47bA8900d84AA36656c92024e9772e",
        "0xd293c684d884d56f8d6abd64fc76757d3664904e309a0645baf8522ab6366d9e",
    ),
    (
        "0xA13c10C0D5bd6f79041B9835c63f91de35A15883",
        "0x850683b40d4a740aa6e745f889a6fdc8327be76e122f5aba645a5b02d0248db8",
    ),
    (
        "0x8002cD98Cfb563492A6fB3E7C8243b7B9Ad4cc92",
        "0xf12e28c0eb1ef4ff90478f6805b68d63737b7f33abfa091601140805da450d93",
    ),
    (
        "0x4F9133D1d3F50011A6859807C837bdCB31Aaab13",
        "0xe667e57a9b8aaa6709e51ff7d093f1c5b73b63f9987e4ab4aa9a5c699e024ee8",
    ),
    (
        "0xbd29A1B981925B94eEc5c4F1125AF02a2Ec4d1cA",
        "0x28a574ab2de8a00364d5dd4b07c4f2f574ef7fcc2a86a197f65abaec836d1959",
    ),
    (
        "0xedB6F5B4aab3dD95C7806Af42881FF12BE7e9daa",
        "0x74d8b3a188f7260f67698eb44da07397a298df5427df681ef68c45b34b61f998",
    ),
    (
        "0xe706e60ab5Dc512C36A4646D719b889F398cbBcB",
        "0xbe79721778b48bcc679b78edac0ce48306a8578186ffcb9f2ee455ae6efeace1",
    ),
    (
        "0xE90E12261CCb0F3F7976Ae611A29e84a6A85f424",
        "0x3eb15da85647edd9a1159a4a13b9e7c56877c4eb33f614546d4db06a51868b1c",
    ),
];

#[allow(clippy::too_many_arguments)]
async fn build_json_http<
    S: std::marker::Sync + std::marker::Send + 'static + ForkSource + std::fmt::Debug + Clone,
>(
    addr: SocketAddr,
    log_level_filter: LevelFilter,
    node: InMemoryNode<S>,
) -> tokio::task::JoinHandle<()> {
    let (sender, recv) = oneshot::channel::<()>();

    let io_handler = {
        let mut io = MetaIoHandler::with_middleware(LoggingMiddleware::new(log_level_filter));

        io.extend_with(NetNamespaceT::to_delegate(node.clone()));
        io.extend_with(ConfigurationApiNamespaceT::to_delegate(node.clone()));
        io.extend_with(DebugNamespaceT::to_delegate(node.clone()));
        io.extend_with(EthNamespaceT::to_delegate(node.clone()));
        io.extend_with(EvmNamespaceT::to_delegate(node.clone()));
        io.extend_with(HardhatNamespaceT::to_delegate(node.clone()));
        io.extend_with(ZksNamespaceT::to_delegate(node));
        io
    };

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .build()
            .unwrap();

        let server = jsonrpc_http_server::ServerBuilder::new(io_handler)
            .threads(1)
            .event_loop_executor(runtime.handle().clone())
            .start_http(&addr)
            .unwrap();

        server.wait();
        let _ = sender;
    });

    tokio::spawn(recv.map(drop))
}

/// Cache type config for the node.
#[derive(ValueEnum, Debug, Clone)]
enum CacheType {
    None,
    Memory,
    Disk,
}

/// System contract options.
#[derive(ValueEnum, Debug, Clone)]
enum DevSystemContracts {
    BuiltIn,
    BuiltInNoVerify,
    Local,
}

#[derive(Debug, Parser)]
#[command(author = "Matter Labs", version, about = "Test Node", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long, default_value = "8011")]
    /// Port to listen on - default: 8011
    port: u16,
    #[arg(long, default_value = "none")]
    /// Show call debug information
    show_calls: ShowCalls,
    #[arg(long, default_value = "none")]
    /// Show storage log information
    show_storage_logs: ShowStorageLogs,
    #[arg(long, default_value = "none")]
    /// Show VM details information
    show_vm_details: ShowVMDetails,

    #[arg(long, default_value = "none")]
    /// Show Gas details information
    show_gas_details: ShowGasDetails,

    #[arg(long)]
    /// If true, the tool will try to contact openchain to resolve the ABI & topic names.
    /// It will make debug log more readable, but will decrease the performance.
    resolve_hashes: bool,

    /// Specifies the option for the system contracts (use compiled built-in with or without signature verification, or load locally).
    /// Default: built-in
    #[arg(long, default_value = "built-in")]
    dev_system_contracts: DevSystemContracts,

    /// Log filter level - default: info
    #[arg(long, default_value = "info")]
    log: LogLevel,

    /// Log file path - default: era_test_node.log
    #[arg(long, default_value = "era_test_node.log")]
    log_file_path: String,

    /// Cache type, can be one of `none`, `memory`, or `disk` - default: "disk"
    #[arg(long, default_value = "disk")]
    cache: CacheType,

    /// If true, will reset the local `disk` cache.
    #[arg(long)]
    reset_cache: bool,

    /// Cache directory location for `disk` cache - default: ".cache"
    #[arg(long, default_value = ".cache")]
    cache_dir: String,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Starts a new empty local network.
    #[command(name = "run")]
    Run,
    /// Starts a local network that is a fork of another network.
    #[command(name = "fork")]
    Fork(ForkArgs),
    /// Starts a local network that is a fork of another network, and replays a given TX on it.
    #[command(name = "replay_tx")]
    ReplayTx(ReplayArgs),
}

#[derive(Debug, Parser)]
struct ForkArgs {
    /// Whether to fork from existing network.
    /// If not set - will start a new network from genesis.
    /// If set - will try to fork a remote network. Possible values:
    ///  - mainnet
    ///  - testnet
    ///  - http://XXX:YY
    network: String,
    #[arg(long)]
    // Fork at a given L2 miniblock height.
    // If not set - will use the current finalized block from the network.
    fork_at: Option<u64>,
}
#[derive(Debug, Parser)]
struct ReplayArgs {
    /// Whether to fork from existing network.
    /// If not set - will start a new network from genesis.
    /// If set - will try to fork a remote network. Possible values:
    ///  - mainnet
    ///  - testnet
    ///  - http://XXX:YY
    network: String,
    /// Transaction hash to replay.
    tx: H256,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Cli::parse();
    let log_level_filter = LevelFilter::from(opt.log);
    let log_file = File::create(opt.log_file_path)?;

    // Initialize the tracing subscriber
    let observability =
        Observability::init(String::from("era_test_node"), log_level_filter, log_file)?;

    if matches!(opt.dev_system_contracts, DevSystemContracts::Local) {
        if let Some(path) = env::var_os("ZKSYNC_HOME") {
            tracing::info!("+++++ Reading local contracts from {:?} +++++", path);
        }
    }
    let cache_config = match opt.cache {
        CacheType::None => CacheConfig::None,
        CacheType::Memory => CacheConfig::Memory,
        CacheType::Disk => CacheConfig::Disk {
            dir: opt.cache_dir,
            reset: opt.reset_cache,
        },
    };

    let fork_details = match &opt.command {
        Command::Run => None,
        Command::Fork(fork) => {
            Some(ForkDetails::from_network(&fork.network, fork.fork_at, cache_config).await)
        }
        Command::ReplayTx(replay_tx) => {
            Some(ForkDetails::from_network_tx(&replay_tx.network, replay_tx.tx, cache_config).await)
        }
    };

    // If we're replaying the transaction, we need to sync to the previous block
    // and then replay all the transactions that happened in
    let transactions_to_replay = if let Command::ReplayTx(replay_tx) = &opt.command {
        fork_details
            .as_ref()
            .unwrap()
            .get_earlier_transactions_in_same_block(replay_tx.tx)
            .await
    } else {
        vec![]
    };
    let system_contracts_options = match opt.dev_system_contracts {
        DevSystemContracts::BuiltIn => system_contracts::Options::BuiltIn,
        DevSystemContracts::BuiltInNoVerify => system_contracts::Options::BuiltInWithoutSecurity,
        DevSystemContracts::Local => system_contracts::Options::Local,
    };

    let node = InMemoryNode::new(
        fork_details,
        Some(observability),
        InMemoryNodeConfig {
            show_calls: opt.show_calls,
            show_storage_logs: opt.show_storage_logs,
            show_vm_details: opt.show_vm_details,
            show_gas_details: opt.show_gas_details,
            resolve_hashes: opt.resolve_hashes,
            system_contracts_options,
        },
    );

    if !transactions_to_replay.is_empty() {
        let _ = node.apply_txs(transactions_to_replay);
    }

    tracing::info!("Rich Accounts");
    tracing::info!("=============");
    for (index, wallet) in RICH_WALLETS.iter().enumerate() {
        let address = wallet.0;
        let private_key = wallet.1;
        node.set_rich_account(H160::from_str(address).unwrap());
        tracing::info!("Account #{}: {} (1_000_000_000_000 ETH)", index, address);
        tracing::info!("Private Key: {}", private_key);
        tracing::info!("");
    }

    let threads = build_json_http(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), opt.port),
        log_level_filter,
        node,
    )
    .await;

    tracing::info!("========================================");
    tracing::info!("  Node is ready at 127.0.0.1:{}", opt.port);
    tracing::info!("========================================");

    future::select_all(vec![threads]).await.0.unwrap();

    Ok(())
}
