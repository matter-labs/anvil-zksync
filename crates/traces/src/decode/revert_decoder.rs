//! Various utilities to decode test results.
//////////////////////////////////////////////////////////////////////////////////////
// Attribution: File adapted from the `foundry-evm` crate                           //
//                                                                                  //
// Full credit goes to its authors. See the original implementation here:           //
// https://github.com/foundry-rs/foundry/blob/master/crates/evm/core/src/decode.rs. //
//                                                                                  //
// Note: These methods are used under the terms of the original project's license.  //
//////////////////////////////////////////////////////////////////////////////////////

use super::{decode_value, SELECTOR_LEN};
use alloy::dyn_abi::JsonAbiExt;
use alloy::json_abi::{Error, JsonAbi};
use alloy::primitives::{map::HashMap, Selector};
use alloy::sol_types::{SolInterface, SolValue};
use anvil_zksync_types::traces::DecodedError;
use std::sync::OnceLock;

/// Decodes revert data.
#[derive(Clone, Debug, Default)]
pub struct RevertDecoder {
    /// The custom errors to use for decoding.
    pub errors: HashMap<Selector, Vec<Error>>,
}

impl Default for &RevertDecoder {
    fn default() -> Self {
        static EMPTY: OnceLock<RevertDecoder> = OnceLock::new();
        EMPTY.get_or_init(RevertDecoder::new)
    }
}

impl RevertDecoder {
    /// Creates a new, empty revert decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the ABIs to use for error decoding.
    ///
    /// Note that this is decently expensive as it will hash all errors for faster indexing.
    pub fn with_abis<'a>(mut self, abi: impl IntoIterator<Item = &'a JsonAbi>) -> Self {
        self.extend_from_abis(abi);
        self
    }

    /// Sets the ABI to use for error decoding.
    ///
    /// Note that this is decently expensive as it will hash all errors for faster indexing.
    pub fn with_abi(mut self, abi: &JsonAbi) -> Self {
        self.extend_from_abi(abi);
        self
    }

    /// Sets the ABI to use for error decoding, if it is present.
    ///
    /// Note that this is decently expensive as it will hash all errors for faster indexing.
    pub fn with_abi_opt(mut self, abi: Option<&JsonAbi>) -> Self {
        if let Some(abi) = abi {
            self.extend_from_abi(abi);
        }
        self
    }

    /// Extends the decoder with the given ABI's custom errors.
    pub fn extend_from_abis<'a>(&mut self, abi: impl IntoIterator<Item = &'a JsonAbi>) {
        for abi in abi {
            self.extend_from_abi(abi);
        }
    }

    /// Extends the decoder with the given ABI's custom errors.
    pub fn extend_from_abi(&mut self, abi: &JsonAbi) {
        for error in abi.errors() {
            self.push_error(error.clone());
        }
    }

    /// Adds a custom error to use for decoding.
    pub fn push_error(&mut self, error: Error) {
        self.errors.entry(error.selector()).or_default().push(error);
    }

    /// Tries to decode an error message from the given revert bytes.
    ///
    /// Note that this is just a best-effort guess, and should not be relied upon for anything other
    /// than user output.
    pub fn decode(&self, err: &[u8]) -> DecodedError {
        self.maybe_decode(err).unwrap_or_else(|| {
            if err.is_empty() {
                // Empty revert data
                DecodedError::Empty
            } else {
                DecodedError::Raw(err.to_vec())
            }
        })
    }

    /// Tries to decode an error message from the given revert bytes.
    ///
    /// See [`decode`](Self::decode) for more information.
    pub fn maybe_decode(&self, err: &[u8]) -> Option<DecodedError> {
        let Some((selector, data)) = err.split_first_chunk::<SELECTOR_LEN>() else {
            return if err.is_empty() {
                None
            } else {
                Some(DecodedError::Raw(err.to_vec()))
            };
        };

        // Solidity's `Error(string)` or `Panic(uint256)`
        if let Ok(e) = alloy::sol_types::ContractError::<std::convert::Infallible>::abi_decode(err)
        {
            return match e {
                alloy::sol_types::ContractError::CustomError(_) => unreachable!(),
                alloy::sol_types::ContractError::Revert(revert) => {
                    Some(DecodedError::Revert(revert.reason))
                }
                alloy::sol_types::ContractError::Panic(panic) => {
                    Some(DecodedError::Panic(panic.to_string()))
                }
            };
        }

        // Custom errors.
        if let Some(errors) = self.errors.get(selector) {
            for error in errors {
                // If we don't decode, don't return an error, try to decode as a string later.
                if let Ok(decoded) = error.abi_decode_input(data) {
                    return Some(DecodedError::CustomError {
                        name: error.name.to_owned(),
                        fields: decoded.into_iter().map(decode_value).collect(),
                    });
                }
            }
        }

        // ABI-encoded `string`.
        if let Ok(s) = String::abi_decode(err) {
            return Some(DecodedError::Revert(s));
        }

        // ASCII string.
        if err.is_ascii() {
            return Some(DecodedError::Revert(
                std::str::from_utf8(err).unwrap().to_string(),
            ));
        }

        // Generic custom error.
        Some(DecodedError::GenericCustomError {
            selector: *selector,
            raw: data.to_vec(),
        })
    }
}
