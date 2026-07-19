## 1. Implementation
- [ ] 1.1 Rust SDK: `WatchEvent` type + envelope decode, `kv.watch(pattern) -> Observable<WatchEvent>` over the existing push transport, teardown issues UNWATCH
- [ ] 1.2 Rust SDK: `watch_with_mode` (value/notify) + reconnect follows pub/sub semantics; `cargo check` + clippy + fmt
- [ ] 1.3 TypeScript SDK: `WatchEvent<T>` type + `kv.watch<T>(pattern, opts?): Observable<WatchEvent<T>>` via rxjs, teardown issues UNWATCH
- [ ] 1.4 TypeScript SDK: `withValueFetch` helper re-GETs on truncated/notify envelopes; `tsc --noEmit` + lint
- [ ] 1.5 README examples for both SDKs

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (decode, watch stream, unwatch on teardown, wildcard, notify mode)
- [ ] 2.3 Run tests and confirm they pass
