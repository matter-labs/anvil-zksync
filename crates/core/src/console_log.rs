use std::{collections::HashMap, str::FromStr};

use crate::utils::format_token;
use alloy_dyn_abi::JsonAbiExt;
use alloy_json_abi::{Function, Param, StateMutability};
use alloy_primitives::Selector;
use colored::Colorize;
use itertools::Itertools;
use std::sync::LazyLock;
use zksync_multivm::interface::Call;
use zksync_types::H160;

/// Maps all the `hardhat/console.log` log selectors that use the legacy ABI (`int`, `uint`) to
/// their normalized counterparts (`int256`, `uint256`).
///
/// `hardhat/console.log` logs its events manually, and in functions that accept integers they're
/// encoded as `abi.encodeWithSignature("log(int)", p0)`, which is not the canonical ABI encoding
/// for `int` that Solidity and [`sol!`] use.
pub static HARDHAT_CONSOLE_SELECTOR_PATCHES: LazyLock<HashMap<[u8; 4], [u8; 4]>> =
    LazyLock::new(|| HashMap::from_iter(include!("./data/patches.rs")));

/// Patches the given Hardhat `console` function selector to its ABI-normalized form.
///
/// See [`HARDHAT_CONSOLE_SELECTOR_PATCHES`] for more details.
pub fn patch_hh_console_selector(input: &mut [u8]) {
    if let Some(selector) = hh_console_selector(input) {
        input[..4].copy_from_slice(selector.as_slice());
    }
}

/// Returns the ABI-normalized selector for the given Hardhat `console` function selector.
///
/// See [`HARDHAT_CONSOLE_SELECTOR_PATCHES`] for more details.
pub fn hh_console_selector(input: &[u8]) -> Option<&'static Selector> {
    if let Some(selector) = input.get(..4) {
        let selector: &[u8; 4] = selector.try_into().unwrap();
        HARDHAT_CONSOLE_SELECTOR_PATCHES
            .get(selector)
            .map(Into::into)
    } else {
        None
    }
}

/// ConsoleLogHandler is responsible for printing the logs, that are created when contract calls 'console.log' method.
/// This is a popular debugging method used by hardhat and foundry.
/// When user wants to log something, these tools are doing a call to a contract at a given (fake) address.
/// The ConsoleLogHandler is catching this call, and printing it out -- but only if TRACE level log is enabled for 'console' target.
#[derive(Debug, Clone)]
pub struct ConsoleLogHandler {
    /// Map from the 4-byte function signature to function itself.
    // This contract has many 'log' methods (depending on argument type) - so we have a map here, to be able to parse the arguments.
    signature_map: HashMap<Selector, Function>,
    /// The 'fake' hardcoded contract, whose calls with have to log.
    target_contract: H160,
}

pub const CONSOLE_ADDRESS: &str = "0x000000000000000000636f6e736f6c652e6c6f67";

impl Default for ConsoleLogHandler {
    fn default() -> Self {
        Self {
            signature_map: get_log_functions()
                .into_iter()
                .map(|func| (func.selector(), func))
                .collect::<HashMap<_, _>>(),
            target_contract: H160::from_str(CONSOLE_ADDRESS).unwrap(),
        }
    }
}

impl ConsoleLogHandler {
    pub fn handle_calls_recursive(&self, calls: &Vec<Call>) {
        let mut messages: Vec<String> = vec![];
        for call in calls {
            self.handle_call_recursive(call, &mut messages);
        }

        if !messages.is_empty() {
            tracing::info!("");
            tracing::info!("==== Console logs: ");
        }
        for message in messages {
            tracing::info!("{}", message.cyan());
        }
    }
    pub fn handle_call_recursive(&self, current_call: &Call, messages: &mut Vec<String>) {
        if let Some(message) = self.handle_call(current_call) {
            messages.push(message);
        };
        for call in &current_call.calls {
            self.handle_call_recursive(call, messages);
        }
    }
    pub fn handle_call(&self, current_call: &Call) -> Option<String> {
        if current_call.to != self.target_contract {
            return None;
        }
        if current_call.input.len() < 4 {
            return None;
        }
        let signature = &current_call.input[..4];
        let message =
            self.signature_map
                .get(signature)
                .map_or("Unknown log call.".to_owned(), |func| {
                    let tokens: Result<Vec<alloy_dyn_abi::DynSolValue>, alloy_dyn_abi::Error> =
                        func.abi_decode_input(&current_call.input.as_slice()[4..], false);
                    tokens.map_or("Failed to parse inputs for log.".to_owned(), |tokens| {
                        tokens.iter().map(|t| format_token(t, false)).join(" ")
                    })
                });
        Some(message)
    }
}

