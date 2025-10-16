# Synap

**High-Performance In-Memory Key-Value Store & Message Broker**

Synap is a modern, high-performance data infrastructure system built in Rust, combining the best features of Redis, RabbitMQ, and Kafka into a unified platform for real-time applications.

## Overview

Synap provides four core capabilities in a single, cohesive system:

1. **Memory Key-Value Store** - Radix-tree based in-memory storage with O(k) lookup
2. **Acknowledgment Queues** - RabbitMQ-style message queues with delivery guarantees
3. **Event Streams** - Kafka-style room-based broadcasting with message history
4. **Pub/Sub Messaging** - Topic-based publish/subscribe with wildcard support

## Key Features

### Performance
- **Sub-millisecond Operations**: < 1ms for key-value operations
- **High Throughput**: 100K+ ops/sec per core
- **Efficient Memory**: Radix-tree provides memory-efficient key storage
- **Async I/O**: Built on Tokio for non-blocking operations
- **Smart Compression**: LZ4/Zstd compression with minimal CPU overhead
- **Hot Data Cache**: Decompressed cache for frequently accessed data

### Durability
- **Optional Persistence**: WAL + Snapshots for crash recovery
- **Replication**: Master-slave for data redundancy
- **PACELC Model**: PC/EL (Consistency during partition, Latency in normal operation)
- **Recovery Time**: 1-10 seconds from snapshots

### Reliability
- **Master-Slave Replication**: 1 write master + N read replicas
- **Message Acknowledgment**: Guaranteed message delivery with ACK/NACK
- **Event Replay**: Stream history and replay capabilities
- **Automatic Failover**: Manual promotion with documented procedures

### Developer Experience
- **StreamableHTTP Protocol**: Simple HTTP-based streaming protocol
- **WebSocket Support**: Persistent connections for real-time updates
- **Multi-language SDKs**: TypeScript, Python, and Rust clients
- **Rich Examples**: Chat, event broadcasting, task queues, and more

### Protocol Support
- **MCP (Model Context Protocol)**: Integration with AI tools and agents
- **UMICP (Universal Matrix Inter-Communication Protocol)**: Matrix operations and federated communication
- **REST API**: Standard HTTP endpoints for all operations
- **WebSocket API**: Real-time bidirectional communication

### Scalability
- **Read Scaling**: Multiple replica nodes for distributed reads
- **Event Rooms**: Isolated event streams per room/channel
- **Topic Routing**: Efficient pub/sub with wildcard matching
- **Connection Pooling**: Client-side connection management

## Architecture

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

## Quick Start

### Installation

**From Package Managers**:

```bash
# Windows (MSI Installer)
# Download from https://github.com/hivellm/synap/releases
synap-0.1.0-x86_64.msi

# Linux (Debian/Ubuntu)
curl -fsSL https://packages.synap.io/gpg.key | sudo apt-key add -
echo "deb https://packages.synap.io/apt stable main" | sudo tee /etc/apt/sources.list.d/synap.list
sudo apt-get update && sudo apt-get install synap

# macOS (Homebrew)
brew tap hivellm/synap
brew install synap

# Docker
docker pull hivellm/synap:latest
docker run -d -p 15500:15500 hivellm/synap:latest
```

**From Source**:

```bash
# Clone repository
git clone https://github.com/hivellm/synap.git
cd synap

# Build from source
cargo build --release --features full

# Run server
./target/release/synap-server --config config.yml
```

See [Packaging & Distribution](docs/PACKAGING_AND_DISTRIBUTION.md) for detailed installation instructions.

### Basic Usage

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

## Use Cases

### Real-Time Chat
Use event streams for room-based messaging with message history and guaranteed delivery.

### Task Distribution
Leverage acknowledgment queues for distributed task processing with retry logic.

### Cache Layer
Utilize key-value store as a high-speed cache with TTL support.

### Event Broadcasting
Implement pub/sub for system-wide notifications and event distribution.

### Microservices Communication
Use queues for reliable inter-service messaging with delivery guarantees.

## Technology Stack

- **Language**: Rust (Edition 2024)
- **Runtime**: Tokio (async/await)
- **Web Framework**: Axum
- **Data Structure**: radix_trie (memory-efficient key-value)
- **Serialization**: serde (JSON, MessagePack)
- **Protocols**: StreamableHTTP + WebSocket + MCP + UMICP

## Documentation

- **[Architecture](docs/ARCHITECTURE.md)** - System architecture and components
- **[Design Decisions](docs/DESIGN_DECISIONS.md)** - Technical choices and rationale
- **[API Reference](docs/api/REST_API.md)** - Complete REST API documentation
- **[Protocol Specification](docs/protocol/STREAMABLE_HTTP.md)** - StreamableHTTP protocol
- **[MCP Integration](docs/protocol/MCP_INTEGRATION.md)** - Model Context Protocol support
- **[UMICP Integration](docs/protocol/UMICP_INTEGRATION.md)** - UMICP protocol support
- **[Compression & Cache](docs/COMPRESSION_AND_CACHE.md)** - Smart compression and caching
- **[Performance](docs/PERFORMANCE.md)** - Benchmarks and optimization
- **[Development Guide](docs/DEVELOPMENT.md)** - Setup and contribution guide
- **[Deployment](docs/DEPLOYMENT.md)** - Production deployment strategies
- **[Packaging & Distribution](docs/PACKAGING_AND_DISTRIBUTION.md)** - Build MSI, DEB, Homebrew packages
- **[GUI Dashboard](docs/GUI_DASHBOARD.md)** - Electron-based desktop application (planned)

