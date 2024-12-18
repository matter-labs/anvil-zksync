use crate::{
    AnvilNamespace, ConfigNamespace, DebugNamespace, EthNamespace, EthTestNamespace, EvmNamespace,
    NetNamespace, Web3Namespace, ZksNamespace,
};
use anvil_zksync_api_decl::{
    AnvilNamespaceServer, ConfigNamespaceServer, DebugNamespaceServer, EthNamespaceServer,
    EthTestNamespaceServer, EvmNamespaceServer, NetNamespaceServer, Web3NamespaceServer,
    ZksNamespaceServer,
};
use anvil_zksync_core::node::InMemoryNode;
use http::Method;
use jsonrpsee::server::middleware::http::ProxyGetRequestLayer;
use jsonrpsee::server::{AlreadyStoppedError, RpcServiceBuilder, ServerBuilder, ServerHandle};
use jsonrpsee::RpcModule;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Debug, Default)]
pub struct NodeServerOptions {
    health_api_enabled: bool,
    cors_enabled: bool,
    allow_origin: AllowOrigin,
}

impl NodeServerOptions {
    pub fn enable_health_api(&mut self) {
        self.health_api_enabled = true;
    }

    pub fn enable_cors(&mut self) {
        self.health_api_enabled = true;
    }

    pub fn set_allow_origin(&mut self, allow_origin: AllowOrigin) {
        self.allow_origin = allow_origin;
    }

    pub fn to_builder(self, node: InMemoryNode) -> NodeServerBuilder {
        NodeServerBuilder::new(self, node)
    }
}

pub struct NodeServerBuilder {
    options: NodeServerOptions,
    rpc: RpcModule<()>,
    server_futs: Vec<Pin<Box<dyn Future<Output = ServerHandle>>>>,
}

impl NodeServerBuilder {
    fn new(options: NodeServerOptions, node: InMemoryNode) -> Self {
        let rpc = Self::default_rpc(node);
        Self {
            options,
            rpc,
            server_futs: Vec::new(),
        }
    }

    fn default_rpc(node: InMemoryNode) -> RpcModule<()> {
        let mut rpc = RpcModule::new(());
        rpc.merge(EthNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(EthTestNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(AnvilNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(EvmNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(DebugNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(NetNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(ConfigNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(ZksNamespace::new(node).into_rpc()).unwrap();
        rpc.merge(Web3Namespace.into_rpc()).unwrap();
        rpc
    }

    pub async fn serve(&mut self, addr: SocketAddr) {
        let cors_layers = tower::util::option_layer(self.options.cors_enabled.then(|| {
            // `CorsLayer` adds CORS-specific headers to responses but does not do filtering by itself.
            // CORS relies on browsers respecting server's access list response headers.
            // See [`tower_http::cors`](https://docs.rs/tower-http/latest/tower_http/cors/index.html)
            // for more details.
            CorsLayer::new()
                .allow_origin(self.options.allow_origin.clone())
                .allow_headers([http::header::CONTENT_TYPE])
                .allow_methods([Method::GET, Method::POST])
        }));
        let health_api_layer = tower::util::option_layer(
            self.options
                .health_api_enabled
                .then(|| ProxyGetRequestLayer::new("/health", "web3_clientVersion").unwrap()),
        );
        let server_builder = ServerBuilder::default()
            .http_only()
            .set_http_middleware(
                tower::ServiceBuilder::new()
                    .layer(cors_layers)
                    .layer(health_api_layer),
            )
            .set_rpc_middleware(RpcServiceBuilder::new().rpc_logger(100));

        let server = server_builder.build(addr).await.unwrap();
        let rpc = self.rpc.clone();
        self.server_futs
            .push(Box::pin(async move { server.start(rpc) }));
    }

    pub async fn run(self) -> NodeServerHandle {
        let handles = futures::future::join_all(self.server_futs).await;
        NodeServerHandle { handles }
    }
}

/// Node's server handle.
///
/// When all [`NodeServerHandle`]'s have been `dropped` or `stop` has been called
/// the server will be stopped.
#[derive(Debug, Clone)]
pub struct NodeServerHandle {
    handles: Vec<ServerHandle>,
}

impl NodeServerHandle {
    /// Tell the server to stop without waiting for the server to stop.
    pub fn stop(&self) -> Result<(), AlreadyStoppedError> {
        self.handles.iter().map(|handle| handle.stop()).collect()
    }

    /// Wait for the server to stop.
    pub async fn stopped(self) {
        futures::future::join_all(self.handles.into_iter().map(|handle| handle.stopped())).await;
    }
}
