## 1. Implementation
- [ ] 1.1 Cross-SDK interop matrix under s2s-tests: SET from each SDK, observe from every other, assert identical envelope decode across all six SDKs
- [ ] 1.2 Fan-out benchmark in `synap-server/benches/`: publish→push latency/throughput for N watchers (exact key and wildcard), including a stalled-watcher slow-consumer case
- [ ] 1.3 Protocol doc: `__watch@0__` channel family, envelope, WATCH/UNWATCH, `/kv/ws`, delivery semantics (best-effort, version gap detection, notify-only cap, streams for replay)
- [ ] 1.4 Per-SDK README watch sections + CHANGELOG entries (server and six SDKs)

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (interop matrix is the test body; ensure it runs green)
- [ ] 2.3 Run tests and confirm they pass
