# Network resource limits

Hard caps that protect the server from a single client exhausting memory, file
descriptors, or CPU (audit M-006, M-007, M-011, M-015). Current values are
compile-time constants; making them configurable is tracked in `phase6i`.

| Limit | Value | Where | Guards against |
|-------|-------|-------|----------------|
| RESP3 bulk / verbatim length | 512 MiB | `synap-protocol` `resp3::parser::MAX_BULK_LEN` | a `$<len>` header claiming gigabytes |
| RESP3 aggregate element count | 1,048,576 | `resp3::parser::MAX_AGGREGATE_LEN` | a `*<count>` header pre-allocating a huge Vec |
| RESP3 protocol line length | 64 KiB | `resp3::parser::MAX_LINE_LEN` | an endless unterminated line |
| SynapRPC frame body | 512 MiB | `synap-protocol` `synap_rpc::codec::MAX_FRAME_SIZE` | a 4-byte length prefix claiming ~4 GiB |
| Pub/Sub per-subscriber buffer | 1,024 messages | `synap-core` `pubsub::SUBSCRIBER_CHANNEL_CAPACITY` | a slow subscriber growing memory without bound |
| Concurrent connections (per binary listener) | 10,000 | `synap-server` `resp3::server::MAX_CONNECTIONS` | a connection flood exhausting FDs/memory |

## Behaviour at the limit

- **RESP3 / SynapRPC size caps**: the frame is rejected with a protocol/IO error
  *before* any buffer is allocated for the claimed size.
- **Pub/Sub buffer**: a subscriber whose bounded channel is full is disconnected
  and counted in the `slow_consumers_dropped` stat, rather than buffered forever.
- **Max connections**: once the listener's semaphore has no free permits, new
  connections are refused (dropped) until an existing connection closes.

## Follow-ups (phase6i)

- Idle-connection timeout (close connections that send nothing, bounding
  slow-loris even within the connection cap).
- Surface all of the above in `config.rs` with the defaults shown here so
  operators can tune them per deployment.
