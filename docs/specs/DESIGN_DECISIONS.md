# Synap Design Decisions

This document explains the technical choices made during Synap's design phase and the rationale behind them.

## Language: Rust (Edition 2024)

### Decision
Use Rust as the primary implementation language with Edition 2024 features.

### Rationale
1. **Memory Safety**: Zero-cost abstractions with compile-time guarantees prevent memory leaks and data races
2. **Performance**: Native code performance matching C/C++ without runtime overhead
3. **Concurrency**: Fearless concurrency with ownership system prevents race conditions
4. **Ecosystem**: Mature async ecosystem (Tokio, serde) proven in production
5. **Edition 2024**: Latest features including improved pattern matching and async traits

### Alternatives Considered
- **Go**: Simpler but GC pauses unacceptable for sub-ms latency requirements
- **C++**: Similar performance but lacks memory safety guarantees
- **Java/JVM**: GC overhead and larger memory footprint

### Trade-offs
- **Learning Curve**: Rust has steeper learning curve than Go
- **Compile Time**: Longer compilation times than dynamic languages
- **Mitigation**: Worth the investment for performance and safety guarantees

## Async Runtime: Tokio

### Decision
Use Tokio as the async runtime instead of alternatives.

### Rationale
1. **Production-Ready**: Battle-tested in production (Discord, AWS, Cloudflare)
2. **Performance**: Excellent multi-threaded work-stealing scheduler
3. **Ecosystem**: Largest async ecosystem in Rust (Axum, Hyper, Tower built on it)
4. **Features**: Complete async I/O, timers, channels, synchronization primitives
5. **Documentation**: Comprehensive docs and examples from Context7

### Alternatives Considered
- **async-std**: Simpler API but smaller ecosystem
- **smol**: Lightweight but lacks features needed for production
- **Custom runtime**: Unnecessary complexity for marginal gains

### Trade-offs
- **Dependency Size**: Tokio is large (~1MB compiled)
- **Mitigation**: Performance and reliability justify the size

## Web Framework: Axum

### Decision
Use Axum for HTTP server and routing.

### Rationale
1. **Modern Design**: Built on Tokio/Tower/Hyper stack
2. **Type Safety**: Compile-time request/response validation
3. **Extractors**: Clean handler syntax with automatic parsing
4. **WebSocket Support**: Built-in WebSocket upgrade path
5. **Performance**: Zero-cost abstractions with minimal overhead

**Example from Context7**:
```rust
use axum::{routing::get, Router};

let app = Router::new()
    .route("/", get(|| async { "Hello, World!" }));
```

### Alternatives Considered
- **actix-web**: Faster but less ergonomic, actor model complexity
- **warp**: Similar features but less active development
- **hyper**: Too low-level, would need custom routing layer

### Trade-offs
- **Compile Time**: Type-heavy code increases compilation time
- **Mitigation**: Developer experience and safety justify the cost

## Data Structure: Radix Trie

### Decision
Use `radix_trie` crate for in-memory key-value storage.

### Rationale
1. **Memory Efficiency**: Shares common prefixes, ~30% less memory than HashMap for string keys
2. **Performance**: O(k) lookup where k = key length (vs O(1) hash with collisions)
3. **Ordered**: Keys naturally sorted, enables range queries
4. **Prefix Search**: Built-in support for key prefix matching
5. **Production Ready**: Mature crate with good maintenance

**Memory Comparison**:
```
HashMap:  "user:1", "user:2", "user:3" → 3 × 7 = 21 bytes
RadixMap: "user:" shared prefix → 5 + (3 × 1) = 8 bytes saved
```

### Alternatives Considered
- **DashMap**: Better for concurrent writes but higher memory overhead
- **HashMap**: Simpler but no memory sharing or ordering
- **BTreeMap**: Good for ranges but slower than radix for strings
- **Custom Trie**: Too much work for marginal gains

### Trade-offs
- **Write Performance**: Slightly slower than HashMap on writes
- **Complexity**: More complex than simple HashMap
- **Mitigation**: Memory savings and prefix search worth the trade-off

## Protocol: Multi-Protocol Support

### Decision
Support multiple protocols: StreamableHTTP, MCP, and UMICP for maximum compatibility.

### Rationale
1. **Flexibility**: Different use cases benefit from different protocols
2. **Integration**: MCP enables AI tool integration
3. **Performance**: UMICP optimized for matrix/vector operations
4. **Universal**: StreamableHTTP for general-purpose usage
5. **Ecosystem**: Leverage existing protocol ecosystems

