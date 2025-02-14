use lazy_static::lazy_static;
use serde::Deserialize;
use std::collections::HashMap;
use zksync_multivm::interface::{Call, ExecutionResult, VmEvent, VmExecutionResultAndLogs};
use zksync_types::web3::Bytes;
use zksync_types::{Address, H160, H256};

// TODO: duplicated types from existing formatter.rs
// will be consolidated pending feedback
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

/// A ZKsync event log object.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(
    feature = "arbitrary",
    derive(derive_arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
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

/// Additional decoded data enhancing the [CallLog].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecodedCallLog {
    /// The decoded event name.
    pub name: Option<String>,
    /// The decoded log parameters, a vector of the parameter name (e.g. foo) and the parameter
    /// value (e.g. 0x9d3...45ca).
    pub params: Option<Vec<(String, String)>>,
}

/// A log with optional decoded data.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CallLog {
    /// The raw log data.
    pub raw_log: VmEvent,
    /// Optional complementary decoded log data.
    pub decoded: DecodedCallEvent,
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
    /// The execution result of the call.
    pub execution_result: VmExecutionResultAndLogs,
    /// Optional complementary decoded call data.
    pub decoded: DecodedCallTrace,
    /// The call trace
    pub call: Call,
}

/// Decoded ZKSync event data enhancing the [CallLog].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecodedCallEvent {
    /// The decoded event name.
    pub name: Option<String>,
    /// The decoded log parameters, a vector of the parameter name (e.g. foo) and the parameter
    /// value (e.g. 0x9d3...45ca).
    pub params: Option<Vec<(String, String)>>,
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
    /// Event logs
    pub logs: Vec<CallLog>,
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
                depth: 0,
                success: true,
                caller: H160::zero(),
                address: H160::zero(),
                execution_result: VmExecutionResultAndLogs::mock(ExecutionResult::Success {
                    output: vec![],
                }),
                decoded: DecodedCallTrace::default(),
                call: Call::default(),
            },
            logs: Vec::new(),
            ordering: Vec::new(),
        }
    }
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

impl Default for CallTraceArena {
    fn default() -> Self {
        let root_node = CallTraceNode {
            parent: None,
            children: Vec::new(),
            idx: 0,
            trace: CallTrace {
                depth: 0,
                success: true,
                caller: H160::zero(),
                address: H160::zero(),
                execution_result: VmExecutionResultAndLogs::mock(ExecutionResult::Success {
                    output: vec![],
                }),
                decoded: DecodedCallTrace::default(),
                call: Call::default(),
            },
            logs: Vec::new(),
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

    /// Checks if the given address is a precompile based on `KNOWN_ADDRESSES`.
    pub fn is_precompile(address: &Address) -> bool {
        if let Some(known) = KNOWN_ADDRESSES.get(address) {
            matches!(known.contract_type, ContractType::Precompile)
        } else {
            false
        }
    }

    /// Filters out precompile nodes from the arena.
    pub fn filter_out_precompiles(&mut self) {
        self.arena
            .retain(|node| !Self::is_precompile(&node.trace.address));
    }

    /// Checks if the given address is a system contract based on `KNOWN_ADDRESSES`.
    pub fn is_system(address: &Address) -> bool {
        if let Some(known) = KNOWN_ADDRESSES.get(address) {
            matches!(known.contract_type, ContractType::System)
        } else {
            false
        }
    }

    /// Filters out system contracts nodes from the arena.
    pub fn filter_out_system_contracts(&mut self) {
        self.arena
            .retain(|node| !Self::is_system(&node.trace.address));
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
            ExecutionResult::Revert { output } => format!("Revert: {}", output),
            ExecutionResult::Halt { reason } => format!("Halt: {:?}", reason),
        }
    }
}
