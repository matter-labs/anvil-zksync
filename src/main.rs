use clap::{Parser, Subcommand};
use fork::ForkDetails;
use zks::ZkMockNamespaceImpl;

mod console_log;
mod deps;
mod fork;
mod formatter;
mod node;
mod resolver;
mod utils;
mod zks;

use core::fmt::Display;
use node::InMemoryNode;
use zksync_core::api_server::web3::namespaces::NetNamespace;

use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};

use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use futures::{
    channel::oneshot,
    future::{self},
    FutureExt,
};
use jsonrpc_core::IoHandler;
use zksync_basic_types::{L2ChainId, H160, H256};

use crate::node::TEST_NODE_NETWORK_ID;
use zksync_core::api_server::web3::backend_jsonrpc::namespaces::{
    eth::EthNamespaceT, net::NetNamespaceT, zks::ZksNamespaceT,
};

/// List of wallets (address, private key) that we seed with tokens at start.
pub const RICH_WALLETS: [(&str, &str); 4] = [
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
];

async fn build_json_http(
    addr: SocketAddr,
    node: InMemoryNode,
    net: NetNamespace,
) -> tokio::task::JoinHandle<()> {
    let (sender, recv) = oneshot::channel::<()>();

    let io_handler = {
        let mut io = IoHandler::new();
        io.extend_with(node.to_delegate());
        io.extend_with(net.to_delegate());
        io.extend_with(ZkMockNamespaceImpl.to_delegate());

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

    #[arg(long)]
    /// If true, the tool will try to contact openchain to resolve the ABI & topic names.
    /// It will make debug log more readable, but will decrease the performance.
    resolve_hashes: bool,

    #[arg(long)]
    /// If true, will load the locally compiled system contracts (useful when doing changes to system contracts or bootloader)
    dev_use_local_contracts: bool,
}

#[derive(Debug, Parser, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum ShowCalls {
    None,
    User,
    System,
    All,
}

impl Display for ShowCalls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
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
    let filter = EnvFilter::from_default_env();

    if opt.dev_use_local_contracts {
        if let Some(path) = env::var_os("ZKSYNC_HOME") {
            println!("+++++ Reading local contracts from {:?} +++++", path);
        }
    }

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .with_env_filter(filter)
        .finish();

    // Initialize the subscriber
    tracing::subscriber::set_global_default(subscriber).expect("failed to set tracing subscriber");

    let fork_details = match &opt.command {
        Command::Run => None,
        Command::Fork(fork) => Some(ForkDetails::from_network(&fork.network, fork.fork_at).await),
        Command::ReplayTx(replay_tx) => {
            Some(ForkDetails::from_network_tx(&replay_tx.network, replay_tx.tx).await)
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

    let node = InMemoryNode::new(
        fork_details,
        opt.show_calls,
        opt.resolve_hashes,
        opt.dev_use_local_contracts,
    );

    if !transactions_to_replay.is_empty() {
        node.apply_txs(transactions_to_replay);
    }

    println!("\nRich Accounts");
    println!("=============");
    for (index, wallet) in RICH_WALLETS.iter().enumerate() {
        let address = wallet.0;
        let private_key = wallet.1;
        node.set_rich_account(H160::from_str(address).unwrap());
        println!("Account #{}: {} (10000 ETH)", index, address);
        println!("Private Key: {}\n", private_key);
    }

    let net = NetNamespace::new(L2ChainId(TEST_NODE_NETWORK_ID));

    let threads = build_json_http(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), opt.port),
        node,
        net,
    )
    .await;

    println!("========================================");
    println!("  Node is ready at 127.0.0.1:{}", opt.port);
    println!("========================================");

    future::select_all(vec![threads]).await.0.unwrap();

    Ok(())
}
