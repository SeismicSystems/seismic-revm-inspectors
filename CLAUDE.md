> **Merge note**: This file exists only on the `seismic` branch. Upstream
> `revm-inspectors` does not have a CLAUDE.md yet. If upstream adds one,
> place their content above the divider below and keep the Seismic section
> intact — it takes precedence where it overlaps with upstream.

---

# Seismic Fork Extensions

Seismic's fork of [revm-inspectors](https://github.com/paradigmxyz/revm-inspectors) — EVM inspector implementations for call tracing, debugging, and execution analysis. Upstream tracked through the `main` branch at commit `696368b`.

## What This Fork Changes

Replaces raw `U256` storage values with `FlaggedStorage` throughout the inspector stack — a `(value, is_private)` tuple that tracks whether a storage slot is associated with a shielded type. Shielded storage changes are filtered out of Geth and Parity trace output to prevent confidential data from appearing in traces. Consumed as a dependency by `seismic-reth` and `seismic-foundry`.

## Key Seismic Modifications

- **`FlaggedStorage` integration**: `StorageChange` in `src/tracing/types.rs` uses `revm::primitives::FlaggedStorage` instead of `U256` for `value` and `had_value` fields. `CallTrace` gains a `tx_type` field.
- **Shielded Geth trace filtering**: `src/tracing/builder/geth.rs` — Geth prestate tracer skips shielded storage slots (`is_public()` checks), preventing confidential data from appearing in traces
- **Shielded Parity trace filtering**: `src/tracing/builder/parity.rs` and `src/tracing/types.rs` — Parity trace builder masks call input/output data for the root call via `_with_shielding` method variants (`parity_transaction_trace_with_shielding`, `parity_action`, `parity_trace_output_with_shielding`)
- **`TraceWriter` formatting**: `src/tracing/writer.rs` — `num_or_hex_value()` accepts `FlaggedStorage` for display
- **JS bindings**: `src/tracing/js/bindings.rs` — `sload` returns `FlaggedStorage`
- **Patched dependencies**: `Cargo.toml` patches `alloy-primitives`, `alloy-sol-types`, `revm`, and `revm-primitives` to Seismic forks

## Build

Rust library crate (no binary output). Requirements: Rust 1.86.0+, Git.

```bash
# Build (default features)
cargo build

# Build (all features, includes JS tracer)
cargo build --all-features
```

### Features

- `default` = `std`
- `std` — standard library support
- `serde` — serialization for trace types
- `js-tracer` — JavaScript tracer via Boa engine (pulls in significant deps)

## Test

```bash
# Default features
cargo test

# All features (includes JS tracer tests)
cargo test --all-features
```

### CI Checks

```bash
# Format check (requires nightly)
cargo +nightly fmt --all --check

# Warnings-as-errors check
RUSTFLAGS="-D warnings" cargo check
```

## CI

**`seismic.yml`** — runs on `seismic` branch/PRs: rustfmt, build, warnings (`-D warnings`), test. Other workflow files (e.g., `ci.yml`) are from upstream and do not run on Seismic branches.

## Branches

- `seismic` — default/production branch (PR target)
- `main` — upstream-only commits, reflects last upstream merge point

## Patched Dependencies

`Cargo.toml` `[patch.crates-io]` points to Seismic forks:
- `alloy-primitives`, `alloy-json-abi`, `alloy-sol-macro-expander`, `alloy-sol-macro-input`, `alloy-sol-types`, `alloy-sol-type-parser` → `seismic-alloy-core`
- `revm`, `revm-primitives` → `seismic-revm`

## Troubleshooting

| Problem | Fix |
| --- | --- |
| `non-exhaustive patterns: Erc7562Tracer not covered` in `src/tracing/mux.rs` | `Cargo.lock` is gitignored (library crate), so `alloy-rpc-types-trace` resolves to latest semver-compatible version which added a new enum variant. Add a wildcard `_ =>` arm to the match at `mux.rs:50`, matching what upstream already does. |
| `cargo fmt` fails with "Unknown option" | `rustfmt.toml` uses nightly-only options (`imports_granularity`, `wrap_comments`, etc.). Run with `cargo +nightly fmt`. |
| `unused_crate_dependencies` warning in tests | Expected — `#![cfg_attr(not(test), warn(unused_crate_dependencies))]` suppresses this in test builds. |
