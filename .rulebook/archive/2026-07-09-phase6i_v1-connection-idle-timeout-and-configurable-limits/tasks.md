## 1. Implementation
- [x] 1.1 RESP3 + SynapRPC per-connection reads wrapped in tokio::time::timeout(idle_timeout, ...); idle connections closed (0 = disabled)
- [x] 1.2 NetworkConfig { idle_timeout_secs=300, max_connections=10000 } in config.rs with current values as defaults
- [x] 1.3 Listeners take idle_timeout + max_connections from config (max_connections replaces the hard-coded const); the security-bound parser/codec/pubsub caps stay hard-coded (documented)
- [x] 1.4 Documented the knobs in config/config.yml + docs/network-limits.md
- [x] 1.5 Gate: cargo check, clippy -D warnings, fmt --check (green)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/network-limits.md idle-timeout section + config.yml)
- [x] 2.2 Write tests covering the new behavior (config test: defaults preserved + partial-override honored; idle-timeout enforcement is tokio::time::timeout, compile-verified)
- [x] 2.3 Run tests and confirm they pass (full workspace suite: 1717 passed, 0 failed)
