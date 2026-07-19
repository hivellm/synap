## 1. Implementation
- [ ] 1.1 Python SDK: `WatchEvent` dataclass + envelope decode, `kv.watch(pattern, mode)` async iterator over the existing push transport, close issues UNWATCH
- [ ] 1.2 Python SDK: type-check (mypy/pyright) + lint clean
- [ ] 1.3 PHP SDK: `WatchEvent` value object + `watch(pattern, callable, mode)` and iterator variant over the existing push transport, explicit `unwatch()`
- [ ] 1.4 PHP SDK: static analysis (phpstan) + lint clean
- [ ] 1.5 README examples for both SDKs

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (decode, watch iteration/callbacks, unwatch, wildcard, notify mode)
- [ ] 2.3 Run tests and confirm they pass
