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
use jsonrpsee::server::{RpcServiceBuilder, ServerBuilder, ServerHandle};
use jsonrpsee::RpcModule;
use std::net::SocketAddr;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Clone)]
pub struct NodeServerBuilder {
    node: InMemoryNode,
    health_api_enabled: bool,
    cors_enabled: bool,
    allow_origin: AllowOrigin,
}

impl NodeServerBuilder {
    pub fn new(node: InMemoryNode, allow_origin: AllowOrigin) -> Self {
        Self {
            node,
            health_api_enabled: false,
            cors_enabled: false,
            allow_origin,
        }
    }

    pub fn enable_health_api(&mut self) {
        self.health_api_enabled = true;
    }

    pub fn enable_cors(&mut self) {
        self.cors_enabled = true;
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

    pub async fn build(self, addr: SocketAddr) -> NodeServer {
        let cors_layers = tower::util::option_layer(self.cors_enabled.then(|| {
            // `CorsLayer` adds CORS-specific headers to responses but does not do filtering by itself.
            // CORS relies on browsers respecting server's access list response headers.
            // See [`tower_http::cors`](https://docs.rs/tower-http/latest/tower_http/cors/index.html)
            // for more details.
            CorsLayer::new()
                .allow_origin(self.allow_origin.clone())
                .allow_headers([http::header::CONTENT_TYPE])
                .allow_methods([Method::GET, Method::POST])
        }));
        let health_api_layer = tower::util::option_layer(
            self.health_api_enabled
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
        let rpc = Self::default_rpc(self.node);
        // `jsonrpsee` does `tokio::spawn` within `start` method, so we cannot invoke it here, as this method
        // should only build the server. This way we delay the launch until the `NodeServer::run` is invoked.
        NodeServer(Box::new(move || server.start(rpc)))
    }
}

pub struct NodeServer(Box<dyn FnOnce() -> ServerHandle>);

impl NodeServer {
    /// Start responding to connections requests.
    ///
    /// This will run on the tokio runtime until the server is stopped or the `ServerHandle` is dropped.
    ///
    /// See [`ServerHandle`](https://docs.rs/jsonrpsee-server/latest/jsonrpsee_server/struct.ServerHandle.html) docs for more details.
    pub fn run(self) -> ServerHandle {
        self.0()
    }
}
