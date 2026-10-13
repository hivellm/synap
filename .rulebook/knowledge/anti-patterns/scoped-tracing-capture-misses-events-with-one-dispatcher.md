# Scoped tracing capture misses events while only one dispatcher is registered

**Category**: testing
**Tags**: tracing, testing, logs, parallel-tests, phase19

## Description

A test that captures logs with `tracing::subscriber::with_default` passed alone
and failed (empty capture) when the other tests in the same binary ran in
parallel. Cause, in tracing-core 0.1.36 `callsite.rs`: while at most one
dispatcher is registered, `Dispatchers::rebuilder()` takes the `JustOne` fast
path, which asks only the *registering thread's* default dispatcher. Another
test thread (no subscriber) that reaches the same `info!` first registers the
callsite with interest `never`, and the event is skipped on every thread,
the capturing one included.

Fix: keep a second dispatcher alive while capturing, so `has_just_one` is false
and interest is computed across every registered dispatcher:

```rust
let capture = tracing::Dispatch::new(fmt().with_writer(logs.clone()).finish());
let _second = tracing::Dispatch::new(tracing_subscriber::registry());
tracing::dispatcher::with_default(&capture, || build_the_thing());
```

See `crates/synap-server/tests/request_limits_tests.rs`
(`rate_limit_log_lines`).

## When to Use

Any test that asserts on log output through a scoped (thread-local) subscriber
in a binary whose other tests emit through the same callsites.

## When NOT to Use

A test binary that installs a global subscriber, or runs the capturing test
alone.
