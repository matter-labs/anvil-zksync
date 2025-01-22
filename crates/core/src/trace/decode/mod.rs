use alloy_dyn_abi::{DecodedEvent, DynSolValue, EventExt, FunctionExt, JsonAbiExt};
use alloy_json_abi::{Error, Event, Function, JsonAbi};
use std::{
    collections::{BTreeMap, HashMap},
    sync::OnceLock,
};
use zksync_types::{Address, H256};
use super::types::Selector;

/// The call trace decoder.
///
/// The decoder collects address labels and ABIs from any number of [TraceIdentifier]s, which it
/// then uses to decode the call trace.
///
/// Note that a call trace decoder is required for each new set of traces, since addresses in
/// different sets might overlap.
#[derive(Clone, Debug, Default)]
pub struct CallTraceDecoder {
    /// Addresses identified to be a specific contract.
    ///
    /// The values are in the form `"<artifact>:<contract>"`.
    pub contracts: HashMap<Address, String>,
    /// Address labels.
    pub labels: HashMap<Address, String>,
    /// Contract addresses that have a receive function.
    pub receive_contracts: Vec<Address>,
    /// Contract addresses that have fallback functions, mapped to function sigs.
    pub fallback_contracts: HashMap<Address, Vec<String>>,

    /// All known functions.
    pub functions: HashMap<Selector, Vec<Function>>,
    /// All known events.
    pub events: BTreeMap<(H256, usize), Vec<Event>>,
    // Revert decoder. Contains all known custom errors.
    // pub revert_decoder: RevertDecoder,

    // Verbosity level
    // pub verbosity: u8,

    // Optional identifier of individual trace steps.
    // pub debug_identifier: Option<DebugTraceIdentifier>,

    // /// Addresses that are contracts on the ZkVm
    // pub zk_contracts: HashSet<Address>,
}