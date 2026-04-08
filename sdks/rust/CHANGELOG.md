# Changelog

All notable changes to the Synap Rust SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.10.0] - 2026-04-08

### Added

- **Multi-transport support**: the client now speaks three wire protocols:
  - **SynapRPC** (default) — MessagePack over persistent TCP on `127.0.0.1:15501`
  - **RESP3** — Redis-compatible text protocol on `127.0.0.1:6379`
  - **HTTP** — original REST transport, always used as fallback for unmapped commands
  Switch at config time via `with_synap_rpc_transport()`,
  `with_resp3_transport()`, `with_http_transport()`, and override endpoints
  with `with_rpc_addr(host, port)` / `with_resp3_addr(host, port)`.
- **E2E test suite** (`tests/e2e_test.rs`, `--features e2e`): spawns the
  release binary and exercises all three transports plus cross-transport
  consistency (write via one, read via the others).

### Changed

- `SynapConfig::new(base_url)` now defaults to `TransportMode::SynapRpc`.
  HTTP remains the fallback channel and must always be reachable —
  queues, streams, pub/sub, scripting and transactions still go over REST.

## [0.9.x] Previously under Unreleased

### Added - Sorted Set Support 🎉 (October 25, 2025)

**New Module: sorted_set.rs with 18 operations**

#### Core Operations (15 methods)
- `add()`, `rem()`, `score()`, `card()`, `incr_by()`
- `range()`, `rev_range()`, `rank()`, `rev_rank()`, `count()`
- `range_by_score()`, `pop_min()`, `pop_max()`
- `rem_range_by_rank()`, `rem_range_by_score()`

#### Set Operations
- `inter_store()` (with weights & aggregation), `union_store()`, `diff_store()`

#### Tests
- 6 comprehensive test cases covering all operations

## [0.2.0] - 2025-10-25

### Added - Redis Data Structures 🎉

**Complete Redis-compatible Hash, List, and Set data structures - 45 new commands**

#### Hash Manager (15 commands)
- `hash().set()`, `hash().get()`, `hash().get_all()`, `hash().del()`, `hash().exists()`
- `hash().keys()`, `hash().values()`, `hash().len()`, `hash().mset()`, `hash().mget()`
- `hash().incr_by()`, `hash().incr_by_float()`, `hash().set_nx()`

#### List Manager (16 commands)
- `list().lpush()`, `list().rpush()`, `list().lpop()`, `list().rpop()`, `list().range()`
- `list().len()`, `list().index()`, `list().set()`, `list().trim()`, `list().rem()`
- `list().insert()`, `list().rpoplpush()`, `list().pos()`, `list().lpushx()`, `list().rpushx()`

#### Set Manager (14 commands)
- `set().add()`, `set().rem()`, `set().is_member()`, `set().members()`, `set().card()`
- `set().pop()`, `set().rand_member()`, `set().r#move()`
- `set().inter()`, `set().union()`, `set().diff()`
- `set().inter_store()`, `set().union_store()`, `set().diff_store()`

**Usage Example**:
```rust
use synap_sdk::{SynapClient, SynapConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;

    // Hash operations
    client.hash().set("user:1", "name", "Alice").await?;
    let name: Option<String> = client.hash().get("user:1", "name").await?;

    // List operations
    client.list().rpush("tasks", vec!["task1".into(), "task2".into()]).await?;
    let tasks = client.list().range("tasks", 0, -1).await?;

    // Set operations
    client.set().add("tags", vec!["rust".into(), "redis".into()]).await?;
    let is_member = client.set().is_member("tags", "rust".into()).await?;

    Ok(())
}
```

## [0.1.1] - 2025-10-24

### Fixed
- **CRITICAL**: Fixed pub/sub publish API - now sends `payload` field instead of `data`
- **CRITICAL**: Fixed response field from `delivered_count` to `subscribers_matched`

### Added
- 6 pub/sub integration tests covering API contract
- Tests for different payload types and edge cases

### Changed
- All 34 tests passing
- Ready for crates.io publication

## [0.1.0] - 2025-10-23

### Added

**Core Features:**
- ✅ Complete Key-Value Store API with TTL support
- ✅ Message Queue API with ACK/NACK and priority
- ✅ Event Stream API with reactive consumption
- ✅ Pub/Sub API with wildcard topic support
- ✅ StreamableHTTP protocol implementation

**Reactive Programming:**
- ✅ RxJS-style reactive patterns via `rx` module
- ✅ Observable with operators (map, filter, take, skip, etc.)
- ✅ Subject for multicasting
- ✅ Advanced operators (retry, debounce, buffer, merge)
- ✅ Reactive stream consumption (`observe_events`, `observe_event`)
- ✅ Reactive pub/sub subscription (coming soon)

**Type Safety & Performance:**
- ✅ Full async/await with Tokio
- ✅ Strong type safety leveraging Rust's type system
- ✅ Zero-copy where possible
- ✅ Efficient connection pooling via reqwest

**Developer Experience:**
- ✅ Comprehensive error handling with `thiserror`
- ✅ Complete API documentation with examples
- ✅ 7 working examples covering all features
- ✅ 15 doctests (100% passing)

**Quality Assurance:**
- ✅ 81 tests (100% passing)
- ✅ 91% overall test coverage
- ✅ 96.5% coverage on core API modules
- ✅ Zero clippy warnings
- ✅ Rust Edition 2024

### Examples

- `basic.rs` - Basic KV operations
- `queue.rs` - Traditional queue consumption
- `reactive_queue.rs` - Reactive queue with continuous consumption
- `stream.rs` - Traditional stream consumption
- `reactive_stream.rs` - Reactive stream with observe pattern
- `pubsub.rs` - Pub/Sub messaging
- `rxjs_style.rs` - RxJS-style reactive patterns

### Dependencies

**Core:**
- tokio 1.48 (async runtime)
- reqwest 0.12 (HTTP client)
- serde 1.0 (serialization)
- thiserror 2.0 (error handling)

**Reactive:**
- futures 0.3 (futures utilities)
- async-stream 0.3 (async stream macros)
- pin-project 1.1 (safe pinning)

**Development:**
- tokio-test 0.4 (async testing)
- mockito 1.6 (HTTP mocking)
- tracing-subscriber 0.3 (logging)

### Documentation

- Complete README with API reference
- Reactive programming guide (REACTIVE.md)
- RxJS module documentation (src/rx/README.md)
- Coverage report (COVERAGE_REPORT.md)
- Full API docs via `cargo doc`

### Compatibility

- Rust: Edition 2024, version 1.85+
- Synap Server: v1.0.0+
- Platforms: Linux, macOS, Windows

### Known Limitations

- WebSocket support is limited to basic streaming
- Pub/Sub reactive subscription is not yet implemented
- Transaction support (planned for v0.2.0)

[0.1.0]: https://github.com/hivellm/synap/releases/tag/rust-sdk-v0.1.0

