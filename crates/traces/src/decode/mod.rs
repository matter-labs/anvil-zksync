////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Attribution: File adapted from the Foundry `evm` crate for ZKsync usage                                        //
//                                                                                                        //
// Full credit goes to its authors. See the original implementation here:                                 //
// https://github.com/foundry-rs/foundry/blob/master/crates/evm/traces/src/decoder/mod.rs.                //
//                                                                                                        //
// Note: These methods are used under the terms of the original project's license.                        //
////////////////////////////////////////////////////////////////////////////////////////////////////////////

use crate::identifier::SingleSignaturesIdentifier;
use alloy::dyn_abi::{DecodedEvent, DynSolValue, EventExt, FunctionExt, JsonAbiExt};
use alloy::json_abi::{Event, Function};
use alloy::primitives::{LogData, Selector, Sign, B256};
use anvil_zksync_common::address_map::KNOWN_ADDRESSES;
use anvil_zksync_types::numbers::SignedU256;
use anvil_zksync_types::traces::{
    CallTrace, CallTraceNode, DecodedCallData, DecodedCallEvent, DecodedCallTrace,
    DecodedReturnData, DecodedRevertData, DecodedValue, LabeledAddress, Word32,
};
use itertools::Itertools;
use std::collections::{BTreeMap, HashMap};
use zksync_multivm::interface::VmEvent;
use zksync_types::{Address, H160};

pub mod revert_decoder;
use revert_decoder::RevertDecoder;

/// The first four bytes of the call data for a function call specifies the function to be called.
pub const SELECTOR_LEN: usize = 4;

/// A base struct for builders. If you want to start building from a predefined
/// [CallTraceDecoder] instance, create your own builder like that:
///
/// ```rust,ignore
/// pub struct CallTraceDecoderBuilder;
/// impl CallTraceDecoderBuilder {
/// #[inline]
/// pub fn default() -> CallTraceDecoderBuilderBase {
/// // Construct an instance here
/// }
/// }
/// ```
#[must_use = "builders do nothing unless you call `build` on them"]
pub struct CallTraceDecoderBuilderBase {
    decoder: CallTraceDecoder,
}

