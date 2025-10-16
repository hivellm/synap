# Synap Architecture

## System Overview

Synap is designed as a modular, high-performance in-memory data system combining multiple messaging and storage patterns in a single cohesive platform.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Client Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ TypeScript   │  │   Python     │  │     Rust     │          │
│  │     SDK      │  │     SDK      │  │     SDK      │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  MCP Clients │  │   AI Tools   │  │ UMICP Clients│          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────────┘
                              │
        StreamableHTTP / WebSocket / MCP / UMICP
                              │
┌─────────────────────────────────────────────────────────────────┐
│                      Protocol Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │StreamableHTTP│  │     MCP      │  │    UMICP     │          │
│  │   Handler    │  │   Handler    │  │   Handler    │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│  │  - Message Envelope Parsing                          │       │
│  │  - Command Routing                                   │       │
│  │  - Response Streaming                                │       │
│  │  - Protocol Negotiation                              │       │
│  └──────────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                     Command Router                              │
│  Routes requests to appropriate subsystem                       │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌──────────────┐      ┌──────────────┐     ┌──────────────┐
│  Key-Value   │      │    Queue     │     │    Event     │
│    Store     │      │    System    │     │   Stream     │
│              │      │              │     │              │
│ Radix Tree   │      │  FIFO Queue  │     │ Room-based   │
│  (in-mem)    │      │   with ACK   │     │  Broadcast   │
└──────────────┘      └──────────────┘     └──────────────┘
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              │
                    ┌─────────────────┐
                    │    Pub/Sub      │
                    │     Router      │
                    │  Topic-based    │
                    └─────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                   Replication Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Master     │→→│   Replica 1  │  │   Replica 2  │          │
│  │  (Write)     │  │   (Read)     │  │   (Read)     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│         │                                                        │
│         └──────── Append-Only Log ─────────────────────→        │
└─────────────────────────────────────────────────────────────────┘
```

## Component Architecture

### 1. Key-Value Store

**Technology**: radix_trie crate  
**Storage**: In-memory with optional persistence  
**Features**:
- O(k) lookup time where k = key length
- TTL support for automatic expiration
- Atomic operations (GET, SET, DEL, INCR, DECR)
- Batch operations for efficiency

**Structure**:
```rust
RadixMap<String, StoredValue>

struct StoredValue {
    data: Vec<u8>,          // Serialized value
    ttl: Option<Instant>,   // Expiration time
    metadata: HashMap<String, String>,
    created_at: Instant,
    accessed_at: Instant,
}
```

### 2. Queue System

**Pattern**: FIFO with acknowledgment  
**Durability**: In-memory with replication  
**Features**:
- Message priorities (0-9)
- Delivery acknowledgment (ACK/NACK)
- Automatic retry with backoff
- Dead letter queue for failed messages
- Prefetch control for consumers

**Structure**:
```rust
struct Queue {
    name: String,
    messages: VecDeque<QueueMessage>,
    pending: HashMap<MessageId, PendingMessage>,
    consumers: Vec<ConsumerId>,
    config: QueueConfig,
}

struct QueueMessage {
    id: MessageId,
    payload: Vec<u8>,
    priority: u8,
    retry_count: u32,
    created_at: Instant,
}
```

### 3. Event Stream

**Pattern**: Room-based broadcasting  
**Storage**: Ring buffer with configurable retention  
**Features**:
- Room isolation (chat-room-1, chat-room-2, etc.)
- Message history and replay
- Offset-based consumption
- Batch publishing
- Stream compaction

**Structure**:
```rust
struct EventStream {
    rooms: HashMap<RoomId, Room>,
}

struct Room {
    id: RoomId,
    events: RingBuffer<Event>,
    subscribers: HashSet<SubscriberId>,
    retention: Duration,
    max_events: usize,
}

struct Event {
    id: EventId,
    offset: u64,
    room: RoomId,
    event_type: String,
    data: Vec<u8>,
    timestamp: Instant,
}
```

### 4. Pub/Sub Router

**Pattern**: Topic-based routing  
**Features**:
- Hierarchical topics (notifications.email.user)
- Wildcard subscriptions (* and #)
- Fan-out messaging
- Subscription filtering

**Structure**:
```rust
struct PubSubRouter {
    topics: RadixMap<String, TopicSubscribers>,
    wildcard_subs: Vec<WildcardSubscription>,
}