### Protocol Details

#### StreamableHTTP (Primary)
- **Purpose**: General-purpose operations
- **Benefits**: Universal HTTP compatibility, easy debugging
- **Use Cases**: REST API, key-value operations, queues

#### MCP (Model Context Protocol)
- **Purpose**: AI tool and agent integration
- **Benefits**: Standardized AI tool interface, resource discovery
- **Use Cases**: AI context storage, agent coordination, LLM integration

#### UMICP (Universal Matrix Inter-Communication Protocol)
- **Purpose**: High-performance matrix and vector operations
- **Benefits**: Efficient binary protocol, federated communication
- **Use Cases**: ML embeddings, similarity search, distributed computing

**Message Envelope**:
```json
{
  "type": "request",
  "command": "kv.get",
  "request_id": "uuid",
  "payload": {...}
}
```

### Alternatives Considered
- **gRPC**: Binary and efficient but harder to debug, less universal
- **WebSocket-only**: Requires persistent connections, harder to scale
- **Custom TCP**: No standard tooling, reinventing the wheel
- **Redis Protocol (RESP)**: Limited to Redis semantics

### Trade-offs
- **Overhead**: HTTP headers add slight overhead vs raw TCP
- **Verbosity**: JSON more verbose than binary protocols
- **Mitigation**: Clarity and compatibility more important than raw speed

## Replication: Master-Slave

### Decision
Implement simple master-slave replication with one write master and N read replicas.

### Rationale
1. **Simplicity**: Easier to implement and reason about than multi-master
2. **Consistency**: Clear source of truth (master) avoids conflicts
3. **Read Scaling**: Linear scaling of read capacity with replicas
4. **Proven Pattern**: Used successfully by Redis, MySQL, PostgreSQL
5. **Sufficient**: Meets requirements for MVP without over-engineering

### Alternatives Considered
- **Raft Consensus**: More robust but significantly more complex
- **Multi-Master**: Complex conflict resolution not needed for V1
- **Leaderless (Dynamo)**: Overkill for target use cases
- **No Replication**: Unacceptable for production deployments

### Trade-offs
- **Write Scaling**: Limited to single master node
- **Manual Failover**: Requires operator intervention
- **Mitigation**: Document clear promotion procedures, plan Raft for V2

### PACELC Classification: PC/EL

Synap adopts a **PC/EL** model, meaning:

**P (Partition Tolerance)**:
- During network partition, **Consistency (C)** is prioritized
- Only master accepts writes, preventing split-brain
- Replicas become read-only until partition heals
- **Result**: No conflicting writes, single source of truth maintained

**E (Else - Normal Operation)**:
- Without partition, **Latency (L)** is prioritized over strong consistency
- Reads from replicas use eventually consistent data
- Typical replication lag: < 10ms
- **Result**: Fast reads at cost of slight staleness

**Why PC/EL**:
1. **Write Consistency**: Critical for correctness (no conflicts)
2. **Read Performance**: Most workloads are read-heavy and tolerate staleness
3. **Practical**: Matches Redis model, familiar to developers
4. **Tunable**: Can read from master for strong consistency when needed

**Comparison**:
- **PA/EL (Cassandra)**: High availability but eventual consistency
- **PC/EC (MongoDB)**: Strong consistency but higher latency
- **PC/EL (Redis, Synap)**: Balanced for read-heavy workloads

## Message Queue: In-Memory with ACK

### Decision
Implement in-memory FIFO queues with message acknowledgment.

### Rationale
1. **Performance**: In-memory is 100x faster than disk-backed queues
2. **Simplicity**: VecDeque provides efficient FIFO operations
3. **Reliability**: ACK/NACK ensures message delivery guarantees
4. **RabbitMQ Pattern**: Familiar model for developers
5. **Replication**: Master-slave replication provides durability

### Alternatives Considered
- **Disk-backed**: Too slow for target latency requirements
- **No ACK**: Unacceptable for reliable messaging
- **Kafka Log**: More complex than needed for queue semantics

### Trade-offs
- **Durability**: Messages lost if master crashes before replication
- **Capacity**: Limited by available RAM
- **Mitigation**: Replication provides durability, dead letter queue for failures

## Event Stream: Room-based Ring Buffer

### Decision
Use room-based isolation with ring buffers for event history.

