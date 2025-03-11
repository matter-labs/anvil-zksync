//! Various utilities to decode test results.
//////////////////////////////////////////////////////////////////////////////////////
// Attribution: File adapted from the `foundry-evm` crate                           //
//                                                                                  //
// Full credit goes to its authors. See the original implementation here:           //
// https://github.com/foundry-rs/foundry/blob/master/crates/evm/core/src/decode.rs. //
//                                                                                  //
// Note: These methods are used under the terms of the original project's license.  //
//////////////////////////////////////////////////////////////////////////////////////

use super::SELECTOR_LEN;
use crate::node::console;
use crate::utils::format_token;
use alloy::dyn_abi::JsonAbiExt;
use alloy::json_abi::{Error, JsonAbi};
use alloy::primitives::{hex, map::HashMap, Log, Selector};
use alloy::sol_types::{SolEventInterface, SolInterface, SolValue};
use itertools::Itertools;
use std::sync::OnceLock;

const EMPTY_REVERT_DATA: &str = "<empty revert data>";

/// Decode a set of logs, only returning logs from DSTest logging events and Hardhat's `console.log`
pub fn decode_console_logs(logs: &[Log]) -> Vec<String> {
    logs.iter().filter_map(decode_console_log).collect()
}

/// Decode a single log.
///
/// This function returns [None] if it is not a DSTest log or the result of a Hardhat
/// `console.log`.
pub fn decode_console_log(log: &Log) -> Option<String> {
    console::ConsoleEvents::decode_log(log, false)
        .ok()
        .map(|decoded| decoded.to_string())
}

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
    pub fn decode(&self, err: &[u8]) -> String {
        self.maybe_decode(err).unwrap_or_else(|| {
            if err.is_empty() {
                EMPTY_REVERT_DATA.to_string()
            } else {
                trimmed_hex(err)
            }
        })
    }

    /// Tries to decode an error message from the given revert bytes.
    ///
    /// See [`decode`](Self::decode) for more information.
    pub fn maybe_decode(&self, err: &[u8]) -> Option<String> {
        let Some((selector, data)) = err.split_first_chunk::<SELECTOR_LEN>() else {
            return if err.is_empty() {
                None
            } else {
                Some(format!("custom error bytes {}", hex::encode_prefixed(err)))
            };
        };

        // Solidity's `Error(string)` or `Panic(uint256)`
        if let Ok(e) =
            alloy::sol_types::ContractError::<std::convert::Infallible>::abi_decode(err, false)
        {
            return Some(e.to_string());
        }

        // Custom errors.
        if let Some(errors) = self.errors.get(selector) {
            for error in errors {
                // If we don't decode, don't return an error, try to decode as a string later.
                if let Ok(decoded) = error.abi_decode_input(data, false) {
                    return Some(format!(
                        "{}({})",
                        error.name,
                        decoded.iter().map(|v| format_token(v, false)).format(", ")
                    ));
                }
            }
        }

        // ABI-encoded `string`.
        if let Ok(s) = String::abi_decode(err, true) {
            return Some(s);
        }

        // ASCII string.
        if err.is_ascii() {
            return Some(std::str::from_utf8(err).unwrap().to_string());
        }

        // Generic custom error.
        Some({
            let mut s = format!("custom error {}", hex::encode_prefixed(selector));
            if !data.is_empty() {
                s.push_str(": ");
                match std::str::from_utf8(data) {
                    Ok(data) => s.push_str(data),
                    Err(_) => s.push_str(&hex::encode(data)),
                }
            }
            s
        })
    }
}

fn trimmed_hex(s: &[u8]) -> String {
    let n = 32;
    if s.len() <= n {
        hex::encode(s)
    } else {
        format!(
            "{}…{} ({} bytes)",
            &hex::encode(&s[..n / 2]),
            &hex::encode(&s[s.len() - n / 2..]),
            s.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_trimmed_hex() {
        assert_eq!(
            trimmed_hex(&hex::decode("1234567890").unwrap()),
            "1234567890"
        );
        assert_eq!(
            trimmed_hex(&hex::decode("492077697368207275737420737570706F72746564206869676865722D6B696E646564207479706573").unwrap()),
            "49207769736820727573742073757070…6865722d6b696e646564207479706573 (41 bytes)"
        );
    }
}