struct TopicSubscribers {
    topic: String,
    subscribers: HashSet<SubscriberId>,
}
```

## Data Flow

### Write Path (Master Node)

```
Client Request
    │
    ▼
Protocol Layer (HTTP/WS)
    │
    ▼
Command Router
    │
    ▼
Core Component (KV/Queue/Stream/PubSub)
    │
    ├──→ Execute Operation
    │
    └──→ Append to Replication Log
         │
         ├──→ Stream to Replica 1
         ├──→ Stream to Replica 2
         └──→ Stream to Replica N
```

### Read Path (Replica Nodes)

```
Client Read Request
    │
    ▼
Protocol Layer
    │
    ▼
Command Router
    │
    ▼
Local Component (Read-Only)
    │
    ▼
Return Response
```

## Threading Model

Synap uses Tokio's multi-threaded runtime with the following architecture:

```
┌────────────────────────────────────────────────┐
│           Tokio Runtime (N threads)            │
├────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│  │ Protocol │  │  Core    │  │  Repli-  │     │
│  │ Handlers │  │  Logic   │  │  cation  │     │
│  │ (Tasks)  │  │ (Tasks)  │  │ (Tasks)  │     │
│  └──────────┘  └──────────┘  └──────────┘     │
└────────────────────────────────────────────────┘
```

**Design Principles**:
- Non-blocking I/O for all network operations
- Lock-free data structures where possible
- Arc + RwLock for shared state
- Channel-based message passing between components

## Memory Management

### Key-Value Store
- **Data Structure**: Radix tree (memory-efficient prefix sharing)
- **Eviction**: LRU for TTL-expired keys
- **Capacity**: Configurable max memory with eviction policies
- **Compression**: LZ4/Zstd for payload compression
- **Hot Cache**: L1/L2 cache for frequently accessed data

### Queue System
- **Buffer**: Bounded channels with backpressure
- **Pending Messages**: HashMap for tracking unacknowledged messages
- **Dead Letters**: Separate queue for failed messages
- **Compression**: Compressed message payloads

### Event Streams
- **Ring Buffer**: Fixed-size circular buffer per room
- **Retention**: Time-based (hours/days) or count-based
- **Compaction**: Remove old events automatically
- **Compression**: Compressed event data

### Compression & Cache System
- **L1 Cache (Hot)**: Decompressed payloads for hot data (10% memory)
- **L2 Cache (Warm)**: Compressed payloads for warm data (20% memory)
- **Algorithms**: LZ4 (speed), Zstd (ratio)
- **Adaptive**: Auto-adjust based on access patterns

## Replication Architecture

### Master Node
- **Role**: Accepts all write operations
- **Log**: Maintains append-only replication log
- **Streaming**: Pushes log entries to replicas in real-time

### Replica Nodes
- **Role**: Read-only operations
- **Sync**: Continuously receives log stream from master
- **Lag**: Tracks replication lag metrics
- **Promotion**: Can be manually promoted to master

### Replication Protocol
```
Master                          Replica
  │                                │
  │─────── Connect Request ───────→│
  │←────── Sync Response ──────────│
  │                                │
  │─────── Log Entry 1 ────────────→│
  │─────── Log Entry 2 ────────────→│
  │─────── Log Entry 3 ────────────→│
  │←────── ACK (offset) ───────────│
