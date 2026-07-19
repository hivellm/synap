# Spec: KV Watch interop, benchmark & documentation

## ADDED Requirements

### Requirement: Cross-SDK envelope interoperability
Every SDK (rust, typescript, python, php, csharp, go) MUST decode the watch envelope
identically: same key, event, version, value bytes, and truncated flag for the same
server-side mutation.

#### Scenario: Set in one SDK observed in all others
Given watchers on key `interop:k` in all six SDKs
When one SDK executes `SET interop:k "v"`
Then every watcher receives an event with key `interop:k`, value `v`, and the same version

### Requirement: Fan-out performance benchmark
The repository SHALL include a benchmark measuring publish-to-push latency and throughput
with N concurrent watchers on an exact key and on a wildcard pattern, including a
stalled-watcher case demonstrating slow-consumer drop.

#### Scenario: Benchmark runs and reports
Given the fan-out benchmark
When it runs against a local server
Then it reports latency/throughput per N and confirms a stalled watcher is dropped without
degrading delivery to healthy watchers

### Requirement: Documented delivery semantics
The documentation MUST state that watch is best-effort (slow consumers are disconnected),
that `version` enables gap detection, that oversized values degrade to notify-only, and
that replay/at-least-once use cases belong to streams.

#### Scenario: Semantics are discoverable
Given the protocol documentation
When a user reads the watch section
Then best-effort delivery, version gap detection, the inline cap, and the streams
alternative are all explicitly described
