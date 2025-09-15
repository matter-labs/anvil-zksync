use anvil_zksync_common::{address_map, utils::format::write_interspersed};
use zksync_multivm::interface::{Call, Halt, VmEvent};
use zksync_types::{
    l2_to_l1_log::{SystemL2ToL1Log, UserL2ToL1Log},
    web3::Bytes,
    Address, H160, H256, U256,
};

use crate::numbers::SignedU256;

/// Enum to represent both user and system L1-L2 logs
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum L2L1Log {
    User(UserL2ToL1Log),
    System(SystemL2ToL1Log),
}

/// A ZKsync event log object. #[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LogData {
    /// The indexed topic list.
    topics: Vec<H256>,
    /// The plain data.
    pub data: Bytes,
}

impl LogData {
    /// Creates a new log, without length-checking. This allows creation of
    /// invalid logs. May be safely used when the length of the topic list is
    /// known to be 4 or less.
    #[inline]
    pub const fn new_unchecked(topics: Vec<H256>, data: Bytes) -> Self {
        Self { topics, data }
    }

    /// Creates a new log.
    #[inline]
    pub fn new(topics: Vec<H256>, data: Bytes) -> Option<Self> {
        let this = Self::new_unchecked(topics, data);
        this.is_valid().then_some(this)
    }

    /// Creates a new empty log.
    #[inline]
    pub const fn empty() -> Self {
        Self {
            topics: Vec::new(),
            data: Bytes(Vec::new()),
        }
    }

    /// True if valid, false otherwise.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.topics.len() <= 4
    }

    /// Get the topic list.
    #[inline]
    pub fn topics(&self) -> &[H256] {
        &self.topics
    }

    /// Get the topic list, mutably. This gives access to the internal
    /// array, without allowing extension of that array.
    #[inline]
    pub fn topics_mut(&mut self) -> &mut [H256] {
        &mut self.topics
    }

    /// Get a mutable reference to the topic list. This allows creation of
    /// invalid logs.
    #[inline]
    pub fn topics_mut_unchecked(&mut self) -> &mut Vec<H256> {
        &mut self.topics
    }

    /// Set the topic list, without length-checking. This allows creation of
    /// invalid logs.
    #[inline]
    pub fn set_topics_unchecked(&mut self, topics: Vec<H256>) {
        self.topics = topics;
    }

    /// Set the topic list, truncating to 4 topics.
    #[inline]
    pub fn set_topics_truncating(&mut self, mut topics: Vec<H256>) {
        topics.truncate(4);
        self.set_topics_unchecked(topics);
    }

    /// Consumes the log data, returning the topic list and the data.
    #[inline]
    pub fn split(self) -> (Vec<H256>, Bytes) {
        (self.topics, self.data)
    }
}

pub type Label = String;
pub type Word32 = [u8; 32];
pub type Word24 = [u8; 24];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LabeledAddress {
    pub label: Option<Label>,
    pub address: Address,
}

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Attribution: the type `DecodedValue` was adapted                                                            //
// from the type `alloy::dyn_abi::DynSolValue` of the crate `alloy_dyn_abi`                                    //
//                                                                                                             //
// Full credit goes to its authors. See the original implementation here:                                      //
// https://github.com/alloy-rs/core/blob/main/crates/dyn-abi/src/dynamic/value.rs                              //
//                                                                                                             //
// Note: This type is used under the terms of the original project's license.                                  //
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///
/// A decoded value in trace.
///
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedValue {
    /// A boolean.
    Bool(bool),
    /// A signed integer. The second parameter is the number of bits, not bytes.
    Int(SignedU256),
    /// An unsigned integer. The second parameter is the number of bits, not bytes.
    Uint(U256),
    /// A fixed-length byte array. The second parameter is the number of bytes.
    FixedBytes(Word32, usize),
    /// An address.
    Address(LabeledAddress),
    /// A function pointer.
    Function(Word24),

    /// A dynamic-length byte array.
    Bytes(Vec<u8>),
    /// A string.
    String(String),

    /// A dynamically-sized array of values.
    Array(Vec<DecodedValue>),
    /// A fixed-size array of values.
    FixedArray(Vec<DecodedValue>),
    /// A tuple of values.
    Tuple(Vec<DecodedValue>),

    /// A named struct, treated as a tuple with a name parameter.
    CustomStruct {
        /// The name of the struct.
        name: String,
        /// The struct's prop names, in declaration order.
        prop_names: Vec<String>,
        /// The inner types.
        tuple: Vec<DecodedValue>,
    },
}