### Rationale
1. **Isolation**: Each room is independent, no cross-contamination
2. **Bounded Memory**: Ring buffer prevents unbounded growth
3. **History**: Clients can replay recent events
4. **Kafka Pattern**: Similar to Kafka topics but simpler
5. **Performance**: Ring buffer is O(1) for append and read

**Structure**:
```rust
struct Room {
    events: RingBuffer<Event>,  // Bounded circular buffer
    subscribers: HashSet<SubscriberId>,
    retention: Duration,
}
```

### Alternatives Considered
- **Unbounded Queue**: Memory leak risk
- **Disk-backed Log**: Too slow for real-time broadcasting
- **No History**: Limits use cases (new subscribers miss events)

### Trade-offs
- **History Limits**: Only recent events available
- **Memory per Room**: Fixed overhead per active room
- **Mitigation**: Configurable retention, room auto-cleanup

## Pub/Sub: Topic-based Routing

### Decision
Use hierarchical topics with wildcard subscriptions.

### Rationale
1. **Flexibility**: Hierarchical topics enable fine-grained routing
2. **Wildcards**: `*` and `#` allow flexible subscription patterns
3. **Fan-out**: Efficient message distribution to multiple subscribers
4. **Standard Pattern**: Similar to MQTT, RabbitMQ exchanges
5. **Radix Tree**: Efficient topic storage and matching

**Topic Examples**:
```
notifications.email.user      → Specific topic
notifications.email.*         → All email notifications
notifications.#               → All notifications
```

### Alternatives Considered
- **Flat Topics**: Less flexible than hierarchical
- **No Wildcards**: Forces exact topic matching
- **Channel-based**: Less organized than topic hierarchy

### Trade-offs
- **Complexity**: Wildcard matching adds CPU overhead
- **Memory**: Storing subscription patterns
- **Mitigation**: Radix tree makes pattern matching efficient

## Serialization: JSON + MessagePack

### Decision
Use JSON for protocol, MessagePack for binary efficiency.

### Rationale
1. **JSON**: Human-readable, debuggable, universal support
2. **MessagePack**: Binary efficiency when needed (30-50% smaller)
3. **serde**: Single API for both formats
4. **Flexibility**: Clients choose format via Content-Type header
5. **Compatibility**: JSON ensures broad compatibility

### Alternatives Considered
- **Protocol Buffers**: Requires schema compilation
- **CBOR**: Less adoption than MessagePack
- **JSON-only**: Wastes bandwidth for large messages

### Trade-offs
- **Two Formats**: Need to support both
- **Size**: JSON larger than binary formats
- **Mitigation**: Default to JSON, MessagePack opt-in for performance

## Connection Model: HTTP + WebSocket Hybrid

### Decision
Support both stateless HTTP and stateful WebSocket connections.

### Rationale
1. **HTTP**: Simple request/response for KV and queue operations
2. **WebSocket**: Persistent connections for streams and pub/sub
3. **Flexibility**: Clients choose based on use case
4. **Upgrade Path**: HTTP can upgrade to WebSocket seamlessly
5. **Load Balancing**: HTTP easier to load balance than WebSocket

**Usage Pattern**:
- KV operations → HTTP (stateless)
- Queue consume → HTTP long-polling or WebSocket
- Event streams → WebSocket (push model)
- Pub/Sub → WebSocket (continuous subscriptions)

### Alternatives Considered
- **HTTP-only**: No efficient push mechanism
- **WebSocket-only**: Complex connection management for simple operations
- **TCP-only**: Harder to debug and work with

### Trade-offs
- **Complexity**: Supporting two connection types
- **State Management**: WebSocket connections require state tracking
- **Mitigation**: Clear separation of stateless vs stateful operations

## Persistence: Optional WAL + Snapshots

### Decision
Implement optional persistence using Write-Ahead Log (WAL) and periodic snapshots.

### Rationale
1. **Flexibility**: Operators choose performance vs durability trade-off
2. **Fast Recovery**: Snapshots enable quick restart (1-10 seconds)
3. **Durability**: WAL ensures no data loss on crash
4. **Performance**: Configurable fsync modes balance speed and safety
5. **Battle-Tested**: Pattern proven by Redis (AOF+RDB), PostgreSQL (WAL)

**Persistence Modes**:
- **None**: Pure in-memory (max performance, no durability)
- **WAL (periodic fsync)**: Good balance (~2ms latency, minimal loss)
- **WAL (always fsync)**: Maximum durability (~10ms latency, zero loss)
- **WAL + Snapshots**: Recommended (fast recovery + durability)

