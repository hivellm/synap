# Project Context - Synap

## Purpose

Synap is a high-performance in-memory data infrastructure system built in Rust that combines the best features of Redis, RabbitMQ, and Kafka into a unified platform for real-time applications.

**Goals**:
- Provide unified data layer for modern applications (KV store, queues, streams, pub/sub)
- Achieve sub-millisecond latency with high throughput (100K+ ops/sec)
- Enable AI integration via MCP and UMICP protocols
- Maintain data durability with WAL + snapshot persistence
- Support master-slave replication for high availability
- Future: Add Redis-compatible data structures (Hashes, Lists, Sets, Sorted Sets)

## Tech Stack

### Core Technologies
- **Language**: Rust Edition 2024 (nightly 1.85+)
- **Runtime**: Tokio (async/await)
- **Web Framework**: Axum (HTTP/WebSocket)
- **Data Structures**: radix_trie (KV), VecDeque (queues), DashMap (concurrent)
- **Serialization**: serde, serde_json, rmp-serde (MessagePack)
- **Compression**: LZ4 (fast) and Zstd (high ratio)

### Persistence
- **WAL**: AsyncWAL + OptimizedWAL (Redis-style batching, 10K ops/batch)
- **Snapshots**: Streaming Snapshot v2 (O(1) memory, CRC32 verified)
- **fsync Modes**: Always, Periodic (10ms batching), Never

### Replication
- **Protocol**: TCP binary with length-prefixed framing
- **Modes**: Full sync (snapshot transfer) + Partial sync (incremental log)
- **Topology**: 1 master + N read replicas
- **Lag Monitoring**: Real-time offset tracking

### Protocols
- **StreamableHTTP**: JSON over HTTP with chunked transfer encoding
- **WebSocket**: Persistent connections for real-time updates
- **MCP (Model Context Protocol)**: AI tool integration (8 tools)
- **UMICP**: Matrix/vector operations for ML workloads

### Testing & Quality
- **Test Framework**: cargo test + cargo nextest (faster execution)
- **Coverage Tool**: cargo llvm-cov (target: 95%+)
- **Linter**: cargo clippy (no warnings allowed)
- **Formatter**: rustfmt with nightly toolchain
- **Benchmarking**: criterion for performance regression testing

### SDKs
- **TypeScript**: Full SDK with reactive patterns
- **Python**: Async/sync client with type hints
- **Rust**: Native SDK with RxJS-style observables

## Project Conventions

### Code Style

**Rust Edition 2024 Rules**:
- Always use nightly toolchain (1.85+)
- Format with `cargo +nightly fmt --all` before commit
- Zero clippy warnings: `cargo clippy --workspace -- -D warnings`
- Prefer explicit types over `impl Trait` for public APIs
- Use `async fn` for async operations (avoid Future types in signatures)

**Naming Conventions**:
- Modules: `snake_case` (e.g., `kv_store`, `event_stream`)
- Types/Structs: `PascalCase` (e.g., `StoredValue`, `QueueMessage`)
- Functions/Methods: `snake_case` (e.g., `get_value`, `publish_event`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `MAX_CONNECTIONS`, `DEFAULT_PORT`)
- Lifetimes: Single letter `'a`, `'b` or descriptive `'static`, `'conn`

**Error Handling**:
- Use `Result<T, SynapError>` for all fallible operations
- Use `thiserror` for custom error types
- Never use `.unwrap()` or `.expect()` in production code
- Log errors before propagating: `error!()` macro from `tracing`

**Async Patterns**:
- Use `tokio::spawn` for fire-and-forget tasks
- Use `tokio::spawn_blocking` for CPU-intensive work
- Use `tokio::select!` for concurrent operations with cancellation
- Always set timeouts on network operations: `tokio::time::timeout`

### Architecture Patterns

**Sharded Storage**:
- 64-way sharding for KV store (shard = CRC32(key) % 64)
- Each shard protected by `Arc<RwLock<T>>`
- Adaptive storage: HashMap (<10K keys) → RadixTrie (≥10K keys)

**Lock Strategies**:
- Read-heavy: `RwLock` for concurrent reads
- Write-heavy: `Mutex` for exclusive access
- Lock-free: `DashMap` for concurrent hash maps
- Lock ordering: Always acquire locks in alphabetical key order (deadlock prevention)

