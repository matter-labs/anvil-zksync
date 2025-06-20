//! Resolving the selectors (both method & event) with external database.
use super::{cache::Cache, cache::CacheConfig, sh_warn};
use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::RwLock;

static SELECTOR_DATABASE_URL: &str = "https://api.openchain.xyz/signature-database/v1/lookup";

/// How many request can time out before we decide this is a spurious connection
const MAX_TIMEDOUT_REQ: usize = 4usize;

/// Global EthClient instance
static GLOBAL_CLIENT: Lazy<RwLock<SignEthClient>> = Lazy::new(|| RwLock::new(SignEthClient::new()));

/// Sets the mode for selector decoding.
/// When `offline` is `true`, network requests will be skipped.
pub async fn function_selector_mode(offline: bool) {
    GLOBAL_CLIENT.write().await.set_offline(offline);
}

/// A client that can request API data from `https://api.openchain.xyz`
#[derive(Debug)]
pub struct SignEthClient {
    /// Whether the connection is spurious, or API is down
    spurious_connection: Arc<AtomicBool>,
    /// How many requests timed out
    timedout_requests: Arc<AtomicUsize>,
    /// Cache for network data.
    pub(crate) cache: Arc<RwLock<Cache>>,
    /// Whether we're in offline mode (skip network requests)
    offline: AtomicBool,
}

#[derive(Deserialize)]
pub struct KnownAbi {
    abi: String,
    name: String,
}

lazy_static! {
    static ref KNOWN_SIGNATURES: HashMap<String, String> = {
        let json_value = serde_json::from_slice(include_bytes!("data/abi_map.json")).unwrap();
        let pairs: Vec<KnownAbi> = serde_json::from_value(json_value).unwrap();

        pairs
            .into_iter()
            .map(|entry| (entry.abi, entry.name))
            .collect()
    };
}

impl SignEthClient {
    /// Creates a new client with default settings
    pub fn new() -> Self {
        Self {
            spurious_connection: Arc::new(Default::default()),
            timedout_requests: Arc::new(Default::default()),
            cache: Arc::new(RwLock::new(Cache::new(CacheConfig::default()))),
            offline: AtomicBool::new(false),
        }
    }

    /// Enable or disable offline mode
    pub fn set_offline(&self, offline: bool) {
        self.offline.store(offline, Ordering::Relaxed);
    }

    /// Convenience method for making a GET request
    pub async fn quick_get(&self, url: &str) -> eyre::Result<String> {
        let resp = reqwest::get(url).await.inspect_err(|e| {
            self.on_reqwest_err(e);
        })?;
        let text = resp.text().await.inspect_err(|e| {
            self.on_reqwest_err(e);
        })?;
        Ok(text)
    }

    fn on_reqwest_err(&self, err: &reqwest::Error) {
        fn is_connectivity_err(err: &reqwest::Error) -> bool {
            if err.is_timeout() || err.is_connect() {
                return true;
            }
            // Error HTTP codes (5xx) are considered connectivity issues and will prompt retry
            if let Some(status) = err.status() {
                let code = status.as_u16();
                if (500..600).contains(&code) {
                    return true;
                }
            }
            false
        }

        if is_connectivity_err(err) {
            sh_warn!("spurious network detected for api.openchain.xyz");
            let previous = self.timedout_requests.fetch_add(1, Ordering::SeqCst);
            if previous >= MAX_TIMEDOUT_REQ {
                self.set_spurious();
            }
        }
    }

    /// Returns whether the connection was marked as spurious
    fn is_spurious(&self) -> bool {
        self.spurious_connection.load(Ordering::Relaxed)
    }

    /// Marks the connection as spurious
    fn set_spurious(&self) {
        self.spurious_connection.store(true, Ordering::Relaxed)
    }

    fn ensure_not_spurious(&self) -> eyre::Result<()> {
        if self.is_spurious() {
            eyre::bail!("Spurious connection detected")
        }
        Ok(())
    }

    /// Decodes the given function or event selector using api.openchain.xyz
    pub async fn decode_selector(
        &self,
        selector: &str,
        selector_type: SelectorType,
    ) -> eyre::Result<Option<String>> {
        // exit early if spurious connection
        self.ensure_not_spurious()?;

        #[derive(Deserialize)]
        struct Decoded {
            name: String,
            filtered: bool,
        }

        #[derive(Deserialize)]
        struct ApiResult {
            event: HashMap<String, Option<Vec<Decoded>>>,
            function: HashMap<String, Option<Vec<Decoded>>>,
        }

        #[derive(Deserialize)]
        struct ApiResponse {
            ok: bool,
            result: ApiResult,
        }

        // using openchain signature database over 4byte
        // see https://github.com/foundry-rs/foundry/issues/1672
        let url = match selector_type {
            SelectorType::Function | SelectorType::Error => {
                format!("{SELECTOR_DATABASE_URL}?function={selector}&filter=true")
            }
            SelectorType::Event => format!("{SELECTOR_DATABASE_URL}?event={selector}&filter=true"),
        };

        let res = self.quick_get(&url).await?;
        let api_response = match serde_json::from_str::<ApiResponse>(&res) {
            Ok(inner) => inner,
            Err(err) => {
                eyre::bail!("Could not decode response:\n {res}.\nError: {err}")
            }
        };

        if !api_response.ok {
            eyre::bail!("Failed to decode:\n {res}")
        }

        let decoded = match selector_type {
            SelectorType::Function | SelectorType::Error => api_response.result.function,
            SelectorType::Event => api_response.result.event,
        };

        // If the search returns null, we should default to using the selector
        let default_decoded = vec![Decoded {
            name: selector.to_string(),
            filtered: false,
        }];

        Ok(decoded
            .get(selector)
            .ok_or(eyre::eyre!("No signature found"))?
            .as_ref()
            .unwrap_or(&default_decoded)
            .iter()
            .filter(|d| !d.filtered)
            .map(|d| d.name.clone())
            .collect::<Vec<String>>()
            .first()
            .cloned())
    }