impl std::fmt::Display for LabeledAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let LabeledAddress { label, address } = self;

        if let Some(label) = label {
            f.write_fmt(format_args!("{label}: "))?;
        }
        write!(f, "[0x{}]", hex::encode(address))
    }
}

impl std::fmt::Display for DecodedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodedValue::Bool(inner) => inner.fmt(f),
            DecodedValue::Int(inner) => inner.fmt(f),
            DecodedValue::Uint(inner) => inner.fmt(f),
            DecodedValue::FixedBytes(word, size) => {
                f.write_fmt(format_args!("0x{}", hex::encode(&word[..*size])))
            }
            DecodedValue::Address(labeled_address) => labeled_address.fmt(f),
            DecodedValue::Function(inner) => f.write_fmt(format_args!("0x{}", hex::encode(inner))),
            DecodedValue::Bytes(inner) => f.write_fmt(format_args!("0x{}", hex::encode(inner))),
            DecodedValue::String(inner) => f.write_str(&inner.escape_debug().to_string()),
            DecodedValue::Array(vec) | DecodedValue::FixedArray(vec) => {
                f.write_str("[")?;
                write_interspersed(f, vec.iter(), ", ")?;
                f.write_str("]")
            }
            DecodedValue::Tuple(vec) => {
                f.write_str("(")?;
                write_interspersed(f, vec.iter(), ", ")?;
                f.write_str(")")
            }
            DecodedValue::CustomStruct { tuple, .. } => DecodedValue::Tuple(tuple.clone()).fmt(f),
        }
    }
}

impl DecodedValue {
    pub fn as_bytes(&self) -> Option<&Vec<u8>> {
        if let Self::Bytes(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

impl std::fmt::Display for DecodedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodedError::Empty => write!(f, ""),
            DecodedError::CustomError { name, fields } => {
                write!(f, "{name}(")?;
                write_interspersed(f, fields.iter(), ", ")?;
                write!(f, ")")
            }
            DecodedError::GenericCustomError { selector, raw } => {
                write!(
                    f,
                    "custom error with function selector 0x{}",
                    hex::encode(selector)
                )?;
                if !raw.is_empty() {
                    write!(f, ": ")?;
                    match std::str::from_utf8(raw) {
                        Ok(data) => write!(f, "{data}"),
                        Err(_) => write!(f, "{}", hex::encode(raw)),
                    }
                } else {
                    Ok(())
                }
            }
            DecodedError::Revert(message) => write!(f, "{message}"),
            DecodedError::Panic(message) => write!(f, "{message}"),
            DecodedError::Raw(data) => {
                if !data.is_empty() {
                    let var_name = write!(
                        f,
                        "{}",
                        anvil_zksync_common::utils::format::trimmed_hex(data)
                    );
                    var_name?;
                } else {
                    write!(f, "<empty revert data>")?;
                }
                Ok(())
            }
            DecodedError::String(message) => write!(f, "{message}"),
        }
    }
}

impl std::fmt::Display for DecodedRevertData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodedRevertData::Value(value) => value.fmt(f),
            DecodedRevertData::Error(error) => error.fmt(f),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodedReturnData {
    NormalReturn(Vec<DecodedValue>),
    Revert(DecodedRevertData),
}

impl Default for DecodedReturnData {
    fn default() -> Self {
        Self::NormalReturn(vec![])
    }
}

impl std::fmt::Display for DecodedReturnData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodedReturnData::NormalReturn(values) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    write_interspersed(f, values.iter(), ", ")
                }
            }
            DecodedReturnData::Revert(revert_data) => revert_data.fmt(f),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodedError {
    Empty,
    CustomError {
        name: String,
        fields: Vec<DecodedValue>,
    },
    GenericCustomError {
        selector: [u8; 4],
        raw: Vec<u8>,
    },
    Revert(String),
    Panic(String),
    Raw(Vec<u8>),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodedRevertData {
    Value(DecodedValue),
    Error(DecodedError),
}

/// Decoded call data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecodedCallData {
    /// The function signature.
    pub signature: String,
    /// The function arguments.
    pub args: Vec<DecodedValue>,
}

