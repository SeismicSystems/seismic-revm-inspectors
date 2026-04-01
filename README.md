# Seismic REVM Inspectors

Seismic's fork of [revm-inspectors](https://github.com/paradigmxyz/revm-inspectors), kept as minimal as possible. The `main` branch tracks upstream; `seismic` is the default/production branch.

## What traces expose after sanitization

| Data field                                | seismic-reth | seismic-foundry | Where filtered                      |
| ----------------------------------------- | ------------ | --------------- | ----------------------------------- |
| Call graph (from, to, value, gas, type)   | kept         | kept            | —                                   |
| Execution flow (pc, opcode, gas per step) | kept         | kept            | —                                   |
| Errors / reverts                          | kept         | kept            | —                                   |
| Logs / events                             | kept         | kept            | —                                   |
| Account balances / nonces / code          | kept         | kept            | —                                   |
| Public storage slots                      | kept         | kept            | —                                   |
| **Private storage slots**                 | **omitted**  | kept            | builders (`filter_private_storage`) |
| **Calldata (all frames)**                 | **stripped** | kept            | sanitizer                           |
| **Return data (all frames)**              | **stripped** | kept            | sanitizer                           |
| **Stack**                                 | **stripped** | kept            | sanitizer (defensive)               |
| **Memory**                                | **stripped** | kept            | sanitizer (defensive)               |
| **VM trace push/mem/store**               | **stripped** | kept            | sanitizer                           |
| **Function selectors (4byte)**            | **stripped** | kept            | sanitizer                           |
| **JS tracer**                             | **disabled** | disabled        | feature flag                        |
| **ERC-7562 tracer**                       | **disabled** | disabled        | not available in pinned alloy       |

## What this fork does

This fork threads `FlaggedStorage` through the tracing pipeline and provides **configurable storage filtering** and a **trace output sanitizer**. Seismic's EVM ([seismic-revm](https://github.com/SeismicSystems/seismic-revm)) uses `FlaggedStorage` instead of `U256` for storage values, attaching an `is_private` flag to each slot. The inspector code reads storage from the EVM journal, so it needs matching types.

### Trace output sanitizer (`trace_sanitizer` module)

This crate also provides a `trace_sanitizer` module with functions to strip private data from built trace output. 
seismic-reth calls these from every debug/trace RPC handler before returning results to callers. 
The sanitizer handles:
- **Calldata**: stripped from all frames unconditionally (callers already know their own calldata; for regular txs it's available via `eth_getTransactionByHash`)
- **Return data**: stripped from all frames
- **Stack/memory**: defensively stripped (should already be disabled via `TracingInspectorConfig`, but stripped as a safety net)
- **VM trace payloads**: push values, memory deltas, storage deltas stripped from `VmExecutedOperation`
- **FourByteTracer**: stripped (function selectors reveal which function was called)
- **Unknown/future trace types**: return NoopFrame (safe by default)
- **Logs**: passed through (public by design)
- **Gas/opcodes**: passed through

All trace privacy logic lives in this crate so auditors can review it in one place. seismic-reth's responsibility is limited to calling `sanitize_geth_trace` / `sanitize_trace_results` / `sanitize_localized_transaction_trace` on every RPC handler return path.

### Storage filtering (configurable)

The trace builders (`GethTraceBuilder`, `ParityTraceBuilder`, `populate_state_diff`) support a `filter_private_storage` flag that omits storage slots where `FlaggedStorage::is_private` is true.
This defaults to **true** (filter on), so private storage is hidden unless the caller explicitly opts out.

- **seismic-reth**: uses the default (`true`) — private slots are omitted from all trace output
- **seismic-foundry**: opts out with `.with_filter_private_storage(false)` — full traces for local debugging

Private slots are omitted entirely (not zeroed out) to avoid leaking access patterns.

#### Why storage filtering is in the builders, not the sanitizer

The trace output types (`PreStateFrame`, `StateDiff`, etc.) come from `alloy-rpc-types-trace` which we don't fork — they use `B256` for storage values, so the `is_private` flag from `FlaggedStorage` is lost after the builders convert storage to output format.
Storage filtering must happen in the builders where `FlaggedStorage` is still available.

### JS tracer disabled

The upstream `js-tracer` feature allows callers to supply arbitrary JavaScript that runs *during* EVM execution with live access to `log.stack.peek()`, `log.memory.slice()`, and `db.getState()`. This hands private data to untrusted code in real time — no post-processing sanitizer can help.

Re-enabling this safely would require making revm's stack use `FlaggedStorage` (taint tracking), so the JS tracer's stack/memory/storage APIs could filter private values before handing them to user code. That's a large engineering lift (every opcode handler needs taint propagation) and not planned currently.

The feature flag is kept in `Cargo.toml` (to avoid breaking transitive dependency chains in seismic-reth) but maps to an empty feature set — enabling it is a no-op. The JS tracer module is commented out and the source files are not compiled.

### ERC-7562 tracer not available

The ERC-7562 tracer (used by AA bundlers for validation rule enforcement) collects keccak preimages, storage access maps, calldata, and opcode usage — all of which can leak private data. The `GethDebugBuiltInTracerType::Erc7562Tracer` variant does not exist in our pinned alloy version (v1.1.0), so it is currently unreachable. When upgrading alloy, this tracer must remain disabled — the sanitizer's catch-all returns a NoopFrame for any unknown/future variant, but an explicit reject in reth's RPC dispatch (similar to the JS tracer) should be added at that time.

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
