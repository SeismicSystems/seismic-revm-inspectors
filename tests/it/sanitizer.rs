//! Tests for trace sanitization (`trace_sanitizer.rs`).
//!
//! These are pure data-transformation tests: construct trace objects,
//! run them through the sanitizer, and assert that private data is stripped.

use alloy_primitives::{address, Address, Bytes, B256, U256};
use alloy_rpc_types_trace::{
    geth::{
        mux::MuxFrame, CallFrame, DefaultFrame, FourByteFrame, GethTrace, StructLog, TraceResult,
    },
    parity::{
        Action, CallAction, CallOutput, CallType, CreateAction, CreationMethod,
        LocalizedTransactionTrace, StateDiff, TraceOutput, TraceResults,
        TraceResultsWithTransactionHash, TransactionTrace, VmExecutedOperation, VmInstruction,
        VmTrace,
    },
};
use revm_inspectors::tracing::trace_sanitizer::{
    sanitize_geth_trace, sanitize_localized_transaction_trace, sanitize_trace_results,
    sanitize_trace_results_vec, sanitize_trace_results_with_hash,
};
use std::collections::BTreeMap;

// ───────────────────────────── Geth: DefaultFrame ─────────────────────────────

#[test]
fn test_sanitize_default_frame() {
    let frame = DefaultFrame {
        failed: false,
        gas: 21000,
        return_value: Bytes::from(vec![0xab, 0xcd]),
        struct_logs: vec![StructLog {
            pc: 0,
            op: "SLOAD".into(),
            gas: 100,
            gas_cost: 3,
            depth: 1,
            stack: Some(vec![U256::from(1)]),
            memory: Some(vec!["0000".to_string()]),
            return_data: Some(Bytes::from(vec![0xff])),
            storage: Some(BTreeMap::from([(B256::ZERO, B256::ZERO)])),
            ..Default::default()
        }],
    };

    let sanitized = sanitize_geth_trace(GethTrace::Default(frame));
    let GethTrace::Default(f) = sanitized else { panic!("expected DefaultFrame") };

    assert!(f.return_value.is_empty(), "return_value must be stripped");
    assert!(!f.failed);
    assert_eq!(f.gas, 21000);
    assert_eq!(f.struct_logs.len(), 1);

    let log = &f.struct_logs[0];
    assert_eq!(log.pc, 0);
    assert_eq!(log.op, "SLOAD");
    assert!(log.stack.is_none(), "stack must be stripped");
    assert!(log.memory.is_none(), "memory must be stripped");
    assert!(log.return_data.is_none(), "return_data must be stripped");
    assert!(log.storage.is_none(), "storage must be stripped");
}

// ───────────────────────────── Geth: CallFrame ────────────────────────────────

#[test]
fn test_sanitize_call_frame() {
    let frame = CallFrame {
        from: address!("0x0000000000000000000000000000000000000001"),
        to: Some(address!("0x0000000000000000000000000000000000000002")),
        input: Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]),
        output: Some(Bytes::from(vec![0xca, 0xfe])),
        gas: U256::from(100000),
        gas_used: U256::from(21000),
        typ: "CALL".to_string(),
        calls: vec![CallFrame {
            from: address!("0x0000000000000000000000000000000000000002"),
            to: Some(address!("0x0000000000000000000000000000000000000003")),
            input: Bytes::from(vec![0x11, 0x22]),
            output: Some(Bytes::from(vec![0x33])),
            typ: "CALL".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };

    let sanitized = sanitize_geth_trace(GethTrace::CallTracer(frame));
    let GethTrace::CallTracer(f) = sanitized else { panic!("expected CallTracer") };

    // Top-level
    assert!(f.input.is_empty(), "input must be stripped");
    assert!(f.output.is_none(), "output must be stripped");
    assert_eq!(f.from, address!("0x0000000000000000000000000000000000000001"), "from preserved");
    assert_eq!(f.to, Some(address!("0x0000000000000000000000000000000000000002")), "to preserved");

    // Nested call
    assert_eq!(f.calls.len(), 1);
    assert!(f.calls[0].input.is_empty(), "nested input must be stripped");
    assert!(f.calls[0].output.is_none(), "nested output must be stripped");
}

