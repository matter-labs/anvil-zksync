use crate::trace::formatterv2::TraceWriter;
use crate::trace::types::{
    CallTrace, CallTraceArena, CallTraceNode, DecodedCallTrace, TraceMemberOrder,
    KNOWN_ADDRESSES, 
};
use crate::trace::decode::CallTraceDecoder;
use zksync_multivm::interface::{Call, VmExecutionResultAndLogs};
use zksync_types::H160;
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

    // Add a virtual root node
    let root_idx = arena.len();
    let root_node = CallTraceNode {
        parent: None,
        children: Vec::new(),
        idx: root_idx,
        trace: CallTrace {
            depth: 0,
            success: true,
            caller: H160::zero(), // Placeholder
            address: H160::zero(), // Placeholder
            execution_result: tx_result.clone(),
            decoded: DecodedCallTrace::default(),
            call: Call::default(),
        },
        ordering: Vec::new(),
    };
    arena.push(root_node);

    for call in calls {
        process_call_and_subcalls(
            call,
            root_idx,
            0,
            &mut arena,
            &tx_result,
        );
    }

    CallTraceArena { arena }
}

fn process_call_and_subcalls(
    call: &Call,
    parent_idx: usize,
    depth: usize,
    arena: &mut Vec<CallTraceNode>,
    tx_result: &VmExecutionResultAndLogs,
) {
    // Only add the current call to the arena if it's not System or Precompile
    let should_add_call = !CallTraceArena::is_precompile(&call.to) && !CallTraceArena::is_system(&call.to);

    let idx = if should_add_call {
        let idx = arena.len();
        let call_trace = convert_call_to_call_trace(call, depth, tx_result.clone());

        let node = CallTraceNode {
            parent: Some(parent_idx),
            children: Vec::new(),
            idx,
            trace: call_trace,
            ordering: Vec::new(),
        };
        arena.push(node);

        // Add as a child of the parent node
        arena[parent_idx].children.push(idx);
        let child_local_idx = arena[parent_idx].children.len() - 1;
        arena[parent_idx].ordering.push(TraceMemberOrder::Call(child_local_idx));

        idx
    } else {
        // If the current call is skipped, maintain the same parent index for its subcalls
        parent_idx
    };

    // Process subcalls recursively
    for subcall in &call.calls {
        process_call_and_subcalls(subcall, idx, depth + 1, arena, tx_result);
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
