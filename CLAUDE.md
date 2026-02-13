# Seismic REVM Inspectors

Seismic's fork of [revm-inspectors](https://github.com/paradigmxyz/revm-inspectors) — a collection of EVM inspector implementations for call tracing, debugging, and execution analysis built on REVM. Upstream tracked through the `main` branch at commit `3353282`.

## What This Does

Provides inspector implementations that hook into the REVM execution engine to observe EVM execution. Inspectors capture call traces (Geth and Parity formats), access lists, opcode counts, internal ETH transfers, edge coverage, and storage access patterns. Seismic's fork replaces raw `U256` storage values with `FlaggedStorage` — a struct that attaches a boolean flag marking whether a value is associated with a **shielded type** — and filters shielded storage changes out of trace output.

## Build

Rust library crate using Cargo. No binary output — consumed as a dependency by `seismic-reth` and `seismic-foundry`.

**Requirements:** Rust 1.86.0+ (MSRV), Git (for patched dependencies fetched via git)

### macOS

```bash
# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build (default features)
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build

# Build (all features, includes JS tracer with boa engine)
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --all-features
```

### Linux (Ubuntu)

```bash
# Dependencies
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build

# Build (all features)
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --all-features
```

### Environment Variable

`CARGO_NET_GIT_FETCH_WITH_CLI=true` is **required** for fetching patched git dependencies (`seismic-alloy-core`, `seismic-revm`). The Seismic CI sets this. Without it, Cargo's built-in git fetch may fail with SSH auth issues.

### Features

- `default` = `std`
- `std` — standard library support
- `serde` — serialization for trace types
- `js-tracer` — JavaScript tracer via Boa engine (pulls in significant deps)

## Test

```bash
# Default features (25 tests: 8 unit + 17 integration)
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test

# All features (62 tests: 38 unit + 24 integration, includes JS tracer tests)
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --all-features
```

### CI Checks (Seismic)

```bash
# Format check (requires nightly)
cargo +nightly fmt --all --check

# Warnings-as-errors check
CARGO_NET_GIT_FETCH_WITH_CLI=true RUSTFLAGS="-D warnings" cargo check
```

## Project Layout

```
src/
  lib.rs                   Library root (re-exports, feature flags)
  access_list.rs           EIP-2930 access list inspector
  edge_cov.rs              Edge coverage tracking
  opcode.rs                Opcode counting and gas measurement
  storage.rs               Storage access tracking
  transfer.rs              Internal ETH transfer tracking
  tracing/
    mod.rs                 TracingInspector core (main entry point)
    arena.rs               CallTraceArena — manages trace call tree
    config.rs              TracingInspectorConfig — controls what gets recorded
    types.rs               Core types (CallTrace, CallTraceNode, StorageChange)
    utils.rs               Utility functions (revert reason decoding)
    fourbyte.rs            4-byte function signature tracking
    opcount.rs             Opcode counting inspector
    mux.rs                 MuxInspector — combines multiple inspectors
    writer.rs              TraceWriter — human-readable trace output
    builder/
      mod.rs               Trace builder entry
      geth.rs              Geth-style trace builder (default/call/prestate/flatcall)
      parity.rs            Parity-style trace builder
      walker.rs            Callgraph traversal
    js/                    JavaScript tracer (feature: js-tracer)
      mod.rs               JS tracer implementation
      bindings.rs          JS runtime bindings
      builtins.rs          Built-in JS functions
tests/it/                  Integration tests
testdata/                  Test fixtures (Counter.sol)
```

## Key Seismic Modifications

- **`FlaggedStorage` integration**: `StorageChange` in `src/tracing/types.rs` uses `revm::primitives::FlaggedStorage` instead of `U256` for `value` and `had_value` fields
- **Shielded trace filtering**: `src/tracing/builder/geth.rs` — Geth prestate tracer skips shielded storage slots (`is_public()` checks), preventing confidential data from appearing in traces
- **`TraceWriter` formatting**: `src/tracing/writer.rs` — `num_or_hex_value()` accepts `FlaggedStorage` for display
- **JS bindings**: `src/tracing/js/bindings.rs` — `sload` returns `FlaggedStorage`
- **Patched dependencies**: `Cargo.toml` patches `alloy-primitives`, `alloy-sol-types`, `revm`, and `revm-primitives` to Seismic forks

## Code Style

Configured via `rustfmt.toml` and lints in `Cargo.toml`.

- **Max line width**: 100 characters
- **Imports**: Crate-level granularity, auto-reordered
- **Format**: `cargo +nightly fmt` (uses nightly-only options like `imports_granularity`)
- **Lints**: `missing_docs = warn`, `unused_must_use = deny`, `rust_2018_idioms = deny`
- **Clippy MSRV**: 1.88 (in `clippy.toml`)
- **`#![no_std]` compatible**: Uses `extern crate alloc`, gated behind `std` feature

## CI

- **`seismic.yml`** — runs on `seismic` branch/PRs: rustfmt, build, warnings (`-D warnings`), test
- **`ci.yml`** — upstream CI on `main`: test matrix (stable/nightly/1.88 MSRV × default/all-features), feature powerset via `cargo hack`, clippy, docs, `cargo-deny`, fmt

## Branches

- `seismic` — default/production branch (PR target)
- `main` — upstream-only commits, reflects last upstream merge point

## Troubleshooting

| Problem                                                                      | Fix                                                                                                                                                                                                                                             |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `non-exhaustive patterns: Erc7562Tracer not covered` in `src/tracing/mux.rs` | `Cargo.lock` is gitignored (library crate), so `alloy-rpc-types-trace` resolves to latest semver-compatible version which added a new enum variant. Add a wildcard `_ =>` arm to the match at `mux.rs:50`, matching what upstream already does. |
| Git fetch fails for patched dependencies                                     | Set `CARGO_NET_GIT_FETCH_WITH_CLI=true` before cargo commands. Ensure SSH keys or Git credentials are configured for GitHub access.                                                                                                             |
| `cargo fmt` fails with "Unknown option"                                      | `rustfmt.toml` uses nightly-only options (`imports_granularity`, `wrap_comments`, etc.). Run with `cargo +nightly fmt`.                                                                                                                         |
| Slow initial build                                                           | First build downloads and compiles ~400 transitive dependencies (crypto, EVM, alloy stack). Subsequent builds are incremental.                                                                                                                  |
| `unused_crate_dependencies` warning in tests                                 | Expected — `#![cfg_attr(not(test), warn(unused_crate_dependencies))]` suppresses this in test builds.                                                                                                                                           |
