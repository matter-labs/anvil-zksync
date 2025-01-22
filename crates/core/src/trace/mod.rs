use crate::trace::formatterv2::TraceWriter;
use crate::trace::types::{
    CallTrace, CallTraceArena, CallTraceNode, DecodedCallTrace, TraceMemberOrder,
    KNOWN_ADDRESSES, 
};
use crate::trace::decode::CallTraceDecoder;
use zksync_multivm::interface::{Call, VmExecutionResultAndLogs};
use zksync_types::tx;
pub mod formatterv2;
pub mod types;
pub mod abi_utils;
pub mod signatures;
pub mod decode;

/// Decode a collection of call traces.
///
/// The traces will be decoded using the given decoder, if possible.
pub async fn decode_trace_arena(
    arena: &mut CallTraceArena,
    decoder: &CallTraceDecoder,
) -> Result<(), std::fmt::Error> {
    decoder.prefetch_signatures(arena.nodes()).await;
    decoder.populate_traces(arena.nodes_mut()).await;

    Ok(())
}

/// Render a collection of call traces to a string optionally including contract creation bytecodes
/// and in JSON format.
pub fn render_trace_arena_inner(arena: &CallTraceArena, with_bytecodes: bool) -> String {
    let mut w = TraceWriter::new(Vec::<u8>::new()).write_bytecodes(with_bytecodes);
    w.write_arena(&arena).expect("Failed to write traces");
    String::from_utf8(w.into_writer()).expect("trace writer wrote invalid UTF-8")
}

pub fn build_call_trace_arena(
    calls: &[Call],
    tx_result: VmExecutionResultAndLogs,
) -> CallTraceArena {
    let mut arena = Vec::new();
    let mut parent_stack = Vec::new(); // Stack to keep track of parent indices

    for call in calls {
        for subcall in &call.calls {
            if CallTraceArena::is_precompile(&subcall.to) {
                continue;
            }
            if CallTraceArena::is_system(&subcall.to) {
                continue;
            }
            println!("subcalls: {:?}", subcall);
        }
        if CallTraceArena::is_precompile(&call.to) {
            continue;
        }
        // println!("aftfer precompile filter calls: {:?}", call);
        if CallTraceArena::is_system(&call.to) {
            continue;
        }

        

        let idx = arena.len();
        let call_trace = convert_call_to_call_trace(call, 0, tx_result.clone());

        // For now, omit capturing logs by leaving the `ordering` empty
        let ordering = Vec::new();

        let node = CallTraceNode {
            parent: parent_stack.last().copied(),
            children: Vec::new(),
            idx,
            trace: call_trace,
            ordering, // Empty ordering to skip logs
        };
        arena.push(node);

        if let Some(&parent_idx) = parent_stack.last() {
            arena[parent_idx].children.push(idx);
            let child_local_idx = arena[parent_idx].children.len() - 1;
            arena[parent_idx].ordering.push(TraceMemberOrder::Call(child_local_idx));
        }


        parent_stack.push(idx);
        process_subcalls(call, &mut arena, &mut parent_stack, tx_result.clone());
        parent_stack.pop();
    }

    CallTraceArena { arena }
}

/// Recursively processes subcalls and populates the arena.
fn process_subcalls(
    call: &Call,
    arena: &mut Vec<CallTraceNode>,
    parent_stack: &mut Vec<usize>,
    tx_result: VmExecutionResultAndLogs,
) {
    for subcall in &call.calls {
         if CallTraceArena::is_precompile(&subcall.to) {
            continue;
        }
        if CallTraceArena::is_system(&subcall.to) {
            continue;
        }

        let parent_idx = *parent_stack
            .last()
            .expect("Parent stack should not be empty");
        let idx = arena.len();
        // Determine the depth based on the parent node
        let parent_depth = arena[parent_idx].trace.depth;
        let sub_depth = parent_depth + 1;

        // Convert `Call` to `CallTrace`
        let sub_call_trace = convert_call_to_call_trace(subcall, sub_depth, tx_result.clone());
        let sub_node = CallTraceNode {
            parent: Some(parent_idx),
            children: Vec::new(),
            idx,
            trace: sub_call_trace,
            ordering: Vec::new(), // To be populated based on logs and subcalls
        };
        arena.push(sub_node);

        // Update the parent node's children and ordering
        arena[parent_idx].children.push(idx);
        let child_local_idx = arena[parent_idx].children.len() - 1;
        arena[parent_idx].ordering.push(TraceMemberOrder::Call(child_local_idx));

        // Push subcall to the stack and process its subcalls recursively
        parent_stack.push(idx);
        process_subcalls(subcall, arena, parent_stack, tx_result.clone());
        parent_stack.pop();
    }
}

/// Converts a single `Call` to a `CallTrace`.
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
