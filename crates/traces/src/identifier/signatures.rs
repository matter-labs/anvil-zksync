////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Attribution: File adapted from the `evm` crate for zksync usage                                        //
//                                                                                                        //
// Full credit goes to its authors. See the original implementation here:                                 //
// https://github.com/foundry-rs/foundry/blob/master/crates/evm/traces/src/idenitfier/signatures.rs.      //
//                                                                                                        //
// Note: These methods are used under the terms of the original project's license.                        //
////////////////////////////////////////////////////////////////////////////////////////////////////////////

use crate::abi_utils::{get_error, get_event, get_func};
use alloy::json_abi::{Error, Event, Function};
use alloy::primitives::hex;
use anvil_zksync_common::{
    resolver::{SelectorType, SignEthClient},
    utils::io::read_json_file,
    utils::io::write_json_file,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

/// Global `SignaturesIdentifier`` instance
static GLOBAL_CLIENT: Lazy<SignaturesIdentifier> = Lazy::new(SignaturesIdentifier::default);

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CachedSignatures {
    pub errors: BTreeMap<String, Option<String>>,
    pub events: BTreeMap<String, Option<String>>,
    pub functions: BTreeMap<String, Option<String>>,
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

    pub fn save(&self, cache_path: &PathBuf) {
        if self.is_empty() {
            // Avoid writing file if there are no signatures.
            return;
        }
        if let Some(parent) = cache_path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                tracing::warn!(target: "trace::signatures", ?parent, ?err, "failed to create cache");
            }
        }
        if let Err(err) = write_json_file(cache_path, &self) {
            tracing::warn!(target: "trace::signatures", ?cache_path, ?err, "failed to flush signature cache");
        } else {
            tracing::trace!(target: "trace::signatures", ?cache_path, "flushed signature cache")
        }
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.events.is_empty() && self.functions.is_empty()
    }
}

/// An identifier that tries to identify functions and events using signatures found at
/// `https://openchain.xyz` or a local cache.
#[derive(Debug, Default, Clone)]
pub struct SignaturesIdentifier {
    inner: Arc<RwLock<SignaturesIdentifierInner>>,
}

#[derive(Debug, Default)]
struct SignaturesIdentifierInner {
    /// Cached selectors for functions, events and custom errors.
    cached: CachedSignatures,
    /// Location where to save `CachedSignatures`.
    cached_path: Option<PathBuf>,
    /// The OpenChain client to fetch signatures from.
    client: Option<SignEthClient>,
}

impl SignaturesIdentifierInner {
    fn new(cache_path: Option<PathBuf>, offline: bool) -> eyre::Result<Self> {
        let client = if !offline {
            Some(SignEthClient::new())
        } else {
            None
        };

        let self_ = if let Some(cache_path) = cache_path {
            let path = cache_path.join("signatures");
            tracing::trace!(target: "trace::signatures", ?path, "reading signature cache");
            let cached = CachedSignatures::load(cache_path);
            SignaturesIdentifierInner {
                cached,
                cached_path: Some(path),
                client,
            }
        } else {
            SignaturesIdentifierInner {
                cached: Default::default(),
                cached_path: None,
                client,
            }
        };
        Ok(self_)
    }

    fn save(&self) {
        if let Some(cached_path) = &self.cached_path {
            self.cached.save(cached_path);
        }
    }

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
                .collect();

            if !query.is_empty() {
                let start = Instant::now();
                let n_queries = query.len();
                // Fetching from remote sources can easily be the slowest part of execution, so we want to track
                // each call to the client.
                let res = client.decode_selectors(selector_type, query.clone()).await;
                if let Ok(res) = res {
                    for (hex_id, selector_result) in query.into_iter().zip(res.into_iter()) {
                        let mut found = false;
                        if let Some(decoded_results) = selector_result {
                            if let Some(decoded_result) = decoded_results.into_iter().next() {
                                cache.insert(hex_id.clone(), Some(decoded_result));
                                found = true;
                            }
                        }
                        if !found {
                            cache.insert(hex_id.clone(), None);
                        }
                    }
                }
                tracing::debug!(
                    "Queried {} signatures from remote source in {:?}",
                    n_queries,
                    start.elapsed()
                );
            }
        }

        hex_identifiers
            .iter()
            .map(|v| {
                if let Some(name) = cache.get(v) {
                    name.as_ref().and_then(|s| get_type(s).ok())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl SignaturesIdentifier {
    pub fn new(cache_path: Option<PathBuf>, offline: bool) -> eyre::Result<Self> {
        let inner = SignaturesIdentifierInner::new(cache_path, offline)?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    pub async fn save(&self) {
        self.inner.read().await.save();
    }

    pub async fn install(cache_path: Option<PathBuf>, offline: bool) -> eyre::Result<()> {
        *GLOBAL_CLIENT.inner.write().await = SignaturesIdentifierInner::new(cache_path, offline)?;

        Ok(())
    }

    pub fn global() -> Self {
        GLOBAL_CLIENT.clone()
    }

    /// Identifies `Function`s from its cache or `https://api.openchain.xyz`
    pub async fn identify_functions(
        &self,
        identifiers: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Vec<Option<Function>> {
        self.inner
            .write()
            .await
            .identify(SelectorType::Function, identifiers, get_func)
            .await
    }

    /// Identifies `Function` from its cache or `https://api.openchain.xyz`
    pub async fn identify_function(&self, identifier: &[u8]) -> Option<Function> {
        self.identify_functions(&[identifier]).await.pop().unwrap()
    }

    /// Identifies `Event`s from its cache or `https://api.openchain.xyz`
    pub async fn identify_events(
        &self,
        identifiers: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Vec<Option<Event>> {
        self.inner
            .write()
            .await
            .identify(SelectorType::Event, identifiers, get_event)
            .await
    }

    /// Identifies `Event` from its cache or `https://api.openchain.xyz`
    pub async fn identify_event(&self, identifier: &[u8]) -> Option<Event> {
        self.identify_events(&[identifier]).await.pop().unwrap()
    }

    /// Identifies `Error`s from its cache or `https://api.openchain.xyz`.
    pub async fn identify_errors(
        &self,
        identifiers: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Vec<Option<Error>> {
        self.inner
            .write()
            .await
            .identify(SelectorType::Error, identifiers, get_error)
            .await
    }

    /// Identifies `Error` from its cache or `https://api.openchain.xyz`.
    pub async fn identify_error(&self, identifier: &[u8]) -> Option<Error> {
        self.identify_errors(&[identifier]).await.pop().unwrap()
    }
}

#[cfg(test)]
#[allow(clippy::needless_return)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn can_query_signatures() {
        let tmp = tempfile::Builder::new()
            .prefix("sig-test")
            .tempdir()
            .expect("failed creating temporary dir");
        {
            let sigs = SignaturesIdentifier::new(Some(tmp.path().into()), false).unwrap();

            assert!(sigs.inner.read().await.cached.events.is_empty());
            assert!(sigs.inner.read().await.cached.functions.is_empty());

            let func = sigs.identify_function(&[35, 184, 114, 221]).await.unwrap();
            let event = sigs
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

            // Save the cache.
            sigs.save().await;
        }

        let sigs = SignaturesIdentifier::new(Some(tmp.path().into()), false).unwrap();
        assert_eq!(sigs.inner.read().await.cached.events.len(), 1);
        assert_eq!(sigs.inner.read().await.cached.functions.len(), 1);
    }
}
