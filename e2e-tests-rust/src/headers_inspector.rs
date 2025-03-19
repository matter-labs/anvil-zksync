use async_trait::async_trait;
use http::HeaderMap;
use reqwest_middleware::{Middleware, Next};
use std::sync::{Arc, RwLock};

/// A [`reqwest_middleware`]-compliant middleware that allows to inspect last seen response headers.
#[derive(Clone, Debug, Default)]
pub struct ResponseHeadersInspector(Arc<RwLock<Option<HeaderMap>>>);

impl ResponseHeadersInspector {
    pub fn last_unwrap(&self) -> HeaderMap {
        self.0.read().unwrap().clone().expect("no headers found")
    }
}

#[async_trait]
impl Middleware for ResponseHeadersInspector {
    async fn handle(
        &self,
        req: reqwest::Request,
        extensions: &mut http::Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<reqwest::Response> {
        let resp = next.run(req, extensions).await?;
        *self.0.write().unwrap() = Some(resp.headers().clone());
        Ok(resp)
    }
}
