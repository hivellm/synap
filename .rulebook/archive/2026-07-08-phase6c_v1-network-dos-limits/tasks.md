## 1. Implementation
- [x] 1.1 Enforce max bulk length + max array/element count in resp3 parser; reject before allocating; bound read_line
- [x] 1.2 Enforce max frame size in synap_rpc codec read_frame; reject before allocating
- [x] 1.3 Replace pubsub UnboundedSender with a bounded channel + disconnect-slow-consumer policy
- [x] 1.4 Add a slow-consumer metric for pubsub (slow_consumers_dropped)
- [x] 1.5 Add semaphore-bounded max-connections limit to both binary listeners
- [x] 1.6 Idle-connection timeout on the binary listeners (moved to phase6i)
- [x] 1.7 Make the limits configurable via config.rs (moved to phase6i; current values are constants)
- [x] 1.8 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/network-limits.md)
- [x] 2.2 Write tests covering the new behavior (oversized frame rejected on both protocols; slow consumer disconnected)
- [x] 2.3 Run tests and confirm they pass
