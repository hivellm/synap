# ⚡ Synap

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust Edition](https://img.shields.io/badge/Rust-2024%20(nightly%201.92%2B)-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-1800%2B-brightgreen.svg)](docs/development/TESTING.md)
[![Version](https://img.shields.io/badge/version-1.2.0-blue.svg)](CHANGELOG.md)

> **High-Performance In-Memory Key-Value Store & Message Broker**

Synap is a modern, high-performance data infrastructure system built in Rust, combining the best features of Redis, RabbitMQ, and Kafka into a unified platform for real-time applications.

## 🎯 Overview

Synap provides multiple core capabilities in a single, cohesive system:

- **💾 Key-Value Store** - Sharded in-memory storage with TTL, LRU/LFU eviction, and atomic operations
- **🧱 Redis-compatible data structures** - Hashes, Lists, Sets, Sorted Sets, Bitmaps, HyperLogLog, Geospatial indexes, and string extensions (APPEND, GETRANGE, SETRANGE, …)
- **📜 Lua Scripting** - Server-side scripting with EVAL/EVALSHA and a redis.call() bridge
- **🔄 Transactions** - MULTI/EXEC/WATCH/DISCARD — atomic, durable (single WAL group-commit), replicated, and isolated from concurrent writers
- **⏸️ Blocking operations** - BLPOP/BRPOP/BRPOPLPUSH and BZPOPMIN/BZPOPMAX with Redis timeout semantics
- **🔍 Cursor scans** - SCAN plus HSCAN/SSCAN/ZSCAN with MATCH glob and COUNT
- **🔔 Keyspace notifications** - Redis-style `notify-keyspace-events` (`__keyspace@0__:*` / `__keyevent@0__:*`) via Pub/Sub
- **📨 Acknowledgment Queues** - RabbitMQ-style message queues with delivery guarantees, DLQ, priorities, and per-consumer prefetch/QoS
- **📡 Event Streams** - Kafka-style partitioned topics with consumer groups, retention policies, and consumer-offset-aware buffering
- **🔔 Pub/Sub Messaging** - Topic-based publish/subscribe with wildcard support

## ✨ Key Features

### ⚡ Performance
- **🏁 Redis 7 parity, measured**: driven by the same `redis-benchmark` binary, Synap matches Redis at `-P 1` on every command and sits at 0.85–0.95 pipelined on a single hot key — see [Performance](#-performance)
- **📈 Wins on realistic workloads**: with a randomized keyspace, Synap **beats Redis 7 on SET/INCR/LPUSH by 14–25%**, and at 200 connections wins SET at **1.19×**
- **🚀 Native SynapRPC protocol**: ~3× faster than the RESP3 compatibility path per operation (166k vs 56k rps at `-P 1`)
- **🧵 Zero-copy hot paths**: values live behind shared `Arc<[u8]>` buffers — parsed once, stored, replied, and replicated by refcount bump, no payload memcpy
- **💾 Efficient Memory**: 92MB for 1M keys (vs ~200MB in Redis)
- **🔄 64-Way Sharding**: multi-core scalability with lock-free atomic stats
- **⚙️ Async I/O**: Built on Tokio for non-blocking operations
- **🗜️ Smart Compression**: LZ4/Zstd compression with minimal CPU overhead
- **⚡ SIMD Acceleration**: Runtime-dispatched AVX2/NEON/SIMD128 for BITCOUNT, BITOP, PFMERGE — up to 9.5× faster than scalar on HyperLogLog merge

### 🔐 Security & Authentication
- **🔒 Authentication on every transport** - HTTP, RESP3, SynapRPC, and MCP all gate commands behind auth; destructive/admin commands (FLUSHALL, CONFIG, SHUTDOWN, …) additionally require admin
- **🔑 bcrypt password hashing** - Constant-time comparison; legacy SHA-512 hashes transparently rehash on next login
- **🛡️ Fine-grained Permissions** - Users, API keys (expiration, IP filtering), resource-based ACLs
- **📝 Audit Logging** - Track all authentication events (login, API key usage, permission denials)
- **🧯 DoS hardening** - Parsers cap client-controlled allocations, bounded pub/sub channels, connection limits + idle timeouts on the binary listeners, no reachable panics
- **🔐 Safe defaults** - Binary listeners (RESP3/SynapRPC) bind to loopback by default

### 💪 Durability
- **💾 Full Persistence**: WAL + Snapshots covering **all datatypes** — KV, Hash, List, Set, Sorted Set, Queue, and Stream
- **🔄 Async WAL with group commit**: Redis-style batching (10K ops/batch, 100µs window); transactions commit as one atomic fsync
- **🧾 Verified snapshots**: CRC64 recomputed on load — corrupt/torn snapshots are rejected, never silently loaded
- **🧠 True `maxmemory`**: a shared budget sums every datatype (not just KV), with per-datatype accounting exposed as metrics
- **⚖️ PACELC Model**: PC/EL (Consistency during partition, Latency in normal operation)
- **⏱️ Recovery Time**: 1-10 seconds from snapshots + WAL replay

### 🛡️ Reliability & High Availability
- **🔄 Master-Slave Replication**: 1 write master + N read replicas, wired directly from config
  - Every datatype converges — KV, hash, list, set, sorted set, queue, stream — via the same applier the WAL recovery path uses
  - Full sync via snapshot transfer (CRC verified) + partial sync via replication log; replicas joining mid-write-stream lose nothing
  - Works with persistence disabled (replication is decoupled from the WAL)
  - Auto-reconnect with intelligent resync; `INFO replication` reports live role, replica count, offset, and lag
- **✅ Message Acknowledgment**: Guaranteed message delivery with ACK/NACK
- **🔁 Event Replay**: Stream history and replay; retention protects events the slowest consumer hasn't read
- **🔀 Manual Failover**: Promote replica to master capability
- **🧩 Cluster mode (preview)**: hash-slot topology (16384 slots), slot migration with rollback, and inter-node quota RPC — initialized from config, disabled by default

### 📊 Monitoring & Observability
- **📈 INFO Command** - Redis-style server introspection (server, memory, stats, replication, keyspace)
- **🐌 SLOWLOG** - Slow query logging with configurable threshold (default 10ms)
- **💾 MEMORY USAGE** - Per-key memory tracking across all data types
- **👥 CLIENT LIST** - Active connection tracking and management
- **📊 Prometheus Metrics** - process-scoped CPU/memory plus broker-level gauges (per-stream length, consumer-group lag, queue depth) at `GET /metrics` — see [Observability](docs/operations/observability.md)

### 👨‍💻 Developer Experience
- **🤖 AI Integration**: MCP support for Cursor, Claude Desktop, and AI assistants
- **🌊 StreamableHTTP Protocol**: Simple HTTP-based streaming protocol
- **🔌 WebSocket Support**: Persistent connections for real-time updates
- **📚 Multi-language SDKs**: TypeScript, Python, Rust (with reactive PubSub), Go, PHP, and C# clients with full authentication support
- **📖 Rich Examples**: Chat, event broadcasting, task queues, authentication examples, and more

### 🔗 Protocol Support

Synap supports **three wire transports**. All SDKs (Rust, TypeScript, Python,
Go, PHP, C#) select the transport via URL scheme — no separate builder options required.

| URL scheme    | Port    | Framing                       | When to use                                             |
|---------------|---------|-------------------------------|---------------------------------------------------------|
| `synap://`    | `15501` | MessagePack over TCP          | **✅ Recommended default** — lowest latency, binary, persistent connection, native type fidelity |
| `resp3://`    | `6379`  | Redis text protocol over TCP  | Redis-compatible tooling, `redis-cli`, existing Redis client libraries |
| `http://` / `https://` | `15500` | JSON over HTTP | Ad-hoc `curl`, webhooks, browsers |

> **💡 Recommendation — use `synap://`.**
> SynapRPC is the preferred transport for production workloads: it keeps a
> persistent multiplexed TCP connection, avoids HTTP framing overhead, and
> preserves integer/float/bool/bytes types on the wire (no stringification).
> All commands — KV, queues, streams, pub/sub, transactions, scripts,
> geospatial, HyperLogLog — are fully supported on every transport.
>
> Since 1.2.0 the `synap://` wire is **[Thunder](https://github.com/hivellm/thunder)**
> — the HiveLLM family's shared binary RPC — on both ends: the server listener
> and the Rust, TypeScript, Python and C# SDKs run the same protocol
> implementation, so the two halves of a connection cannot drift. Wire v1 is
> frozen; a pre-1.2.0 client still interoperates. Every SDK — Rust,
> TypeScript, Python, C#, Go and PHP — runs the same protocol implementation,
> verified against one server build by the interop matrix, see
> `docs/thunder-interop-matrix.md`.
>
> ```ts
> // TypeScript
> const synap = new SynapClient("synap://127.0.0.1:15501");
> ```
> ```python
> # Python
> client = SynapClient(SynapConfig("synap://127.0.0.1:15501"))
> ```
> ```rust
> // Rust
> let cfg = SynapConfig::new("synap://127.0.0.1:15501");
> ```
> ```php
> // PHP
> $client = new SynapClient(new SynapConfig("synap://127.0.0.1:15501"));
> ```
> ```csharp
> // C#
> var client = new SynapClient(SynapConfig.Create("synap://127.0.0.1:15501"));
> ```

> **⚠️ No silent HTTP fallback.**
> Native transports (`synap://`, `resp3://`) raise `UnsupportedCommandError` for
> any command not mapped on that transport instead of silently falling back to HTTP.
> Use `http://` if you need full REST access for commands outside the parity matrix.

Additional integration protocols:

- **🤖 MCP (Model Context Protocol)**: Configurable tools (KV, Hash, List, Set, Queue, Sorted Set) at `/mcp` endpoint with authentication support
- **🌐 UMICP (Universal Matrix Inter-Communication Protocol)**: 13 operations via MCP bridge with TLS support

### 📊 Scalability
- **📖 Read Scaling**: Multiple replica nodes for distributed reads
- **🏠 Event Rooms**: Isolated event streams per room/channel
- **🎯 Partitioned Topics**: Kafka-style horizontal scaling with multiple partitions
- **👥 Consumer Groups**: Coordinated consumption with automatic rebalancing
- **🔀 Topic Routing**: Efficient pub/sub with wildcard matching
- **🔗 Connection Pooling**: Client-side connection management

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Synap Server                               │
├──────────────────┬──────────────────────┬───────────────────────────┤
│  HTTP/REST       │  SynapRPC (TCP)      │  RESP3 (TCP)              │
│  :15500          │  :15501 (msgpack)    │  :6379 (Redis text)       │
├──────────────────┴──────────────────────┴───────────────────────────┤
│  server/handlers/  (17 modules)                                     │
│  kv · hash · list · set · sorted_set · hll · bitmap · geospatial    │
│  queue · stream · pubsub · script · websocket · partition · cluster │
├─────────────────────────────────────────────────────────────────────┤
│  protocol/resp3/command/        protocol/synap_rpc/dispatch/        │
│  kv · collections · advanced   kv · collections · advanced          │
├─────────────────────────────────────────────────────────────────────┤
│  core/  (data stores)                                               │
│  kv_store/  bitmap/  hash  list  set  sorted_set  hll  geospatial   │
│  queue  stream  pubsub  transactions  scripting                     │
├─────────────────────────────────────────────────────────────────────┤
│  Replication Log (Append-Only)  ·  WAL + Snapshots persistence      │
├─────────────────────────────────────────────────────────────────────┤
│  Master Node                    Replica Nodes (Read-Only)           │
└─────────────────────────────────────────────────────────────────────┘
```

### Source layout

Since v1.0.0 the workspace is split into focused crates under `crates/`
(Vectorizer/Nexus layout). Layers go Foundation → Core → Features → Presentation;
higher layers depend on lower, never the reverse.

```
crates/
├── synap-core/src/core/     # In-memory data engine (leaf crate — no server deps)
│   ├── kv_store/            # Sharded KV store (TTL, eviction, atomic LRU)
│   ├── hash · list · set · sorted_set · bitmap · hyperloglog · geospatial
│   └── queue · stream · pubsub · partition · transaction · consumer_group
├── synap-server/src/        # HTTP/WS/MCP/UMICP + protocol dispatch
│   ├── server/handlers/     # REST handlers (kv, hash, list, set, queue, stream, pubsub, …)
│   ├── protocol/resp3/      # RESP3 parser/writer + listener + command dispatcher
│   ├── protocol/synap_rpc/  # SynapRPC command catalog + config + listener (wire: thunder)
│   ├── persistence/         # WAL (async group-commit) + snapshots
│   ├── replication/         # master / replica
│   └── auth/ · hub/         # users/api-keys/ACL + HiveHub.Cloud multi-tenant
├── synap-cli/               # Command-line client
└── synap-migrate/           # Migration utilities

sdks/rust/src/               # Rust SDK — Thunder's client under the hood
└── transport/               # SynapRPC / RESP3 / HTTP transports + command mappers
```

> The binary RPC wire layer is **not in this repository**. It is
> [Thunder](https://github.com/hivellm/thunder) (`thunder-rpc`), the HiveLLM
> family's shared implementation, which both the server and the Rust SDK depend
> on — so the two ends of the wire cannot drift.
>
> Rust library consumers: `synap_server::core::*` is now `synap_core::*`
> (umbrella re-exports kept on `synap_server` for transition), and the former
> `synap-protocol` crate is gone — see the [CHANGELOG](CHANGELOG.md) for the
> type-by-type migration to `thunder-rpc`.

## 🚀 Quick Start

### 📦 Installation

**From GitHub Releases** (Recommended):

Pre-built binaries for Linux (x86_64), macOS (Intel and Apple Silicon), and
Windows (x86_64) are published on the
[GitHub Releases page](https://github.com/hivellm/synap/releases).

```bash
# Example (Linux x86_64) — replace <version> with the latest release
wget https://github.com/hivellm/synap/releases/download/v<version>/synap-linux-x86_64.tar.gz
tar -xzf synap-linux-x86_64.tar.gz
cd synap-linux-x86_64
./synap-server --config config/config.yml
```

**🐳 Docker**:

**Quick Start (Docker Hub)**:
```bash
# Pull and run latest image
docker pull hivehub/synap:latest
docker run -d \
  --name synap \
  -p 15500:15500 \
  -p 15501:15501 \
  -v synap-data:/data \
  hivehub/synap:latest

# Check health
curl http://localhost:15500/health
```

**Build Locally**:
```bash
# Clone and build
git clone https://github.com/hivellm/synap.git
cd synap
docker build -t hivehub/synap:latest .

# Run container
docker run -d \
  --name synap-server \
  -p 15500:15500 \
  -p 15501:15501 \
  -v synap-data:/data \
  hivehub/synap:latest
```

**With Authentication**:
```bash
docker run -d \
  --name synap-server \
  -p 15500:15500 \
  -p 15501:15501 \
  -v synap-data:/data \
  -e SYNAP_AUTH_ENABLED=true \
  -e SYNAP_AUTH_REQUIRE_AUTH=true \
  -e SYNAP_AUTH_ROOT_USERNAME=admin \
  -e SYNAP_AUTH_ROOT_PASSWORD=SecurePassword123! \
  -e SYNAP_AUTH_ROOT_ENABLED=true \
  hivehub/synap:latest
```

**Multi-Architecture Build**:
```bash
# Build and push multi-arch images (AMD64 + ARM64)
./scripts/docker/docker-publish.sh 1.2.0

# Or using PowerShell
.\scripts\docker\docker-publish.ps1 1.2.0
```

**Docker Compose**:
```bash
# Use docker-compose for replication setup
docker-compose up -d

# With authentication (set environment variables)
export SYNAP_AUTH_ENABLED=true
export SYNAP_AUTH_REQUIRE_AUTH=true
export SYNAP_AUTH_ROOT_USERNAME=admin
export SYNAP_AUTH_ROOT_PASSWORD=SecurePassword123!
docker-compose up -d
```

**Available Images**:
- `hivehub/synap:latest` - Latest stable release
- `hivehub/synap:<version>` - Specific version (e.g. `hivehub/synap:1.2.0`)
- Supports `linux/amd64` and `linux/arm64` architectures

📖 **For detailed Docker documentation, see [DOCKER_README.md](DOCKER_README.md)**

**🛠️ From Source**:

```bash
# Clone repository
git clone https://github.com/hivellm/synap.git
cd synap

# Build from source (requires Rust nightly 1.92+)
cargo build --release

# Run server
./target/release/synap-server --config config/config.yml
```

See [Development Guide](docs/specs/DEVELOPMENT.md) for detailed build instructions.

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

# With Authentication (if enabled)
curl -X POST http://localhost:15500/kv/set \
  -H "Authorization: Basic $(echo -n 'admin:password' | base64)" \
  -H "Content-Type: application/json" \
  -d '{"key": "user:1", "value": "John Doe"}'

# Or with API Key
curl -X POST http://localhost:15500/kv/set \
  -H "Authorization: Bearer sk_your_api_key_here" \
  -H "Content-Type: application/json" \
  -d '{"key": "user:1", "value": "John Doe"}'
```

### 🔒 Authentication & Security

Authentication is **disabled by default** for development. Enable it for production:

**Features:**
- ✅ **Root User Management** - Configurable root user with full permissions
- ✅ **User Management** - Create, delete, enable/disable users
- ✅ **API Key Management** - Generate, revoke, and manage API keys with expiration
- ✅ **Fine-grained Permissions** - Resource-based permissions (RabbitMQ-style)
- ✅ **Basic Auth & Bearer Token** - Support for both authentication methods
- ✅ **Audit Logging** - Track all authentication events (login, API key usage, permission denials)
- ✅ **Password Validation** - Configurable password requirements (length, complexity)
- ✅ **IP Filtering** - Restrict API keys to specific IP addresses

**Via Config File** (`config/config.yml`):
```yaml
auth:
  enabled: true
  require_auth: true
  root:
    username: "root"
    password: "your_secure_password"
    enabled: true
```

**Via Docker Environment Variables**:
```bash
docker run -d -p 15500:15500 \
  -e SYNAP_AUTH_ENABLED=true \
  -e SYNAP_AUTH_REQUIRE_AUTH=true \
  -e SYNAP_AUTH_ROOT_USERNAME=root \
  -e SYNAP_AUTH_ROOT_PASSWORD=your_secure_password \
  synap:latest
```

**Using Authentication**:
```bash
# Basic Auth
curl -u root:password http://localhost:15500/kv/get/user:1

# Bearer Token (API Key)
curl -H "Authorization: Bearer sk_XXXXX..." http://localhost:15500/kv/get/user:1

# Query Parameter
curl "http://localhost:15500/kv/get/user:1?api_key=sk_XXXXX..."
```

See [Authentication Guide](docs/features/AUTHENTICATION.md) for complete details.

### 🔄 Replication Setup

Synap supports master-slave replication for high availability and read scaling.

#### Quick Start with Docker

```bash
# Start 1 master + 3 replicas
docker-compose up -d

# Master available at: localhost:15500
# Replica 1 at: localhost:15510
# Replica 2 at: localhost:15520
# Replica 3 at: localhost:15530
```

#### Manual Setup

**Master Node Configuration** (`config/config-master.yml`):

```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "master"
  replica_listen_address: "0.0.0.0:15600"  # 15600 = replication (SynapRPC uses 15501)
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

**Replica Node Configuration** (`config/config-replica.yml`):

```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "replica"
  master_address: "master:15600"  # Master's replication port
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
synap-server --config config/config-master.yml

# Terminal 2: Start replica 1
synap-server --config config/config-replica.yml

# Terminal 3: Start replica 2
synap-server --config config/config-replica.yml
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
curl http://localhost:15510/kv/get/user:100

# Read from replica 2
curl http://localhost:15520/kv/get/user:100

# Read from replica 3
curl http://localhost:15530/kv/get/user:100
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

For detailed Docker deployment guide, see:
- **[DOCKER_README.md](DOCKER_README.md)** - Complete Docker Hub documentation with examples
- **[docs/specs/DOCKER_DEPLOYMENT.md](docs/specs/DOCKER_DEPLOYMENT.md)** - Advanced deployment guide

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

- **Language**: Rust (Edition 2024, workspace of focused crates: `synap-core`, `synap-server`, `synap-cli`, `synap-migrate`)
- **Runtime**: Tokio (async/await)
- **Web Framework**: Axum
- **Storage**: 64-way sharded stores (ahash) with `Arc<[u8]>` shared values; radix trie for pub/sub topic routing
- **Serialization**: serde (JSON, MessagePack)
- **Protocols**: SynapRPC + RESP3 + HTTP/StreamableHTTP + WebSocket + MCP + UMICP

## 📚 Documentation

### 📖 Getting Started
- **[User Guide](docs/guides/USER_GUIDE.md)** - Complete getting started guide (Installation, Quick Start, Operations)
- **[Admin Guide](docs/guides/ADMIN_GUIDE.md)** - Operations handbook (Deployment, Monitoring, HA, Security)
- **[Tutorials](docs/guides/TUTORIALS.md)** - 8 practical tutorials (Chat, Queues, Caching, Pub/Sub)

### 🔧 Core Documentation
- **[Architecture](docs/ARCHITECTURE.md)** - System architecture and components
- **[Configuration](docs/specs/CONFIGURATION.md)** - Complete configuration reference
- **[CLI Guide](docs/guides/CLI_GUIDE.md)** - Synap CLI usage and commands
- **[Transports](docs/protocol/transports.md)** - SynapRPC / RESP3 / HTTP command-parity matrix
- **[Transactions](docs/features/transactions.md)** - MULTI/EXEC durability, replication, and isolation
- **[Replication](docs/features/REPLICATION.md)** - Setup, sync semantics, and monitoring
- **[Memory Accounting](docs/internals/memory-accounting.md)** - `maxmemory` across all datatypes
- **[Observability](docs/operations/observability.md)** - Prometheus metrics reference

### 🔒 Security & Authentication
- **[Authentication](docs/features/AUTHENTICATION.md)** - Complete auth guide (users, roles, API keys, ACL)
- **[Network Limits](docs/operations/network-limits.md)** - Connection caps, idle timeouts, parser bounds

### 🌐 API & Protocols
- **[REST API](docs/api/REST_API.md)** - Complete REST API documentation
- **[OpenAPI Spec](docs/api/openapi.yml)** - OpenAPI 3.0 specification (YAML/JSON)
- **[StreamableHTTP](docs/protocol/STREAMABLE_HTTP.md)** - StreamableHTTP protocol
- **[MCP Integration](docs/protocol/MCP_USAGE.md)** - Model Context Protocol
- **[UMICP Integration](docs/protocol/UMICP_INTEGRATION.md)** - UMICP protocol

### 📊 Performance & Testing
- **[Redis vs Synap](docs/benchmarks/redis-vs-synap.md)** - Live `redis-benchmark` head-to-head
- **[Benchmarks](docs/benchmarks/README.md)** - All benchmark suites and results
- **[Queue Concurrency](docs/benchmarks/QUEUE_CONCURRENCY_TESTS.md)** - Zero-duplicate guarantees
- **[Testing Strategy](docs/development/TESTING.md)** - Test coverage and approach

### 🔧 Development & Planning
- **[Development Guide](docs/specs/DEVELOPMENT.md)** - Setup and contribution guide
- **[Design Decisions](docs/specs/DESIGN_DECISIONS.md)** - Technical choices
- **[Roadmap](docs/ROADMAP.md)** - Development roadmap and timeline
- **[Project DAG](docs/PROJECT_DAG.md)** - Component dependencies and implementation order
- **[Deployment](docs/specs/DEPLOYMENT.md)** - Production deployment
- **[Packaging](docs/specs/PACKAGING_AND_DISTRIBUTION.md)** - Distribution packages

### 🧩 Component Specifications

- **[Key-Value Store](docs/specs/KEY_VALUE_STORE.md)** - Sharded storage system
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

### ✅ Head-to-Head vs Redis 7 — live `redis-benchmark` (July 2026) ⚡

Both servers driven by the **same** `redis-benchmark` binary over RESP
(release build, containers on one Docker network, persistence and auth off on
both sides). Full methodology and history:
[`docs/benchmarks/redis-vs-synap.md`](docs/benchmarks/redis-vs-synap.md)

**Single hot key** (Redis's best case — sharding buys Synap nothing):

| Shape | Result |
|-------|--------|
| `-P 1` (per-op latency) | **Parity** — within ~5% of Redis on every command; Synap ahead on GET, RPUSH, LRANGE |
| `-P 16` (pipelined) | **0.85–0.95 of Redis** on GET/SET/SADD/INCR (~750–815k rps); Synap **wins LPUSH at 1.51×**, ties LRANGE |

**Randomized 1M keyspace** (the realistic shape — Synap's shards parallelize,
Redis stays serial):

| Op | Synap rps | Redis rps | Synap/Redis |
|---|---:|---:|---:|
| SET (c=50) | 797,872 | 700,935 | ✅ **1.14** |
| INCR (c=50) | 761,421 | 607,287 | ✅ **1.25** |
| LPUSH (c=50) | 641,026 | 513,699 | ✅ **1.25** |
| GET (c=50) | 742,574 | 789,474 | 0.94 |
| SET (c=200) | 742,000 | — | ✅ **1.19** |
| INCR (c=200) | 675,000 | — | ✅ **1.03** |

**Native SynapRPC** (Synap's own protocol; RESP3 exists for compatibility) is
**~3× faster per operation** than the RESP3 path — 166–170k rps vs ~56k at
`-P 1` — and ~2.8× faster than HTTP/JSON on a SET+GET round-trip.

**Memory**: 92 MB for 1M keys vs ~200 MB in Redis (**54% less**).

### 📈 What the v1.0 optimization rounds delivered

| Optimization | Effect |
|--------------|--------|
| `TCP_NODELAY` + buffered writes | Non-pipelined GET 1.1k → 56k rps (52×); pipelined 17k → 833k (48×) |
| Per-key write lock: mutex → sharded `RwLock` + `try_read` fast path | Pipelined SET 130k → 785k; c=200 writes went from 0.24× to 1.19× Redis |
| Zero-copy values (`Arc<[u8]>` end-to-end) | SET stores the parser's buffer by refcount bump; GET/MGET reach the socket without copying |
| Hot-path allocations removed (dispatch, replies, metrics, INCR in-place) | INCR 480k → ~800k rps; SADD 630k → ~800k |
| SIMD (AVX2/NEON) BITCOUNT/BITOP/PFMERGE | Up to 64 GiB/s popcount, 9.5× faster HLL merge |
| O(1) stream consume seek + per-queue deadline min-heap | Consume/ACK sweeps no longer scan whole buffers |

**Tests**: 1,800+ across the workspace • **Benchmarks**: 13 criterion suites + `synap-bench` load generator

## ⚖️ Comparison

| Feature | Synap | Redis | RabbitMQ | Kafka |
|---------|-------|-------|----------|-------|
| Key-Value | ✅ | ✅ | ❌ | ❌ |
| Hashes / Lists / Sets / Sorted Sets | ✅ | ✅ | ❌ | ❌ |
| Geospatial / Bitmaps / HyperLogLog | ✅ | ✅ | ❌ | ❌ |
| Lua Scripting | ✅ | ✅ | ❌ | ❌ |
| Transactions (MULTI/EXEC/WATCH) | ✅ (durable + replicated) | ✅ | ❌ | ❌ |
| Blocking Pops (BLPOP/BZPOPMIN…) | ✅ | ✅ | ❌ | ❌ |
| Keyspace Notifications | ✅ | ✅ | ❌ | ❌ |
| Queues (ACK) | ✅ | ❌ | ✅ | ❌ |
| Consumer Prefetch/QoS | ✅ | ❌ | ✅ | ❌ |
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
| Replication | ✅ (Master-Slave, all datatypes) | ✅ | ✅ | ✅ |
| Persistence | ✅ (WAL+Snapshot, CRC-verified) | ✅ (AOF/RDB) | ✅ (Disk) | ✅ (Log) |
| PACELC Model | PC/EL | PC/EL | PC/EC | PA/EL |
| Native Compression | ✅ (LZ4/Zstd) | ❌ | ❌ | ✅ (Snappy) |
| StreamableHTTP | ✅ | ❌ | ❌ | ❌ |
| MCP Support | ✅ (Configurable, Auth) | ❌ | ❌ | ❌ |
| UMICP Support | ✅ (13 ops, TLS) | ❌ | ❌ | ❌ |
| Enhanced Monitoring | ✅ (INFO, SLOWLOG, MEMORY, Prometheus) | ✅ (INFO) | ❌ | ❌ |
| Password Hashing | ✅ (bcrypt) | ✅ (SHA256) | ✅ (bcrypt) | ✅ (SASL) |
| AI Integration | ✅ (MCP+UMICP) | ❌ | ❌ | ❌ |
| Matrix Operations | ✅ (via UMICP) | ❌ | ❌ | ❌ |
| Single Binary | ✅ | ✅ | ❌ | ❌ |
| Zero-Duplicate Guarantee | ✅ (Tested) | N/A | ✅ | ✅ |

**Legend**: ✅ Implemented | 🔄 In Progress | ❌ Not Available

## 📄 License

Apache License 2.0 - See [LICENSE](LICENSE) for details.

## 🤝 Contributing

See [DEVELOPMENT.md](docs/specs/DEVELOPMENT.md) for development setup and contribution guidelines.