```

### Consistency Model
- **Write**: Synchronous on master
- **Replication**: Asynchronous to replicas
- **Read**: Eventually consistent from replicas
- **Guarantee**: All replicas will eventually reflect master state

### PACELC Classification: PC/EL

Synap follows the **PC/EL** model:

**During Network Partition (P)**:
- **Chooses Consistency (C)**: Only master accepts writes
- Replicas become read-only during partition
- Prevents split-brain and conflicting writes
- Guarantees strong consistency on master

**Else (Normal Operation, E)**:
- **Chooses Latency (L)**: Reads from replicas may be slightly stale
- Eventual consistency with < 10ms lag typical
- Read operations optimized for low latency
- Acceptable for most read-heavy workloads

**Trade-offs**:
- ✅ **Consistency**: No conflicting writes, single source of truth
- ✅ **Low Latency**: Fast reads from replicas
- ❌ **Write Availability**: Master is single point of failure for writes
- ⚠️ **Read Staleness**: Replicas may lag by milliseconds

**Comparison with Other Systems**:
- **Redis**: Similar PC/EL model with master-slave
- **Cassandra**: PA/EL (chooses availability during partition)
- **MongoDB**: PC/EC (configurable, can choose consistency)
- **DynamoDB**: PA/EL (eventual consistency, high availability)

## Protocol Layer

### StreamableHTTP (Primary)

**Base**: HTTP/1.1 or HTTP/2  
**Encoding**: Chunked transfer for streaming  
**Format**: JSON message envelopes

**Message Structure**:
```json
{
  "type": "request",
  "command": "kv.set",
  "request_id": "req-12345",
  "payload": {
    "key": "user:1",
    "value": "data"
  }
}
```

**Use Cases**: REST API, general operations, debugging

### MCP (Model Context Protocol)

**Base**: JSON-RPC over stdio/HTTP/WebSocket  
**Format**: MCP standard messages  
**Features**: Resource discovery, tool execution, prompts

**Message Structure**:
```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "synap_kv_get",
    "arguments": {
      "key": "user:1"
    }
  },
  "id": 1
}
```

**Use Cases**: AI tool integration, context storage, agent coordination

### UMICP (Universal Matrix Inter-Communication Protocol)

**Base**: WebSocket or HTTP/2  
**Format**: UMICP envelopes with matrix/vector data  
**Features**: Matrix operations, vector processing, federated communication

**Message Structure**:
```json
{
  "type": "request",
  "operation": "data",
  "from": "client-001",
  "to": "synap-server",
  "message_id": "msg-12345",
  "capabilities": {
    "operation": "vector.dot",
    "vector1": [1.0, 2.0, 3.0],
    "vector2": [4.0, 5.0, 6.0]
  }
}
```

**Use Cases**: ML embeddings, similarity search, matrix operations

### WebSocket

**Usage**: Long-lived connections for streams and pub/sub  
**Upgrade**: HTTP → WebSocket via upgrade header  
**Format**: Binary frames with MessagePack or JSON  
**Support**: All protocols (StreamableHTTP, MCP, UMICP)

## Concurrency & Thread Safety

### Shared State Protection
- **Key-Value Store**: `Arc<RwLock<RadixMap>>`
- **Queues**: `Arc<RwLock<HashMap<QueueName, Queue>>>`
- **Event Streams**: `Arc<RwLock<HashMap<RoomId, Room>>>`
- **Subscribers**: `DashMap` for concurrent access

### Lock Strategies
- **Read-heavy operations**: RwLock for multiple concurrent reads
- **Write-heavy operations**: Mutex for exclusive access
- **Lock-free**: DashMap for concurrent hash maps

## Error Handling

### Error Categories
1. **Protocol Errors**: Invalid messages, malformed requests
2. **Resource Errors**: Queue full, memory limit exceeded
3. **Replication Errors**: Connection lost, sync failures
4. **Application Errors**: Key not found, invalid operation

### Error Propagation
- Protocol layer returns HTTP status codes
- Internal errors use Result<T, SynapError>
- Client SDKs expose typed exceptions

## Monitoring & Observability

### Metrics Exposed
- Operations per second (per component)
- Memory usage and allocation
- Replication lag (milliseconds)
- Queue depth and consumer count
- Event stream subscriber count
- Error rates by type

### Health Checks
- `/health` - Overall system health
- `/health/kv` - Key-value store status
- `/health/queue` - Queue system status
- `/health/stream` - Event stream status
- `/health/replication` - Replication lag and status

## Configuration

### Server Configuration
```yaml
server:
  host: "0.0.0.0"
  port: 15500
  websocket_enabled: true

protocols:
  streamable_http:
    enabled: true
    path: /api
  
  mcp:
    enabled: true
    port: 15501
    features: ["resources", "tools", "prompts"]
  
  umicp:
    enabled: true
    websocket_path: /umicp
    http2_path: /umicp/http

