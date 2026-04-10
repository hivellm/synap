# Synap Documentation Index

## Quick Links

- **[README](README.md)** - Project overview and quick start
- **[ARCHITECTURE](ARCHITECTURE.md)** - System architecture and design
- **[API Reference](api/REST_API.md)** - Complete API documentation
- **[Documentation Organization](ORGANIZATION.md)** - How documentation is structured

## Getting Started

1. [Project Overview](README.md) - Features, use cases, and quick start
2. [Design Decisions](DESIGN_DECISIONS.md) - Technical choices and rationale
3. [Architecture](ARCHITECTURE.md) - System components and data flow
4. [Configuration](specs/CONFIGURATION.md) - Configuration reference
5. [Development](specs/DEVELOPMENT.md) - Setup and development guide

## Core Specifications

### Components
- **[Key-Value Store](specs/KEY_VALUE_STORE.md)** - Radix-tree based in-memory storage
- **[Queue System](specs/QUEUE_SYSTEM.md)** - RabbitMQ-style message queues
- **[Event Stream](specs/EVENT_STREAM.md)** - Kafka-style room broadcasting
- **[Pub/Sub](specs/PUBSUB.md)** - Topic-based publish/subscribe
- **[Replication](specs/REPLICATION.md)** - Master-slave replication
- **[Persistence](specs/PERSISTENCE.md)** - WAL and snapshot system

### Protocol & API
- **[StreamableHTTP Protocol](protocol/STREAMABLE_HTTP.md)** - Protocol specification
- **[MCP Integration](protocol/MCP_INTEGRATION.md)** - Model Context Protocol for AI tools
- **[UMICP Integration](protocol/UMICP_INTEGRATION.md)** - Universal Matrix Inter-Communication Protocol
- **[REST API Reference](api/REST_API.md)** - HTTP endpoints
- **[Protocol Messages](api/PROTOCOL_MESSAGES.md)** - Message formats

### Performance & Optimization
- **[Compression & Cache](specs/COMPRESSION_AND_CACHE.md)** - Smart compression and hot data caching
- **[Performance](specs/PERFORMANCE.md)** - Benchmarks and targets
- **[Optimization](specs/OPTIMIZATION.md)** - Optimization strategies
- **[Performance Optimizations](specs/PERFORMANCE_OPTIMIZATIONS.md)** - Advanced optimization techniques

## Client SDKs

- **[TypeScript SDK](sdks/TYPESCRIPT.md)** - Node.js and browser support
- **[Python SDK](sdks/PYTHON.md)** - Async and sync Python client
- **[Rust SDK](sdks/RUST.md)** - Native Rust client library

## Examples

- **[Real-Time Chat](examples/CHAT_SAMPLE.md)** - Multi-room chat application
- **[Event Broadcasting](examples/EVENT_BROADCAST.md)** - System-wide events
- **[Task Queue](examples/TASK_QUEUE.md)** - Distributed task processing
- **[Pub/Sub Patterns](examples/PUBSUB_PATTERN.md)** - Notification systems

## Benchmarks

- **[Benchmark Results](benchmarks/BENCHMARK_RESULTS_EXTENDED.md)** - Extended performance benchmarks
- **[Persistence Benchmarks](benchmarks/PERSISTENCE_BENCHMARKS.md)** - WAL and snapshot performance
- **[Queue Concurrency Tests](benchmarks/QUEUE_CONCURRENCY_TESTS.md)** - Queue system concurrency analysis

## Architecture Diagrams

- **[System Architecture](diagrams/system-architecture.mmd)** - Overall system design
- **[Replication Flow](diagrams/replication-flow.mmd)** - Master-slave replication
- **[Event Stream](diagrams/event-stream.mmd)** - Event streaming architecture
- **[Message Flow](diagrams/message-flow.mmd)** - Message flow between components

## Operations

- **[Configuration](specs/CONFIGURATION.md)** - Configuration reference
- **[Deployment](specs/DEPLOYMENT.md)** - Deployment guide
- **[Packaging & Distribution](specs/PACKAGING_AND_DISTRIBUTION.md)** - Build installers for all platforms
- **[Build Guide](BUILD.md)** - Build instructions
- **[Testing](TESTING.md)** - Testing guide

## User Interface

- **[GUI Dashboard](specs/GUI_DASHBOARD.md)** - Electron-based desktop application (planned)
- **[CLI Guide](CLI_GUIDE.md)** - Command-line interface documentation

## Project Planning

- **[Roadmap](ROADMAP.md)** - Development roadmap and release timeline
- **[Project DAG](PROJECT_DAG.md)** - Component dependencies and critical path
- **[Contributing](../CONTRIBUTING.md)** - Contribution guidelines

## By Use Case

### Building a Chat Application
1. [Event Stream Specification](specs/EVENT_STREAM.md)
2. [Chat Sample Implementation](examples/CHAT_SAMPLE.md)
3. [TypeScript SDK](sdks/TYPESCRIPT.md) or [Python SDK](sdks/PYTHON.md)

