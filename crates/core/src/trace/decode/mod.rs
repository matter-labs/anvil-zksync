use super::types::{CallTrace, CallTraceArena, CallTraceNode, DecodedCallData, DecodedCallTrace};
use crate::node::hardhat::{HardhatConsole, HARDHAT_CONSOLE_SELECTOR_PATCHES};
use crate::trace::signatures::SingleSignaturesIdentifier;
use crate::trace::types::KNOWN_ADDRESSES;
use crate::utils::format_token;
use alloy_dyn_abi::{DecodedEvent, DynSolValue, EventExt, FunctionExt, JsonAbiExt};
use alloy_json_abi::{Error, Event, Function, JsonAbi};
use alloy_primitives::Selector;
use itertools::Itertools;
use std::{collections::HashMap, sync::OnceLock};
use zksync_types::{Address, H160};

/// The first four bytes of the call data for a function call specifies the function to be called.
pub const SELECTOR_LEN: usize = 4;

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

    /// A signature identifier for events and functions.
    pub signature_identifier: Option<SingleSignaturesIdentifier>,
}

impl CallTraceDecoder {
    /// Creates a new call trace decoder.
    ///
    /// The call trace decoder always knows how to decode calls to the cheatcode address, as well
    /// as DSTest-style logs.
    pub fn new() -> &'static Self {
        // If you want to take arguments in this function, assign them to the fields of the cloned
        // lazy instead of removing it
        static INIT: OnceLock<CallTraceDecoder> = OnceLock::new();
        INIT.get_or_init(Self::init)
    }

    fn init() -> Self {
        fn hh_funcs() -> impl Iterator<Item = (Selector, Function)> {
            let functions = HardhatConsole::abi::functions();
            let mut functions: Vec<_> = functions
                .into_values()
                .flatten()
                .map(|func| (func.selector(), func))
                .collect();
            let len = functions.len();
            // `functions` is the list of all patched functions; duplicate the unpatched ones
            for (unpatched, patched) in HARDHAT_CONSOLE_SELECTOR_PATCHES.iter() {
                if let Some((_, func)) = functions[..len].iter().find(|(sel, _)| sel == patched) {
                    functions.push((unpatched.into(), func.clone()));
                }
            }
            functions.into_iter()
        }

        let labels: HashMap<H160, String> = KNOWN_ADDRESSES
            .iter()
            .map(|(address, known_address)| (address.clone(), known_address.name.clone()))
            .collect();

        Self {
            contracts: Default::default(),
            labels,
            receive_contracts: Default::default(),
            fallback_contracts: Default::default(),
            functions: hh_funcs()
                .map(|(selector, func)| (selector, vec![func]))
                .collect(),
            signature_identifier: None,
        }
    }

    /// Populates the traces with decoded data by mutating the
    /// [CallTrace] in place. See [CallTraceDecoder::decode_function] and
    /// [CallTraceDecoder::decode_event] for more details.
    pub async fn populate_traces(&self, traces: &mut Vec<CallTraceNode>) {
        for node in traces {
            node.trace.decoded = self.decode_function(&node.trace).await;
            // TODO: Decode logs
            // for log in node.logs.iter_mut() {
            //     log.decoded = self.decode_event(&log.raw_log).await;
            // }
        }
    }

    pub async fn populate_function(&mut self, arena: CallTraceArena) {
        // Collect unique selectors from the arena that are not already in `functions`
        let selectors: Vec<Selector> = arena
            .nodes()
            .iter()
            .filter_map(|node| {
                let input = &node.trace.call.input;
                if input.len() >= SELECTOR_LEN {
                    Some(Selector::from_slice(&input[..SELECTOR_LEN]))
                } else {
                    None
                }
            })
            .filter(|sel| !self.functions.contains_key(sel))
            .collect();

        // Identify the functions
        if !selectors.is_empty() {
            if let Some(identifier) = &self.signature_identifier {
                let identified_functions = identifier
                    .write()
                    .await
                    .identify_functions(&selectors)
                    .await;
                // Iterate over selectors and their corresponding functions
                for (selector, func_option) in selectors.iter().zip(identified_functions.iter()) {
                    if let Some(func) = func_option {
                        self.functions
                            .entry(*selector)
                            .or_default()
                            .push(func.clone());
                    } else {
                        // Optionally handle unidentified functions
                        // For example, you might want to log or store selectors with no matching functions
                        tracing::warn!("No function found for selector: {:?}", selector);
                    }
                }
            }
        }
    }

    /// Decodes a call trace.
    pub async fn decode_function(&self, trace: &CallTrace) -> DecodedCallTrace {
        // if let Some(trace) = precompiles::decode(trace, 1) {
        //     return trace;
        // }
        let label = self.labels.get(&trace.address).cloned();
        let cdata = &trace.call.input;
        // if trace.address == DEFAULT_CREATE2_DEPLOYER {
        //     return DecodedCallTrace {
        //         label,
        //         call_data: Some(DecodedCallData { signature: "create2".to_string(), args: vec![] }),
        //         return_data: self.default_return_data(trace),
        //     };
        // }

        if cdata.len() >= SELECTOR_LEN {
            let selector = &cdata[..SELECTOR_LEN];
            let mut functions = Vec::new();
            //println!("self functions {:?}", self.functions);
            let functions = match self.functions.get(selector) {
                Some(fs) => fs,
                None => {
                    if let Some(identifier) = &self.signature_identifier {
                        if let Some(function) =
                            identifier.write().await.identify_function(selector).await
                        {
                            functions.push(function);
                        }
                    }
                    &functions
                }
            };
            let [func, ..] = &functions[..] else {
                return DecodedCallTrace {
                    label,
                    call_data: None,
                    return_data: self.default_return_data(trace),
                };
            };

            // If traced contract is a fallback contract, check if it has the decoded function.
            // If not, then replace call data signature with `fallback`.
            let mut call_data = self.decode_function_input(trace, func);
            if let Some(fallback_functions) = self.fallback_contracts.get(&trace.address) {
                if !fallback_functions.contains(&func.signature()) {
                    call_data.signature = "fallback()".into();
                }
            }

            DecodedCallTrace {
                label,
                call_data: Some(call_data),
                return_data: self.decode_function_output(trace, functions),
            }
        } else {
            let has_receive = self.receive_contracts.contains(&trace.address);
            let signature = if cdata.is_empty() && has_receive {
                "receive()"
            } else {
                "fallback()"
            }
            .into();
            let args = if cdata.is_empty() {
                Vec::new()
            } else {
                vec![hex::encode(&cdata)]
            };
            DecodedCallTrace {
                label,
                call_data: Some(DecodedCallData { signature, args }),
                return_data: self.default_return_data(trace),
            }
        }
    }

    /// Decodes a function's input into the given trace.
    fn decode_function_input(&self, trace: &CallTrace, func: &Function) -> DecodedCallData {
        let mut args = None;
        if trace.call.input.len() >= SELECTOR_LEN {
            if args.is_none() {
                if let Ok(v) = func.abi_decode_input(&trace.call.input[SELECTOR_LEN..], false) {
                    args = Some(v.iter().map(|value| self.format_value(value)).collect());
                }
            }
        }
        DecodedCallData {
            signature: func.signature(),
            args: args.unwrap_or_default(),
        }
    }

    /// Decodes a function's output into the given trace.
    fn decode_function_output(&self, trace: &CallTrace, funcs: &[Function]) -> Option<String> {
        if !trace.success {
            return self.default_return_data(trace);
        }

        if let Some(values) = funcs
            .iter()
            .find_map(|func| func.abi_decode_output(&trace.call.output, false).ok())
        {
            // Functions coming from an external database do not have any outputs specified,
            // and will lead to returning an empty list of values.
            if values.is_empty() {
                return None;
            }

            return Some(
                values
                    .iter()
                    .map(|value| self.format_value(value))
                    .format(", ")
                    .to_string(),
            );
        }

        None
    }

    /// Prefetches function and event signatures into the identifier cache
    pub async fn prefetch_signatures(&self, nodes: &[CallTraceNode]) {
        let Some(identifier) = &self.signature_identifier else {
            return;
        };

        // TODO: events and logs
        // let events_it = nodes
        //     .iter()
        //     .flat_map(|node| node.logs.iter().filter_map(|log| log.raw_log.topics().first()))
        //     .unique();
        // identifier.write().await.identify_events(events_it).await;

        let funcs_it = nodes
            .iter()
            .filter_map(|n| match n.trace.address {
                _ => n.trace.call.input.get(..SELECTOR_LEN),
            })
            .filter(|v| !self.functions.contains_key(*v));

        identifier.write().await.identify_functions(funcs_it).await;
    }

    /// The default decoded return data for a trace.
    fn default_return_data(&self, trace: &CallTrace) -> Option<String> {
        (!trace.success).then(|| "Revert - to do decode output strings".to_string())
    }

    /// Pretty-prints a value.
    fn format_value(&self, value: &DynSolValue) -> String {
        if let DynSolValue::Address(addr) = value {
            // TODO: handle error
            let zksync_address = Address::from(<[u8; 20]>::try_from(addr.0.as_slice()).unwrap());

            if let Some(label) = self.labels.get(&zksync_address) {
                return format!("{label}: [{addr}]");
            }
        }
        format_token(value, false)
    }
}

/// Build a new [CallTraceDecoder].
#[derive(Default)]
#[must_use = "builders do nothing unless you call `build` on them"]
pub struct CallTraceDecoderBuilder {
    decoder: CallTraceDecoder,
}

impl CallTraceDecoderBuilder {
    /// Create a new builder.
    #[inline]
    pub fn new() -> Self {
        Self {
            decoder: CallTraceDecoder::new().clone(),
        }
    }

    /// Add known labels to the decoder.
    #[inline]
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = (Address, String)>) -> Self {
        self.decoder.labels.extend(labels);
        self
    }

    /// Sets the signature identifier for events and functions.
    #[inline]
    pub fn with_signature_identifier(mut self, identifier: SingleSignaturesIdentifier) -> Self {
        self.decoder.signature_identifier = Some(identifier);
        self
    }

    /// Build the decoder.
    #[inline]
    pub fn build(self) -> CallTraceDecoder {
        self.decoder
    }
}
