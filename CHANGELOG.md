# Changelog

All notable changes to Synap will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Advanced Logging** with tracing-subscriber
  - JSON format support (structured logging for production)
  - Pretty format support (colored output for development)
  - File/line number tracking
  - Thread ID and name tracking
  - Span context support
  - Configurable via `config.yml`

- **Configuration System**
  - YAML-based configuration (Redis-compatible style)
  - Multiple config files (dev, prod, example)
  - CLI argument overrides (--config, --host, --port)
  - Environment variable support (RUST_LOG)
  - Comprehensive inline documentation

- **Synap CLI** (Redis-compatible client)
  - Interactive REPL mode with rustyline
  - Command mode for scripting
  - 18 Redis-compatible commands
  - Colored output with timing information
  - Command history and completion
  - Full documentation in docs/CLI_GUIDE.md

- **New KV Commands**
  - KEYS - List all keys
  - DBSIZE - Get database size
  - FLUSHDB/FLUSHALL - Clear database
  - EXPIRE - Set key expiration
  - PERSIST - Remove key expiration

### Changed

- Updated tracing-subscriber to 0.3 with JSON and env-filter features
- Enhanced main.rs with configurable logging
- Updated AGENTS.md with Context7 dependency check rule

## [0.1.0-alpha] - 2025-10-21

### Added

#### Core Features
- **Key-Value Store** with radix trie implementation
  - GET, SET, DELETE operations
  - TTL support with background cleanup
  - Atomic operations (INCR, DECR)
  - Batch operations (MSET, MGET, MDEL)
  - Prefix SCAN capability
  - Memory tracking and statistics

#### HTTP REST API
- POST `/kv/set` - Store key-value pair
- GET `/kv/get/:key` - Retrieve value
- DELETE `/kv/del/:key` - Delete key
- GET `/kv/stats` - Get store statistics
- GET `/health` - Health check endpoint

#### StreamableHTTP Protocol
- POST `/api/v1/command` - Command routing endpoint
- Supported commands:
  - `kv.set`, `kv.get`, `kv.del`, `kv.exists`
  - `kv.incr`, `kv.decr`
  - `kv.mset`, `kv.mget`, `kv.mdel`
  - `kv.scan`, `kv.stats`
- Request/Response envelope pattern
- UUID request tracking

#### Infrastructure
- Rust Edition 2024 support
- Tokio async runtime
- Axum web framework
- Comprehensive error handling
- Structured logging with tracing
- CORS and request tracing middleware

#### Testing
- 11 unit tests for core KV operations
- 8 integration tests for HTTP API
- TTL expiration testing
- Batch operations testing
- StreamableHTTP protocol testing

#### Documentation
- Complete architecture documentation
- API reference guide
- Build instructions
- Configuration reference
- Performance benchmarks setup

### Technical Details

- **Rust Version**: 1.85+ (nightly)
- **Edition**: 2024
- **Dependencies**:
  - tokio 1.35
  - axum 0.7
  - radix_trie 0.2
  - parking_lot 0.12
  - serde 1.0
  - tracing 0.1

### Performance

- Memory-efficient radix tree storage
- Sub-millisecond operation latency (target)
- Concurrent request handling with Tokio
- Efficient RwLock from parking_lot

### Known Limitations

- In-memory only (no persistence yet)
- No replication support
- No authentication/authorization
- No TLS/SSL support
- No WebSocket support
- Single-node deployment only

These limitations will be addressed in future phases.

---

## Future Releases

### [0.2.0-beta] - Planned Q2 2025
- Queue System (FIFO with ACK/NACK)
- Event Streams (room-based broadcasting)
- Pub/Sub Router (topic-based messaging)
- Persistence Layer (WAL + Snapshots)
- WebSocket support

### [0.3.0-rc] - Planned Q3 2025
- Master-Slave Replication
- Compression (LZ4/Zstd)
- L1/L2 Cache System
- MCP Protocol Integration
- UMICP Protocol Integration
- TCP Protocol Support

### [1.0.0] - Planned Q4 2025
- Production hardening
- Security features (Auth, TLS, RBAC)
- Distribution packages (MSI, DEB, Homebrew)
- GUI Dashboard
- Complete documentation

---

**Legend**:
- 🆕 New feature
- 🔧 Improvement
- 🐛 Bug fix
- 🗑️ Deprecation
- 🔥 Breaking change
- 📝 Documentation
- 🔒 Security

[Unreleased]: https://github.com/hivellm/synap/compare/v0.1.0-alpha...HEAD
[0.1.0-alpha]: https://github.com/hivellm/synap/releases/tag/v0.1.0-alpha

