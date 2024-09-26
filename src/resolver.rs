//! Resolving the selectors (both method & event) with external database.
use lazy_static::lazy_static;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use std::iter::FromIterator;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::warn;

use crate::{cache::Cache, config::cache::CacheConfig};

static SELECTOR_DATABASE_URL: &str = "https://api.openchain.xyz/signature-database/v1/lookup";

/// The standard request timeout for API requests
const REQ_TIMEOUT: Duration = Duration::from_secs(15);

/// How many request can time out before we decide this is a spurious connection
const MAX_TIMEDOUT_REQ: usize = 4usize;

/// A client that can request API data from `https://api.openchain.xyz`
#[derive(Debug, Clone)]
pub struct SignEthClient {
    inner: reqwest::Client,
    /// Whether the connection is spurious, or API is down
    spurious_connection: Arc<AtomicBool>,
    /// How many requests timed out
    timedout_requests: Arc<AtomicUsize>,
    /// Max allowed request that can time out
    max_timedout_requests: usize,
    /// Cache for network data.
    pub(crate) cache: Arc<RwLock<Cache>>,
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
    pub fn new() -> reqwest::Result<Self> {
        let inner = reqwest::Client::builder()
            .default_headers(HeaderMap::from_iter([(
                HeaderName::from_static("user-agent"),
                HeaderValue::from_static("zksync"),
            )]))
            .timeout(REQ_TIMEOUT)
            .build()?;
        Ok(Self {
            inner,
            spurious_connection: Arc::new(Default::default()),
            timedout_requests: Arc::new(Default::default()),
            max_timedout_requests: MAX_TIMEDOUT_REQ,
            cache: Arc::new(RwLock::new(Cache::new(CacheConfig::default()))),
        })
    }

    async fn get_text(&self, url: &str) -> reqwest::Result<String> {
        self.inner
            .get(url)
            .send()
            .await
            .inspect_err(|err| {
                self.on_reqwest_err(err);
            })?
            .text()
            .await
            .inspect_err(|err| {
                self.on_reqwest_err(err);
            })
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
            warn!("spurious network detected for api.openchain.xyz");
            let previous = self.timedout_requests.fetch_add(1, Ordering::SeqCst);
            if previous >= self.max_timedout_requests {
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
            SelectorType::Function => {
                format!("{SELECTOR_DATABASE_URL}?function={selector}&filter=true")
            }
            SelectorType::Event => format!("{SELECTOR_DATABASE_URL}?event={selector}&filter=true"),
        };

        let res = self.get_text(&url).await?;
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
            SelectorType::Function => api_response.result.function,
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

#[derive(Clone, Copy)]
pub enum SelectorType {
    Function,
    Event,
}
/// Fetches a function signature given the selector using api.openchain.xyz
pub async fn decode_function_selector(selector: &str) -> eyre::Result<Option<String>> {
    let client = SignEthClient::new();
    {
        // Check cache
        if let Some(resolved_selector) = client
            .as_ref()
            .unwrap() // Safe to do as client is created within this function
            .cache
            .read()
            .await
            .get_resolver_selector(&(selector.to_string()))
        {
            tracing::debug!("Using cached function selector for {selector}");
            return Ok(Some(resolved_selector.clone()));
        }
    }

    tracing::debug!("Making external request to resolve function selector for {selector}");
    let result = client
        .as_ref()
        .unwrap() // Safe to do as client is created within this function
        .decode_function_selector(selector)
        .await;

    if let Ok(result) = &result {
        client
            .as_ref()
            .unwrap() // Safe to do as client is created within this function
            .cache
            .write()
            .await
            .insert_resolver_selector(
                selector.to_string(),
                result.clone().unwrap_or_else(|| "".to_string()),
            );
    }
    result
}

pub async fn decode_event_selector(selector: &str) -> eyre::Result<Option<String>> {
    let client = SignEthClient::new();
    {
        // Check cache
        if let Some(resolved_selector) = client
            .as_ref()
            .unwrap() // Safe to do as client is created within this function
            .cache
            .read()
            .await
            .get_resolver_selector(&(selector.to_string()))
        {
            tracing::debug!("Using cached event selector for {selector}");
            return Ok(Some(resolved_selector.clone()));
        }
    }

    tracing::debug!("Making external request to resolve event selector for {selector}");
    let result = client
        .as_ref()
        .unwrap()
        .decode_selector(selector, SelectorType::Event)
        .await;

    if let Ok(result) = &result {
        client
            .as_ref()
            .unwrap() // Safe to do as client is created within this function
            .cache
            .write()
            .await
            .insert_resolver_selector(
                selector.to_string(),
                result.clone().unwrap_or_else(|| "".to_string()),
            );
    }
    result
}
