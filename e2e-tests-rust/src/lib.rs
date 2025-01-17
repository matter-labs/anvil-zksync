#![allow(async_fn_in_trait)]

mod ext;
mod provider;
mod utils;

pub use ext::{ReceiptExt, ZksyncWalletProviderExt};
pub use provider::{init_testing_provider, AnvilZKsyncApi, TestingProvider, DEFAULT_TX_VALUE};
