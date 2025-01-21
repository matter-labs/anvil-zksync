use zksync_multivm::interface::{Call, VmExecutionResultAndLogs};
use zksync_types::Address;

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
