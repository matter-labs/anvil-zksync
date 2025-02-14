////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Attribution: File adapted from the `evm` crate for zksync usage                                        //
//                                                                                                        //
// Full credit goes to its authors. See the original implementation here:                                 //
// https://github.com/foundry-rs/foundry/blob/master/crates/evm/traces/src/idenitfier/signatures.rs.      //
//                                                                                                        //
// Note: These methods are used under the terms of the original project's license.                        //
////////////////////////////////////////////////////////////////////////////////////////////////////////////

use crate::resolver::{SelectorType, SignEthClient};
use crate::trace::abi_utils::{get_error, get_event, get_func};
use crate::utils::{read_json_file, write_json_file};
use alloy::json_abi::{Error, Event, Function};
use alloy::primitives::hex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::RwLock;

pub type SingleSignaturesIdentifier = Arc<RwLock<SignaturesIdentifier>>;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CachedSignatures {
    pub errors: BTreeMap<String, String>,
    pub events: BTreeMap<String, String>,
    pub functions: BTreeMap<String, String>,
}

impl CachedSignatures {
    pub fn load(cache_path: PathBuf) -> Self {
        let path = cache_path.join("signatures");
        if path.is_file() {
            read_json_file(&path)
                .map_err(
                    |err| tracing::warn!(target: "trace::signatures", ?path, ?err, "failed to read cache file"),
                )
                .unwrap_or_default()
        } else {
            if let Err(err) = std::fs::create_dir_all(cache_path) {
                tracing::warn!(target: "trace::signatures", "could not create signatures cache dir: {:?}", err);
            }
            Self::default()
        }
    }
}
/// An identifier that tries to identify functions and events using signatures found at
/// `https://openchain.xyz` or a local cache.
#[derive(Debug)]
pub struct SignaturesIdentifier {
    /// Cached selectors for functions, events and custom errors.
    cached: CachedSignatures,
    /// Location where to save `CachedSignatures`.
    cached_path: Option<PathBuf>,
    /// Selectors that were unavailable during the session.
    unavailable: HashMap<String, String>,
    /// The OpenChain client to fetch signatures from.
    client: Option<SignEthClient>,
}

impl SignaturesIdentifier {
    pub fn new(
        cache_path: Option<PathBuf>,
        offline: bool,
    ) -> eyre::Result<SingleSignaturesIdentifier> {
        let client = if !offline {
            Some(SignEthClient::new()?)
        } else {
            None
        };

        let identifier = if let Some(cache_path) = cache_path {
            let path = cache_path.join("signatures");
            tracing::trace!(target: "trace::signatures", ?path, "reading signature cache");
            let cached = CachedSignatures::load(cache_path);
            Self {
                cached,
                cached_path: Some(path),
                unavailable: HashMap::default(),
                client,
            }
        } else {
            Self {
                cached: Default::default(),
                cached_path: None,
                unavailable: HashMap::default(),
                client,
            }
        };

        Ok(Arc::new(RwLock::new(identifier)))
    }

    pub fn save(&self) {
        if let Some(cached_path) = &self.cached_path {
            if let Some(parent) = cached_path.parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    tracing::warn!(target: "trace::signatures", ?parent, ?err, "failed to create cache");
                }
            }
            if let Err(err) = write_json_file(cached_path, &self.cached) {
                tracing::warn!(target: "trace::signatures", ?cached_path, ?err, "failed to flush signature cache");
            } else {
                tracing::trace!(target: "trace::signatures", ?cached_path, "flushed signature cache")
            }
        }
    }
}