impl CallTraceDecoderBuilderBase {
    /// Create a new builder.
    #[inline]
    pub fn new(starting_decoder_state: CallTraceDecoder) -> Self {
        Self {
            decoder: starting_decoder_state,
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

/// The call trace decoder.
///
/// The decoder collects address labels which it
/// then uses to decode the call trace.
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
    /// All known events.
    pub events: BTreeMap<(B256, usize), Vec<Event>>,
    /// Revert decoder. Contains all known custom errors.
    pub revert_decoder: RevertDecoder,
    /// All known functions.
    pub functions: HashMap<Selector, Vec<Function>>,
    /// A signature identifier for events and functions.
    pub signature_identifier: Option<SingleSignaturesIdentifier>,
}

impl CallTraceDecoder {
    /// Creates a new call trace decoder.
    pub fn new(
        functions: HashMap<Selector, Vec<Function>>,
        events: BTreeMap<(B256, usize), Vec<Event>>,
    ) -> Self {
        // Add known addresses (system contracts, precompiles) to the labels
        let labels: HashMap<H160, String> = KNOWN_ADDRESSES
            .iter()
            .map(|(address, known_address)| (*address, known_address.name.clone()))
            .collect();

        Self {
            contracts: Default::default(),
            labels,
            receive_contracts: Default::default(),
            fallback_contracts: Default::default(),
            functions,
            events,
            revert_decoder: Default::default(),
            signature_identifier: None,
        }
    }

    /// Populates the traces with decoded data by mutating the
    /// [CallTrace] in place. See [CallTraceDecoder::decode_function] and
    /// [CallTraceDecoder::decode_event] for more details.
    pub async fn populate_traces(&self, traces: &mut Vec<CallTraceNode>) {
        for node in traces {
            node.trace.decoded = self.decode_function(&node.trace).await;
            for log in node.logs.iter_mut() {
                log.decoded = self.decode_event(&log.raw_log).await;
            }
        }
    }

    /// Decodes a call trace.
    pub async fn decode_function(&self, trace: &CallTrace) -> DecodedCallTrace {
        let label = self.labels.get(&trace.address).cloned();
        let cdata = &trace.call.input;

        if cdata.len() >= SELECTOR_LEN {
            let selector = &cdata[..SELECTOR_LEN];
            let mut functions = Vec::new();
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
                vec![DecodedValue::Bytes(cdata.clone())]
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
        if trace.call.input.len() >= SELECTOR_LEN && args.is_none() {
            if let Ok(v) = func.abi_decode_input(&trace.call.input[SELECTOR_LEN..], false) {
                args = Some(
                    v.into_iter()
                        .map(|value| -> DecodedValue {
                            let decoded = decode_value(value);
                            label_value(decoded, |value| self.labels.get(value).cloned())
                        })
                        .collect(),
                );
            }
        }
        DecodedCallData {
            signature: func.signature(),
            args: args.unwrap_or_default(),
        }
    }

    /// Decodes a function's output into the given trace.
    fn decode_function_output(&self, trace: &CallTrace, funcs: &[Function]) -> DecodedReturnData {
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
                return DecodedReturnData::NormalReturn(vec![]);
            }

            return DecodedReturnData::NormalReturn(
                values
                    .into_iter()
                    .map(|value| self.decode_value(value))
                    .collect(),
            );
        }
        DecodedReturnData::NormalReturn(vec![])
    }

    /// Decodes an event from ZKsync type VmEvent.
    pub async fn decode_event(&self, vm_event: &VmEvent) -> DecodedCallEvent {
        let Some(&t0) = vm_event.indexed_topics.first() else {
            return DecodedCallEvent {
                name: None,
                params: None,
            };
        };

        let mut events = Vec::new();
        let b256_t0 = B256::from_slice(t0.as_bytes());
        let key = (b256_t0, indexed_inputs_zksync(vm_event) - 1);
        let events = match self.events.get(&key) {
            Some(es) => es,
            None => {
                if let Some(identifier) = &self.signature_identifier {
                    if let Some(event) = identifier.write().await.identify_event(&t0[..]).await {
                        events.push(get_indexed_event_from_vm_event(event, vm_event));
                    }
                }
                &events
            }
        };
        let log_data = vm_event_to_log_data(vm_event);
        for event in events {
            if let Ok(decoded) = event.decode_log(&log_data, false) {
                let params = reconstruct_params(event, &decoded);
                return DecodedCallEvent {
                    name: Some(event.name.clone()),
                    params: Some(
                        params
                            .into_iter()
                            .zip(event.inputs.iter())
                            .map(|(param, input)| -> (String, DecodedValue) {
                                // undo patched names
                                let name: String = input.name.clone();
                                let value: DecodedValue = self.decode_value(param);
                                (name, value)
                            })
                            .collect(),
                    ),
                };
            }
        }

        DecodedCallEvent {
            name: None,
            params: None,
        }
    }

    /// Prefetches function and event signatures into the identifier cache
    pub async fn prefetch_signatures(&self, nodes: &[CallTraceNode]) {
        let Some(identifier) = &self.signature_identifier else {
            return;
        };

        let events: Vec<_> = nodes
            .iter()
            .flat_map(|node| {
                node.logs
                    .iter()
                    .filter_map(|log| log.raw_log.indexed_topics.first().cloned())
            })
            .unique()
            .collect();
        identifier.write().await.identify_events(events).await;

        let funcs: Vec<_> = nodes
            .iter()
            .filter_map(|n| n.trace.call.input.get(..SELECTOR_LEN).map(|s| s.to_vec()))
            .filter(|s| !self.functions.contains_key(s.as_slice()))
            .collect();
        identifier.write().await.identify_functions(funcs).await;

        // Need to decode revert reasons and errors as well
    }

    /// The default decoded return data for a trace.
    fn default_return_data(&self, trace: &CallTrace) -> DecodedReturnData {
        if trace.success {
            DecodedReturnData::NormalReturn(vec![])
        } else {
            DecodedReturnData::Revert(DecodedRevertData::Error(
                self.revert_decoder.decode(&trace.call.output),
            ))
        }
    }

    fn decode_value(&self, value: DynSolValue) -> DecodedValue {
        label_value(decode_value(value), |addr| self.labels.get(addr).cloned())
    }
}

pub fn label_value(
    value: DecodedValue,
    labeler: impl Fn(&Address) -> Option<String>,
) -> DecodedValue {
    fn label_values(
        vec: Vec<DecodedValue>,
        labeler: &dyn Fn(&Address) -> Option<String>,
    ) -> Vec<DecodedValue> {
        vec.into_iter().map(|v| label_value(v, labeler)).collect()
    }

    match value {
        DecodedValue::Address(LabeledAddress {
            label: None,
            address,
        }) => {
            let label = labeler(&address);
            if let Some(label) = &label {
                tracing::info!("Address {address:?} resolved to the label {label}.");
            };
            DecodedValue::Address(LabeledAddress { label, address })
        }
        DecodedValue::Array(vec) => DecodedValue::Array(label_values(vec, &labeler)),
        DecodedValue::FixedArray(vec) => DecodedValue::FixedArray(label_values(vec, &labeler)),
        DecodedValue::Tuple(vec) => DecodedValue::Tuple(label_values(vec, &labeler)),
        DecodedValue::CustomStruct {
            name,
            prop_names,
            tuple,
        } => DecodedValue::CustomStruct {
            name,
            prop_names,
            tuple: label_values(tuple, &labeler),
        },
        other => other,
    }
}

pub fn decode_value(value: DynSolValue) -> DecodedValue {
    match value {
        DynSolValue::Bool(b) => DecodedValue::Bool(b),
        DynSolValue::Int(i, _) => match i.into_sign_and_abs() {
            (Sign::Positive, value) => {
                DecodedValue::Int(zksync_types::U256(value.into_limbs()).into())
            }
            (Sign::Negative, value) => DecodedValue::Int(SignedU256 {
                sign: anvil_zksync_types::numbers::Sign::Negative,
                inner: zksync_types::U256(value.into_limbs()),
            }),
        },
        DynSolValue::Uint(u, _) => DecodedValue::Uint(zksync_types::U256(u.into_limbs())),
        DynSolValue::FixedBytes(word, size) => {
            // Convert Word to Word32 (assuming proper conversion exists)
            let word32 = Word32::from(word); // This assumes there's a conversion method
            DecodedValue::FixedBytes(word32, size)
        }
        DynSolValue::Address(addr) => {
            let address = Address::from(addr.0 .0);
            DecodedValue::Address(LabeledAddress {
                label: None,
                address,
            })
        }
        DynSolValue::Function(func) => DecodedValue::Function(*func.0),
        DynSolValue::Bytes(bytes) => DecodedValue::Bytes(bytes),
        DynSolValue::String(s) => DecodedValue::String(s),
        DynSolValue::Array(arr) => {
            let decoded = arr.into_iter().map(decode_value).collect();
            DecodedValue::Array(decoded)
        }
        DynSolValue::FixedArray(arr) => {
            let decoded = arr.into_iter().map(decode_value).collect();
            DecodedValue::FixedArray(decoded)
        }
        DynSolValue::Tuple(tup) => {
            let decoded = tup.into_iter().map(decode_value).collect();
            DecodedValue::Tuple(decoded)
        }
        DynSolValue::CustomStruct {
            name,
            prop_names,
            tuple,
        } => {
            let decoded = tuple.into_iter().map(decode_value).collect();
            DecodedValue::CustomStruct {
                name,
                prop_names,
                tuple: decoded,
            }
        }
    }
}

/// Restore the order of the params of a decoded event,
/// as Alloy returns the indexed and unindexed params separately.
fn reconstruct_params(event: &Event, decoded: &DecodedEvent) -> Vec<DynSolValue> {
    let mut indexed = 0;
    let mut unindexed = 0;
    let mut inputs = vec![];
    for input in event.inputs.iter() {
        // Prevent panic of event `Transfer(from, to)` decoded with a signature
        // `Transfer(address indexed from, address indexed to, uint256 indexed tokenId)` by making
        // sure the event inputs is not higher than decoded indexed / un-indexed values.
        if input.indexed && indexed < decoded.indexed.len() {
            inputs.push(decoded.indexed[indexed].clone());
            indexed += 1;
        } else if unindexed < decoded.body.len() {
            inputs.push(decoded.body[unindexed].clone());
            unindexed += 1;
        }
    }

    inputs
}
fn indexed_inputs_zksync(event: &VmEvent) -> usize {
    event.indexed_topics.len()
}

/// Given an `Event` without indexed parameters and a `VmEvent`, it tries to
/// return the `Event` with the proper indexed parameters. Otherwise,
/// it returns the original `Event`.
pub fn get_indexed_event_from_vm_event(mut event: Event, vm_event: &VmEvent) -> Event {
    if !event.anonymous && vm_event.indexed_topics.len() > 1 {
        let indexed_params = vm_event.indexed_topics.len() - 1;
        let num_inputs = event.inputs.len();
        let num_address_params = event.inputs.iter().filter(|p| p.ty == "address").count();

        event
            .inputs
            .iter_mut()
            .enumerate()
            .for_each(|(index, param)| {
                if param.name.is_empty() {
                    param.name = format!("param{index}");
                }
                if num_inputs == indexed_params
                    || (num_address_params == indexed_params && param.ty == "address")
                {
                    param.indexed = true;
                }
            })
    }
    event
}

/// Converts a `VmEvent` to a `LogData`.
pub fn vm_event_to_log_data(event: &VmEvent) -> LogData {
    LogData::new_unchecked(
        event
            .indexed_topics
            .iter()
            .map(|h| B256::from_slice(h.as_bytes()))
            .collect(),
        event.value.clone().into(),
    )
}
