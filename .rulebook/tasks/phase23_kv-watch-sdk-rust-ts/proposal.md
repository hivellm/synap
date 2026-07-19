# Proposal: phase23_kv-watch-sdk-rust-ts

Source: docs/analysis/kv-watch-observable/ (F-009, F-010, F-011)

## Why

With WATCH live on the server (phases 21-22), SDKs need an ergonomic `kv.watch()` so
applications can observe keys without hand-rolling pub/sub channel names and envelope
decoding. Rust and TypeScript go first because they have the richest reactive scaffolding
(`sdks/rust/src/rx/` with RxJS-parity Observables; TS uses rxjs directly) and serve as the
reference implementations for the remaining SDKs.

## What Changes

- Rust SDK: `kv.watch(pattern) -> Observable<WatchEvent>` (and `watch_with_mode`), built on
  the existing `subscribePush` transport path mirroring `pubsub_reactive.rs`. Typed
  `WatchEvent { key, event, version, value, truncated }` decoded from the MessagePack envelope.
- TypeScript SDK: `kv.watch<T>(pattern, opts?): Observable<WatchEvent<T>>` via rxjs,
  mirroring `pubsub.ts`. Includes a `withValueFetch` helper that transparently re-GETs when
  an envelope arrives truncated or in notify mode.
- Both: unsubscribe on Observable teardown maps to `UNWATCH`; reconnect behavior follows
  the SDK's existing pub/sub reconnect semantics.

## Impact

- Affected specs: specs/kv-watch-sdk/spec.md (ADDED)
- Affected code: sdks/rust/src/ (new kv_watch module + kv surface), sdks/typescript/src/
  (kv module + transports/synap-rpc.ts reuse)
- Breaking change: NO (additive API)
- User benefit: one-line reactive key observation in the two reference SDKs.
