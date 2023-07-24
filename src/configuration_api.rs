// Built-in uses
use std::sync::{Arc, RwLock};

// External uses
use jsonrpc_core::Result;
use jsonrpc_derive::rpc;

// Workspace uses

// Local uses
use crate::{node::InMemoryNodeInner, ShowCalls};

pub struct ConfigurationApiNamespace {
    node: Arc<RwLock<InMemoryNodeInner>>,
}

impl ConfigurationApiNamespace {
    pub fn new(node: Arc<RwLock<InMemoryNodeInner>>) -> Self {
        Self { node }
    }
}

#[rpc]
pub trait ConfigurationApiNamespaceT {
    #[rpc(name = "config_getShowCalls", returns = "String")]
    fn config_get_show_calls(&self) -> Result<String>;
    
    #[rpc(name = "config_setShowCalls", returns = "String")]
    fn config_set_show_calls(&self, value: String) -> Result<String>;

    #[rpc(name = "config_setResolveHashes", returns = "bool")]
    fn config_get_resolve_hashes(&self, value: bool) -> Result<bool>;
}

impl ConfigurationApiNamespaceT for ConfigurationApiNamespace {
    fn config_get_show_calls(&self) -> Result<String> {
        let reader = self.node.read().unwrap();
        Ok(reader.show_calls.to_string())
    }

    fn config_set_show_calls(&self, value: String) -> Result<String> {
        let show_calls = ShowCalls::from_str(&value);
        
        match show_calls {
            Some(show_calls) => {
                let mut inner = self.node.write().unwrap();
                inner.show_calls = show_calls;
                Ok(inner.show_calls.to_string())
            },
            None => {
                let reader = self.node.read().unwrap();
                Ok(reader.show_calls.to_string())
            }
        }
    }

    fn config_get_resolve_hashes(&self, value: bool) -> Result<bool> {
        let mut inner = self.node.write().unwrap();
        inner.resolve_hashes = value;
        Ok(inner.resolve_hashes)
    }
}
