# Changelog

All notable changes to the Synap Rust SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