### Alternatives Considered
- **No Persistence**: Unacceptable for production data
- **RocksDB**: External dependency, LSM-tree overhead
- **Memory-Mapped Files**: Complex concurrency, corruption risks
- **Event Sourcing**: Overkill, replay too slow

### Trade-offs
- **Performance Impact**: 
  - In-memory: 0.5ms write latency
  - WAL (periodic): 1-2ms write latency (200K ops/sec)
  - WAL (always): 10-20ms write latency (10K ops/sec)
- **Disk Space**: WAL grows until snapshot, typically < 1GB
- **Recovery Time**: 1-10 seconds with snapshots, minutes with WAL-only
- **Mitigation**: Configurable modes allow optimization per use case

### Implementation Strategy

**Write Path**:
```
1. Append to WAL (async buffered write)
2. Execute in memory
3. Return to client
4. Periodic fsync (default: 1 second)
5. Replicate to slaves (async)
```

**Recovery Path**:
```
1. Load latest snapshot (if exists)
2. Replay WAL from snapshot offset
3. Resume normal operation
Total time: 1-10 seconds typical
```

### Configuration Flexibility

```yaml
# Maximum performance (cache use case)
persistence:
  enabled: false

# Balanced (production)
persistence:
  enabled: true
  wal:
    fsync_mode: "periodic"
    fsync_interval_ms: 1000
  snapshot:
    interval_secs: 300

# Maximum durability (critical data)
persistence:
  enabled: true
  wal:
    fsync_mode: "always"
```

## Error Handling Strategy

### Decision
Use Result<T, SynapError> throughout with typed errors.

### Rationale
1. **Explicit**: Errors must be handled, no silent failures
2. **Type Safety**: Compiler enforces error handling
3. **Context**: Error types carry context information
4. **Idiomatic**: Standard Rust pattern with `?` operator
5. **Client Friendly**: Maps cleanly to SDK exceptions

**Error Types**:
```rust
enum SynapError {
    KeyNotFound(String),
    QueueFull(String),
    InvalidMessage(String),
    ReplicationError(String),
    ProtocolError(String),
}
```

### Alternatives Considered
- **Panic on Error**: Unacceptable for production server
- **Error Codes**: Less type-safe than Result
- **Exceptions**: Rust doesn't have exceptions

### Trade-offs
- **Verbosity**: Need `?` or `match` for error handling
- **Propagation**: Errors must be explicitly converted
- **Mitigation**: Standard Rust patterns make this natural

## Logging & Monitoring

### Decision
Use `tracing` for structured logging and `prometheus` for metrics.

### Rationale
1. **Structured Logging**: Better than tracing::info! for production
2. **Filtering**: Runtime log level control
3. **Context**: Span-based context propagation
4. **Metrics**: Industry-standard Prometheus format
5. **Tooling**: Works with standard observability tools

### Alternatives Considered
- **env_logger**: Less features than tracing
- **Custom metrics**: Reinventing the wheel
- **No logging**: Unacceptable for production

### Trade-offs
- **Overhead**: Slight performance cost for logging
- **Mitigation**: Log levels allow disabling in production

## Testing Strategy

### Decision
Comprehensive unit, integration, and benchmark tests.

### Test Types
1. **Unit Tests**: Per-module functionality (inline in modules)
2. **Integration Tests**: Full system workflows (`tests/` directory)
3. **Benchmarks**: Performance validation (`benches/` directory)
4. **Examples**: Working samples also serve as tests

### Rationale
- **Reliability**: Catch bugs before production
- **Regression**: Prevent breaking existing functionality
- **Performance**: Validate latency and throughput targets
- **Documentation**: Examples demonstrate proper usage

## Compression Strategy: LZ4 + Zstd

### Decision
Implement dual compression strategy: LZ4 for real-time operations, Zstd for storage.

### Rationale
1. **LZ4 Speed**: Sub-millisecond decompression critical for real-time operations
2. **Minimal CPU**: 3-5% CPU overhead acceptable for 2-3x space savings
3. **Zstd Efficiency**: Better compression for persistence (3-5x) with acceptable speed
4. **Proven**: Both used in production by major systems (Kafka, RocksDB, Cassandra)
5. **Adaptive**: Different algorithms for different use cases