#[test]
fn test_sanitize_call_frame_deep_nesting() {
    let level3 = CallFrame {
        input: Bytes::from(vec![0x33]),
        output: Some(Bytes::from(vec![0x33])),
        typ: "CALL".to_string(),
        ..Default::default()
    };
    let level2 = CallFrame {
        input: Bytes::from(vec![0x22]),
        output: Some(Bytes::from(vec![0x22])),
        calls: vec![level3],
        typ: "CALL".to_string(),
        ..Default::default()
    };
    let level1 = CallFrame {
        input: Bytes::from(vec![0x11]),
        output: Some(Bytes::from(vec![0x11])),
        calls: vec![level2],
        typ: "CALL".to_string(),
        ..Default::default()
    };

    let sanitized = sanitize_geth_trace(GethTrace::CallTracer(level1));
    let GethTrace::CallTracer(f) = sanitized else { panic!("expected CallTracer") };

    assert!(f.input.is_empty());
    assert!(f.output.is_none());
    assert!(f.calls[0].input.is_empty());
    assert!(f.calls[0].output.is_none());
    assert!(f.calls[0].calls[0].input.is_empty());
    assert!(f.calls[0].calls[0].output.is_none());
}

// ───────────────────────────── Geth: FourByteTracer ───────────────────────────

#[test]
fn test_sanitize_four_byte_cleared() {
    let mut map = BTreeMap::new();
    map.insert("0xdeadbeef-128".to_string(), 5u64);
    map.insert("0xcafebabe-64".to_string(), 3u64);
    let frame = FourByteFrame(map);

    let sanitized = sanitize_geth_trace(GethTrace::FourByteTracer(frame));
    let GethTrace::FourByteTracer(f) = sanitized else { panic!("expected FourByteTracer") };

    assert!(f.0.is_empty(), "FourByteTracer must be completely emptied");
}

// ───────────────────────────── Geth: PreStateTracer passthrough ───────────────

#[test]
fn test_sanitize_prestate_passthrough() {
    use alloy_rpc_types_trace::geth::AccountState;

    let mut accounts = BTreeMap::new();
    accounts.insert(
        address!("0x0000000000000000000000000000000000000001"),
        AccountState {
            balance: Some(U256::from(1000)),
            nonce: Some(1),
            storage: BTreeMap::from([(B256::ZERO, B256::from(U256::from(42)))]),
            ..Default::default()
        },
    );

    use alloy_rpc_types_trace::geth::{PreStateFrame, PreStateMode};
    let frame = PreStateFrame::Default(PreStateMode(accounts.clone()));

    let sanitized = sanitize_geth_trace(GethTrace::PreStateTracer(frame));
    let GethTrace::PreStateTracer(PreStateFrame::Default(mode)) = sanitized else {
        panic!("expected PreStateTracer::Default")
    };

    assert_eq!(mode.0, accounts, "PreStateTracer must pass through unchanged");
}

// ───────────────────────────── Geth: NoopTracer passthrough ───────────────────

#[test]
fn test_sanitize_noop_passthrough() {
    let sanitized = sanitize_geth_trace(GethTrace::NoopTracer(Default::default()));
    assert!(matches!(sanitized, GethTrace::NoopTracer(_)));
}

// ───────────────────────────── Geth: MuxTracer ────────────────────────────────

