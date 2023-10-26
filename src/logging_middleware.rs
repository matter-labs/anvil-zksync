use colored::Colorize;
use futures::Future;
use futures::{future::Either, FutureExt};
use itertools::Itertools;
use jsonrpc_core::{
    middleware, Call, FutureResponse, Metadata, Middleware, Params, Request, Response,
};
use tracing_subscriber::filter::LevelFilter;

#[derive(Clone, Debug, Default)]
pub struct Meta();
impl Metadata for Meta {}

pub struct LoggingMiddleware {
    log_level_filter: LevelFilter,
}

impl LoggingMiddleware {
    pub fn new(log_level_filter: LevelFilter) -> Self {
        Self { log_level_filter }
    }
}

/// Logging Middleware for all in-bound requests
/// Logs out incoming requests and their parameters
/// Useful for debugging applications that are pointed at this service
impl Middleware<Meta> for LoggingMiddleware {
    type Future = FutureResponse;
    type CallFuture = middleware::NoopCallFuture;

    fn on_request<F, X>(&self, request: Request, meta: Meta, next: F) -> Either<Self::Future, X>
    where
        F: FnOnce(Request, Meta) -> X + Send,
        X: Future<Output = Option<Response>> + Send + 'static,
    {
        if let Request::Single(Call::MethodCall(method_call)) = &request {
            match self.log_level_filter {
                LevelFilter::TRACE => {
                    let full_params = match &method_call.params {
                        Params::Array(values) => {
                            if values.is_empty() {
                                String::default()
                            } else {
                                format!("with [{}]", values.iter().join(", "))
                            }
                        }
                        _ => String::default(),
                    };

                    tracing::trace!("{} was called {}", method_call.method.cyan(), full_params);
                }
                _ => {
                    // Generate truncated params for requests with massive payloads
                    let truncated_params = match &method_call.params {
                        Params::Array(values) => {
                            if values.is_empty() {
                                String::default()
                            } else {
                                format!(
                                    "with [{}]",
                                    values
                                        .iter()
                                        .map(|s| {
                                            let s_str = s.to_string();
                                            if s_str.len() > 70 {
                                                format!("{:.67}...", s_str)
                                            } else {
                                                s_str
                                            }
                                        })
                                        .collect::<Vec<String>>()
                                        .join(", ")
                                )
                            }
                        }
                        _ => String::default(),
                    };

                    tracing::debug!(
                        "{} was called {}",
                        method_call.method.cyan(),
                        truncated_params
                    );
                }
            }
        };

        Either::Left(Box::pin(next(request, meta).map(move |res| {
            tracing::trace!("API response => {:?}", res);
            res
        })))
    }
}