impl SignaturesIdentifier {
    async fn identify<T>(
        &mut self,
        selector_type: SelectorType,
        identifiers: impl IntoIterator<Item = impl AsRef<[u8]>>,
        get_type: impl Fn(&str) -> eyre::Result<T>,
    ) -> Vec<Option<T>> {
        let cache = match selector_type {
            SelectorType::Function => &mut self.cached.functions,
            SelectorType::Event => &mut self.cached.events,
            SelectorType::Error => &mut self.cached.errors,
        };

        let hex_identifiers: Vec<String> =
            identifiers.into_iter().map(hex::encode_prefixed).collect();

        if let Some(client) = &self.client {
            let query: Vec<_> = hex_identifiers
                .iter()
                .filter(|v| !cache.contains_key(v.as_str()))
                .filter(|v| !self.unavailable.contains_key(v.as_str()))
                .collect();

            if let Ok(res) = client.decode_selectors(selector_type, query.clone()).await {
                for (hex_id, selector_result) in query.into_iter().zip(res.into_iter()) {
                    let mut found = false;
                    if let Some(decoded_results) = selector_result {
                        if let Some(decoded_result) = decoded_results.into_iter().next() {
                            cache.insert(hex_id.clone(), decoded_result);
                            found = true;
                        }
                    }
                    if !found {
                        self.unavailable.insert(hex_id.clone(), hex_id.clone());
                    }
                }
            }
        }

        hex_identifiers
            .iter()
            .map(|v| cache.get(v).and_then(|v| get_type(v).ok()))
            .collect()
    }

    /// Identifies `Function`s from its cache or `https://api.openchain.xyz`
    pub async fn identify_functions(
        &mut self,
        identifiers: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Vec<Option<Function>> {
        self.identify(SelectorType::Function, identifiers, get_func)
            .await
    }

    /// Identifies `Function` from its cache or `https://api.openchain.xyz`
    pub async fn identify_function(&mut self, identifier: &[u8]) -> Option<Function> {
        self.identify_functions(&[identifier]).await.pop().unwrap()
    }

    /// Identifies `Event`s from its cache or `https://api.openchain.xyz`
    pub async fn identify_events(
        &mut self,
        identifiers: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Vec<Option<Event>> {
        self.identify(SelectorType::Event, identifiers, get_event)
            .await
    }

    /// Identifies `Event` from its cache or `https://api.openchain.xyz`
    pub async fn identify_event(&mut self, identifier: &[u8]) -> Option<Event> {
        self.identify_events(&[identifier]).await.pop().unwrap()
    }

    /// Identifies `Error`s from its cache or `https://api.openchain.xyz`.
    pub async fn identify_errors(
        &mut self,
        identifiers: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Vec<Option<Error>> {
        self.identify(SelectorType::Error, identifiers, get_error)
            .await
    }

    /// Identifies `Error` from its cache or `https://api.openchain.xyz`.
    pub async fn identify_error(&mut self, identifier: &[u8]) -> Option<Error> {
        self.identify_errors(&[identifier]).await.pop().unwrap()
    }
}

impl Drop for SignaturesIdentifier {
    fn drop(&mut self) {
        self.save();
    }
}

#[cfg(test)]
#[allow(clippy::needless_return)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[tokio::test(flavor = "multi_thread")]
    async fn can_query_signatures() {
        let tmp = TempDir::new("sig-test").expect("failed creating temporary dir");
        {
            let sigs = SignaturesIdentifier::new(Some(tmp.path().into()), false).unwrap();

            assert!(sigs.read().await.cached.events.is_empty());
            assert!(sigs.read().await.cached.functions.is_empty());

            let func = sigs
                .write()
                .await
                .identify_function(&[35, 184, 114, 221])
                .await
                .unwrap();
            let event = sigs
                .write()
                .await
                .identify_event(&[
                    39, 119, 42, 220, 99, 219, 7, 170, 231, 101, 183, 30, 178, 181, 51, 6, 79, 167,
                    129, 189, 87, 69, 126, 27, 19, 133, 146, 216, 25, 141, 9, 89,
                ])
                .await
                .unwrap();

            assert_eq!(
                func,
                get_func("transferFrom(address,address,uint256)").unwrap()
            );
            assert_eq!(
                event,
                get_event("Transfer(address,address,uint128)").unwrap()
            );

            // dropping saves the cache
        }

        let sigs = SignaturesIdentifier::new(Some(tmp.path().into()), false).unwrap();
        assert_eq!(sigs.read().await.cached.events.len(), 1);
        assert_eq!(sigs.read().await.cached.functions.len(), 1);
    }
}
