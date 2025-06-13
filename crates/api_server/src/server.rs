use crate::{
    AnvilNamespace, AnvilZksNamespace, ConfigNamespace, DebugNamespace, EthNamespace,
    EthTestNamespace, EvmNamespace, NetNamespace, Web3Namespace, ZksNamespace,
};
use anvil_zksync_api_decl::{
    AnvilNamespaceServer, AnvilZksNamespaceServer, ConfigNamespaceServer, DebugNamespaceServer,
    EthNamespaceServer, EthTestNamespaceServer, EvmNamespaceServer, NetNamespaceServer,
    Web3NamespaceServer, ZksNamespaceServer,
};
use anvil_zksync_core::node::InMemoryNode;
use anvil_zksync_l1_sidecar::L1Sidecar;
use futures::future::BoxFuture;
use futures::FutureExt;
use http::Method;
use jsonrpsee::server::middleware::http::ProxyGetRequestLayer;
use jsonrpsee::server::middleware::rpc::RpcServiceT;
use jsonrpsee::server::{MethodResponse, RpcServiceBuilder, ServerBuilder, ServerHandle};
use jsonrpsee::types::Request;
use jsonrpsee::RpcModule;
use std::net::SocketAddr;
use tower_http::cors::{AllowOrigin, CorsLayer};
use zksync_telemetry::{get_telemetry, TelemetryProps};

#[derive(Clone)]
pub struct NodeServerBuilder {
    node: InMemoryNode,
    l1_sidecar: L1Sidecar,
    health_api_enabled: bool,
    cors_enabled: bool,
    allow_origin: AllowOrigin,
}

impl NodeServerBuilder {
    pub fn new(node: InMemoryNode, l1_sidecar: L1Sidecar, allow_origin: AllowOrigin) -> Self {
        Self {
            node,
            l1_sidecar,
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

    fn default_rpc(node: InMemoryNode, l1_sidecar: L1Sidecar) -> RpcModule<()> {
        let mut rpc = RpcModule::new(());
        rpc.merge(EthNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(EthTestNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(AnvilNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(AnvilZksNamespace::new(l1_sidecar.clone()).into_rpc())
            .unwrap();
        rpc.merge(EvmNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(DebugNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(NetNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(ConfigNamespace::new(node.clone()).into_rpc())
            .unwrap();
        rpc.merge(ZksNamespace::new(node, l1_sidecar).into_rpc())
            .unwrap();
        rpc.merge(Web3Namespace.into_rpc()).unwrap();
        rpc
    }

    pub async fn build(self, addr: SocketAddr) -> Result<NodeServer, String> {
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
            .set_rpc_middleware(RpcServiceBuilder::new().rpc_logger(100))
            .set_rpc_middleware(
                RpcServiceBuilder::new().layer_fn(|svc| TelemetryReporter { service: svc }),
            );

        match server_builder.build(addr).await {
            Ok(server) => {
                let local_addr = server.local_addr().unwrap();
                let rpc = Self::default_rpc(self.node, self.l1_sidecar);
                // `jsonrpsee` does `tokio::spawn` within `start` method, so we cannot invoke it here, as this method
                // should only build the server. This way we delay the launch until the `NodeServer::run` is invoked.
                Ok(NodeServer {
                    local_addr,
                    run_fn: Box::new(move || server.start(rpc)),
                })
            }
            Err(e) => Err(format!("Failed to bind to address {}: {}", addr, e)),
        }
    }
}

pub struct NodeServer {
    local_addr: SocketAddr,
    run_fn: Box<dyn FnOnce() -> ServerHandle>,
}

impl NodeServer {
    /// Returns the address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Start responding to connections requests.
    ///
    /// This will run on the tokio runtime until the server is stopped or the `ServerHandle` is dropped.
    ///
    /// See [`ServerHandle`](https://docs.rs/jsonrpsee-server/latest/jsonrpsee_server/struct.ServerHandle.html) docs for more details.
    pub fn run(self) -> ServerHandle {
        (self.run_fn)()
    }
}

#[derive(Clone)]
pub struct TelemetryReporter<S> {
    service: S,
}

impl<'a, S> RpcServiceT<'a> for TelemetryReporter<S>
where
    S: RpcServiceT<'a> + Send + Sync + Clone + 'static,
{
    type Future = BoxFuture<'a, MethodResponse>;

    fn call(&self, req: Request<'a>) -> Self::Future {
        let svc = self.service.clone();
        let telemetry_opt = get_telemetry();

        async move {
            if let Some(tel) = telemetry_opt {
                let method = req.method_name();
                if method.starts_with("anvil_") || method.starts_with("config_") {
                    let _ = tel
                        .track_event(
                            "rpc_call",
                            TelemetryProps::new().insert("method", Some(method)).take(),
                        )
                        .await;
                }
            }
            svc.call(req).await
        }
        .boxed()
    }
}