memory:
  max_memory_mb: 4096
  eviction_policy: "lru"

replication:
  mode: "master"  # or "replica"
  master_host: null  # for replica nodes
  master_port: null
  sync_interval_ms: 100
```

### Component Configuration
- **Key-Value**: Max keys, TTL defaults, eviction policies
- **Queues**: Max depth, retry limits, dead letter settings
- **Streams**: Retention period, max events per room
- **Pub/Sub**: Max topics, subscriber limits

## Deployment Topologies

### Single Node (Development)
```
┌──────────────┐
│    Synap     │
│   (Master)   │
└──────────────┘
```

### Master-Slave (Production)
```
┌──────────────┐
│    Synap     │     ┌──────────────┐
│   (Master)   │────→│    Synap     │
│   Write      │     │  (Replica 1) │
└──────────────┘     │   Read       │
      │              └──────────────┘
      │
      │              ┌──────────────┐
      └─────────────→│    Synap     │
                     │  (Replica 2) │
                     │   Read       │
                     └──────────────┘
```

### Load Balanced Reads
```
           ┌──────────────┐
           │ Load Balancer│
           └──────────────┘
                  │
      ┌───────────┼───────────┐
      ▼           ▼           ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│ Replica  │ │ Replica  │ │ Replica  │
│    1     │ │    2     │ │    3     │
└──────────┘ └──────────┘ └──────────┘
      ▲           ▲           ▲
      └───────────┴───────────┘
                  │
           ┌──────────────┐
           │    Master    │
           │   (Write)    │
           └──────────────┘
```

## Component Interactions

### Key-Value with Replication
```
Client SET request
    │
    ▼
KV Store.set(key, value)
    │
    ├──→ Store in Radix tree
    │
    └──→ Append to ReplicationLog
         │
         └──→ Stream to Replicas
```

### Queue with Acknowledgment
```
Client PUBLISH
    │
    ▼
Queue.publish(message)
    │
    ├──→ Add to queue VecDeque
    │
    └──→ Replicate to slaves
         │
Client CONSUME
    │
    ▼
Queue.consume() → Message (pending)
    │
Client ACK
    │
    ▼
Queue.ack(message_id) → Remove from pending
```

### Event Stream Broadcasting
```
Client PUBLISH to room
    │
    ▼
EventStream.publish(room, event)
    │
    ├──→ Add to room.events (ring buffer)
    │
    └──→ Broadcast to room.subscribers
         │
         ├──→ Subscriber 1 (WebSocket)
         ├──→ Subscriber 2 (WebSocket)
         └──→ Subscriber N (WebSocket)
```

## Module Structure

```
synap/
├── src/
│   ├── main.rs                  # Server entry point
│   ├── lib.rs                   # Library exports
│   │
│   ├── protocol/
│   │   ├── mod.rs               # Protocol module
│   │   ├── streamable_http.rs   # StreamableHTTP handler
│   │   ├── websocket.rs         # WebSocket upgrade
│   │   └── envelope.rs          # Message envelope
│   │
│   ├── core/
│   │   ├── mod.rs
│   │   ├── kv_store.rs          # Key-value store
│   │   ├── queue.rs             # Queue system
│   │   ├── event_stream.rs      # Event streaming
│   │   └── pubsub.rs            # Pub/sub router
│   │
│   ├── replication/
│   │   ├── mod.rs
│   │   ├── master.rs            # Master node logic
│   │   ├── replica.rs           # Replica node logic
│   │   └── log.rs               # Replication log
│   │
│   ├── storage/
│   │   ├── mod.rs
│   │   └── persistence.rs       # Optional disk persistence
│   │
│   ├── server/
│   │   ├── mod.rs
│   │   ├── handlers.rs          # HTTP handlers
│   │   └── router.rs            # Route configuration
│   │
│   └── utils/
│       ├── mod.rs
│       ├── error.rs             # Error types
│       └── metrics.rs           # Monitoring
│
├── client-sdks/
│   ├── typescript/              # TypeScript SDK
│   ├── python/                  # Python SDK
│   └── rust/                    # Rust SDK
│
└── examples/
    ├── chat/                    # Chat sample
    ├── task-queue/              # Task queue sample
    └── event-broadcast/         # Broadcasting sample
