use anvil_zksync_types::traces::{
    CallLog, CallTrace, CallTraceArena, CallTraceNode, DecodedCallEvent, DecodedCallTrace, L2L1Log,
    L2L1Logs, TraceMemberOrder, KNOWN_ADDRESSES,
};
use decode::CallTraceDecoder;
use writer::TraceWriter;
use zksync_multivm::interface::{
    Call, ExecutionResult, Halt, VmExecutionResultAndLogs, VmRevertReason,
};
use zksync_types::H160;

pub mod abi_utils;
pub mod decode;
pub mod identifier;
pub mod writer;

/// Converts a single call into a CallTrace.
#[inline]
fn convert_call_to_call_trace(call: &Call) -> CallTrace {
    let label = KNOWN_ADDRESSES
        .get(&call.to)
        .map(|known| known.name.clone());

    // Determine the execution result based on individual call
    let execution_result = if let Some(ref revert_reason) = call.revert_reason {
        ExecutionResult::Revert {
            output: VmRevertReason::from(revert_reason.as_bytes()),
        }
    } else if let Some(ref err) = call.error {
        ExecutionResult::Halt {
            reason: Halt::TracerCustom(err.to_string()),
        }
    } else {
        ExecutionResult::Success {
            output: call.output.clone(),
        }
    };

    CallTrace {
        success: !execution_result.is_failed(),
        caller: call.from,
        address: call.to,
        execution_result,
        decoded: DecodedCallTrace {
            label,
            ..Default::default()
        },
        call: call.clone(),
    }
}

/// Builds a call trace arena from the calls and transaction result.
pub fn build_call_trace_arena(
    calls: &[Call],
    tx_result: &VmExecutionResultAndLogs,
) -> CallTraceArena {
    let mut arena = CallTraceArena::default();

    // Update the root node's execution result.
    if let Some(root_node) = arena.arena.get_mut(0) {
        root_node.trace.execution_result = tx_result.result.clone();
    }

    for call in calls {
        process_call_and_subcalls(call, 0, 0, &mut arena, tx_result);
    }
    arena
}

/// Recursively process a call and its subcalls, adding them to the arena.
fn process_call_and_subcalls(
    call: &Call,
    parent_idx: usize,
    depth: usize,
    arena: &mut CallTraceArena,
    tx_result: &VmExecutionResultAndLogs,
) {
    // Collect logs for the current call.
    let logs_for_call: Vec<CallLog> = tx_result
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
        .collect();

    // Collect user and system L2-L1 logs associated with this call.
    let l2_l1_logs_for_call: Vec<L2L1Logs> = tx_result
        .logs
        .user_l2_to_l1_logs
        .iter()
        .filter(|log| log.0.sender == call.to)
        .map(|log| L2L1Logs {
            raw_log: L2L1Log::User(log.clone()),
            position: log.0.tx_number_in_block as u64,
        })
        .chain(
            tx_result
                .logs
                .system_l2_to_l1_logs
                .iter()
                .filter(|log| log.0.sender == call.to)
                .map(|log| L2L1Logs {
                    raw_log: L2L1Log::System(log.clone()),
                    position: log.0.tx_number_in_block as u64,
                }),
        )
        .collect();

    let call_trace = convert_call_to_call_trace(call);

    let node = CallTraceNode {
        parent: None,
        children: Vec::new(),
        idx: 0,
        trace: call_trace,
        logs: logs_for_call,
        l2_l1_logs: l2_l1_logs_for_call,
        ordering: Vec::new(),
    };

    let new_parent_idx = arena.add_node(Some(parent_idx), node);

    // Process subcalls under the new parent.
    for subcall in &call.calls {
        process_call_and_subcalls(subcall, new_parent_idx, depth + 1, arena, tx_result);
    }
}

/// Render a collection of call traces to a string
pub fn render_trace_arena_inner(arena: &CallTraceArena, with_bytecodes: bool) -> String {
    let mut w = TraceWriter::new(Vec::<u8>::new()).write_bytecodes(with_bytecodes);
    w.write_arena(arena).expect("Failed to write traces");
    String::from_utf8(w.into_writer()).expect("trace writer wrote invalid UTF-8")
}

/// Decode a collection of call traces.
///
/// The traces will be decoded if possible using openchain.
pub async fn decode_trace_arena(
    arena: &mut CallTraceArena,
    decoder: &CallTraceDecoder,
) -> Result<(), anyhow::Error> {
    decoder.prefetch_signatures(&arena.arena).await;
    decoder.populate_traces(&mut arena.arena).await;

    Ok(())
}

/// Filter a call trace arena based on verbosity level.
pub fn filter_call_trace_arena(arena: &CallTraceArena, verbosity: u8) -> CallTraceArena {
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
    filtered.arena.push(root_copy);

    filter_node_recursively(
        &arena.arena[root_idx],
        arena,
        &mut filtered,
        Some(0),
        verbosity,
    );

    // Rebuild ordering
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

            // Filter the L2-L1 logs within the node.
            child_copy.l2_l1_logs.retain(|log| match &log.raw_log {
                L2L1Log::User(_) => verbosity >= 2, // include user logs if verbosity is >= 2
                L2L1Log::System(_) => verbosity >= 3, // include system logs if verbosity is >= 3
            });

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

/// Returns whether we should include the call in the trace based on
/// its address type and the current verbosity level.
///
/// Verbosity levels (for quick reference):
/// - 2: user calls and logs only
/// - 3: user + system calls and logs
/// - 4: user + system + precompile  logs
/// - 5+: everything (future-proof)
#[inline]
fn should_include_call(address: &H160, verbosity: u8) -> bool {
    let is_system = CallTraceArena::is_system(address);
    let is_precompile = CallTraceArena::is_precompile(address);

    match verbosity {
        // -v or less => 0 or 1 => show nothing
        0 | 1 => false,
        // -vv => 2 => user calls only (incl. L2–L1 user logs)
        2 => !(is_system || is_precompile),
        // -vvv => 3 => user + system (incl. L2–L1 system logs)
        3 => !is_precompile,
        // -vvvv => 4 => user + system + precompile
        4 => true,
        // -vvvvv => 5 => everything + future logs (e.g. maybe storage logs)
        _ => true,
    }
}

fn rebuild_ordering(node: &mut CallTraceNode) {
    node.ordering.clear();
    for i in 0..node.logs.len() {
        node.ordering.push(TraceMemberOrder::Log(i));
    }
    for i in 0..node.l2_l1_logs.len() {
        node.ordering.push(TraceMemberOrder::L1L2Log(i));
    }
    for i in 0..node.children.len() {
        node.ordering.push(TraceMemberOrder::Call(i));
    }
}
