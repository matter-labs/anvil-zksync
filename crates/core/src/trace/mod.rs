use crate::trace::decode::CallTraceDecoder;
use crate::trace::types::{
    CallTrace, CallTraceArena, CallTraceNode, DecodedCallTrace, TraceMemberOrder, KNOWN_ADDRESSES,
};
use crate::trace::writer::TraceWriter;
use types::{CallLog, DecodedCallEvent};
use zksync_multivm::interface::{Call, VmExecutionResultAndLogs};
use zksync_types::H160;
pub mod abi_utils;
pub mod decode;
pub mod signatures;
pub mod types;
pub mod writer;

/// Decode a collection of call traces.
///
/// The traces will be decoded if possible using openchain.
pub async fn decode_trace_arena(
    arena: &mut CallTraceArena,
    decoder: &CallTraceDecoder,
) -> Result<(), anyhow::Error> {
    decoder.prefetch_signatures(arena.nodes()).await;
    decoder.populate_traces(arena.nodes_mut()).await;

    Ok(())
}

/// Render a collection of call traces to a string
pub fn render_trace_arena_inner(arena: &CallTraceArena, with_bytecodes: bool) -> String {
    let mut w = TraceWriter::new(Vec::<u8>::new()).write_bytecodes(with_bytecodes);
    w.write_arena(arena).expect("Failed to write traces");
    String::from_utf8(w.into_writer()).expect("trace writer wrote invalid UTF-8")
}

/// A builder that incrementally constructs a `CallTraceArena`.
pub struct TraceArenaBuilder<'a> {
    calls: &'a [Call],
    tx_result: &'a VmExecutionResultAndLogs,
    arena: CallTraceArena,
}

impl<'a> TraceArenaBuilder<'a> {
    /// Initialize a builder with references to the calls and the VM result.
    pub fn new(calls: &'a [Call], tx_result: &'a VmExecutionResultAndLogs) -> Self {
        let mut arena = CallTraceArena::default();
        
        if let Some(root) = arena.arena.first_mut() {
            root.trace.execution_result = tx_result.clone();
        }

        Self {
            calls,
            tx_result,
            arena,
        }
    }

    /// Build the arena by adding all top-level calls (and their subcalls) under the root node.
    pub fn build_arena(mut self) -> CallTraceArena {
        let root_idx = 0;
        for call in self.calls {
            self.add_call_and_subcalls(call, root_idx, 0);
        }
        self.rebuild_all_orderings();
        self.arena
    }

    /// Recursively add a call (and its subcalls) to the arena under the given `parent_idx`.
    fn add_call_and_subcalls(&mut self, call: &Call, parent_idx: usize, depth: usize) {
        let new_node_idx = self.arena.arena.len();
        let node = self.make_node(call, depth, new_node_idx, Some(parent_idx));
        self.arena.arena.push(node);

        let parent_node = &mut self.arena.arena[parent_idx];
        parent_node.children.push(new_node_idx);

        for subcall in &call.calls {
            self.add_call_and_subcalls(subcall, new_node_idx, depth + 1);
        }
    }

    /// Constructs a `CallTraceNode` for a single call.
    fn make_node(
        &self,
        call: &Call,
        depth: usize,
        idx: usize,
        parent: Option<usize>,
    ) -> CallTraceNode {
        let logs_for_call = self.logs_for_call(call);
        let trace = convert_call_to_call_trace(call, depth, self.tx_result.clone());
        CallTraceNode {
            parent,
            children: Vec::new(),
            idx,
            trace,
            logs: logs_for_call,
            ordering: Vec::new(),
        }
    }

    fn logs_for_call(&self, call: &Call) -> Vec<CallLog> {
        self.tx_result
            .logs
            .events
            .iter()
            .enumerate()
            .filter_map(|(i, vm_event)| {
                if vm_event.address == call.to {
                    Some(CallLog {
                        raw_log: vm_event.clone(),
                        decoded: DecodedCallEvent::default(),
                        position: i as u64,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
    fn rebuild_all_orderings(&mut self) {
        let len = self.arena.arena.len();
        for i in 0..len {
            rebuild_ordering(&mut self.arena.arena[i]);
        }
    }
}

fn rebuild_ordering(node: &mut CallTraceNode) {
    node.ordering.clear();
    for i in 0..node.logs.len() {
        node.ordering.push(TraceMemberOrder::Log(i));
    }
    for i in 0..node.children.len() {
        node.ordering.push(TraceMemberOrder::Call(i));
    }
}

/// Converts a single call into a `CallTrace`, including a label for well-known addresses.
fn convert_call_to_call_trace(
    call: &Call,
    depth: usize,
    tx_result: VmExecutionResultAndLogs,
) -> crate::trace::types::CallTrace {
    let label = KNOWN_ADDRESSES.get(&call.to).map(|known| known.name.clone());
    crate::trace::types::CallTrace {
        depth,
        success: !tx_result.result.is_failed(),
        caller: call.from,
        address: call.to,
        execution_result: tx_result,
        decoded: DecodedCallTrace {
            label,
            // … other decoded fields default or left empty
            ..Default::default()
        },
        call: call.clone(),
    }
}

/// Filters the given `CallTraceArena` by verbosity,
/// returning a new arena with only the included calls.
/// Unwanted calls’ children are “bubbled up” to the parent.
pub fn filter_call_trace_arena(
    arena: &CallTraceArena,
    verbosity: u8,
) -> CallTraceArena {
    let mut filtered = CallTraceArena::default();

    if arena.arena.is_empty() {
        return filtered;
    }

    let root_idx = 0;
    let mut root_copy = arena.arena[root_idx].clone();
    root_copy.parent = None;
    root_copy.idx = 0;
    root_copy.children.clear();
    root_copy.ordering.clear();

    filtered.arena.clear();
    filtered.arena.push(root_copy);

    filter_node_recursively(
        &arena.arena[root_idx],
        arena,
        &mut filtered,
        Some(0),
        verbosity,
    );

    for node in &mut filtered.arena {
        rebuild_ordering(node);
    }

    filtered
}
fn filter_node_recursively(
    orig_node: &CallTraceNode,
    orig_arena: &CallTraceArena,
    filtered_arena: &mut CallTraceArena,
    parent_idx: Option<usize>,
    verbosity: u8,
) {
    for &child_idx in &orig_node.children {
        let child = &orig_arena.arena[child_idx];
        if should_include_call(&child.trace.address, verbosity) {
            let new_idx = filtered_arena.arena.len();
            let mut child_copy = child.clone();
            child_copy.idx = new_idx;
            child_copy.parent = parent_idx;
            child_copy.children.clear();
            child_copy.ordering.clear();

            filtered_arena.arena.push(child_copy);

            if let Some(p_idx) = parent_idx {
                filtered_arena.arena[p_idx].children.push(new_idx);
            }

            filter_node_recursively(child, orig_arena, filtered_arena, Some(new_idx), verbosity);
        } else {
            filter_node_recursively(child, orig_arena, filtered_arena, parent_idx, verbosity);
        }
    }
}

/// A helper that checks the address vs. the user’s verbosity.
fn should_include_call(address: &H160, verbosity: u8) -> bool {
    let is_system = CallTraceArena::is_system(address);
    let is_precompile = CallTraceArena::is_precompile(address);

    match verbosity {
        0 | 1 => false,            // show nothing
        2 => !(is_system || is_precompile), // user only
        3 => !is_precompile,       // user + system
        4 => true,                 // user + system + precompile
        _ => true,                 // everything
    }
}
