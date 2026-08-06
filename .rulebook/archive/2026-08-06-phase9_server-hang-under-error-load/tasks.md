## 1. Reproduce

- [x] 1.1 Replay the load concurrently across transports against a release build
- [x] 1.2 Include the malformed queue publishes that preceded the observed hang
- [x] 1.3 Find the ingredient that actually triggers it — connections that subscribe and vanish

## 2. Diagnose

- [x] 2.1 Read the failure out of the log: a `tracing-subscriber` panic on a worker thread
- [x] 2.2 Identify the mechanism: `Span::enter()` guards held across `.await`
- [x] 2.3 Audit every `.enter()` in the async paths (three sites, all fixed)

## 3. Fix

- [x] 3.1 Instrument the futures instead of entering their spans
- [x] 3.2 Regression test asserting the async protocol paths never call `.enter()`
- [x] 3.3 Soak harness kept at `scripts/test/transport-soak.py`

## 4. Tail (docs + tests)

- [x] 4.1 Update or create documentation covering the implementation
- [x] 4.2 Write tests covering the new behavior
- [x] 4.3 Run tests and confirm they pass

## Result

Not a deadlock — a panic, once per affected worker thread.

`tracing::Span::enter()` returns a thread-local guard. All three async protocol
paths held one across an `.await`, so when a task yielded the span stayed
entered on that worker and the next task scheduled there ran inside another
connection's span. The log shows it directly: four different RESP3 peers nested
inside one another. Eventually the subscriber was asked to clone a span an
earlier task had already closed:

```
thread 'tokio-rt-worker' panicked at tracing-subscriber-0.3.23/src/registry/sharded.rs:317:
assertion `left != right` failed: tried to clone a span (Id(..)) that already closed
```

Each panic kills a worker. With enough of them the runtime has nothing left to
poll: listeners still accept TCP, nothing answers, and the periodic snapshot
task goes quiet too — which is exactly what was reported.

Reproduction, on the release build of the pre-fix code: connections that
`SUBSCRIBE` and then vanish without unsubscribing (what a finishing PHP or CLI
process does). The server stopped answering **2.0 seconds** into the run,
1288 client errors. After the fix, the same harness ran 180s — 107,886 churned
connections, 54k RESP3 rounds, 132k HTTP requests — with **zero** errors and
the server answering throughout.
