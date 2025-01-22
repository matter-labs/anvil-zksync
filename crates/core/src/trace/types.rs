use zksync_multivm::interface::{Call, VmExecutionResultAndLogs, ExecutionResult};
use zksync_types::{Address, H256, H160};
use zksync_types::web3::Bytes;
use alloy_primitives::FixedBytes;
use std::collections::HashMap;
use serde::Deserialize;
use lazy_static::lazy_static;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum ContractType {
    System,
    Precompile,
    Popular,
    Unknown,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KnownAddress {
    pub address: H160,
    pub name: String,
    contract_type: ContractType,
}

lazy_static! {
    /// Loads the known contact addresses from the JSON file.
    pub static ref KNOWN_ADDRESSES: HashMap<H160, KnownAddress> = {
        let json_value = serde_json::from_slice(include_bytes!("../data/address_map.json")).unwrap();
        let pairs: Vec<KnownAddress> = serde_json::from_value(json_value).unwrap();

        pairs
            .into_iter()
            .map(|entry| (entry.address, entry))
            .collect()
    };
}

/// Solidity contract functions are addressed using the first four bytes of the
/// Keccak-256 hash of their signature.
pub type Selector = FixedBytes<4>;

/// An Ethereum event log object.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(derive_arbitrary::Arbitrary, proptest_derive::Arbitrary))]
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
        Self { topics: Vec::new(), data: Bytes(Vec::new()) }
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

/// Decoded call data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecodedCallData {
    /// The function signature.
    pub signature: String,
    /// The function arguments.
    pub args: Vec<String>,
}

/// Additional decoded data enhancing the [CallTrace].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecodedCallTrace {
    /// Optional decoded label for the call.
    pub label: Option<String>,
    /// Optional decoded return data.
    pub return_data: Option<String>,
    /// Optional decoded call data.
    pub call_data: Option<DecodedCallData>,
}

/// A trace of a call with optional decoded data.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CallTrace {
    /// The depth of the call.
    pub depth: usize,
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
    /// Whether this is a call to a precompile.
    ///
    /// Note: This is optional because not all tracers make use of this.
   // pub maybe_precompile: Option<bool>,
    pub execution_result: VmExecutionResultAndLogs,
    /// Optional complementary decoded call data.
    pub decoded: DecodedCallTrace,
    pub call: Call,
}


/// A node in the arena
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CallTraceNode {
    /// Parent node index in the arena
    pub parent: Option<usize>,
    /// Children node indexes in the arena
    pub children: Vec<usize>,
    /// This node's index in the arena
    pub idx: usize,
    /// The call trace
    pub trace: CallTrace,
    /// Ordering of child calls and logs
    pub ordering: Vec<TraceMemberOrder>,
}

/// Ordering enum for calls, logs and steps
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TraceMemberOrder {
    /// Contains the index of the corresponding log
    Log(usize),
    /// Contains the index of the corresponding trace node
    Call(usize),
}

/// An arena of recorded traces.
///
/// This type will be populated via the [TracingInspector](super::TracingInspector).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CallTraceArena {
    /// The arena of recorded trace nodes
    pub(crate) arena: Vec<CallTraceNode>,
}

// impl Default for CallTraceArena {
//     fn default() -> Self {
//         // The first node is the root node
//         Self { arena: vec![Default::default()] }
//     }
// }

impl CallTraceArena {
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
        // TODO: circle back to this
        //self.arena.push(Default::default());
    }

    /// Pushes a new trace into the arena, returning the trace ID
    ///
    /// This appends a new trace to the arena, and also inserts a new entry in the node's parent
    /// node children set if `attach_to_parent` is `true`. E.g. if calls to precompiles should
    /// not be included in the call graph this should be called with [PushTraceKind::PushOnly].
    pub(crate) fn push_trace(
        &mut self,
        mut entry: usize,
        kind: PushTraceKind,
        new_trace: CallTrace,
    ) -> usize {
        loop {
            match new_trace.depth {
                // The entry node, just update it
                0 => {
                    self.arena[0].trace = new_trace;
                    return 0;
                }
                // We found the parent node, add the new trace as a child
                _ if self.arena[entry].trace.depth == new_trace.depth - 1 => {
                    let id = self.arena.len();
                    let node = CallTraceNode {
                        parent: Some(entry),
                        trace: new_trace,
                        idx: id,
                        children: Vec::new(),
                        ordering: Vec::new(),
                    };
                    self.arena.push(node);

                    // also track the child in the parent node
                    if kind.is_attach_to_parent() {
                        let parent = &mut self.arena[entry];
                        let trace_location = parent.children.len();
                        parent.ordering.push(TraceMemberOrder::Call(trace_location));
                        parent.children.push(id);
                    }

                    return id;
                }
                _ => {
                    // We haven't found the parent node, go deeper
                    entry = *self.arena[entry].children.last().expect("Disconnected trace");
                }
            }
        }
    }
}

/// How to push a trace into the arena
pub(crate) enum PushTraceKind {
    /// This will _only_ push the trace into the arena.
    PushOnly,
    /// This will push the trace into the arena, and also insert a new entry in the node's parent
    /// node children set.
    PushAndAttachToParent,
}

impl PushTraceKind {
    #[inline]
    const fn is_attach_to_parent(&self) -> bool {
        matches!(self, Self::PushAndAttachToParent)
    }
}

pub trait ExecutionResultDisplay {
    fn display(&self) -> String;
}

impl ExecutionResultDisplay for ExecutionResult {
    fn display(&self) -> String {
        match self {
            ExecutionResult::Success { .. } => "Success".to_string(),
            ExecutionResult::Revert { output } => format!("Revert: {}", output),
            ExecutionResult::Halt { reason } => format!("Halt: {:?}", reason),
        }
    }
}