```

## Technology Choices

### Core Stack
- **Rust 2024**: Memory safety, zero-cost abstractions, fearless concurrency
- **Tokio**: Production-grade async runtime with excellent performance
- **Axum**: Modern web framework built on Tokio/Tower/Hyper
- **radix_trie**: Memory-efficient key-value storage
- **DashMap**: Lock-free concurrent HashMap

### Serialization
- **serde**: Rust standard for serialization
- **serde_json**: JSON support for protocol
- **rmp-serde**: MessagePack for binary efficiency

### Network
- **hyper**: HTTP/1.1 and HTTP/2 support
- **tokio-tungstenite**: WebSocket implementation
- **tower-http**: HTTP middleware (CORS, compression, tracing)

## Performance Characteristics

### Latency
- **Key-Value GET**: 0.1-0.5ms (in-memory lookup)
  - L1 Cache Hit: < 0.1ms (decompressed)
  - L2 Cache Hit: ~0.3ms (needs decompression)
  - Cold Read: 0.5-1ms (full lookup + decompression)
- **Key-Value SET**: 0.5-1ms (with replication)
- **Queue Operations**: 1-2ms (with acknowledgment)
- **Event Broadcast**: 0.5-1ms (per subscriber)
- **Pub/Sub**: 0.2-0.5ms (topic routing)

### Throughput
- **KV Operations**: 100K-500K ops/sec per core
- **Queue Messages**: 50K-100K msgs/sec
- **Event Broadcasting**: 10K-50K events/sec
- **Pub/Sub**: 100K+ msgs/sec

### Memory
- **Radix Tree Overhead**: ~30% less than HashMap for string keys
- **Queue Memory**: O(n) where n = pending messages
- **Event Stream**: Fixed ring buffer size per room
- **Replication Log**: Bounded by retention policy
- **Compression**: 2-3x space savings with LZ4, 3-5x with Zstd
- **Cache System**: 30% of memory (10% L1 + 20% L2)

### CPU Efficiency
- **Compression Overhead**: ~3-5% CPU with LZ4
- **Cache Hit Rate**: >80% L1, >90% L2 (typical)
- **CPU Savings**: ~80% reduction in decompression overhead with cache

## Scalability Limits

### Single Node
- **Max Keys**: Limited by available RAM (100M+ keys feasible)
- **Max Queues**: 10K+ concurrent queues
- **Max Rooms**: 100K+ concurrent event rooms
- **Max Topics**: Unlimited (radix tree)
- **Max Connections**: 100K+ concurrent clients (Tokio)

### Clustered (Master + Replicas)
- **Read Scaling**: Linear with replica count
- **Write Scaling**: Limited to master node
- **Replication Lag**: < 10ms typical, < 100ms under load

## Security Considerations

### Authentication
- API key-based authentication
- Optional JWT token support
- Per-client rate limiting

### Authorization
- Role-based access control (RBAC)
- Per-queue permissions
- Per-room access control

### Network Security
- TLS support for encrypted connections
- IP whitelisting
- Connection rate limiting

## Future Enhancements

### Phase 2
- Persistent storage (RocksDB backend)
- Clustering with sharding
- Raft consensus for multi-master

### Phase 3
- Geo-replication
- Query language (Synap Query Language)
- Full-text search integration

### Phase 4
- Time-series data support
- Graph data structures
- Lua scripting support

## See Also

- [Design Decisions](DESIGN_DECISIONS.md) - Rationale for technology choices
- [Key-Value Store Spec](specs/KEY_VALUE_STORE.md) - Detailed KV specification
- [Queue System Spec](specs/QUEUE_SYSTEM.md) - Queue implementation details
- [Replication Spec](specs/REPLICATION.md) - Replication protocol details
- [MCP Integration](protocol/MCP_INTEGRATION.md) - Model Context Protocol support
- [UMICP Integration](protocol/UMICP_INTEGRATION.md) - UMICP protocol support
- [Compression & Cache](COMPRESSION_AND_CACHE.md) - Smart compression and caching system
- [Performance Guide](PERFORMANCE.md) - Performance tuning and benchmarks

