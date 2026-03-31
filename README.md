# Seismic REVM Inspectors

Seismic's fork of [revm-inspectors](https://github.com/paradigmxyz/revm-inspectors), kept as minimal as possible. The `main` branch tracks upstream; `seismic` is the production branch.

## What this fork does

This fork exists solely to **thread `FlaggedStorage` through the tracing pipeline**. Seismic's EVM ([seismic-revm](https://github.com/SeismicSystems/seismic-revm)) uses `FlaggedStorage` instead of `U256` for storage values, attaching an `is_private` flag to each slot. The inspector code reads storage from the EVM journal, so it needs matching types.

### JS tracer disabled

The upstream `js-tracer` feature allows callers to supply arbitrary JavaScript that runs *during* EVM execution with live access to `log.stack.peek()`, `log.memory.slice()`, and `db.getState()`. This hands private data to untrusted code in real time — no post-processing sanitizer can help.

Re-enabling this safely would require making revm's stack use `FlaggedStorage` (taint tracking), so the JS tracer's stack/memory/storage APIs could filter private values before handing them to user code. That's a large engineering lift (every opcode handler needs taint propagation) and not planned currently.

The feature flag is kept in `Cargo.toml` (to avoid breaking transitive dependency chains in seismic-reth) but maps to an empty feature set — enabling it is a no-op. The JS tracer module is commented out and the source files are not compiled.

### What this fork does NOT do

This fork does **not** sanitize or filter trace output. No shielding, no masking, no `is_public()` checks. Traces are recorded faithfully, exactly as upstream does.

## Trace sanitization architecture

Privacy-sensitive filtering of trace output happens in [seismic-reth](https://github.com/SeismicSystems/seismic-reth), not here. A centralized `sanitize_trace()` function in the RPC layer processes all trace output before returning it to callers:

- **Storage values**: filtered by `is_public()` using the `FlaggedStorage` flag already recorded in `StorageChange`
- **Stack/memory**: not recorded (disabled via `TracingInspectorConfig`)
- **Calldata/returndata**: stripped or replaced with encrypted form for `TxSeismic`
- **Logs**: passed through (public by design)
- **JS tracer**: disabled at the feature level (this fork) and compile-time (seismic-reth)

Because the inspectors record traces faithfully (no filtering here), `sforge` and other foundry tools get full unredacted traces during local development, which is helpful for debugging - there's no privacy to protect on a local devnet anyways.

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
