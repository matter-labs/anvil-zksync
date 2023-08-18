//! zkSync Era In-Memory Node
//!
//! The `era-test-node` crate provides an in-memory node designed primarily for local testing.
//! It supports forking the state from other networks, making it a valuable tool for integration testing,
//! bootloader and system contract testing, and prototyping.
//!
//! ## Overview
//!
//! - **In-Memory Database**: The node uses an in-memory database for storing state information,
//!   and employs simplified hashmaps for tracking blocks and transactions.
//!
//! - **Forking**: In fork mode, the node fetches missing storage data from a remote source if not available locally.
//!
//! - **Remote Server Interaction**: The node can use the remote server (openchain) to resolve the ABI and topics
//!   to human-readable names.
//!
//! - **Local Testing**: Designed for local testing, this node is not intended for production use.
//!
//! ## Features
//!
//! - Fork the state of mainnet, testnet, or a custom network.
//! - Replay existing mainnet or testnet transactions.
//! - Use local bootloader and system contracts.
//! - Operate deterministically in non-fork mode.
//! - Start quickly with pre-configured 'rich' accounts.
//! - Resolve names of ABI functions and Events using openchain.
//!
//! ## Limitations
//!
//! - No communication between Layer 1 and Layer 2.
//! - Many APIs are not yet implemented.
//! - No support for accessing historical data.
//! - Only one transaction allowed per Layer 1 batch.
//! - Fixed values returned for zk Gas estimation.
//!
//! ## Usage
//!
//! To start the node, use the command `era_test_node run`. For more advanced functionalities like forking or
//! replaying transactions, refer to the official documentation.
//!
//! ## Contributions
//!
//! Contributions to improve `era-test-node` are welcome. Please refer to the contribution guidelines for more details.

use clap::{Parser, Subcommand};
use configuration_api::ConfigurationApiNamespaceT;
use fork::ForkDetails;
use zks::ZkMockNamespaceImpl;

mod configuration_api;
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

use crate::{configuration_api::ConfigurationApiNamespace, node::TEST_NODE_NETWORK_ID};
use zksync_core::api_server::web3::backend_jsonrpc::namespaces::{
    eth::EthNamespaceT, net::NetNamespaceT, zks::ZksNamespaceT,
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

async fn build_json_http(
    addr: SocketAddr,
    node: InMemoryNode,
    net: NetNamespace,
    config_api: ConfigurationApiNamespace,
    zks: ZkMockNamespaceImpl,
) -> tokio::task::JoinHandle<()> {
    let (sender, recv) = oneshot::channel::<()>();

    let io_handler = {
        let mut io = IoHandler::new();
        io.extend_with(node.to_delegate());
        io.extend_with(net.to_delegate());
        io.extend_with(config_api.to_delegate());
        io.extend_with(zks.to_delegate());

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

impl FromStr for ShowCalls {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "none" => Ok(ShowCalls::None),
            "user" => Ok(ShowCalls::User),
            "system" => Ok(ShowCalls::System),
            "all" => Ok(ShowCalls::All),
            _ => Err(()),
        }
    }
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
        let _ = node.apply_txs(transactions_to_replay);
    }

    println!("\nRich Accounts");
    println!("=============");
    for (index, wallet) in RICH_WALLETS.iter().enumerate() {
        let address = wallet.0;
        let private_key = wallet.1;
        node.set_rich_account(H160::from_str(address).unwrap());
        println!("Account #{}: {} (1000000000000 ETH)", index, address);
        println!("Private Key: {}\n", private_key);
    }

    let net = NetNamespace::new(L2ChainId(TEST_NODE_NETWORK_ID));
    let config_api = ConfigurationApiNamespace::new(node.get_inner());
    let zks = ZkMockNamespaceImpl::new(node.get_inner());

    let threads = build_json_http(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), opt.port),
        node,
        net,
        config_api,
        zks,
    )
    .await;

    println!("========================================");
    println!("  Node is ready at 127.0.0.1:{}", opt.port);
    println!("========================================");

    future::select_all(vec![threads]).await.0.unwrap();

    Ok(())
}
