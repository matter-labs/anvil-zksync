use std::{collections::HashMap, str::FromStr};

use colored::Colorize;
use ethabi::{Contract, Function};
use itertools::Itertools;
use serde_json::Value;
use zksync_types::vm_trace::Call;
use zksync_types::H160;

/// ConsoleLogHandler is responsible for printing the logs, that are created when contract calls 'console.log' method.
/// This is a popular debugging method used by hardhat and foundry.
/// When user wants to log something, these tools are doing a call to a contract at a given (fake) address.
/// The ConsoleLogHandler is catching this call, and printing it out -- but only if TRACE level log is enabled for 'console' target.
#[derive(Debug, Clone)]
pub struct ConsoleLogHandler {
    /// Map from the 4-byte function signature to function itself.
    // This contract has many 'log' methods (depending on argument type) - so we have a map here, to be able to parse the arguments.
    signature_map: HashMap<[u8; 4], Function>,
    /// The 'fake' hardcoded contract, whose calls with have to log.
    target_contract: H160,
}

pub const CONSOLE_ADDRESS: &str = "0x000000000000000000636f6e736f6c652e6c6f67";

impl Default for ConsoleLogHandler {
    fn default() -> Self {
        let mut val: Value =
            serde_json::from_slice(include_bytes!("deps/contracts/Console.json")).unwrap();

        let contract: Contract = serde_json::from_value(val["abi"].take()).unwrap();

        let mut signature_map: HashMap<[u8; 4], Function> = Default::default();

        for (_, functions) in contract.functions.iter() {
            for func in functions.iter() {
                signature_map.insert(func.short_signature(), func.clone());
            }
        }
        Self {
            signature_map,
            target_contract: H160::from_str(CONSOLE_ADDRESS).unwrap(),
        }
    }
}

impl ConsoleLogHandler {
    pub fn handle_call_recursive(&self, current_call: &Call) {
        self.handle_call(current_call);
        for call in &current_call.calls {
            self.handle_call_recursive(call);
        }
    }
    pub fn handle_call(&self, current_call: &Call) {
        if current_call.to != self.target_contract {
            return;
        }
        if current_call.input.len() < 4 {
            return;
        }
        let signature = &current_call.input[..4];
        let message =
            self.signature_map
                .get(signature)
                .map_or("Unknown log call.".to_owned(), |func| {
                    let tokens = func.decode_input(&current_call.input.as_slice()[4..]);

                    tokens.map_or("Failed to parse inputs for log.".to_owned(), |tokens| {
                        tokens.iter().map(|t| format!("{}", t)).join(" ")
                    })
                });
        log::info!("{}", message.cyan());
    }
}
