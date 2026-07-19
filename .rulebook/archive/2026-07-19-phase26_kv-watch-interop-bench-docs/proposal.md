# Proposal: phase26_kv-watch-interop-bench-docs

Source: docs/analysis/kv-watch-observable/ (all findings; risks F-005, F-007, F-008)

## Why

Watch spans six SDKs and two server protocols; without cross-SDK interop tests the envelope
contract will drift, and without a fan-out benchmark the "efficient for millions of
connections" claim is unvalidated. The delivery semantics (best-effort, version-based gap
detection, notify-only degradation) must be documented so users don't assume guaranteed
delivery — replay users must be pointed at streams.

## What Changes

- Interop matrix (s2s-tests feature, requires a running server): SET from each SDK,
  observe the event from every other SDK; asserts identical envelope decode (key, event,
  version, value, truncated) across rust/ts/python/php/csharp/go.
- Fan-out benchmark (`synap-server/benches/`): N watchers on one key and on a wildcard,
  measuring publish→push latency and throughput as N grows; documents slow-consumer drop
  behavior under a stalled watcher.
- Docs: protocol document for the `__watch@0__` channel family + envelope + WATCH/UNWATCH +
  `/kv/ws`; per-SDK README watch sections; delivery-semantics section (best-effort, version
  gaps, notify-only cap, streams for replay); CHANGELOG entries for server and each SDK.

## Impact

- Affected specs: specs/kv-watch/spec.md (ADDED interop/bench requirements)
- Affected code: crates/synap-server/tests/ (s2s), crates/synap-server/benches/,
  docs/, per-SDK READMEs and CHANGELOGs
- Breaking change: NO
- User benefit: verified cross-language contract, published performance profile, and honest
  documented semantics.