### Project Planning

- **[Roadmap](docs/ROADMAP.md)** - Development roadmap and timeline
- **[Project DAG](docs/PROJECT_DAG.md)** - Component dependencies and implementation order

### Component Specifications

- **[Key-Value Store](docs/specs/KEY_VALUE_STORE.md)** - Radix-tree storage system
- **[Queue System](docs/specs/QUEUE_SYSTEM.md)** - Message queues with ACK
- **[Event Stream](docs/specs/EVENT_STREAM.md)** - Room-based broadcasting
- **[Pub/Sub](docs/specs/PUBSUB.md)** - Topic-based messaging
- **[Replication](docs/specs/REPLICATION.md)** - Master-slave architecture

### SDKs

- **[TypeScript SDK](docs/sdks/TYPESCRIPT.md)** - Node.js and browser support
- **[Python SDK](docs/sdks/PYTHON.md)** - Async/sync Python client
- **[Rust SDK](docs/sdks/RUST.md)** - Native Rust client library

### Examples

- **[Real-Time Chat](docs/examples/CHAT_SAMPLE.md)** - Multi-room chat application
- **[Event Broadcasting](docs/examples/EVENT_BROADCAST.md)** - System-wide events
- **[Task Queue](docs/examples/TASK_QUEUE.md)** - Distributed task processing
- **[Pub/Sub Pattern](docs/examples/PUBSUB_PATTERN.md)** - Notification system

## Performance Goals

| Operation | Target | Notes |
|-----------|--------|-------|
| KV Get | < 0.5ms | Single key lookup |
| KV Set | < 1ms | Including persistence log |
| Queue Publish | < 2ms | With durability guarantee |
| Queue Consume | < 1ms | Single message |
| Event Publish | < 1ms | Single room broadcast |
| Pub/Sub Publish | < 0.5ms | Topic routing |
| Replication Lag | < 10ms | Master to replica |

## Comparison

| Feature | Synap | Redis | RabbitMQ | Kafka |
|---------|-------|-------|----------|-------|
| Key-Value | ✅ | ✅ | ❌ | ❌ |
| Queues (ACK) | ✅ | ❌ | ✅ | ❌ |
| Event Streams | ✅ | ✅ (Limited) | ❌ | ✅ |
| Pub/Sub | ✅ | ✅ | ✅ | ✅ |
| Replication | ✅ | ✅ | ✅ | ✅ |
| Persistence | ✅ (WAL+Snapshot) | ✅ (AOF/RDB) | ✅ (Disk) | ✅ (Log) |
| PACELC Model | PC/EL | PC/EL | PC/EC | PA/EL |
| Native Compression | ✅ (LZ4/Zstd) | ❌ | ❌ | ✅ (Snappy) |
| Hot Data Cache | ✅ (L1/L2) | ✅ (Single) | ❌ | ❌ |
| StreamableHTTP | ✅ | ❌ | ❌ | ❌ |
| MCP Support | ✅ | ❌ | ❌ | ❌ |
| UMICP Support | ✅ | ❌ | ❌ | ❌ |
| AI Integration | ✅ | ❌ | ❌ | ❌ |
| Matrix Operations | ✅ | ❌ | ❌ | ❌ |
| Single Binary | ✅ | ✅ | ❌ | ❌ |

## License

MIT License - See LICENSE for details

## Contributing

See [DEVELOPMENT.md](docs/DEVELOPMENT.md) for development setup and contribution guidelines.

## Project Status

**Status**: Documentation Phase  
**Version**: 0.1.0-alpha  
**Last Updated**: October 16, 2025

This project is currently in the design and specification phase. Implementation will begin after documentation review and approval.

### Documentation Complete ✅

- ✅ Core architecture and design decisions
- ✅ Component specifications (KV, Queue, Stream, Pub/Sub)
- ✅ Multi-protocol support (HTTP, MCP, UMICP)
- ✅ Compression system (LZ4/Zstd)
- ✅ Cache system (L1/L2 tiered)
- ✅ Replication and persistence
- ✅ Packaging system (MSI, DEB, Homebrew)
- ✅ Build automation scripts
- ✅ GUI dashboard specification (Electron)
- ✅ Project roadmap (5 phases, Q1-Q4 2025)
- ✅ Component dependency graph (DAG)
- ✅ SDK specifications
- ✅ API documentation
- ✅ Performance benchmarks

**Total**: 28 documentation files, ~23,400 lines

