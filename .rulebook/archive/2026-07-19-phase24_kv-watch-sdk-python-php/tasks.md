## 1. Implementation
- [x] 1.1 Python SDK: `WatchEvent` dataclass + envelope decode, `kv.watch(pattern, mode)` async iterator over the existing push transport, close issues UNWATCH — `watch_push` on the RPC transport (KV.WATCH twin of `subscribe_push`); the async generator's `finally` cancels, and cancel issues `KV.UNWATCH` before closing
- [x] 1.2 Python SDK: type-check (mypy/pyright) + lint clean — ruff + mypy clean on all touched files (pre-existing debt elsewhere untouched)
- [x] 1.3 PHP SDK: `WatchEvent` value object + `watch(pattern, callable, mode)` and iterator variant over the existing push transport, explicit `unwatch()` — `watchPushIterator` Generator is the core (pump yields between PING/collect cycles, thunder#11 workaround); `watchPush` is the callback wrapper; `KV.UNWATCH` fires in the Generator's `finally`. Committed in the `synap-sdk-php` submodule as 5634b75 (push + superproject pointer bump left to the user)
- [x] 1.4 PHP SDK: static analysis (phpstan) + lint clean — phpstan level=max reports zero findings in the new code (186 pre-existing errors elsewhere untouched); phpunit run via docker (no local PHP runtime)
- [x] 1.5 README examples for both SDKs — "KV Watch" sections in both READMEs

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation — both READMEs + both CHANGELOGs
- [x] 2.2 Write tests covering the new behavior (decode, watch iteration/callbacks, unwatch, wildcard, notify mode) — 6 Python tests (decode/defaults/truncated, pattern+mode passthrough, aclose→cancel, HTTP rejection), 6 PHP tests (envelope decode/defaults/truncated/roundtrip + both surfaces reject HTTP)
- [x] 2.3 Run tests and confirm they pass — Python 187 green (6 new), PHP watch tests green in docker; pre-existing PHP S2S tests still require a live server
