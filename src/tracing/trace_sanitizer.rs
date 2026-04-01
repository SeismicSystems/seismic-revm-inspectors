//! Trace sanitization for Seismic's privacy model.
//!
//! Strips private data from trace output before returning to RPC callers.
//! Storage filtering is handled upstream in revm-inspectors builders via
//! `filter_private_storage` (defaults to true). This module handles everything else:
//!
//! - Calldata: stripped from all frames (callers already know their own calldata;
//!   for regular txs it's available via eth_getTransactionByHash)
//! - Return data: stripped from all frames
//! - VM trace payloads: push values, memory deltas, storage deltas stripped
//! - Stack/memory: should already be disabled via TracingInspectorConfig

use alloc::vec::Vec;
use alloy_primitives::Bytes;
use alloy_rpc_types_trace::{
    geth::{mux::MuxFrame, CallFrame, DefaultFrame, GethTrace, TraceResult},
    parity::{
        Action, LocalizedTransactionTrace, TraceOutput, TraceResults,
        TraceResultsWithTransactionHash, TransactionTrace, VmInstruction, VmTrace,
    },
};

/// Sanitizes a [`GethTrace`] by stripping private data.
pub fn sanitize_geth_trace(trace: GethTrace) -> GethTrace {
    match trace {
        GethTrace::Default(frame) => GethTrace::Default(sanitize_default_frame(frame)),
        GethTrace::CallTracer(frame) => {
            GethTrace::CallTracer(sanitize_call_frame(frame))
        }
        GethTrace::FlatCallTracer(frames) => GethTrace::FlatCallTracer(
            frames
                .into_iter()
                .map(sanitize_localized_transaction_trace)
                .collect(),
        ),
        GethTrace::PreStateTracer(frame) => {
            // Storage already filtered by builders. Nothing else to strip.
            GethTrace::PreStateTracer(frame)
        }
        GethTrace::FourByteTracer(_) => {
            // Function selectors reveal which function was called even for encrypted
            // calldata. Strip unconditionally.
            GethTrace::FourByteTracer(Default::default())
        }
        GethTrace::NoopTracer(frame) => GethTrace::NoopTracer(frame),
        GethTrace::MuxTracer(mux) => GethTrace::MuxTracer(MuxFrame(
            mux.0
                .into_iter()
                .map(|(k, v)| (k, sanitize_geth_trace(v)))
                .collect(),
        )),
        // JS tracer is disabled, ERC-7562 is not reachable.
        // Default to stripping: return an empty noop trace for any unknown/future variant.
        // This ensures new tracer types added by upstream alloy don't silently leak data.
        _ => GethTrace::NoopTracer(Default::default()),
    }
}

/// Sanitizes a [`DefaultFrame`] (structlog trace).
///
/// Stack and memory should already be disabled via TracingInspectorConfig.
/// We strip return_value and defensively clear stack/memory/storage.
fn sanitize_default_frame(mut frame: DefaultFrame) -> DefaultFrame {
    // Strip return value (may contain private storage values).
    frame.return_value = Bytes::new();

    // Defensive: strip stack/memory/storage even if TracingInspectorConfig
    // should have prevented recording them.
    for log in &mut frame.struct_logs {
        log.stack = None;
        log.memory = None;
        log.return_data = None;
        log.storage = None;
    }

    frame
}

/// Sanitizes a [`CallFrame`] (call tracer) by stripping calldata and return data.
fn sanitize_call_frame(mut frame: CallFrame) -> CallFrame {
    frame.input = Bytes::new();
    frame.output = None;

    // Recursively sanitize nested calls.
    frame.calls = frame.calls.into_iter().map(sanitize_call_frame).collect();

    frame
}

/// Sanitizes a [`TransactionTrace`] (parity trace).
fn sanitize_transaction_trace(mut trace: TransactionTrace) -> TransactionTrace {
    // Strip calldata from actions.
    match &mut trace.action {
        Action::Call(call) => {
            call.input = Bytes::new();
        }
        Action::Create(create) => {
            create.init = Bytes::new();
        }
        _ => {}
    }

    // Strip return data from results.
    if let Some(TraceOutput::Call(ref mut call_output)) = trace.result {
        call_output.output = Bytes::new();
    }
    // Deployed bytecode (TraceOutput::Create) is public on-chain, keep it.

    trace
}

/// Sanitizes a [`LocalizedTransactionTrace`].
pub fn sanitize_localized_transaction_trace(
    mut trace: LocalizedTransactionTrace,
) -> LocalizedTransactionTrace {
    trace.trace = sanitize_transaction_trace(trace.trace);
    trace
}

/// Sanitizes a [`VmTrace`] by stripping execution payloads.
fn sanitize_vm_trace(mut vm_trace: VmTrace) -> VmTrace {
    for op in &mut vm_trace.ops {
        sanitize_vm_instruction(op);
    }
    vm_trace
}

/// Strips push values, memory deltas, and storage deltas from a [`VmInstruction`].
fn sanitize_vm_instruction(instruction: &mut VmInstruction) {
    if let Some(ref mut ex) = instruction.ex {
        ex.push = Vec::new();
        ex.mem = None;
        ex.store = None;
    }
    // Recursively sanitize subcalls.
    if let Some(ref mut sub) = instruction.sub {
        *sub = sanitize_vm_trace(std::mem::take(sub));
    }
}

/// Sanitizes [`TraceResults`] (parity trace_call / replay output).
pub fn sanitize_trace_results(mut results: TraceResults) -> TraceResults {
    // Strip output (return data).
    results.output = Bytes::new();

    // Sanitize call traces.
    results.trace = results
        .trace
        .into_iter()
        .map(sanitize_transaction_trace)
        .collect();

    // Sanitize VM trace.
    if let Some(vm_trace) = results.vm_trace {
        results.vm_trace = Some(sanitize_vm_trace(vm_trace));
    }

    // StateDiff: storage already filtered by builders. Nothing else to strip.

    results
}

/// Sanitizes [`TraceResultsWithTransactionHash`].
pub fn sanitize_trace_results_with_hash(
    mut results: TraceResultsWithTransactionHash,
) -> TraceResultsWithTransactionHash {
    results.full_trace = sanitize_trace_results(results.full_trace);
    results
}

/// Sanitizes a `Vec<TraceResult>` (used by `debug_traceBlock` and variants).
pub fn sanitize_trace_results_vec(results: Vec<TraceResult>) -> Vec<TraceResult> {
    results
        .into_iter()
        .map(|r| match r {
            TraceResult::Success { result, tx_hash } => {
                TraceResult::Success { result: sanitize_geth_trace(result), tx_hash }
            }
            err => err,
        })
        .collect()
}
