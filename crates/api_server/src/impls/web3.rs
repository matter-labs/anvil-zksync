use anvil_zksync_api_decl::Web3NamespaceServer;
use jsonrpsee::core::RpcResult;

pub struct Web3Namespace;

impl Web3NamespaceServer for Web3Namespace {
    fn client_version(&self) -> RpcResult<String> {
        Ok("zkSync/v2.0".to_string())
    }
}
