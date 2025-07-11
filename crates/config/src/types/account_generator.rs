use crate::constants::{DERIVATION_PATH, TEST_NODE_NETWORK_ID};
use alloy::signers::Signer;
use alloy::signers::local::coins_bip39::{English, Mnemonic};
use alloy::signers::local::{MnemonicBuilder, PrivateKeySigner};
use serde::Deserialize;

/// Account Generator
/// Manages the generation of accounts for anvil-zksync
#[derive(Clone, Debug, Deserialize)]
pub struct AccountGenerator {
    chain_id: u32,
    amount: usize,
    phrase: String,
    derivation_path: Option<String>,
}

impl AccountGenerator {
    pub fn new(amount: usize) -> Self {
        Self {
            chain_id: TEST_NODE_NETWORK_ID,
            amount,
            phrase: Mnemonic::<English>::new(&mut rand::thread_rng()).to_phrase(),
            derivation_path: None,
        }
    }

    #[must_use]
    pub fn phrase(mut self, phrase: impl Into<String>) -> Self {
        self.phrase = phrase.into();
        self
    }

    pub fn get_phrase(&self) -> &str {
        &self.phrase
    }

    #[must_use]
    pub fn chain_id(mut self, chain_id: impl Into<u32>) -> Self {
        self.chain_id = chain_id.into();
        self
    }

    #[must_use]
    pub fn derivation_path(mut self, derivation_path: impl Into<String>) -> Self {
        let mut derivation_path = derivation_path.into();
        if !derivation_path.ends_with('/') {
            derivation_path.push('/');
        }
        self.derivation_path = Some(derivation_path);
        self
    }

    pub fn get_derivation_path(&self) -> &str {
        self.derivation_path.as_deref().unwrap_or(DERIVATION_PATH)
    }

    pub fn generate(&self) -> Vec<PrivateKeySigner> {
        let builder = MnemonicBuilder::<English>::default().phrase(self.phrase.as_str());

        let derivation_path = self.derivation_path.as_deref().unwrap_or(DERIVATION_PATH);

        (0..self.amount)
            .map(|idx| {
                let builder = builder
                    .clone()
                    .derivation_path(format!("{derivation_path}{idx}"))
                    .unwrap();
                builder
                    .build()
                    .unwrap()
                    .with_chain_id(Some(self.chain_id.into()))
            })
            .collect()
    }
}
