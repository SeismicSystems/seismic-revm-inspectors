# Seismic REVM Inspectors

Seismic's fork of [revm-inspectors](https://github.com/paradigmxyz/revm-inspectors), kept as minimal as possible. The `main` branch tracks upstream; `seismic` is the default/production branch.

## What this fork does

This fork threads `FlaggedStorage` through the tracing pipeline and provides **configurable storage filtering**. Seismic's EVM ([seismic-revm](https://github.com/SeismicSystems/seismic-revm)) uses `FlaggedStorage` instead of `U256` for storage values, attaching an `is_private` flag to each slot. The inspector code reads storage from the EVM journal, so it needs matching types.

### Storage filtering (configurable)

The trace builders (`GethTraceBuilder`, `ParityTraceBuilder`, `populate_state_diff`) support a `filter_private_storage` flag that omits storage slots where `FlaggedStorage::is_private` is true. This defaults to **true** (filter on), so private storage is hidden unless the caller explicitly opts out.

- **seismic-reth**: uses the default (`true`) — private slots are omitted from all trace output
- **seismic-foundry**: opts out with `.with_filter_private_storage(false)` — full traces for local debugging

Private slots are omitted entirely (not zeroed out) to avoid leaking access patterns.

### JS tracer disabled

The upstream `js-tracer` feature allows callers to supply arbitrary JavaScript that runs *during* EVM execution with live access to `log.stack.peek()`, `log.memory.slice()`, and `db.getState()`. This hands private data to untrusted code in real time — no post-processing sanitizer can help.

Re-enabling this safely would require making revm's stack use `FlaggedStorage` (taint tracking), so the JS tracer's stack/memory/storage APIs could filter private values before handing them to user code. That's a large engineering lift (every opcode handler needs taint propagation) and not planned currently.

The feature flag is kept in `Cargo.toml` (to avoid breaking transitive dependency chains in seismic-reth) but maps to an empty feature set — enabling it is a no-op. The JS tracer module is commented out and the source files are not compiled.

## Trace sanitization architecture

Ideally all privacy filtering would live in a single place (seismic-reth's RPC layer). However, the trace output types (`PreStateFrame`, `StateDiff`, etc.) come from `alloy-rpc-types-trace` which we don't fork — they use `B256` for storage values, so the `is_private` flag from `FlaggedStorage` is lost after the builders convert storage to output format. This means storage filtering must happen in the builders in this fork, where `FlaggedStorage` is still available.

The result is a two-layer architecture:

**This fork (revm-inspectors)** handles:
- **Storage values**: filtered by `is_private` via the configurable `filter_private_storage` flag on builders.

**seismic-reth RPC layer** handles everything else:
- **Stack/memory**: not recorded (disabled via `TracingInspectorConfig` with `record_stack_snapshots: false`, `record_memory_snapshots: false`)
- **Calldata/returndata**: stripped or replaced with encrypted form for `TxSeismic` transactions
- **VM trace payloads**: push values, memory deltas, storage deltas stripped from `VmExecutedOperation`
- **Logs**: passed through (public by design)
- **Gas/opcodes**: passed through

The split follows data availability — storage filtering happens here where `FlaggedStorage` is available, while calldata/returndata sanitization happens in seismic-reth where the original `TransactionSigned` (with encrypted calldata) is available.

## Users

* [`seismic-reth`] — execution client
* [`seismic-foundry`] — dev tools (sforge, sanvil, scast)

[`seismic-reth`]: https://github.com/SeismicSystems/seismic-reth/
[`seismic-foundry`]: https://github.com/SeismicSystems/seismic-foundry/

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in these crates by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
</sub>
