//! Tests for FlaggedStorage filtering in Geth and Parity trace builders.
//!
//! Verifies that storage slots marked as private (`FlaggedStorage::is_private == true`)
//! are omitted from trace output when `filter_private_storage` is enabled (the default).

use alloy_primitives::{address, Address, U256};
use alloy_rpc_types_trace::{
    geth::{PreStateConfig, PreStateFrame},
    parity::{Delta, StateDiff},
};
use revm::{
    context_interface::result::{ExecutionResult, Output, ResultAndState},
    database::CacheDB,
    database_interface::EmptyDB,
    state::{Account, AccountInfo, AccountStatus, EvmState, EvmStorageSlot, FlaggedStorage},
};
use revm_inspectors::tracing::parity::populate_state_diff;

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Build an `EvmState` with a single account that has the given storage slots.
fn make_state_single(
    addr: Address,
    nonce: u64,
    balance: U256,
    slots: Vec<(U256, EvmStorageSlot)>,
    status: AccountStatus,
) -> EvmState {
    let storage = slots.into_iter().collect();
    let account = Account {
        info: AccountInfo { nonce, balance, ..Default::default() },
        storage,
        transaction_id: 0,
        status,
    };
    let mut state = EvmState::default();
    state.insert(addr, account);
    state
}

/// Create a `CacheDB` with a single account that has the given info (no storage).
fn make_db(addr: Address, nonce: u64, balance: U256) -> CacheDB<EmptyDB> {
    let mut db = CacheDB::new(EmptyDB::default());
    db.insert_account_info(addr, AccountInfo { nonce, balance, ..Default::default() });
    db
}

/// Create a `ResultAndState` with successful execution and the given state.
fn make_result_and_state(state: EvmState) -> ResultAndState {
    ResultAndState {
        result: ExecutionResult::Success {
            reason: revm::context_interface::result::SuccessReason::Return,
            gas_used: 21000,
            gas_refunded: 0,
            logs: vec![],
            output: Output::Call(Default::default()),
        },
        state,
    }
}

const ADDR: Address = address!("0x0000000000000000000000000000000000001234");
const SLOT_0: U256 = U256::ZERO;
const SLOT_1: U256 = U256::from_limbs([1, 0, 0, 0]);