#[test]
fn test_sanitize_mux_tracer() {
    use alloy_primitives::map::HashMap;
    use alloy_rpc_types_trace::geth::GethDebugBuiltInTracerType;

    let call_frame = CallFrame {
        input: Bytes::from(vec![0xde, 0xad]),
        output: Some(Bytes::from(vec![0xbe, 0xef])),
        typ: "CALL".to_string(),
        ..Default::default()
    };

    let mut four_byte_map = BTreeMap::new();
    four_byte_map.insert("0xdeadbeef-128".to_string(), 1u64);

    let mut inner = HashMap::default();
    inner.insert(GethDebugBuiltInTracerType::CallTracer, GethTrace::CallTracer(call_frame));
    inner.insert(
        GethDebugBuiltInTracerType::FourByteTracer,
        GethTrace::FourByteTracer(FourByteFrame(four_byte_map)),
    );

    let sanitized = sanitize_geth_trace(GethTrace::MuxTracer(MuxFrame(inner)));
    let GethTrace::MuxTracer(mux) = sanitized else { panic!("expected MuxTracer") };

    // CallTracer should be sanitized
    let call = mux.0.get(&GethDebugBuiltInTracerType::CallTracer).unwrap();
    let GethTrace::CallTracer(cf) = call else { panic!("expected CallTracer") };
    assert!(cf.input.is_empty());
    assert!(cf.output.is_none());

    // FourByteTracer should be emptied
    let four = mux.0.get(&GethDebugBuiltInTracerType::FourByteTracer).unwrap();
    let GethTrace::FourByteTracer(ff) = four else { panic!("expected FourByteTracer") };
    assert!(ff.0.is_empty());
}

// ───────────────────────────── Geth: FlatCallTracer ───────────────────────────

#[test]
fn test_sanitize_flat_call_tracer() {
    let trace = LocalizedTransactionTrace {
        trace: TransactionTrace {
            action: Action::Call(CallAction {
                from: Address::ZERO,
                to: address!("0x0000000000000000000000000000000000000001"),
                input: Bytes::from(vec![0xab]),
                gas: 21000,
                value: U256::ZERO,
                call_type: CallType::Call,
            }),
            result: Some(TraceOutput::Call(CallOutput {
                gas_used: 100,
                output: Bytes::from(vec![0xcd]),
            })),
            subtraces: 0,
            trace_address: vec![],
            ..Default::default()
        },
        block_hash: None,
        block_number: None,
        transaction_hash: None,
        transaction_position: None,
    };

    let sanitized = sanitize_geth_trace(GethTrace::FlatCallTracer(vec![trace]));
    let GethTrace::FlatCallTracer(traces) = sanitized else { panic!("expected FlatCallTracer") };

    assert_eq!(traces.len(), 1);
    let Action::Call(ref call) = traces[0].trace.action else { panic!("expected Call") };
    assert!(call.input.is_empty(), "input must be stripped");

    let Some(TraceOutput::Call(ref out)) = traces[0].trace.result else {
        panic!("expected Call output")
    };
    assert!(out.output.is_empty(), "output must be stripped");
}

// ──────────────────────── Parity: LocalizedTransactionTrace ───────────────────

#[test]
fn test_sanitize_localized_trace_call() {
    let trace = LocalizedTransactionTrace {
        trace: TransactionTrace {
            action: Action::Call(CallAction {
                from: address!("0x0000000000000000000000000000000000000001"),
                to: address!("0x0000000000000000000000000000000000000002"),
                input: Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]),
                gas: 100000,
                value: U256::from(1000),
                call_type: CallType::Call,
            }),
            result: Some(TraceOutput::Call(CallOutput {
                gas_used: 21000,
                output: Bytes::from(vec![0xca, 0xfe]),
            })),
            subtraces: 0,
            trace_address: vec![],
            ..Default::default()
        },
        block_hash: None,
        block_number: None,
        transaction_hash: None,
        transaction_position: None,
    };

    let sanitized = sanitize_localized_transaction_trace(trace);

    let Action::Call(ref call) = sanitized.trace.action else { panic!("expected Call") };
    assert!(call.input.is_empty(), "calldata must be stripped");
    assert_eq!(call.from, address!("0x0000000000000000000000000000000000000001"), "from preserved");
    assert_eq!(call.to, address!("0x0000000000000000000000000000000000000002"), "to preserved");
    assert_eq!(call.value, U256::from(1000), "value preserved");
    assert_eq!(call.gas, 100000, "gas preserved");

    let Some(TraceOutput::Call(ref out)) = sanitized.trace.result else {
        panic!("expected Call output")
    };
    assert!(out.output.is_empty(), "return data must be stripped");
    assert_eq!(out.gas_used, 21000, "gas_used preserved");
}