/// Additional decoded data enhancing the [CallTrace].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecodedCallTrace {
    /// Optional decoded label for the call.
    pub label: Option<Label>,
    /// Optional decoded return data.
    pub return_data: DecodedReturnData,
    /// Optional decoded call data.
    pub call_data: Option<DecodedCallData>,
}

/// Additional decoded data enhancing the [CallLog].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecodedCallLog {
    /// The decoded event name.
    pub name: Option<String>,
    /// The decoded log parameters, a vector of the parameter name (e.g. foo) and the parameter
    /// value (e.g. 0x9d3...45ca).
    pub params: Option<Vec<(String, DecodedValue)>>,
}

/// A log with optional decoded data.
#[derive(Clone, Debug, Default)]
pub struct CallLog {
    /// The raw log data.
    pub raw_log: VmEvent,
    /// Optional complementary decoded log data.
    pub decoded: DecodedCallEvent,
    /// The position of the log relative to subcalls within the same trace.
    pub position: u64,
}

/// A log with optional decoded data.
#[derive(Clone, Debug)]
pub struct L2L1Logs {
    /// The raw log data.
    pub raw_log: L2L1Log,
    /// The position of the log relative to subcalls within the same trace.
    pub position: u64,
}

impl CallLog {
    /// Sets the position of the log.
    #[inline]
    pub fn with_position(mut self, position: u64) -> Self {
        self.position = position;
        self
    }
}

/// A variant of [`zksync_multivm::interface::ExecutionResult`] but with revert reason replaced by a
/// human-readable string. This is needed because [`Call::error`] is already a human-readable string
/// parsing which would be error-prone.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionResult {
    /// Returned successfully
    Success { output: Vec<u8> },
    /// Reverted by contract
    Revert { output: String },
    /// Reverted for various reasons
    Halt { reason: Halt },
}

impl ExecutionResult {
    /// Returns `true` if the execution was failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Revert { .. } | Self::Halt { .. })
    }
}

impl From<zksync_multivm::interface::ExecutionResult> for ExecutionResult {
    fn from(value: zksync_multivm::interface::ExecutionResult) -> Self {
        match value {
            zksync_multivm::interface::ExecutionResult::Success { output } => {
                ExecutionResult::Success { output }
            }
            zksync_multivm::interface::ExecutionResult::Revert { output } => {
                ExecutionResult::Revert {
                    output: output.to_user_friendly_string(),
                }
            }
            zksync_multivm::interface::ExecutionResult::Halt { reason } => {
                ExecutionResult::Halt { reason }
            }
        }
    }
}

/// A trace of a call with optional decoded data.
#[derive(Clone, Debug)]
pub struct CallTrace {
    /// Whether the call was successful.
    pub success: bool,
    /// The caller address.
    pub caller: Address,
    /// The target address of this call.
    ///
    /// This is:
    /// - [`CallKind::Call`] and alike: the callee, the address of the contract being called
    /// - [`CallKind::Create`] and alike: the address of the created contract
    pub address: Address,
    /// The execution result of the call.
    pub execution_result: ExecutionResult,
    /// Optional complementary decoded call data.
    pub decoded: DecodedCallTrace,
    /// The call trace
    pub call: Call,
}

/// Decoded ZKSync event data enhancing the [CallLog].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecodedCallEvent {
    /// The decoded event name.
    pub name: Option<String>,
    /// The decoded log parameters, a vector of the parameter name (e.g. foo) and the parameter
    /// value (e.g. 0x9d3...45ca).
    pub params: Option<Vec<(String, DecodedValue)>>,
}

/// A node in the arena
#[derive(Clone, Debug)]
pub struct CallTraceNode {
    /// Parent node index in the arena
    pub parent: Option<usize>,
    /// Children node indexes in the arena
    pub children: Vec<usize>,
    /// This node's index in the arena
    pub idx: usize,
    /// The call trace
    pub trace: CallTrace,
    /// Event logs
    pub logs: Vec<CallLog>,
    /// L2-L1 logs
    pub l2_l1_logs: Vec<L2L1Logs>,
    /// Ordering of child calls and logs
    pub ordering: Vec<TraceMemberOrder>,
}