// ═══════════════════════════════════════════════════════════════════════════════
// Geth PreState Tracer — Pre Mode
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_geth_prestate_filters_private_slot() {
    let state = make_state_single(
        ADDR,
        1,
        U256::from(1000),
        vec![(SLOT_0, EvmStorageSlot::new(FlaggedStorage::private(42u64), 0))],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .geth_prestate_traces(&result, &PreStateConfig::default(), &db)
        .unwrap();

    let PreStateFrame::Default(mode) = frame else { panic!("expected Default") };
    let acc = mode.0.get(&ADDR).expect("account should be present");
    assert!(acc.storage.is_empty(), "private slot must be filtered out");
}

#[test]
fn test_geth_prestate_shows_public_slot() {
    let state = make_state_single(
        ADDR,
        1,
        U256::from(1000),
        vec![(SLOT_0, EvmStorageSlot::new(FlaggedStorage::public(42u64), 0))],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .geth_prestate_traces(&result, &PreStateConfig::default(), &db)
        .unwrap();

    let PreStateFrame::Default(mode) = frame else { panic!("expected Default") };
    let acc = mode.0.get(&ADDR).expect("account should be present");
    assert_eq!(acc.storage.len(), 1, "public slot must be visible");
}

#[test]
fn test_geth_prestate_filter_disabled_shows_private() {
    let state = make_state_single(
        ADDR,
        1,
        U256::from(1000),
        vec![(SLOT_0, EvmStorageSlot::new(FlaggedStorage::private(42u64), 0))],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .with_filter_private_storage(false)
        .geth_prestate_traces(&result, &PreStateConfig::default(), &db)
        .unwrap();

    let PreStateFrame::Default(mode) = frame else { panic!("expected Default") };
    let acc = mode.0.get(&ADDR).expect("account should be present");
    assert_eq!(acc.storage.len(), 1, "private slot must be visible when filter disabled");
}

#[test]
fn test_geth_prestate_mixed_public_private() {
    let state = make_state_single(
        ADDR,
        1,
        U256::from(1000),
        vec![
            (SLOT_0, EvmStorageSlot::new(FlaggedStorage::private(42u64), 0)),
            (SLOT_1, EvmStorageSlot::new(FlaggedStorage::public(100u64), 0)),
        ],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .geth_prestate_traces(&result, &PreStateConfig::default(), &db)
        .unwrap();

    let PreStateFrame::Default(mode) = frame else { panic!("expected Default") };
    let acc = mode.0.get(&ADDR).expect("account should be present");
    assert_eq!(acc.storage.len(), 1, "only public slot should be visible");
    assert!(
        acc.storage.values().any(|v| *v == alloy_primitives::B256::from(U256::from(100))),
        "public slot value should be 100"
    );
}

#[test]
fn test_geth_prestate_zero_value_private_slot() {
    // A private slot with value=0 should still be filtered (don't leak access pattern)
    let state = make_state_single(
        ADDR,
        1,
        U256::from(1000),
        vec![(SLOT_0, EvmStorageSlot::new(FlaggedStorage::private(0u64), 0))],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .geth_prestate_traces(&result, &PreStateConfig::default(), &db)
        .unwrap();

    let PreStateFrame::Default(mode) = frame else { panic!("expected Default") };
    let acc = mode.0.get(&ADDR).expect("account should be present");
    assert!(acc.storage.is_empty(), "zero-value private slot must still be filtered");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Geth PreState Tracer — Diff Mode
// ═══════════════════════════════════════════════════════════════════════════════

fn diff_config() -> PreStateConfig {
    PreStateConfig { diff_mode: Some(true), ..Default::default() }
}

#[test]
fn test_geth_diff_filters_private_slot() {
    let state = make_state_single(
        ADDR,
        2, // nonce changed from 1 -> 2 to ensure account survives retain_changed
        U256::from(1000),
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::private(42u64),
                FlaggedStorage::private(43u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .geth_prestate_traces(&result, &diff_config(), &db)
        .unwrap();

    let PreStateFrame::Diff(diff) = frame else { panic!("expected Diff") };
    let pre = diff.pre.get(&ADDR).expect("account in pre");
    let post = diff.post.get(&ADDR).expect("account in post");
    assert!(pre.storage.is_empty(), "private slot absent from pre");
    assert!(post.storage.is_empty(), "private slot absent from post");
}

#[test]
fn test_geth_diff_original_public_present_private() {
    // Public -> private transition: still filtered (either side private = skip)
    let state = make_state_single(
        ADDR,
        2,
        U256::from(1000),
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::public(42u64),
                FlaggedStorage::private(43u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .geth_prestate_traces(&result, &diff_config(), &db)
        .unwrap();

    let PreStateFrame::Diff(diff) = frame else { panic!("expected Diff") };
    let pre = diff.pre.get(&ADDR).expect("account in pre");
    assert!(pre.storage.is_empty(), "slot filtered when present is private");
}

#[test]
fn test_geth_diff_original_private_present_public() {
    // Private -> public transition: still filtered
    let state = make_state_single(
        ADDR,
        2,
        U256::from(1000),
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::private(42u64),
                FlaggedStorage::public(43u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .geth_prestate_traces(&result, &diff_config(), &db)
        .unwrap();

    let PreStateFrame::Diff(diff) = frame else { panic!("expected Diff") };
    let pre = diff.pre.get(&ADDR).expect("account in pre");
    assert!(pre.storage.is_empty(), "slot filtered when original is private");
}

#[test]
fn test_geth_diff_shows_public_slot() {
    let state = make_state_single(
        ADDR,
        2,
        U256::from(1000),
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::public(42u64),
                FlaggedStorage::public(43u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .geth_prestate_traces(&result, &diff_config(), &db)
        .unwrap();

    let PreStateFrame::Diff(diff) = frame else { panic!("expected Diff") };
    let pre = diff.pre.get(&ADDR).expect("account in pre");
    let post = diff.post.get(&ADDR).expect("account in post");
    assert_eq!(pre.storage.len(), 1, "public slot visible in pre");
    assert_eq!(post.storage.len(), 1, "public slot visible in post");
}

#[test]
fn test_geth_diff_filter_disabled() {
    let state = make_state_single(
        ADDR,
        2,
        U256::from(1000),
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::private(42u64),
                FlaggedStorage::private(43u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));
    let result = make_result_and_state(state);

    let frame = revm_inspectors::tracing::GethTraceBuilder::new(vec![])
        .with_filter_private_storage(false)
        .geth_prestate_traces(&result, &diff_config(), &db)
        .unwrap();

    let PreStateFrame::Diff(diff) = frame else { panic!("expected Diff") };
    let pre = diff.pre.get(&ADDR).expect("account in pre");
    let post = diff.post.get(&ADDR).expect("account in post");
    assert_eq!(pre.storage.len(), 1, "private slot visible when filter disabled");
    assert_eq!(post.storage.len(), 1, "private slot visible when filter disabled");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Parity StateDiff — populate_state_diff
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_parity_statediff_filters_private_new_account() {
    // Newly created account with private storage
    let state = make_state_single(
        ADDR,
        0,
        U256::ZERO,
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(FlaggedStorage::ZERO, FlaggedStorage::private(42u64), 0),
        )],
        AccountStatus::Created | AccountStatus::Touched,
    );
    let db = CacheDB::new(EmptyDB::default()); // account not in DB (newly created)

    let mut diff = StateDiff::default();
    populate_state_diff(&mut diff, &db, state.iter(), true).unwrap();

    let entry = diff.0.get(&ADDR).expect("account should be in diff");
    assert!(entry.storage.is_empty(), "private slot must be filtered from new account");
}

#[test]
fn test_parity_statediff_shows_public_new_account() {
    let state = make_state_single(
        ADDR,
        0,
        U256::ZERO,
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(FlaggedStorage::ZERO, FlaggedStorage::public(42u64), 0),
        )],
        AccountStatus::Created | AccountStatus::Touched,
    );
    let db = CacheDB::new(EmptyDB::default());

    let mut diff = StateDiff::default();
    populate_state_diff(&mut diff, &db, state.iter(), true).unwrap();

    let entry = diff.0.get(&ADDR).expect("account should be in diff");
    assert_eq!(entry.storage.len(), 1, "public slot must be visible");
    let delta = entry.storage.values().next().unwrap();
    assert!(matches!(delta, Delta::Added(_)), "new account slot should be Delta::Added");
}

#[test]
fn test_parity_statediff_filters_private_modified_account() {
    // Existing account whose storage changed (both values private)
    let state = make_state_single(
        ADDR,
        2,
        U256::from(500), // balance changed to keep account in diff
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::private(10u64),
                FlaggedStorage::private(20u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));

    let mut diff = StateDiff::default();
    populate_state_diff(&mut diff, &db, state.iter(), true).unwrap();

    let entry = diff.0.get(&ADDR).expect("account should be in diff (balance changed)");
    assert!(entry.storage.is_empty(), "private slot must be filtered");
}

#[test]
fn test_parity_statediff_mixed_privacy_transition() {
    // Public original -> private present: filtered
    let state = make_state_single(
        ADDR,
        2,
        U256::from(500),
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::public(10u64),
                FlaggedStorage::private(20u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));

    let mut diff = StateDiff::default();
    populate_state_diff(&mut diff, &db, state.iter(), true).unwrap();

    let entry = diff.0.get(&ADDR).expect("account in diff");
    assert!(entry.storage.is_empty(), "slot filtered when present is private");
}

#[test]
fn test_parity_statediff_public_modified_shown() {
    let state = make_state_single(
        ADDR,
        2,
        U256::from(500),
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::public(10u64),
                FlaggedStorage::public(20u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));

    let mut diff = StateDiff::default();
    populate_state_diff(&mut diff, &db, state.iter(), true).unwrap();

    let entry = diff.0.get(&ADDR).expect("account in diff");
    assert_eq!(entry.storage.len(), 1, "public slot must be visible");
    let delta = entry.storage.values().next().unwrap();
    assert!(matches!(delta, Delta::Changed(_)), "modified slot should be Delta::Changed");
}

#[test]
fn test_parity_statediff_filter_disabled() {
    let state = make_state_single(
        ADDR,
        2,
        U256::from(500),
        vec![(
            SLOT_0,
            EvmStorageSlot::new_changed(
                FlaggedStorage::private(10u64),
                FlaggedStorage::private(20u64),
                0,
            ),
        )],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));

    let mut diff = StateDiff::default();
    populate_state_diff(&mut diff, &db, state.iter(), false).unwrap();

    let entry = diff.0.get(&ADDR).expect("account in diff");
    assert_eq!(entry.storage.len(), 1, "private slot visible when filter disabled");
}

#[test]
fn test_parity_statediff_mixed_slots_new_account() {
    let state = make_state_single(
        ADDR,
        0,
        U256::ZERO,
        vec![
            (
                SLOT_0,
                EvmStorageSlot::new_changed(
                    FlaggedStorage::ZERO,
                    FlaggedStorage::private(42u64),
                    0,
                ),
            ),
            (
                SLOT_1,
                EvmStorageSlot::new_changed(
                    FlaggedStorage::ZERO,
                    FlaggedStorage::public(100u64),
                    0,
                ),
            ),
        ],
        AccountStatus::Created | AccountStatus::Touched,
    );
    let db = CacheDB::new(EmptyDB::default());

    let mut diff = StateDiff::default();
    populate_state_diff(&mut diff, &db, state.iter(), true).unwrap();

    let entry = diff.0.get(&ADDR).expect("account in diff");
    assert_eq!(entry.storage.len(), 1, "only public slot should appear");
}

#[test]
fn test_parity_statediff_mixed_slots_modified_account() {
    let state = make_state_single(
        ADDR,
        2,
        U256::from(500),
        vec![
            (
                SLOT_0,
                EvmStorageSlot::new_changed(
                    FlaggedStorage::private(10u64),
                    FlaggedStorage::private(20u64),
                    0,
                ),
            ),
            (
                SLOT_1,
                EvmStorageSlot::new_changed(
                    FlaggedStorage::public(30u64),
                    FlaggedStorage::public(40u64),
                    0,
                ),
            ),
        ],
        AccountStatus::Touched,
    );
    let db = make_db(ADDR, 1, U256::from(1000));

    let mut diff = StateDiff::default();
    populate_state_diff(&mut diff, &db, state.iter(), true).unwrap();

    let entry = diff.0.get(&ADDR).expect("account in diff");
    assert_eq!(entry.storage.len(), 1, "only public slot should appear");
}