### Alternatives Considered
- **No Compression**: Unacceptable memory usage for large datasets
- **Gzip/Deflate**: Too slow for real-time (10x slower than LZ4)
- **Snappy**: Similar to LZ4 but slightly slower decompression
- **Brotli**: Excellent ratio but too slow for real-time
- **LZ4 Only**: Good but Zstd better for storage layer

### Trade-offs
- **CPU Overhead**: 3-5% CPU vs no compression
- **Complexity**: Two algorithms vs one
- **Mitigation**: CPU overhead minimal, space savings worth it

**Benchmark Comparison**:
```
Algorithm | Compress | Decompress | Ratio | Use Case
----------|----------|------------|-------|----------
LZ4       | 500MB/s  | 2000MB/s   | 2.3x  | Real-time
Zstd (3)  | 400MB/s  | 1000MB/s   | 3.5x  | Storage
Snappy    | 550MB/s  | 1800MB/s   | 2.0x  | Alternative
Gzip (6)  | 20MB/s   | 200MB/s    | 3.5x  | Too slow
```

## Hot Data Cache System

### Decision
Implement L1/L2 tiered cache with decompressed hot data and compressed warm data.

### Rationale
1. **CPU Efficiency**: Eliminate decompression for frequently accessed data
2. **80/20 Rule**: 20% of data accounts for 80% of requests (Pareto principle)
3. **Memory Trade-off**: Use 30% memory for cache to save 80% CPU on hot paths
4. **Adaptive TTL**: Auto-adjust based on access patterns
5. **Proven Pattern**: Similar to CPU cache hierarchy (L1/L2/L3)

**Performance Impact**:
```
Scenario          | Without Cache | With Cache | Improvement
------------------|---------------|------------|------------
Hot Read (L1 hit) | 0.5ms        | 0.08ms     | 6.2x faster
Warm Read (L2)    | 0.5ms        | 0.3ms      | 1.7x faster
CPU Usage         | 100%         | 20%        | 80% reduction
```

### Architecture
```
┌─────────────────────────────────────────┐
│  L1 Cache (10% memory)                  │
│  - Decompressed payloads                │
│  - 2-10s TTL                            │
│  - LRU eviction                         │
│  - Target: >80% hit rate                │
└─────────────────────────────────────────┘
                  │ miss
┌─────────────────────────────────────────┐
│  L2 Cache (20% memory)                  │
│  - Compressed payloads                  │
│  - 30-60s TTL                           │
│  - FIFO eviction                        │
│  - Target: >90% cumulative hit rate     │
└─────────────────────────────────────────┘
                  │ miss
┌─────────────────────────────────────────┐
│  Primary Storage (70% memory)           │
│  - Radix tree (compressed)              │
│  - 100% hit rate (disk fallback)        │
└─────────────────────────────────────────┘
```

### Alternatives Considered
- **Single-tier Cache**: Simpler but less efficient
- **No Decompression Cache**: CPU overhead remains
- **Redis-style Single Pool**: Less flexible than tiered approach
- **Memcached Integration**: External dependency, added complexity

### Trade-offs
- **Memory Usage**: 30% memory for cache vs 100% storage
- **Complexity**: Tiered cache vs simple cache
- **Mitigation**: Configurable cache sizes, auto-tuning

### Cache Promotion Strategy

**Access Pattern → Cache Level**:
```
< 2 accesses in 60s    → No cache (primary storage)
2-3 accesses in 30s    → L2 cache (compressed)
> 3 accesses in 10s    → L1 cache (decompressed)
No access for 10s      → Evict from L1 → L2
No access for 60s      → Evict from L2
```

**Adaptive Behavior**:
- Monitor access patterns every second
- Adjust cache sizes dynamically (5-20% L1, 10-30% L2)
- Promote/demote based on access frequency
- Track CPU savings vs memory cost

## Documentation-First Approach

### Decision
Write complete technical documentation before implementation.

### Rationale
1. **Design Validation**: Catch design issues early
2. **Team Alignment**: Clear specifications for all contributors
3. **API Stability**: Finalize API before writing code
4. **Better Architecture**: Thinking through design before coding
5. **User Experience**: Documentation drives API design

### Benefits
- Clear scope and requirements
- Fewer breaking changes during implementation
- Better API design with user perspective
- Easier onboarding for new contributors

## Summary of Key Decisions

