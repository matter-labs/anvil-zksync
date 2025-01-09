/// This file is a an adapted copy of [`alloy::transports::http::Http`] that can work with
/// [`reqwest_middleware`].
// TODO: Consider upstreaming support to alloy
use alloy::rpc::json_rpc::{RequestPacket, ResponsePacket};
use alloy::transports::{TransportError, TransportErrorKind, TransportFut};
use reqwest_middleware::ClientWithMiddleware;
use std::task::{Context, Poll};
use tower::Service;

#[derive(Clone)]
pub struct HttpWithMiddleware {
    client: ClientWithMiddleware,
    url: reqwest::Url,
}

impl HttpWithMiddleware {
    /// Create a new [`HttpWithMiddleware`] transport with a custom client.
    pub const fn with_client(client: ClientWithMiddleware, url: reqwest::Url) -> Self {
        Self { client, url }
    }

    /// Make a request.
    fn request_reqwest(&self, req: RequestPacket) -> TransportFut<'static> {
        let this = self.clone();
        Box::pin(async move {
            let resp = this
                .client
                .post(this.url)
                .json(&req)
                .send()
                .await
                .map_err(TransportErrorKind::custom)?;
            let status = resp.status();

            // Unpack data from the response body. We do this regardless of
            // the status code, as we want to return the error in the body
            // if there is one.
            let body = resp.bytes().await.map_err(TransportErrorKind::custom)?;

            if status != reqwest::StatusCode::OK {
                return Err(TransportErrorKind::http_error(
                    status.as_u16(),
                    String::from_utf8_lossy(&body).into_owned(),
                ));
            }

            // Deserialize a Box<RawValue> from the body. If deserialization fails, return
            // the body as a string in the error. The conversion to String
            // is lossy and may not cover all the bytes in the body.
            serde_json::from_slice(&body)
                .map_err(|err| TransportError::deser_err(err, String::from_utf8_lossy(&body)))
        })
    }
}

impl Service<RequestPacket> for HttpWithMiddleware {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // reqwest always returns ok
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        self.request_reqwest(req)
    }
}