### Task Processing System
1. [Queue System Specification](specs/QUEUE_SYSTEM.md)
2. [Task Queue Example](examples/TASK_QUEUE.md)
3. [Rust SDK](sdks/RUST.md) for high-performance workers

### Caching Layer
1. [Key-Value Store Specification](specs/KEY_VALUE_STORE.md)
2. [REST API Reference](api/REST_API.md)
3. SDK of choice for your stack

### Event-Driven Architecture
1. [Pub/Sub Specification](specs/PUBSUB.md)
2. [Event Broadcasting Example](examples/EVENT_BROADCAST.md)
3. [Pub/Sub Patterns](examples/PUBSUB_PATTERN.md)

### AI/ML Integration
1. [MCP Integration](protocol/MCP_INTEGRATION.md) - AI tool integration
2. [UMICP Integration](protocol/UMICP_INTEGRATION.md) - Matrix operations
3. Use Synap as context store for AI agents
4. Vector similarity search with UMICP

### High Availability Deployment
1. [Replication Specification](specs/REPLICATION.md)
2. [Deployment Guide](specs/DEPLOYMENT.md)
3. [Configuration Reference](specs/CONFIGURATION.md)

## By Component

### Key-Value Store
- [Specification](specs/KEY_VALUE_STORE.md)
- [API Reference](api/REST_API.md#key-value-store-api)
- [Performance](specs/PERFORMANCE.md#scenario-1-key-value-workload)

### Queue System
- [Specification](specs/QUEUE_SYSTEM.md)
- [API Reference](api/REST_API.md#queue-system-api)
- [Task Queue Example](examples/TASK_QUEUE.md)
- [Performance](specs/PERFORMANCE.md#scenario-2-queue-processing)

### Event Stream
- [Specification](specs/EVENT_STREAM.md)
- [API Reference](api/REST_API.md#event-stream-api)
- [Chat Example](examples/CHAT_SAMPLE.md)
- [Architecture Diagram](diagrams/event-stream.mmd)

### Pub/Sub
- [Specification](specs/PUBSUB.md)
- [API Reference](api/REST_API.md#pubsub-api)
- [Patterns Example](examples/PUBSUB_PATTERN.md)

### Replication
- [Specification](specs/REPLICATION.md)
- [Flow Diagram](diagrams/replication-flow.mmd)
- [Deployment Guide](specs/DEPLOYMENT.md#2-master-slave-production)

## Quick Reference

### API Endpoints

```
POST /api/v1/command        # All commands
GET  /api/v1/ws             # WebSocket upgrade
GET  /health                # Health check
GET  /metrics               # Prometheus metrics
```

### Command Namespaces

```
kv.*        → Key-Value operations
queue.*     → Queue operations
stream.*    → Event stream operations
pubsub.*    → Pub/Sub operations
admin.*     → Administrative operations
```

### SDKs Installation

```bash
# TypeScript
npm install @hivehub/synap

# Python
pip install synap-client

# Rust
cargo add synap-client
```

## Documentation Status

| Section | Files | Status | Last Updated |
|---------|-------|--------|--------------|
| Overview | 1 | ✅ Complete | 2025-10-16 |
| Architecture | 3 | ✅ Complete | 2025-10-16 |
| Core Specs | 16 | ✅ Complete | 2025-10-21 |
| Protocol | 3 | ✅ Complete | 2025-10-16 |
| API Reference | 2 | ✅ Complete | 2025-10-15 |
| SDKs | 3 | ✅ Complete | 2025-10-15 |
| Examples | 4 | ✅ Complete | 2025-10-15 |
| Diagrams | 4 | ✅ Complete | 2025-10-15 |
| Benchmarks | 3 | ✅ Complete | 2025-10-21 |
| Operations | 5 | ✅ Complete | 2025-10-21 |
| User Interface | 2 | ✅ Complete | 2025-10-16 |
| Project Planning | 2 | ✅ Complete | 2025-10-16 |

**Total Documentation**: 48 files, organized and consolidated

### Key Additions
- ✅ **Persistence System**: WAL + Snapshot specification added
- ✅ **PACELC Classification**: PC/EL model documented
- ✅ **Recovery Procedures**: Crash recovery and backup strategies
- ✅ **Durability Modes**: Multiple persistence configurations
- ✅ **MCP Integration**: Model Context Protocol support for AI tools
- ✅ **UMICP Integration**: Universal Matrix Inter-Communication Protocol
- ✅ **Compression System**: LZ4/Zstd dual compression strategy
- ✅ **Cache System**: L1/L2 tiered hot data caching
- ✅ **Packaging System**: MSI (Windows), DEB (Linux), Homebrew (macOS)
- ✅ **Build Scripts**: Automated build scripts for all platforms
- ✅ **GUI Dashboard**: Electron-based desktop application specification
- ✅ **Roadmap**: 5-phase development plan with milestones
- ✅ **Project DAG**: Component dependencies and critical path analysis

## Contributing

See [DEVELOPMENT.md](specs/DEVELOPMENT.md) for contribution guidelines.

## Project Status

**Phase**: Documentation Complete  
**Next Phase**: Implementation  
**Version**: 0.1.0-alpha

