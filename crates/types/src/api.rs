use serde::{Deserialize, Serialize};
use zksync_types::web3::Bytes;
use zksync_types::U64;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct DetailedTransaction {
    #[serde(flatten)]
    pub inner: zksync_types::api::Transaction,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub output: Option<Bytes>,
    #[serde(rename = "revertReason")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub revert_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetRequestForking {
    pub json_rpc_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_number: Option<U64>,
}

#[derive(Clone, Debug, PartialEq, Default, Deserialize)]
pub struct ResetRequest {
    /// The block number to reset the state to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<U64>,
    /// Forking to a specified URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forking: Option<ResetRequestForking>,
}
