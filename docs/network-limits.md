# Network resource limits

Hard caps that protect the server from a single client exhausting memory, file
descriptors, or CPU (audit M-006, M-007, M-011, M-015). Current values are
compile-time constants; making them configurable is tracked in `phase6i`.

| Limit | Value | Where | Guards against |
|-------|-------|-------|----------------|
| RESP3 bulk / verbatim length | 512 MiB | `synap-server` `protocol::resp3::parser::MAX_BULK_LEN` | a `$<len>` header claiming gigabytes |
| RESP3 aggregate element count | 1,048,576 | `resp3::parser::MAX_AGGREGATE_LEN` | a `*<count>` header pre-allocating a huge Vec |
| RESP3 protocol line length | 64 KiB | `resp3::parser::MAX_LINE_LEN` | an endless unterminated line |
| SynapRPC frame body | 512 MiB | `synap-server` `protocol::synap_rpc::config::MAX_FRAME_BYTES` (Thunder `Config::max_frame_bytes`) | a 4-byte length prefix claiming ~4 GiB |
| Pub/Sub per-subscriber buffer | 1,024 messages | `synap-core` `pubsub::SUBSCRIBER_CHANNEL_CAPACITY` | a slow subscriber growing memory without bound |
| Concurrent connections (per binary listener) | 10,000 | `synap-server` `resp3::server::MAX_CONNECTIONS` | a connection flood exhausting FDs/memory |

## Behaviour at the limit

- **RESP3 / SynapRPC size caps**: the frame is rejected with a protocol/IO error
  *before* any buffer is allocated for the claimed size.
- **Pub/Sub buffer**: a subscriber whose bounded channel is full is disconnected
  and counted in the `slow_consumers_dropped` stat, rather than buffered forever.
- **Max connections**: once the listener's semaphore has no free permits, new
  connections are refused (dropped) until an existing connection closes. Both
  listeners enforce it — RESP3 with its own semaphore, SynapRPC through
  Thunder's `ListenerConfig::max_connections`. Each refusal on the RPC port
  increments `synap_rpc_connections_refused_total`.

## Idle timeout & configurable limits (phase6i)

The binary listeners (RESP3 and SynapRPC) now enforce a **configurable idle
timeout** and expose the **max-connections** cap via config:

```yaml
network:
  idle_timeout_secs: 300   # close a connection idle this long (0 = disabled)
  max_connections: 10000   # per binary listener; new connections refused beyond
```

- **Idle timeout** bounds slow-loris even within the connection cap: a client
  that connects and sends nothing is closed after `idle_timeout_secs`, freeing
  its connection permit. Implemented by wrapping each per-connection read in a
  `tokio::time::timeout`.
- **max_connections** replaces the previous hard-coded `MAX_CONNECTIONS`; setting
  it lower lets an operator refuse connections sooner.
- Defaults preserve the phase6c behavior (5-minute idle, 10 000 connections).

The parser/codec caps (`MAX_BULK_LEN`, `MAX_AGGREGATE_LEN`, `MAX_LINE_LEN`,
`MAX_FRAME_SIZE`) and the per-subscriber pub/sub buffer remain hard-coded at their
safe defaults — they are security bounds rather than tuning knobs, and live in the
`synap-server` protocol layer and the shared `synap-core` crate.