**Memory Management**:
- Compression: LZ4 for hot data (fast), Zstd for cold data (ratio)
- L1 Cache: Decompressed payloads (10% memory)
- L2 Cache: Compressed payloads (20% memory)
- TTL cleanup: Background task runs every 1s (batch delete expired keys)

**Persistence Design**:
- Write path: Operation → WAL → Apply to memory → Response
- fsync batching: Collect writes for 100µs window, fsync once
- Snapshot: Background thread creates snapshot without blocking writes
- Recovery: Load snapshot + replay WAL from last checkpoint

**Replication Protocol**:
- Master: Append to replication log, stream to replicas
- Replica: Receive log entries, apply to local state, ACK offset
- Full sync: Snapshot transfer for new replicas or large lag
- Partial sync: Incremental log replay for small lag (<10K ops)

**Protocol Layering**:
```
Application Layer
    ↓
Protocol Layer (StreamableHTTP/MCP/UMICP)
    ↓
Command Router (route by command type)
    ↓
Core Layer (KV/Queue/Stream/PubSub)
    ↓
Persistence Layer (WAL + Snapshot)
    ↓
Replication Layer (Master/Replica sync)
```

### Testing Strategy

**Test Coverage Requirements**:
- Minimum: 95% line coverage (enforced in CI)
- Unit tests: In same file with `#[cfg(test)]`
- Integration tests: In `/tests` directory
- Benchmark tests: In `/benches` directory