| Decision | Choice | Main Reason |
|----------|--------|-------------|
| **Language** | Rust 2024 | Performance + Safety |
| **Runtime** | Tokio | Production-ready async |
| **Framework** | Axum | Modern, type-safe HTTP |
| **KV Store** | radix_trie | Memory efficiency |
| **Protocols** | HTTP + MCP + UMICP | Universal + AI + Performance |
| **Replication** | Master-Slave | Simple, proven pattern |
| **PACELC Model** | PC/EL | Consistency + Low Latency |
| **Persistence** | Optional WAL+Snapshot | Flexibility + Fast recovery |
| **Serialization** | JSON + MessagePack | Flexibility + Efficiency |
| **Connections** | HTTP + WebSocket | Stateless + Stateful hybrid |
| **Compression** | LZ4 + Zstd | Speed + Ratio balance |
| **Cache Strategy** | L1/L2 Tiered | Hot data optimization |
| **Error Handling** | Result types | Type safety |
| **Logging** | tracing | Structured, production-ready |
| **Testing** | Comprehensive | Quality assurance |

## Lessons from Existing Projects

### From Redis
- **Adopted**: Simple protocol, in-memory speed, replication
- **Avoided**: Custom protocol (RESP), single-threaded design

### From RabbitMQ
- **Adopted**: Message acknowledgment, queue patterns
- **Avoided**: AMQP complexity, Erlang ecosystem

### From Kafka
- **Adopted**: Event streaming, offset-based consumption
- **Avoided**: Disk-first design, partition complexity

### From HiveLLM Ecosystem
- **Adopted**: StreamableHTTP protocol design, SDK structure
- **Reused**: UMICP envelope pattern, client SDK patterns
- **Improved**: Simplified message format, clearer routing
- **Integrated**: MCP for AI tool compatibility, UMICP for matrix operations

## Non-Functional Requirements

### Performance Requirements
- **Latency**: < 1ms for 95th percentile operations
- **Throughput**: 100K ops/sec minimum per core
- **Memory**: Efficient usage with radix tree
- **CPU**: Multi-core utilization via Tokio

### Reliability Requirements
- **Uptime**: 99.9% availability target
- **Replication**: < 10ms lag to replicas
- **Durability**: No data loss with >= 2 nodes
- **Recovery**: < 5s replica sync on restart

### Scalability Requirements
- **Connections**: 100K concurrent clients
- **Keys**: 100M+ keys per node
- **Queues**: 10K concurrent queues
- **Rooms**: 100K concurrent event rooms
- **Topics**: Unlimited pub/sub topics

### Security Requirements
- **Authentication**: API key mandatory
- **Encryption**: TLS for production
- **Authorization**: Role-based access control
- **Rate Limiting**: Per-client throttling

## Development Principles

### Code Quality
1. **No unsafe code** without thorough justification
2. **Comprehensive tests** (>80% coverage target)
3. **Documentation** for all public APIs
4. **Benchmarks** for performance-critical paths
5. **Clippy clean** with pedantic lints

### API Design
1. **Consistency**: Similar patterns across all components
2. **Simplicity**: Minimize required parameters
3. **Explicit**: Clear error messages and types
4. **Backwards Compatible**: Versioned protocol
5. **SDK-First**: Design with client SDKs in mind

### Performance
1. **Measure**: Benchmark before optimizing
2. **Profile**: Use flamegraph for bottlenecks
3. **Lock-free**: Avoid locks where possible
4. **Batching**: Support batch operations
5. **Zero-copy**: Minimize data copying

## Future-Proofing

### Extensibility Points
- **Command System**: Pluggable command handlers
- **Storage Backend**: Abstract trait for future persistence
- **Protocol**: Versioned for backward compatibility
- **Replication**: Designed for upgrade to Raft

### Planned Evolution
1. **V1**: In-memory, master-slave, StreamableHTTP
2. **V2**: Optional persistence (RocksDB), improved replication
3. **V3**: Clustering, sharding, Raft consensus
4. **V4**: Geo-replication, advanced features

## References

### External Resources
- [Tokio Documentation](https://tokio.rs) - Async runtime guide
- [Axum Examples](https://github.com/tokio-rs/axum/tree/main/examples) - Web framework patterns
- [Redis Architecture](https://redis.io/docs/latest/operate/oss_and_stack/management/replication/) - Replication patterns
- [radix_trie Crate](https://docs.rs/radix_trie/) - Data structure documentation

### Internal Documents
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture
- [PERFORMANCE.md](PERFORMANCE.md) - Performance targets and benchmarks
- [DEVELOPMENT.md](DEVELOPMENT.md) - Development setup

