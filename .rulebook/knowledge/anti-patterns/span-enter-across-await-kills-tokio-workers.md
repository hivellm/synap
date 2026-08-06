# `Span::enter()` across an await kills tokio workers

**Category**: concurrency
**Tags**: tracing, tokio, resp3, synaprpc, panic, phase9

## Description

`tracing::Span::enter()` returns a **thread-local** guard. In async code, a task
that yields while holding one leaves the span entered on that worker thread, so
the next task scheduled there runs inside a span it does not own. The damage is
not cosmetic:

```text
thread 'tokio-rt-worker' panicked at tracing-subscriber-0.3.23/src/registry/sharded.rs:317:
assertion `left != right` failed: tried to clone a span (Id(..)) that already closed
```

Each panic kills a worker thread. Once enough are gone the runtime has nothing
left to poll, and the process presents as a **total hang**: every listener still
accepts TCP, no request is ever answered, and background tasks (in Synap's case
the periodic snapshot) go silent at the same moment. That signature reads like a
lock deadlock and sends you hunting in the wrong place — the give-away is in the
log's span field, where unrelated peers appear nested inside one another.

Synap 1.3.1 had it in all three async protocol paths (`resp3.conn`, `resp3.cmd`,
`rpc.req`). Trigger: connections that subscribe and then vanish, which is what a
finishing CLI or PHP process does. Two seconds of that killed the server.

## The rule

In async code, never `let _g = span.enter();`. Attach the span to the future:

```rust
use tracing::Instrument;

handle_connection(stream, state, idle_timeout)
    .instrument(tracing::info_span!("resp3.conn", peer = %peer))
    .await
```

`enter()` is for synchronous code only, where the guard's scope and the thread's
work genuinely coincide.

## Guardrails

`clippy::await_holding_lock` does **not** catch this — a span guard is not a
lock. Synap pins it with a source-level test
(`crates/synap-server/tests/tracing_span_discipline_tests.rs`) that fails if
`.enter()` reappears under the async protocol paths, plus a soak harness
(`scripts/test/transport-soak.py`) that churns subscribing connections while a
watchdog polls `/health`.