#[test]
fn test_sanitize_localized_trace_create() {
    let trace = LocalizedTransactionTrace {
        trace: TransactionTrace {
            action: Action::Create(CreateAction {
                from: Address::ZERO,
                gas: 100000,
                init: Bytes::from(vec![0x60, 0x80, 0x60, 0x40]),
                value: U256::ZERO,
                creation_method: CreationMethod::Create,
            }),
            result: Some(TraceOutput::Create(alloy_rpc_types_trace::parity::CreateOutput {
                address: address!("0x0000000000000000000000000000000000000042"),
                code: Bytes::from(vec![0x60, 0x80]),
                gas_used: 50000,
            })),
            subtraces: 0,
            trace_address: vec![],
            ..Default::default()
        },
        block_hash: None,
        block_number: None,
        transaction_hash: None,
        transaction_position: None,
    };

    let sanitized = sanitize_localized_transaction_trace(trace);

    let Action::Create(ref create) = sanitized.trace.action else { panic!("expected Create") };
    assert!(create.init.is_empty(), "init code must be stripped");

    // Deployed bytecode is public on-chain — should be preserved.
    let Some(TraceOutput::Create(ref out)) = sanitized.trace.result else {
        panic!("expected Create output")
    };
    assert_eq!(out.code, Bytes::from(vec![0x60, 0x80]), "deployed bytecode preserved");
    assert_eq!(
        out.address,
        address!("0x0000000000000000000000000000000000000042"),
        "address preserved"
    );
}

// ──────────────────────────── Parity: TraceResults ────────────────────────────

#[test]
fn test_sanitize_trace_results() {
    let results = TraceResults {
        output: Bytes::from(vec![0xab, 0xcd]),
        state_diff: None,
        trace: vec![TransactionTrace {
            action: Action::Call(CallAction {
                input: Bytes::from(vec![0xde, 0xad]),
                ..Default::default()
            }),
            result: Some(TraceOutput::Call(CallOutput {
                gas_used: 100,
                output: Bytes::from(vec![0xbe, 0xef]),
            })),
            subtraces: 0,
            trace_address: vec![],
            ..Default::default()
        }],
        vm_trace: Some(VmTrace {
            code: Bytes::from(vec![0x60]),
            ops: vec![VmInstruction {
                cost: 3,
                pc: 0,
                ex: Some(VmExecutedOperation {
                    used: 97,
                    push: vec![U256::from(42)],
                    mem: Some(alloy_rpc_types_trace::parity::MemoryDelta {
                        off: 0,
                        data: Bytes::from(vec![0xff]),
                    }),
                    store: Some(alloy_rpc_types_trace::parity::StorageDelta {
                        key: U256::from(1),
                        val: U256::from(2),
                    }),
                }),
                sub: None,
                op: Some("PUSH1".to_string()),
                idx: None,
            }],
        }),
    };

    let sanitized = sanitize_trace_results(results);

    // Output stripped
    assert!(sanitized.output.is_empty(), "output must be stripped");

    // Trace action input stripped
    let Action::Call(ref call) = sanitized.trace[0].action else { panic!("expected Call") };
    assert!(call.input.is_empty());

    // Trace result output stripped
    let Some(TraceOutput::Call(ref out)) = sanitized.trace[0].result else {
        panic!("expected Call output")
    };
    assert!(out.output.is_empty());

    // VM trace sanitized
    let vm = sanitized.vm_trace.as_ref().unwrap();
    assert_eq!(vm.code, Bytes::from(vec![0x60]), "code preserved");
    let ex = vm.ops[0].ex.as_ref().unwrap();
    assert!(ex.push.is_empty(), "push values must be stripped");
    assert!(ex.mem.is_none(), "memory delta must be stripped");
    assert!(ex.store.is_none(), "storage delta must be stripped");
    assert_eq!(ex.used, 97, "used preserved");
}