    /// Decodes the given function, error or event selectors using OpenChain.
    pub async fn decode_selectors(
        &self,
        selector_type: SelectorType,
        selectors: impl IntoIterator<Item = impl Into<String>>,
    ) -> eyre::Result<Vec<Option<Vec<String>>>> {
        let selectors: Vec<String> = selectors
            .into_iter()
            .map(Into::into)
            .map(|s| s.to_lowercase())
            .map(|s| {
                if s.starts_with("0x") {
                    s
                } else {
                    format!("0x{s}")
                }
            })
            .collect();

        if selectors.is_empty() {
            return Ok(vec![]);
        }

        tracing::debug!(len = selectors.len(), "decoding selectors");
        tracing::trace!(?selectors, "decoding selectors");

        // exit early if spurious connection
        self.ensure_not_spurious()?;

        let expected_len = match selector_type {
            SelectorType::Function | SelectorType::Error => 10, // 0x + hex(4bytes)
            SelectorType::Event => 66,                          // 0x + hex(32bytes)
        };
        if let Some(s) = selectors.iter().find(|s| s.len() != expected_len) {
            eyre::bail!(
                "Invalid selector {s}: expected {expected_len} characters (including 0x prefix)."
            )
        }

        #[derive(Deserialize)]
        struct Decoded {
            name: String,
        }

        #[derive(Deserialize)]
        struct ApiResult {
            event: HashMap<String, Option<Vec<Decoded>>>,
            function: HashMap<String, Option<Vec<Decoded>>>,
        }

        #[derive(Deserialize)]
        struct ApiResponse {
            ok: bool,
            result: ApiResult,
        }

        let url = format!(
            "{SELECTOR_DATABASE_URL}?{ltype}={selectors_str}",
            ltype = match selector_type {
                SelectorType::Function | SelectorType::Error => "function",
                SelectorType::Event => "event",
            },
            selectors_str = selectors.join(",")
        );

        let res = self.quick_get(&url).await?;
        let api_response = match serde_json::from_str::<ApiResponse>(&res) {
            Ok(inner) => inner,
            Err(err) => {
                eyre::bail!("Could not decode response:\n {res}.\nError: {err}")
            }
        };

        if !api_response.ok {
            eyre::bail!("Failed to decode:\n {res}")
        }

        let decoded = match selector_type {
            SelectorType::Function | SelectorType::Error => api_response.result.function,
            SelectorType::Event => api_response.result.event,
        };

        Ok(selectors
            .into_iter()
            .map(|selector| match decoded.get(&selector) {
                Some(Some(r)) => Some(r.iter().map(|d| d.name.clone()).collect()),
                _ => None,
            })
            .collect())
    }

    /// Fetches a function signature given the selector using api.openchain.xyz
    pub async fn decode_function_selector(&self, selector: &str) -> eyre::Result<Option<String>> {
        let prefixed_selector = format!("0x{}", selector.strip_prefix("0x").unwrap_or(selector));
        if prefixed_selector.len() != 10 {
            eyre::bail!("Invalid selector: expected 8 characters (excluding 0x prefix), got {} characters (including 0x prefix).", prefixed_selector.len())
        }

        if let Some(r) = KNOWN_SIGNATURES.get(&prefixed_selector) {
            return Ok(Some(r.clone()));
        }

        self.decode_selector(&prefixed_selector[..10], SelectorType::Function)
            .await
    }
}

/// Default for SignEthClient
impl Default for SignEthClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
pub enum SelectorType {
    Function,
    Event,
    Error,
}
/// Fetches a function signature given the selector using api.openchain.xyz
pub async fn decode_function_selector(selector: &str) -> eyre::Result<Option<String>> {
    {
        let client = GLOBAL_CLIENT.read().await;
        if let Some(cached) = client
            .cache
            .read()
            .await
            .get_resolver_selector(&selector.to_string())
        {
            tracing::debug!("Using cached function selector for {}", selector);
            return Ok(Some(cached.clone()));
        }
        if client.offline.load(Ordering::Relaxed) {
            tracing::debug!("Offline mode: skipping network request for {}", selector);
            return Ok(None);
        }
    }
    tracing::debug!("Resolving function selector for {} via network", selector);
    let client = GLOBAL_CLIENT.read().await;
    let result = client.decode_function_selector(selector).await?;
    // cache the result
    client
        .cache
        .write()
        .await
        .insert_resolver_selector(selector.to_string(), result.clone().unwrap_or_default());
    Ok(result)
}

/// Decode an event selector using the global client
pub async fn decode_event_selector(selector: &str) -> eyre::Result<Option<String>> {
    {
        let client = GLOBAL_CLIENT.read().await;
        if let Some(cached) = client
            .cache
            .read()
            .await
            .get_resolver_selector(&selector.to_string())
        {
            tracing::debug!("Using cached event selector for {}", selector);
            return Ok(Some(cached.clone()));
        }
        if client.offline.load(Ordering::Relaxed) {
            tracing::debug!("Offline mode: skipping network request for {}", selector);
            return Ok(None);
        }
    }
    tracing::debug!("Resolving event selector for {} via network", selector);
    let client = GLOBAL_CLIENT.read().await;
    let result = client
        .decode_selector(selector, SelectorType::Event)
        .await?;
    client
        .cache
        .write()
        .await
        .insert_resolver_selector(selector.to_string(), result.clone().unwrap_or_default());
    Ok(result)
}