impl Default for CallTraceNode {
    fn default() -> Self {
        Self {
            parent: None,
            children: Vec::new(),
            idx: 0,
            trace: CallTrace {
                success: true,
                caller: H160::zero(),
                address: H160::zero(),
                execution_result: ExecutionResult::Success { output: vec![] },
                decoded: DecodedCallTrace::default(),
                call: Call::default(),
            },
            logs: Vec::new(),
            l2_l1_logs: Vec::new(),
            ordering: Vec::new(),
        }
    }
}

/// Ordering enum for calls, logs and steps
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceMemberOrder {
    /// Contains the index of the corresponding log
    Log(usize),
    /// Contains the index of the corresponding trace node
    Call(usize),
    /// Contains the index of the corresponding l1-l2 log
    L1L2Log(usize),
}

/// An arena of recorded traces.
///
/// This type will be populated via the [TracingInspector](super::TracingInspector).
#[derive(Clone, Debug)]
pub struct CallTraceArena {
    /// The arena of recorded trace nodes
    pub arena: Vec<CallTraceNode>,
}

impl Default for CallTraceArena {
    fn default() -> Self {
        let root_node = CallTraceNode {
            parent: None,
            children: Vec::new(),
            idx: 0,
            trace: CallTrace {
                success: true,
                caller: H160::zero(),
                address: H160::zero(),
                execution_result: ExecutionResult::Success { output: vec![] },
                decoded: DecodedCallTrace::default(),
                call: Call::default(),
            },
            logs: Vec::new(),
            l2_l1_logs: Vec::new(),
            ordering: Vec::new(),
        };

        // Initialize CallTraceArena with the root node
        Self {
            arena: vec![root_node],
        }
    }
}

impl CallTraceArena {
    /// Adds a node to the arena, updating parent–child relationships and ordering.
    pub fn add_node(&mut self, parent: Option<usize>, mut node: CallTraceNode) -> usize {
        let idx = self.arena.len();
        node.idx = idx;
        node.parent = parent;

        // Build ordering for the node based on its logs.
        node.ordering = (0..node.logs.len()).map(TraceMemberOrder::Log).collect();

        self.arena.push(node);

        // Update parent's children and ordering if a parent exists.
        if let Some(parent_idx) = parent {
            self.arena[parent_idx].children.push(idx);
            let child_local_idx = self.arena[parent_idx].children.len() - 1;
            self.arena[parent_idx]
                .ordering
                .push(TraceMemberOrder::Call(child_local_idx));
        }
        idx
    }

    /// Returns the nodes in the arena.
    pub fn nodes(&self) -> &[CallTraceNode] {
        &self.arena
    }

    /// Returns a mutable reference to the nodes in the arena.
    pub fn nodes_mut(&mut self) -> &mut Vec<CallTraceNode> {
        &mut self.arena
    }

    /// Consumes the arena and returns the nodes.
    pub fn into_nodes(self) -> Vec<CallTraceNode> {
        self.arena
    }

    /// Clears the arena
    ///
    /// Note that this method has no effect on the allocated capacity of the arena.
    #[inline]
    pub fn clear(&mut self) {
        self.arena.clear();
        self.arena.push(Default::default());
    }

    /// Filters out precompile nodes from the arena.
    pub fn filter_out_precompiles(&mut self) {
        self.arena
            .retain(|node| !address_map::is_precompile(&node.trace.address));
    }

    /// Filters out system contracts nodes from the arena.
    pub fn filter_out_system_contracts(&mut self) {
        self.arena
            .retain(|node| !address_map::is_system(&node.trace.address));
    }
}

/// A trait for displaying the execution result.
pub trait ExecutionResultDisplay {
    fn display(&self) -> String;
}

impl ExecutionResultDisplay for ExecutionResult {
    fn display(&self) -> String {
        match self {
            ExecutionResult::Success { .. } => "Success".to_string(),
            ExecutionResult::Revert { output } => format!("Revert: {output}"),
            ExecutionResult::Halt { reason } => format!("Halt: {reason:?}"),
        }
    }
}