#[test]
fn test_sanitize_trace_results_preserves_state_diff() {
    use alloy_rpc_types_trace::parity::{AccountDiff, Delta};

    let mut diff = StateDiff(BTreeMap::new());
    diff.0.insert(
        address!("0x0000000000000000000000000000000000000001"),
        AccountDiff {
            balance: Delta::Changed(alloy_rpc_types_trace::parity::ChangedType {
                from: U256::from(100),
                to: U256::from(200),
            }),
            ..Default::default()
        },
    );

    let results = TraceResults {
        output: Bytes::from(vec![0xab]),
        state_diff: Some(diff),
        trace: vec![],
        vm_trace: None,
    };

    let sanitized = sanitize_trace_results(results);

    // state_diff must pass through (storage filtering is upstream in builders)
    let sd = sanitized.state_diff.as_ref().unwrap();
    assert!(sd.0.contains_key(&address!("0x0000000000000000000000000000000000000001")));
    let acc = &sd.0[&address!("0x0000000000000000000000000000000000000001")];
    assert!(matches!(acc.balance, Delta::Changed(_)));
}

// ─────────────────────── TraceResultsWithTransactionHash ──────────────────────

#[test]
fn test_sanitize_trace_results_with_hash() {
    let tx_hash = B256::from([0xaa; 32]);
    let results = TraceResultsWithTransactionHash {
        full_trace: TraceResults {
            output: Bytes::from(vec![0xab]),
            state_diff: None,
            trace: vec![TransactionTrace {
                action: Action::Call(CallAction {
                    input: Bytes::from(vec![0xde, 0xad]),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            vm_trace: None,
        },
        transaction_hash: tx_hash,
    };

    let sanitized = sanitize_trace_results_with_hash(results);

    assert_eq!(sanitized.transaction_hash, tx_hash, "tx_hash preserved");
    assert!(sanitized.full_trace.output.is_empty(), "output stripped");
    let Action::Call(ref call) = sanitized.full_trace.trace[0].action else {
        panic!("expected Call")
    };
    assert!(call.input.is_empty(), "input stripped");
}

// ──────────────────────────── sanitize_trace_results_vec ──────────────────────

#[test]
fn test_sanitize_trace_results_vec() {
    let tx_hash = Some(B256::from([0xbb; 32]));

    let results = vec![
        TraceResult::Success {
            result: GethTrace::Default(DefaultFrame {
                return_value: Bytes::from(vec![0xab]),
                struct_logs: vec![StructLog {
                    stack: Some(vec![U256::from(1)]),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            tx_hash,
        },
        TraceResult::Error { error: "out of gas".to_string(), tx_hash: None },
    ];

    let sanitized = sanitize_trace_results_vec(results);

    // Success entry sanitized
    let TraceResult::Success { ref result, tx_hash: ref hash } = sanitized[0] else {
        panic!("expected Success")
    };
    assert_eq!(*hash, tx_hash, "tx_hash preserved");
    let GethTrace::Default(ref f) = result else { panic!("expected Default") };
    assert!(f.return_value.is_empty());
    assert!(f.struct_logs[0].stack.is_none());

    // Error entry unchanged
    let TraceResult::Error { ref error, .. } = sanitized[1] else { panic!("expected Error") };
    assert_eq!(error, "out of gas");
}