/// Returns the well-known log [Function]s.
pub fn get_log_functions() -> Vec<Function> {
    LOG_FUNCTIONS
        .iter()
        .map(|func_decl| {
            let (name, params) = func_decl
                .trim_end_matches(')')
                .split_once('(')
                .unwrap_or_else(|| panic!("unable to obtain function name for '{}'", func_decl));

            Function {
                name: String::from(name),
                inputs: params
                    .split(',')
                    .map(|param| {
                        Param::parse(param).unwrap_or_else(|err| {
                            panic!("Failed to parse parameter '{}': {:?}", param, err)
                        })
                    })
                    .collect(),
                outputs: vec![],
                state_mutability: StateMutability::View,
            }
        })
        .collect()
}

/// Represents the common log functions.
/// See https://github.com/NomicFoundation/hardhat/blob/main/packages/hardhat-core/console.sol
const LOG_FUNCTIONS: [&str; 378] = [
    "log(int256)",
    "log(uint256)",
    "log(string)",
    "log(bool)",
    "log(address)",
    "log(bytes)",
    "log(bytes1)",
    "log(bytes2)",
    "log(bytes3)",
    "log(bytes4)",
    "log(bytes5)",
    "log(bytes6)",
    "log(bytes7)",
    "log(bytes8)",
    "log(bytes9)",
    "log(bytes10)",
    "log(bytes11)",
    "log(bytes12)",
    "log(bytes13)",
    "log(bytes14)",
    "log(bytes15)",
    "log(bytes16)",
    "log(bytes17)",
    "log(bytes18)",
    "log(bytes19)",
    "log(bytes20)",
    "log(bytes21)",
    "log(bytes22)",
    "log(bytes23)",
    "log(bytes24)",
    "log(bytes25)",
    "log(bytes26)",
    "log(bytes27)",
    "log(bytes28)",
    "log(bytes29)",
    "log(bytes30)",
    "log(bytes31)",
    "log(bytes32)",
    "log(uint256)",
    "log(string)",
    "log(bool)",
    "log(address)",
    "log(uint256,uint256)",
    "log(uint256,string)",
    "log(uint256,bool)",
    "log(uint256,address)",
    "log(string,uint256)",
    "log(string,string)",
    "log(string,bool)",
    "log(string,address)",
    "log(bool,uint256)",
    "log(bool,string)",
    "log(bool,bool)",
    "log(bool,address)",
    "log(address,uint256)",
    "log(address,string)",
    "log(address,bool)",
    "log(address,address)",
    "log(uint256,uint256,uint256)",
    "log(uint256,uint256,string)",
    "log(uint256,uint256,bool)",
    "log(uint256,uint256,address)",
    "log(uint256,string,uint256)",
    "log(uint256,string,string)",
    "log(uint256,string,bool)",
    "log(uint256,string,address)",
    "log(uint256,bool,uint256)",
    "log(uint256,bool,string)",
    "log(uint256,bool,bool)",
    "log(uint256,bool,address)",
    "log(uint256,address,uint256)",
    "log(uint256,address,string)",
    "log(uint256,address,bool)",
    "log(uint256,address,address)",
    "log(string,uint256,uint256)",
    "log(string,uint256,string)",
    "log(string,uint256,bool)",
    "log(string,uint256,address)",
    "log(string,string,uint256)",
    "log(string,string,string)",
    "log(string,string,bool)",
    "log(string,string,address)",
    "log(string,bool,uint256)",
    "log(string,bool,string)",
    "log(string,bool,bool)",
    "log(string,bool,address)",
    "log(string,address,uint256)",
    "log(string,address,string)",
    "log(string,address,bool)",
    "log(string,address,address)",
    "log(bool,uint256,uint256)",
    "log(bool,uint256,string)",
    "log(bool,uint256,bool)",
    "log(bool,uint256,address)",
    "log(bool,string,uint256)",
    "log(bool,string,string)",
    "log(bool,string,bool)",
    "log(bool,string,address)",
    "log(bool,bool,uint256)",
    "log(bool,bool,string)",
    "log(bool,bool,bool)",
    "log(bool,bool,address)",
    "log(bool,address,uint256)",
    "log(bool,address,string)",
    "log(bool,address,bool)",
    "log(bool,address,address)",
    "log(address,uint256,uint256)",
    "log(address,uint256,string)",
    "log(address,uint256,bool)",
    "log(address,uint256,address)",
    "log(address,string,uint256)",
    "log(address,string,string)",
    "log(address,string,bool)",
    "log(address,string,address)",
    "log(address,bool,uint256)",
    "log(address,bool,string)",
    "log(address,bool,bool)",
    "log(address,bool,address)",
    "log(address,address,uint256)",
    "log(address,address,string)",
    "log(address,address,bool)",
    "log(address,address,address)",
    "log(uint256,uint256,uint256,uint256)",
    "log(uint256,uint256,uint256,string)",
    "log(uint256,uint256,uint256,bool)",
    "log(uint256,uint256,uint256,address)",
    "log(uint256,uint256,string,uint256)",
    "log(uint256,uint256,string,string)",
    "log(uint256,uint256,string,bool)",
    "log(uint256,uint256,string,address)",
    "log(uint256,uint256,bool,uint256)",
    "log(uint256,uint256,bool,string)",
    "log(uint256,uint256,bool,bool)",
    "log(uint256,uint256,bool,address)",
    "log(uint256,uint256,address,uint256)",
    "log(uint256,uint256,address,string)",
    "log(uint256,uint256,address,bool)",
    "log(uint256,uint256,address,address)",
    "log(uint256,string,uint256,uint256)",
    "log(uint256,string,uint256,string)",
    "log(uint256,string,uint256,bool)",
    "log(uint256,string,uint256,address)",
    "log(uint256,string,string,uint256)",
    "log(uint256,string,string,string)",
    "log(uint256,string,string,bool)",
    "log(uint256,string,string,address)",
    "log(uint256,string,bool,uint256)",
    "log(uint256,string,bool,string)",
    "log(uint256,string,bool,bool)",
    "log(uint256,string,bool,address)",
    "log(uint256,string,address,uint256)",
    "log(uint256,string,address,string)",
    "log(uint256,string,address,bool)",
    "log(uint256,string,address,address)",
    "log(uint256,bool,uint256,uint256)",
    "log(uint256,bool,uint256,string)",
    "log(uint256,bool,uint256,bool)",
    "log(uint256,bool,uint256,address)",
    "log(uint256,bool,string,uint256)",
    "log(uint256,bool,string,string)",
    "log(uint256,bool,string,bool)",
    "log(uint256,bool,string,address)",
    "log(uint256,bool,bool,uint256)",
    "log(uint256,bool,bool,string)",
    "log(uint256,bool,bool,bool)",
    "log(uint256,bool,bool,address)",
    "log(uint256,bool,address,uint256)",
    "log(uint256,bool,address,string)",
    "log(uint256,bool,address,bool)",
    "log(uint256,bool,address,address)",
    "log(uint256,address,uint256,uint256)",
    "log(uint256,address,uint256,string)",
    "log(uint256,address,uint256,bool)",
    "log(uint256,address,uint256,address)",
    "log(uint256,address,string,uint256)",
    "log(uint256,address,string,string)",
    "log(uint256,address,string,bool)",
    "log(uint256,address,string,address)",
    "log(uint256,address,bool,uint256)",
    "log(uint256,address,bool,string)",
    "log(uint256,address,bool,bool)",
    "log(uint256,address,bool,address)",
    "log(uint256,address,address,uint256)",
    "log(uint256,address,address,string)",
    "log(uint256,address,address,bool)",
    "log(uint256,address,address,address)",
    "log(string,uint256,uint256,uint256)",
    "log(string,uint256,uint256,string)",
    "log(string,uint256,uint256,bool)",
    "log(string,uint256,uint256,address)",
    "log(string,uint256,string,uint256)",
    "log(string,uint256,string,string)",
    "log(string,uint256,string,bool)",
    "log(string,uint256,string,address)",
    "log(string,uint256,bool,uint256)",
    "log(string,uint256,bool,string)",
    "log(string,uint256,bool,bool)",
    "log(string,uint256,bool,address)",
    "log(string,uint256,address,uint256)",
    "log(string,uint256,address,string)",
    "log(string,uint256,address,bool)",
    "log(string,uint256,address,address)",
    "log(string,string,uint256,uint256)",
    "log(string,string,uint256,string)",
    "log(string,string,uint256,bool)",
    "log(string,string,uint256,address)",
    "log(string,string,string,uint256)",
    "log(string,string,string,string)",
    "log(string,string,string,bool)",
    "log(string,string,string,address)",
    "log(string,string,bool,uint256)",
    "log(string,string,bool,string)",
    "log(string,string,bool,bool)",
    "log(string,string,bool,address)",
    "log(string,string,address,uint256)",
    "log(string,string,address,string)",
    "log(string,string,address,bool)",
    "log(string,string,address,address)",
    "log(string,bool,uint256,uint256)",
    "log(string,bool,uint256,string)",
    "log(string,bool,uint256,bool)",
    "log(string,bool,uint256,address)",
    "log(string,bool,string,uint256)",
    "log(string,bool,string,string)",
    "log(string,bool,string,bool)",
    "log(string,bool,string,address)",
    "log(string,bool,bool,uint256)",
    "log(string,bool,bool,string)",
    "log(string,bool,bool,bool)",
    "log(string,bool,bool,address)",
    "log(string,bool,address,uint256)",
    "log(string,bool,address,string)",
    "log(string,bool,address,bool)",
    "log(string,bool,address,address)",
    "log(string,address,uint256,uint256)",
    "log(string,address,uint256,string)",
    "log(string,address,uint256,bool)",
    "log(string,address,uint256,address)",
    "log(string,address,string,uint256)",
    "log(string,address,string,string)",
    "log(string,address,string,bool)",
    "log(string,address,string,address)",
    "log(string,address,bool,uint256)",
    "log(string,address,bool,string)",
    "log(string,address,bool,bool)",
    "log(string,address,bool,address)",
    "log(string,address,address,uint256)",
    "log(string,address,address,string)",
    "log(string,address,address,bool)",
    "log(string,address,address,address)",
    "log(bool,uint256,uint256,uint256)",
    "log(bool,uint256,uint256,string)",
    "log(bool,uint256,uint256,bool)",
    "log(bool,uint256,uint256,address)",
    "log(bool,uint256,string,uint256)",
    "log(bool,uint256,string,string)",
    "log(bool,uint256,string,bool)",
    "log(bool,uint256,string,address)",
    "log(bool,uint256,bool,uint256)",
    "log(bool,uint256,bool,string)",
    "log(bool,uint256,bool,bool)",
    "log(bool,uint256,bool,address)",
    "log(bool,uint256,address,uint256)",
    "log(bool,uint256,address,string)",
    "log(bool,uint256,address,bool)",
    "log(bool,uint256,address,address)",
    "log(bool,string,uint256,uint256)",
    "log(bool,string,uint256,string)",
    "log(bool,string,uint256,bool)",
    "log(bool,string,uint256,address)",
    "log(bool,string,string,uint256)",
    "log(bool,string,string,string)",
    "log(bool,string,string,bool)",
    "log(bool,string,string,address)",
    "log(bool,string,bool,uint256)",
    "log(bool,string,bool,string)",
    "log(bool,string,bool,bool)",
    "log(bool,string,bool,address)",
    "log(bool,string,address,uint256)",
    "log(bool,string,address,string)",
    "log(bool,string,address,bool)",
    "log(bool,string,address,address)",
    "log(bool,bool,uint256,uint256)",
    "log(bool,bool,uint256,string)",
    "log(bool,bool,uint256,bool)",
    "log(bool,bool,uint256,address)",
    "log(bool,bool,string,uint256)",
    "log(bool,bool,string,string)",
    "log(bool,bool,string,bool)",
    "log(bool,bool,string,address)",
    "log(bool,bool,bool,uint256)",
    "log(bool,bool,bool,string)",
    "log(bool,bool,bool,bool)",
    "log(bool,bool,bool,address)",
    "log(bool,bool,address,uint256)",
    "log(bool,bool,address,string)",
    "log(bool,bool,address,bool)",
    "log(bool,bool,address,address)",
    "log(bool,address,uint256,uint256)",
    "log(bool,address,uint256,string)",
    "log(bool,address,uint256,bool)",
    "log(bool,address,uint256,address)",
    "log(bool,address,string,uint256)",
    "log(bool,address,string,string)",
    "log(bool,address,string,bool)",
    "log(bool,address,string,address)",
    "log(bool,address,bool,uint256)",
    "log(bool,address,bool,string)",
    "log(bool,address,bool,bool)",
    "log(bool,address,bool,address)",
    "log(bool,address,address,uint256)",
    "log(bool,address,address,string)",
    "log(bool,address,address,bool)",
    "log(bool,address,address,address)",
    "log(address,uint256,uint256,uint256)",
    "log(address,uint256,uint256,string)",
    "log(address,uint256,uint256,bool)",
    "log(address,uint256,uint256,address)",
    "log(address,uint256,string,uint256)",
    "log(address,uint256,string,string)",
    "log(address,uint256,string,bool)",
    "log(address,uint256,string,address)",
    "log(address,uint256,bool,uint256)",
    "log(address,uint256,bool,string)",
    "log(address,uint256,bool,bool)",
    "log(address,uint256,bool,address)",
    "log(address,uint256,address,uint256)",
    "log(address,uint256,address,string)",
    "log(address,uint256,address,bool)",
    "log(address,uint256,address,address)",
    "log(address,string,uint256,uint256)",
    "log(address,string,uint256,string)",
    "log(address,string,uint256,bool)",
    "log(address,string,uint256,address)",
    "log(address,string,string,uint256)",
    "log(address,string,string,string)",
    "log(address,string,string,bool)",
    "log(address,string,string,address)",
    "log(address,string,bool,uint256)",
    "log(address,string,bool,string)",
    "log(address,string,bool,bool)",
    "log(address,string,bool,address)",
    "log(address,string,address,uint256)",
    "log(address,string,address,string)",
    "log(address,string,address,bool)",
    "log(address,string,address,address)",
    "log(address,bool,uint256,uint256)",
    "log(address,bool,uint256,string)",
    "log(address,bool,uint256,bool)",
    "log(address,bool,uint256,address)",
    "log(address,bool,string,uint256)",
    "log(address,bool,string,string)",
    "log(address,bool,string,bool)",
    "log(address,bool,string,address)",
    "log(address,bool,bool,uint256)",
    "log(address,bool,bool,string)",
    "log(address,bool,bool,bool)",
    "log(address,bool,bool,address)",
    "log(address,bool,address,uint256)",
    "log(address,bool,address,string)",
    "log(address,bool,address,bool)",
    "log(address,bool,address,address)",
    "log(address,address,uint256,uint256)",
    "log(address,address,uint256,string)",
    "log(address,address,uint256,bool)",
    "log(address,address,uint256,address)",
    "log(address,address,string,uint256)",
    "log(address,address,string,string)",
    "log(address,address,string,bool)",
    "log(address,address,string,address)",
    "log(address,address,bool,uint256)",
    "log(address,address,bool,string)",
    "log(address,address,bool,bool)",
    "log(address,address,bool,address)",
    "log(address,address,address,uint256)",
    "log(address,address,address,string)",
    "log(address,address,address,bool)",
    "log(address,address,address,address)",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_log_functions_compiles() {
        assert_eq!(378, get_log_functions().len());
    }
}