**Test Structure**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_feature() {
        // Arrange
        let input = setup_test_data();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected_value);
    }
    
    #[tokio::test]
    async fn test_async_feature() {
        // Async test using tokio runtime
        let result = async_function().await.unwrap();
        assert!(result.is_ok());
    }
}
```

**Test Categories**:
1. **Unit Tests**: Single function/method, mocked dependencies
2. **Integration Tests**: Multiple components, real database
3. **Concurrency Tests**: 100+ threads, race condition detection
4. **Performance Tests**: Latency/throughput benchmarks vs targets
5. **Persistence Tests**: WAL replay, snapshot recovery
6. **Replication Tests**: Full sync, partial sync, failover

**Test Data**:
- Use `fake` crate for realistic test data generation
- Use `quickcheck` for property-based testing
- Use `criterion` for deterministic benchmarks

### Git Workflow

**Branch Strategy**:
- `main`: Production-ready code (protected)
- `feature/*`: New features (e.g., `feature/add-hashes`)
- `fix/*`: Bug fixes (e.g., `fix/wal-corruption`)
- `refactor/*`: Code improvements (e.g., `refactor/storage-layer`)

**Commit Conventions** (Conventional Commits):
```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code formatting (no logic change)
- `refactor`: Code refactoring
- `perf`: Performance improvement
- `test`: Adding/updating tests
- `build`: Build system changes
- `ci`: CI/CD changes
- `chore`: Maintenance tasks

**Examples**:
```bash
feat(hash): Add HSET, HGET, HDEL commands

- Implement HashMap storage within RadixMap
- Add REST endpoints for hash operations
- Integrate with persistence layer
- Add 20+ unit tests (95% coverage)

Closes #123

fix(replication): Handle replica reconnection edge case

- Add exponential backoff for reconnection attempts
- Improve partial sync detection logic
- Add regression test for scenario

Fixes #456
```

**Pre-Commit Checklist**:
```bash
# 1. Format code
cargo +nightly fmt --all

# 2. Run clippy (no warnings)
cargo clippy --workspace -- -D warnings

# 3. Run tests (all pass)
cargo test --workspace --tests --verbose

# 4. Check coverage (≥95%)
cargo llvm-cov --all --ignore-filename-regex 'examples'

# 5. Build release
cargo build --release
```

**Push Policy**:
- **NEVER** push automatically (SSH password required)
- **ALWAYS** provide push commands for manual execution
- **NEVER** force push to `main` branch

## Domain Context

**Data Structures**:
- **Key-Value Store**: Radix-tree based, TTL support, atomic operations (INCR/DECR)
- **Message Queues**: FIFO with priorities (0-9), ACK/NACK, Dead Letter Queue (DLQ)
- **Event Streams**: Kafka-style partitioned topics, consumer groups, 5 retention policies
- **Pub/Sub**: Topic-based routing with wildcards (`*` and `#`)

**Consistency Model** (PACELC: PC/EL):
- **Partition (P)**: Chooses Consistency (C) - only master accepts writes
- **Else (E)**: Chooses Latency (L) - replicas may lag ~5-10ms

**Performance Characteristics**:
- KV GET: 87ns (12M ops/sec)
- KV SET: 22.5µs (44K ops/sec with persistence)
- Queue Publish: 52µs (19K msgs/sec)
- Queue Consume+ACK: 607µs
- Memory (1M keys): 92MB (54% less than baseline)

**Use Cases**:
1. Real-time chat (event streams with room isolation)
2. Task distribution (queues with ACK/retry)
3. Cache layer (KV with TTL)
4. Event broadcasting (pub/sub with wildcards)
5. Microservices messaging (queues + streams)

## Important Constraints

**Technical Constraints**:
- Rust Edition 2024 (nightly only) - no stable toolchain
- Single-node writes (master-only) - no multi-master yet
- In-memory first (persistence optional) - not a disk-based DB
- Binary compatibility: Not Redis RESP compatible (uses StreamableHTTP/JSON)

**Performance Constraints**:
- Sub-millisecond latency requirement (p99 < 1ms for reads)
- High throughput target (100K+ ops/sec)
- Memory efficiency (< 200MB for 1M keys)
- Replication lag (< 10ms typical, < 100ms under load)

**Security Constraints**:
- Authentication: User+password (bcrypt) or API keys
- Authorization: Role-Based Access Control (RBAC)
- Network: TLS support for encrypted connections
- IP filtering: Whitelist/blacklist per API key

**Operational Constraints**:
- Zero-downtime deployments (rolling updates)
- Backward compatibility (no breaking changes within minor versions)
- Migration tools required for major version upgrades

## External Dependencies

**Core Dependencies** (Always verify latest versions via Context7):
- `tokio` (>=1.40): Async runtime
- `axum` (>=0.7): Web framework
- `serde` (>=1.0): Serialization
- `tracing` (>=0.1): Logging/diagnostics
- `radix_trie`: Memory-efficient key-value storage
- `parking_lot`: Faster RwLock/Mutex than std
- `dashmap`: Concurrent HashMap

**Persistence**:
- `rocksdb` (future): Optional persistent backend
- Native filesystem I/O for WAL and snapshots

**Compression**:
- `lz4_flex`: Fast compression (LZ4)
- `zstd`: High-ratio compression (Zstandard)

**Testing**:
- `criterion`: Benchmarking
- `fake`: Test data generation
- `quickcheck`: Property-based testing

**MCP/UMICP**:
- Internal implementations (no external dependencies)
- JSON-RPC for MCP protocol

**Monitoring** (Planned):
- Prometheus metrics export
- Grafana dashboards
- OpenTelemetry tracing

**Client SDKs**:
- TypeScript: axios, ws (WebSocket)
- Python: httpx, websockets
- Rust: reqwest, tokio-tungstenite

## OpenSpec Usage Notes

**When to Create Proposals**:
- ✅ New data structures (Hashes, Lists, Sets, Sorted Sets)
- ✅ Breaking API changes (new endpoints, changed responses)
- ✅ Architecture changes (clustering, sharding)
- ✅ Performance optimizations that change behavior
- ✅ Security pattern updates (new auth methods)
- ❌ Bug fixes (restore intended behavior)
- ❌ Typos, formatting, comments
- ❌ Dependency updates (non-breaking)

**Capability Naming Examples**:
- `kv-store`: Key-value operations
- `message-queue`: Queue operations
- `event-stream`: Stream operations
- `pubsub-router`: Pub/sub routing
- `persistence-layer`: WAL + snapshots
- `replication-protocol`: Master-slave sync
- `hash-operations`: Hash data structure (future)
- `list-operations`: List data structure (future)

**Change ID Examples**:
- `add-hash-data-structure`
- `update-wal-batching-logic`
- `remove-deprecated-endpoints`
- `refactor-storage-layer`
- `optimize-replication-protocol`

**Validation Workflow**:
```bash
# Before creating proposal
openspec list --specs           # Check existing capabilities
openspec spec list --long       # See all specs

# After creating proposal
openspec validate add-hashes --strict
openspec diff add-hashes        # Review changes

# Before implementation
openspec show add-hashes        # Review full proposal
```

---

**Last Updated**: October 24, 2025  
**Version**: 0.3.0-rc  
**Maintainers**: Core Team
