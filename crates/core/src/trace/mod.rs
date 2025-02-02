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

/// Builds a call trace arena from the call tracer calls and transaction result.
pub fn build_call_trace_arena(
    calls: &[Call],
    tx_result: VmExecutionResultAndLogs,
    verbosity: u8,
) -> CallTraceArena {
    let mut arena = CallTraceArena::default();
    let root_idx = 0;
    // Update the root node's execution_result with the actual transaction result
    if let Some(root_node) = arena.arena.get_mut(root_idx) {
        root_node.trace.execution_result = tx_result.clone();
    }
    // Process calls and their subcalls
    for call in calls {
        process_call_and_subcalls(call, root_idx, 0, &mut arena.arena, &tx_result, verbosity);
    }

    arena
}

// Process a call and its subcalls recursively, adding them to the arena.
fn process_call_and_subcalls(
    call: &Call,
    parent_idx: usize,
    depth: usize,
    arena: &mut Vec<CallTraceNode>,
    tx_result: &VmExecutionResultAndLogs,
    verbosity: u8,
) {
    // Determine if the current call should be shown at this verbosity level
    let should_add_call = should_include_call(&call.to, verbosity);

    let idx = if should_add_call {
        let idx = arena.len();
        // collect event logs
        let logs_for_call: Vec<CallLog> = tx_result
            .logs
            .events
            .iter()
            .filter(|vm_event| vm_event.address == call.to)
            .cloned()
            .enumerate()
            .map(|(i, vm_event)| CallLog {
                raw_log: vm_event,
                decoded: DecodedCallEvent::default(),
                position: i as u64,
            })
            .collect();

        let call_trace = convert_call_to_call_trace(call, depth, tx_result.clone());

        let mut node = CallTraceNode {
            parent: Some(parent_idx),
            children: Vec::new(),
            idx,
            trace: call_trace,
            logs: logs_for_call,
            ordering: Vec::new(),
        };

        let logs_count = node.logs.len();
        for i in 0..logs_count {
            node.ordering.push(TraceMemberOrder::Log(i));
        }
        arena.push(node);

        // Add as a child of the parent node
        arena[parent_idx].children.push(idx);
        let child_local_idx = arena[parent_idx].children.len() - 1;
        arena[parent_idx]
            .ordering
            .push(TraceMemberOrder::Call(child_local_idx));

        idx
    } else {
        // If the current call is skipped, maintain the same parent index for its subcalls
        parent_idx
    };

    // Process subcalls recursively
    for subcall in &call.calls {
        process_call_and_subcalls(subcall, idx, depth + 1, arena, tx_result, verbosity);
    }
}

/// Returns whether we should include the call in the trace based on
/// its address type and the current verbosity level.
///
/// Verbosity levels (for quick reference):
/// - 2: user only
/// - 3: user + system
/// - 4: user + system + precompile
/// - 5+: everything + L1–L2 logs (future-proof)
fn should_include_call(address: &H160, verbosity: u8) -> bool {
    let is_system = CallTraceArena::is_system(address);
    let is_precompile = CallTraceArena::is_precompile(address);

    match verbosity {
        // -v or less => 0 or 1 => show nothing
        0 | 1 => false,
        // -vv => 2 => user calls only
        2 => !(is_system || is_precompile),
        // -vvv => 3 => user + system
        3 => !is_precompile,
        // -vvvv => 4 => user + system + precompile
        4 => true,
        // -vvvvv => 5 => everything + future logs (e.g. L1-L2 logs, perhaps storage logs?)
        _ => true,
    }
}

// Converts a single `Call` to a `CallTrace`.
fn convert_call_to_call_trace(
    call: &Call,
    depth: usize,
    tx_result: VmExecutionResultAndLogs,
) -> CallTrace {
    let label = KNOWN_ADDRESSES
        .get(&call.to)
        .map(|known| known.name.clone());

    CallTrace {
        depth,
        success: !tx_result.result.is_failed(),
        caller: call.from,
        address: call.to,
        execution_result: tx_result,
        decoded: DecodedCallTrace {
            label,
            ..Default::default()
        },
        call: call.clone(),
    }
}
