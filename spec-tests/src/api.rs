use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use anyhow::Context;
use itertools::Itertools;
use serde_json::Value;
use std::str::FromStr;
use std::time::Duration;
use zksync_basic_types::U256;

pub struct EraApi {
    client: reqwest::Client,
    url: String,
}

impl EraApi {
    pub fn local(port: u16) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        let url = format!("http://127.0.0.1:{}", port);
        Ok(Self { client, url })
    }

    pub async fn make_request(
        &self,
        method_name: &str,
        params: Vec<Value>,
    ) -> anyhow::Result<Value> {
        let body = format!(
            r#"{{"jsonrpc": "2.0", "id": 1, "method": "{}", "params": [{}]}}"#,
            method_name,
            &params.into_iter().map(|p| p.to_string()).join(","),
        );
        let response = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            // Return early if response code is 4XX/5XX
            .error_for_status()?;
        let data = response.bytes().await?;
        let mut response: Value = serde_json::from_slice(data.as_ref())?;
        let response = response
            .as_object_mut()
            .context("root response object is not a JSON object")?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("request failed with '{}'", error);
        }
        if let Some(result) = response.remove("result") {
            return Ok(result);
        }

        anyhow::bail!("failed to parse response: {:?}", response);
    }

    pub async fn transfer_eth(&self, value: U256) -> anyhow::Result<()> {
        // TODO: Make signer configurable, leave taking a random rich wallet as the default option
        let signer = PrivateKeySigner::from_str(
            "0x3d3cbc973389cb26f657686445bcc75662b415b656078503592ac8c1abb8810e",
        )?;
        let wallet = EthereumWallet::from(signer);
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.url.clone().parse()?);

        // TODO: Make `to` configurable, leave taking a random wallet as the default option
        let tx = TransactionRequest::default()
            .to(Address::from_str(
                "0x55bE1B079b53962746B2e86d12f158a41DF294A6",
            )?)
            .value(value.to_string().parse()?)
            .with_gas_price(100000000000);

        // FIXME: this does not work yet because we include pre EIP-98/EIP-658 `root` field in tx
        // receipts.
        provider.send_transaction(tx).await?.watch().await?;

        Ok(())
    }
}
