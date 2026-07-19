## 1. Implementation
- [x] 1.1 Rust SDK: `WatchEvent` type + envelope decode, `kv.watch(pattern) -> Observable<WatchEvent>` over the existing push transport, teardown issues UNWATCH — `kv_watch.rs` returns `(impl Stream<Item = WatchEvent>, SubscriptionHandle)`, the SDK's established push idiom (mirrors `pubsub_reactive.rs`); teardown calls `KV.UNWATCH` on the push connection before closing it
- [x] 1.2 Rust SDK: `watch_with_mode` (value/notify) + reconnect follows pub/sub semantics; `cargo check` + clippy + fmt — notify mode honored on RPC; the `/kv/ws` WebSocket fallback covers `http://` clients (value mode, warned). Reconnect = the stream ends with the connection and the app re-watches, exactly like pub/sub
- [x] 1.3 TypeScript SDK: `WatchEvent<T>` type + `kv.watch<T>(pattern, opts?): Observable<WatchEvent<T>>` via rxjs, teardown issues UNWATCH — `new Observable` with real teardown-on-unsubscribe → `KV.UNWATCH` + connection close, including the torn-down-mid-handshake race
- [x] 1.4 TypeScript SDK: `withValueFetch` helper re-GETs on truncated/notify envelopes; `tsc --noEmit` + lint — rxjs operator, skips terminal events (del/expired/evicted); tsc clean, lint clean on new code (21 pre-existing lint errors elsewhere in the TS SDK remain)
- [x] 1.5 README examples for both SDKs — "KV Watch (Reactive)" sections in both READMEs

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation — docs/kv-watch.md points at the SDK surfaces; CHANGELOG updated
- [x] 2.2 Write tests covering the new behavior (decode, watch stream, unwatch on teardown, wildcard, notify mode) — 5 Rust unit tests, 1 Rust↔server e2e (wildcard watch + UNWATCH unwind via router assertion), 9 TS unit tests over a mocked transport
- [x] 2.3 Run tests and confirm they pass — Rust SDK 284 green, TS SDK 379 green, e2e green
