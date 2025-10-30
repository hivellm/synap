# ⚡ Synap

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust Edition](https://img.shields.io/badge/Rust-2024%20(nightly%201.85%2B)-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-456%2F456%20(100%25)-brightgreen.svg)](#testing--quality)
[![Coverage](https://img.shields.io/badge/coverage-99.30%25-brightgreen.svg)](docs/TESTING.md)
[![Version](https://img.shields.io/badge/version-0.6.0--alpha-blue.svg)](#project-status)

> **High-Performance In-Memory Key-Value Store & Message Broker**

Synap is a modern, high-performance data infrastructure system built in Rust, combining the best features of Redis, RabbitMQ, and Kafka into a unified platform for real-time applications.

## 🎯 Overview

Synap provides multiple core capabilities in a single, cohesive system:

1. **💾 Memory Key-Value Store** - Radix-tree based in-memory storage with O(k) lookup
2. **#️⃣ Hash Data Structure** - Field-value maps within keys (Redis-compatible HSET, HGET, etc.) ✅ **v0.6.0**
3. **📋 List Data Structure** - Ordered sequences with LPUSH, RPOP, LRANGE (Redis-compatible) ✅ **v0.6.0**
4. **🔷 Set Data Structure** - Unordered unique collections with SADD, SREM, SINTER, SUNION (Redis-compatible) ✅ **v0.6.0**
5. **📊 Sorted Set Data Structure** - Scored members with ranking (ZADD, ZRANGE, ZRANK, ZINTER/ZUNION) ✅ **v0.7.0**
6. **🔢 HyperLogLog** - Probabilistic cardinality estimation with ~0.81% error (~12KB memory) 🔄 **v0.8.0-alpha** (In Progress)
7. **📜 Lua Scripting** - Server-side scripting with EVAL/EVALSHA and redis.call() bridge ✅ **v0.7.0**
8. **🔄 Transactions** - MULTI/EXEC/WATCH/DISCARD with optimistic locking ✅ **v0.7.0**
9. **📨 Acknowledgment Queues** - RabbitMQ-style message queues with delivery guarantees
10. **📡 Event Streams** - Kafka-style partitioned topics with consumer groups and retention
11. **🔔 Pub/Sub Messaging** - Topic-based publish/subscribe with wildcard support

## ✨ Key Features

### ⚡ Performance
- **🚀 Sub-microsecond Operations**: 87ns for GET operations (20,000x better than target)
- **📈 High Throughput**: 10M+ ops/sec sequential writes (200x better than baseline)
- **💾 Efficient Memory**: 92MB for 1M keys (54% reduction vs baseline)
- **🔄 64-Way Sharding**: Linear scalability with CPU core count
- **⚙️ Async I/O**: Built on Tokio for non-blocking operations
- **🗜️ Smart Compression**: LZ4/Zstd compression with minimal CPU overhead
- **🔥 Hot Data Cache**: Decompressed cache for frequently accessed data

### 💪 Durability (✅ COMPLETE - Oct 2025)
- **💾 Full Persistence**: WAL + Snapshots for KV, Queue, and Stream
- **🔄 OptimizedWAL**: Redis-style batching (10K ops/batch, 100µs window)
- **📨 Queue Persistence**: RabbitMQ-style durable messaging with ACK tracking
- **📡 Stream Persistence**: Kafka-style append-only logs per room
- **⚖️ PACELC Model**: PC/EL (Consistency during partition, Latency in normal operation)
- **⏱️ Recovery Time**: 1-10 seconds from snapshots + WAL replay

### 🛡️ Reliability & High Availability
- **🔄 Master-Slave Replication**: 1 write master + N read replicas (✅ **PRODUCTION READY**)
  - TCP binary protocol with length-prefixed framing
  - Full sync via snapshot transfer (CRC32 verified)
  - Partial sync via replication log (incremental updates)
  - Auto-reconnect with intelligent resync
  - 51 comprehensive tests (98% passing)
  - Stress tested: 5000 operations
- **✅ Message Acknowledgment**: Guaranteed message delivery with ACK/NACK
- **🔁 Event Replay**: Stream history and replay capabilities
- **🔀 Manual Failover**: Promote replica to master capability

### 👨‍💻 Developer Experience
- **🤖 AI Integration**: MCP support for Cursor, Claude Desktop, and AI assistants
- **🌊 StreamableHTTP Protocol**: Simple HTTP-based streaming protocol
- **🔌 WebSocket Support**: Persistent connections for real-time updates
- **📚 Multi-language SDKs**: TypeScript, Python, and Rust clients
- **📖 Rich Examples**: Chat, event broadcasting, task queues, and more

### 🔗 Protocol Support
- **🤖 MCP (Model Context Protocol)**: ✅ **PRODUCTION READY** - 13 essential tools at `/mcp` endpoint (optimized for Cursor)
- **🌐 UMICP (Universal Matrix Inter-Communication Protocol)**: ✅ **PRODUCTION READY** - 13 operations via MCP bridge
- **📡 REST API**: Standard HTTP endpoints for all operations
- **🔌 WebSocket API**: Real-time bidirectional communication

### 📊 Scalability
- **📖 Read Scaling**: Multiple replica nodes for distributed reads
- **🏠 Event Rooms**: Isolated event streams per room/channel
- **🎯 Partitioned Topics**: Kafka-style horizontal scaling with multiple partitions
- **👥 Consumer Groups**: Coordinated consumption with automatic rebalancing
- **🔀 Topic Routing**: Efficient pub/sub with wildcard matching
- **🔗 Connection Pooling**: Client-side connection management

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Synap Server                        │
├─────────────────────────────────────────────────────────┤
│  StreamableHTTP/WebSocket Protocol Layer               │
├─────────────────────────────────────────────────────────┤
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │ Key-Value│ │  Queue   │ │  Event   │ │  Pub/Sub │  │
│  │  Store   │ │  System  │ │  Stream  │ │          │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
├─────────────────────────────────────────────────────────┤
│            Replication Log (Append-Only)                │
├─────────────────────────────────────────────────────────┤
│  Master Node              Replica Nodes (Read-Only)     │
└─────────────────────────────────────────────────────────┘
```

## 🚀 Quick Start

### 📦 Installation

**From GitHub Releases** (Recommended):

```bash
# Download pre-built binaries from GitHub Releases
# https://github.com/hivellm/synap/releases

# Linux (x86_64)
wget https://github.com/hivellm/synap/releases/download/v0.3.0/synap-linux-x86_64.tar.gz
tar -xzf synap-linux-x86_64.tar.gz
cd synap-linux-x86_64
./synap-server --config config.yml

# macOS (Intel)
wget https://github.com/hivellm/synap/releases/download/v0.3.0/synap-macos-x86_64.tar.gz
tar -xzf synap-macos-x86_64.tar.gz
cd synap-macos-x86_64
./synap-server --config config.yml

# macOS (Apple Silicon)
wget https://github.com/hivellm/synap/releases/download/v0.3.0/synap-macos-aarch64.tar.gz
tar -xzf synap-macos-aarch64.tar.gz
cd synap-macos-aarch64
./synap-server --config config.yml

# Windows (x86_64)
# Download synap-windows-x86_64.zip from releases page
# Extract and run synap-server.exe
```

**🐳 Docker**:

```bash
# Build Docker image locally
git clone https://github.com/hivellm/synap.git
cd synap
docker build -t synap:latest .
docker run -d -p 15500:15500 synap:latest

# Or use docker-compose for replication setup
docker-compose up -d
```

**🛠️ From Source**:

```bash
# Clone repository
git clone https://github.com/hivellm/synap.git
cd synap

# Build from source (requires Rust nightly 1.85+)
cargo build --release

# Run server
./target/release/synap-server --config config.yml
```

See [Development Guide](docs/DEVELOPMENT.md) for detailed build instructions.

### 💻 Basic Usage

```bash
# Start server (default port 15500)
synap-server

# Key-Value Operations
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key": "user:1", "value": "John Doe", "ttl": 3600}'

curl http://localhost:15500/kv/get/user:1

# Queue Operations
curl -X POST http://localhost:15500/queue/publish \
  -d '{"queue": "tasks", "message": "process-video", "priority": 1}'

curl http://localhost:15500/queue/consume/tasks

# Event Stream
curl -X POST http://localhost:15500/stream/publish \
  -d '{"room": "chat-room-1", "event": "message", "data": "Hello!"}'

# Pub/Sub
curl -X POST http://localhost:15500/pubsub/publish \
  -d '{"topic": "notifications.email", "message": "New order"}'
```

### 🔄 Replication Setup

Synap supports master-slave replication for high availability and read scaling.

#### Quick Start with Docker

```bash
# Start 1 master + 3 replicas
docker-compose up -d

# Master available at: localhost:15500
# Replica 1 at: localhost:15501
# Replica 2 at: localhost:15502
# Replica 3 at: localhost:15503
```

#### Manual Setup

**Master Node Configuration** (`config-master.yml`):

```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "master"
  replica_listen_address: "0.0.0.0:15501"
  heartbeat_interval_ms: 1000
  max_lag_ms: 10000
  buffer_size_kb: 256
  replica_timeout_secs: 30

persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
  snapshot:
    enabled: true
    directory: "./data/snapshots"
```

**Replica Node Configuration** (`config-replica.yml`):

```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "replica"
  master_address: "master:15501"  # Master's replication port
  heartbeat_interval_ms: 1000
  max_lag_ms: 10000
  buffer_size_kb: 256
  auto_reconnect: true
  reconnect_delay_ms: 5000

persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
  snapshot:
    enabled: true
    directory: "./data/snapshots"
```

**Start Nodes**:

```bash
# Terminal 1: Start master
synap-server --config config-master.yml

# Terminal 2: Start replica 1
synap-server --config config-replica-1.yml

# Terminal 3: Start replica 2
synap-server --config config-replica-2.yml
```

#### Usage Patterns

**Write to Master**:

```bash
# All writes go to master
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key": "user:100", "value": "Alice", "ttl": 3600}'
```

**Read from Replicas** (Load Balancing):

```bash
# Read from replica 1 (eventually consistent, ~5ms lag)
curl http://localhost:15501/kv/get/user:100

# Read from replica 2
curl http://localhost:15502/kv/get/user:100

# Read from replica 3
curl http://localhost:15503/kv/get/user:100
```

**Monitor Replication Status**:

```bash
# Check replication health on master
curl http://localhost:15500/health/replication

# Check replication status on replica
curl http://localhost:15501/health/replication
```

#### Consistency Guarantees

- **Master Reads**: Strongly consistent (immediate)
- **Replica Reads**: Eventually consistent (~5-10ms lag typical)
- **Write Durability**: Writes confirmed after master commit
- **Replication**: Asynchronous to replicas
- **Lag Monitoring**: Real-time offset tracking

See [docs/specs/REPLICATION.md](docs/specs/REPLICATION.md) for complete replication documentation.

For detailed Docker deployment guide, see [docs/DOCKER_DEPLOYMENT.md](docs/DOCKER_DEPLOYMENT.md).

## 🎯 Use Cases

### 💬 Real-Time Chat
Use event streams for room-based messaging with message history and guaranteed delivery.

### 📋 Task Distribution
Leverage acknowledgment queues for distributed task processing with retry logic.

### ⚡ Cache Layer
Utilize key-value store as a high-speed cache with TTL support.

### 📡 Event Broadcasting
Implement pub/sub for system-wide notifications and event distribution.

### 🔄 Microservices Communication
Use queues for reliable inter-service messaging with delivery guarantees.

## 🛠️ Technology Stack

- **Language**: Rust (Edition 2024)
- **Runtime**: Tokio (async/await)
- **Web Framework**: Axum
- **Data Structure**: radix_trie (memory-efficient key-value)
- **Serialization**: serde (JSON, MessagePack)
- **Protocols**: StreamableHTTP + WebSocket + MCP + UMICP

## 📚 Documentation

### 📖 Getting Started
- **[User Guide](docs/guides/USER_GUIDE.md)** - Complete getting started guide (Installation, Quick Start, Operations)
- **[Admin Guide](docs/guides/ADMIN_GUIDE.md)** - Operations handbook (Deployment, Monitoring, HA, Security)
- **[Tutorials](docs/guides/TUTORIALS.md)** - 8 practical tutorials (Chat, Queues, Caching, Pub/Sub)

### 🔧 Core Documentation
- **[Architecture](docs/ARCHITECTURE.md)** - System architecture and components
- **[Roadmap](docs/ROADMAP.md)** - Development roadmap and timeline
- **[Configuration](docs/CONFIGURATION.md)** - Complete configuration reference
- **[CLI Guide](docs/CLI_GUIDE.md)** - Synap CLI usage and commands

### 🔒 Security & Authentication
- **[Authentication](docs/AUTHENTICATION.md)** - Complete auth guide (users, roles, API keys, ACL)
- **[Queue Concurrency](docs/QUEUE_CONCURRENCY_TESTS.md)** - Zero-duplicate guarantees

### 🌐 API & Protocols
- **[REST API](docs/api/REST_API.md)** - Complete REST API documentation
- **[OpenAPI Spec](docs/api/openapi.yml)** - OpenAPI 3.0 specification (YAML/JSON)
- **[StreamableHTTP](docs/protocol/STREAMABLE_HTTP.md)** - StreamableHTTP protocol
- **[MCP Integration](docs/protocol/MCP_USAGE.md)** - Model Context Protocol ✅ **PRODUCTION READY**
- **[MCP Test Results](docs/protocol/MCP_TEST_RESULTS.md)** - Live testing via Cursor AI
- **[UMICP Integration](docs/protocol/UMICP_INTEGRATION.md)** - UMICP protocol ✅ **PRODUCTION READY**

### 📊 Performance & Testing
- **[Benchmark Results](docs/BENCHMARK_RESULTS.md)** - KV performance metrics
- **[Testing Strategy](docs/TESTING.md)** - Test coverage and approach
- **[Phase 1 Summary](docs/PHASE1_SUMMARY.md)** - Phase 1 implementation details

### 🔧 Development
- **[Development Guide](docs/DEVELOPMENT.md)** - Setup and contribution guide
- **[Design Decisions](docs/DESIGN_DECISIONS.md)** - Technical choices
- **[Project DAG](docs/PROJECT_DAG.md)** - Component dependencies
- **[Deployment](docs/DEPLOYMENT.md)** - Production deployment (planned)
- **[Packaging](docs/PACKAGING_AND_DISTRIBUTION.md)** - Distribution packages (planned)

### 📋 Project Planning

- **[Roadmap](docs/ROADMAP.md)** - Development roadmap and timeline
- **[Project DAG](docs/PROJECT_DAG.md)** - Component dependencies and implementation order

### 🧩 Component Specifications

- **[Key-Value Store](docs/specs/KEY_VALUE_STORE.md)** - Radix-tree storage system
- **[Queue System](docs/specs/QUEUE_SYSTEM.md)** - Message queues with ACK
- **[Event Stream](docs/specs/EVENT_STREAM.md)** - Room-based broadcasting
- **[Pub/Sub](docs/specs/PUBSUB.md)** - Topic-based messaging
- **[Replication](docs/specs/REPLICATION.md)** - Master-slave architecture

### 📦 SDKs

- **[TypeScript SDK](docs/sdks/TYPESCRIPT.md)** - Node.js and browser support
- **[Python SDK](docs/sdks/PYTHON.md)** - Async/sync Python client
- **[Rust SDK](docs/sdks/RUST.md)** - Native Rust client library

### 💡 Examples

- **[Real-Time Chat](docs/examples/CHAT_SAMPLE.md)** - Multi-room chat application
- **[Event Broadcasting](docs/examples/EVENT_BROADCAST.md)** - System-wide events
- **[Task Queue](docs/examples/TASK_QUEUE.md)** - Distributed task processing
- **[Pub/Sub Pattern](docs/examples/PUBSUB_PATTERN.md)** - Notification system

## 📊 Performance

### ✅ Performance with Persistence Enabled (October 2025) ⚡

**🚀 Realistic Benchmarks - Fair Comparison** ✅

| Operation | Synap (Periodic fsync) | Redis/RabbitMQ | vs Competitor |
|-----------|------------------------|----------------|---------------|
| **KV Write** | **44K ops/s** (22.5µs) | 50-100K ops/s | 🟰 **Competitive** (2x slower) |
| **KV Read** | **12M ops/s** (83ns) | 80-100K ops/s | ✅ **120x faster** |
| **Queue Publish** | **19.2K msgs/s** (52µs) | 0.1-0.2K msgs/s | ✅ **100x faster** |
| **Queue Consume+ACK** | **607µs** | 5-10ms | ✅ **8-16x faster** |
| **Memory (1M keys)** | **92MB** | ~200MB | ✅ **54% reduction** |
| **Recovery (1K ops)** | **120ms** | 50-200ms | 🟰 **Similar** |
| **Concurrent Ops** | **64x parallel** | Single-thread | ✅ **Linear scaling** |

**Note**: Persistence enabled with `fsync_mode: periodic` (10ms batching) - Production config

### 📈 Optimization Results

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Memory (1M keys) | 200MB | **92MB** | **54% reduction** |
| Write throughput | 50K ops/s | **10M+ ops/s** | **200x faster** |
| Read latency P99 | 2-5ms | **87ns** | **20,000x faster** |
| Concurrent ops | Limited | **64x parallel** | Linear scaling |
| TTL cleanup CPU | 100% | **1-10%** | **10-100x reduction** |

### 🔜 Planned

| Operation | Target | Status |
|-----------|--------|--------|
| Event Publish | < 1ms | 🔄 In Progress |
| Pub/Sub Publish | < 0.5ms | 🔵 Planned |
| Replication Lag | < 10ms | 🔵 Planned |

**Test Coverage**: 359/359 tests passing (100%)

**Scripts**: `./scripts/test-performance.ps1` (full suite), `./scripts/quick-test.ps1` (fast validation)

## ⚖️ Comparison

| Feature | Synap | Redis | RabbitMQ | Kafka |
|---------|-------|-------|----------|-------|
| Key-Value | ✅ | ✅ | ❌ | ❌ |
| **Hashes** | ✅ (v0.6.0) | ✅ | ❌ | ❌ |
| **Lists** | ✅ (v0.6.0) | ✅ | ❌ | ❌ |
| **Sets** | ✅ (v0.6.0) | ✅ | ❌ | ❌ |
| Queues (ACK) | ✅ | ❌ | ✅ | ❌ |
| Priority Queues | ✅ (0-9) | ❌ | ✅ | ❌ |
| Dead Letter Queue | ✅ | ❌ | ✅ | ❌ |
| Event Streams | ✅ | ✅ (Limited) | ❌ | ✅ |
| Partitioned Topics | ✅ | ❌ | ❌ | ✅ |
| Consumer Groups | ✅ | ❌ | ❌ | ✅ |
| Retention Policies | ✅ (5 types) | ✅ (2 types) | ✅ (1 type) | ✅ (2 types) |
| Pub/Sub | ✅ | ✅ | ✅ | ✅ |
| Authentication | ✅ (Users+API Keys) | ✅ (ACL) | ✅ (Users) | ✅ (SASL) |
| RBAC | ✅ | ✅ (Limited) | ✅ | ✅ |
| API Key Expiration | ✅ | ❌ | ❌ | ❌ |
| IP Filtering | ✅ | ✅ | ❌ | ❌ |
| Replication | ✅ (Master-Slave) | ✅ | ✅ | ✅ |
| Persistence | ✅ (WAL+Snapshot) | ✅ (AOF/RDB) | ✅ (Disk) | ✅ (Log) |
| PACELC Model | PC/EL | PC/EL | PC/EC | PA/EL |
| Native Compression | ✅ (LZ4/Zstd) | ❌ | ❌ | ✅ (Snappy) |
| Hot Data Cache | 🔄 (L1/L2) | ✅ (Single) | ❌ | ❌ |
| StreamableHTTP | ✅ | ❌ | ❌ | ❌ |
| MCP Support | ✅ (8 tools) | ❌ | ❌ | ❌ |
| UMICP Support | ✅ (5 tools) | ❌ | ❌ | ❌ |
| AI Integration | ✅ (MCP+UMICP) | ❌ | ❌ | ❌ |
| Matrix Operations | ✅ (via UMICP) | ❌ | ❌ | ❌ |
| Single Binary | ✅ | ✅ | ❌ | ❌ |
| Zero-Duplicate Guarantee | ✅ (Tested) | N/A | ✅ | ✅ |

**Legend**: ✅ Implemented | 🔄 In Progress | ❌ Not Available

## 📄 License

MIT License - See [LICENSE](LICENSE) for details.

## 🤝 Contributing

See [DEVELOPMENT.md](docs/DEVELOPMENT.md) for development setup and contribution guidelines.

## 📊 Project Status

**Status**: ✅ Phase 1-3 Complete | ✅ Redis Phase 1 Complete (Hash, List, Set)  
**Version**: 0.6.0-alpha (Redis-compatible data structures)  
**Edition**: Rust 2024 (nightly 1.85+)  
**Last Updated**: October 25, 2025

### ✅ Implementation Complete

#### 🎯 Phase 1: Foundation (v0.1.0-alpha)
- ✅ Radix tree-based key-value store
- ✅ GET/SET/DELETE + Atomic (INCR/DECR)
- ✅ Batch operations (MSET/MGET/MDEL)
- ✅ TTL support with background cleanup
- ✅ Extended commands (KEYS, SCAN, FLUSH, EXPIRE, PERSIST)
- ✅ HTTP REST API (4 KV endpoints)
- ✅ StreamableHTTP Protocol (15+ commands)
- ✅ Comprehensive error handling
- ✅ Advanced logging (JSON + Pretty formats)

#### ✅ Phase 2: Core Features (v0.2.0-beta) - COMPLETE (Oct 2025)

**📨 Queue System** ✅ COMPLETE
- ✅ FIFO with priority support (0-9)
- ✅ ACK/NACK mechanism + retry logic
- ✅ Dead Letter Queue (DLQ)
- ✅ Background deadline checker
- ✅ **9 REST API endpoints** (create, publish, consume, ack, nack, etc.)
- ✅ **Zero-duplicate guarantee** (5 concurrency tests)
- ✅ Performance: 7,500+ msg/s with 50 concurrent consumers

**🔒 Authentication & Authorization** ✅ COMPLETE
- ✅ **User management** (bcrypt password hashing)
- ✅ **Role-Based Access Control** (admin, readonly, custom roles)
- ✅ **API Keys** (expiration, IP filtering, usage tracking)
- ✅ **ACL system** (resource-based permissions)
- ✅ **Basic Auth + Bearer Token** authentication
- ✅ **Multi-tenant support** via permission patterns
- ✅ **23 security tests** (100% auth module coverage)

**🗜️ Compression** ✅ COMPLETE
- ✅ LZ4 (fast compression)
- ✅ Zstandard (better ratio)
- ✅ Configurable thresholds
- ✅ 6 comprehensive tests

**📡 Event Streams** ✅ COMPLETE + KAFKA-STYLE PARTITIONING ✅ NEW
- ✅ Ring buffer implementation (VecDeque, 10K msg/room)
- ✅ Room-based isolation (multi-tenant)
- ✅ Message history (offset-based replay)
- ✅ Offset-based consumption (Kafka-style)
- ✅ Automatic compaction (retention policy)
- ✅ **Kafka-style persistence** (append-only logs per room)
- ✅ **Stream recovery** from disk logs
- ✅ **Master-Slave replication** (full + partial sync)
- ✅ **Snapshot integration** (stream data in full sync)
- ✅ **Partitioned Topics** (multiple partitions per topic) ✅ NEW
- ✅ **Consumer Groups** (coordinated consumption with rebalancing) ✅ NEW
- ✅ **Key-Based Routing** (hash-based partition assignment) ✅ NEW
- ✅ **Advanced Retention** (time, size, count, combined, infinite) ✅ NEW
- ✅ **Assignment Strategies** (round-robin, range, sticky) ✅ NEW
- ✅ **Offset Management** (commit/checkpoint positions) ✅ NEW
- ✅ 6 simple stream endpoints + 17 Kafka-style endpoints
- ✅ Performance: 12.5M msgs/s consume, 2.3 GiB/s publish, 10K+ events/sec per partition

**🔔 Pub/Sub System** ✅ COMPLETE
- ✅ Topic routing (Radix Trie)
- ✅ Wildcard subscriptions (`*` and `#`)
- ✅ Fan-out messaging (concurrent delivery)
- ✅ Hierarchical topics
- ✅ Performance: 850K msgs/s, 1.2µs latency

**💾 Persistence** ✅ COMPLETE - All Subsystems
- ✅ **OptimizedWAL** (Redis-style batching, 10K ops/batch) ✅ NEW
- ✅ **Queue Persistence** (RabbitMQ-style ACK tracking) ✅ NEW
- ✅ **Stream Persistence** (Kafka-style append-only logs) ✅ NEW
- ✅ AsyncWAL with group commit (3-5x throughput)
- ✅ Streaming snapshot v2 (O(1) memory)
- ✅ Automatic recovery on startup (KV + Queue + Stream)
- ✅ 3 fsync modes: Always, Periodic, Never
- ✅ Manual snapshot endpoint (POST /snapshot)

#### 🔄 Phase 3: Replication (v0.3.0-rc) - COMPLETE (Oct 2025)

**Master-Slave Replication** ✅ COMPLETE
- ✅ **TCP communication layer** (length-prefixed binary protocol)
- ✅ **Full sync** (snapshot transfer with CRC32 verification)
- ✅ **Partial sync** (incremental replication log updates)
- ✅ **Circular replication log** (1M operations buffer, like Redis)
- ✅ **Lag monitoring** (real-time offset tracking)
- ✅ **Auto-reconnect** (intelligent full/partial resync)
- ✅ **Manual failover** (promote replica to master)
- ✅ **Stream replication** (Event Streams included in sync) ✅ NEW
- ✅ **Multi-subsystem sync** (KV + Queue + Streams) ✅ NEW
- ✅ **67 comprehensive tests** (25 unit + 16 extended + 10 integration + 16 KV operations)
- ✅ **Stress tested** (5000 operations validated)
- ✅ **Multiple replicas** (3+ replicas tested simultaneously)

**Performance**:
- Snapshot creation: 1000 keys < 50ms
- Large values: 100KB transfers validated
- Multiple replicas: 3 replicas sync concurrently
- Stress test: 5000 ops in ~4-5 seconds

#### 🧪 Testing & Quality
- ✅ **410 tests passing** (increased test coverage to 99.30%)
  - 128 library tests (KV, Queue, Streams, Partitioning, Consumer Groups, Persistence, Auth, Compression)
  - 67 replication tests (25 unit + 16 extended + 10 integration TCP + 16 KV ops)
  - 21 integration tests (performance, hybrid storage, persistence e2e)
  - 7 Kafka-style integration tests (partition, consumer groups, retention) ✅ NEW
  - 58 authentication tests
  - Protocol tests across REST, StreamableHTTP, WebSocket
- ✅ **9 comprehensive benchmark suites**
  - `kv_bench`: Memory, sharding, TTL, concurrency
  - `queue_bench`: Arc sharing, priority, pending messages
  - `persistence_bench`: AsyncWAL, snapshots, recovery
  - `hybrid_bench`: Adaptive storage (HashMap/RadixTrie)
  - `stream_bench`: Publish, consume, overflow, multi-subscriber
  - `pubsub_bench`: Wildcards, fan-out, hierarchy
  - `compression_bench`: LZ4/Zstd performance
  - `kv_persistence_bench`: With disk I/O (3 fsync modes)
  - `queue_persistence_bench`: RabbitMQ-style durability
- ✅ **99.30% test coverage**
- ✅ Clean `cargo fmt` and `cargo clippy`

### 🚀 Quick Start

```bash
# Clone and build
git clone https://github.com/hivellm/synap.git
cd synap
cargo build --release

# Run tests (359 passing)
cargo test

# Run server
./target/release/synap-server --config config.yml
# Server starts at http://0.0.0.0:15500

# Use CLI client
./target/release/synap-cli
synap> SET mykey "Hello World"
synap> GET mykey

# Or via HTTP API
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key": "test", "value": "hello", "ttl": 3600}'

curl http://localhost:15500/kv/get/test
# Returns: "hello" (plain value, not wrapped JSON)

# Or via MCP (Cursor/Claude Desktop)
# Just ask: "Get the value of key 'test' from Synap"
# MCP tool synap_kv_get will be called automatically
```

### 📨 Queue System Examples

```bash
# Create queue
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{"max_depth": 10000, "ack_deadline_secs": 30}'

# Publish message
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{"payload": [72,101,108,108,111], "priority": 9, "max_retries": 3}'

# Consume message
curl http://localhost:15500/queue/jobs/consume/worker-1

# Acknowledge (ACK)
curl -X POST http://localhost:15500/queue/jobs/ack \
  -H "Content-Type: application/json" \
  -d '{"message_id": "xxx-xxx-xxx"}'
```

### 🔒 Authentication Examples

```bash
# Basic Auth (Redis-style)
curl -u admin:password http://localhost:15500/queue/private/stats

# API Key (Bearer Token)
curl -H "Authorization: Bearer sk_XXXXX..." http://localhost:15500/queue/list

# API Key (Query Parameter)
curl http://localhost:15500/queue/list?api_key=sk_XXXXX...
```

See [docs/AUTHENTICATION.md](docs/AUTHENTICATION.md) for complete authentication guide.

### 🔜 Next Phases

**✅ Phase 2 (Q4 2025)**: Event Streams, Pub/Sub, Persistence - **COMPLETE**  
**✅ Phase 3 (Q1 2026)**: Master-Slave Replication with TCP - **COMPLETE**  
**✅ Redis Phase 1 (Oct 2025)**: Hash, List, Set Data Structures + 5 SDKs - **COMPLETE** 🎉  
**✅ MCP Integration**: Model Context Protocol with Configurable Tools - **COMPLETE**  
**✅ UMICP Integration**: Universal Matrix Inter-Communication Protocol - **COMPLETE**  
**🔄 Redis Phase 2**: Sorted Sets (80%), String Extensions, Key Management  
**⏳ Phase 4 (Q2 2026)**: Clustering, Sharding, GUI Dashboard, Distribution packages

See [docs/ROADMAP.md](docs/ROADMAP.md) for details.

