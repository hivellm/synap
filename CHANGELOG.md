# Changelog

All notable changes to Synap will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added - Phase 2 Features (Q4 2025)

#### 🔐 Authentication & Authorization System
- **User Management** with bcrypt password hashing (DEFAULT_COST = 12)
  - Create/delete users with secure password storage
  - Enable/disable user accounts
  - Last login tracking
  - Password change capability
  - Case-sensitive usernames

- **Role-Based Access Control (RBAC)**
  - Built-in roles: `admin`, `readonly`
  - Custom role creation with fine-grained permissions
  - Permission patterns with wildcards (`*`, `prefix:*`)
  - Actions: Read, Write, Delete, Admin, All
  - Role assignment to users

- **API Key Management**
  - Auto-generated secure keys (32-char, `sk_` prefix)
  - Configurable expiration (days from creation)
  - IP address filtering/whitelisting
  - Usage tracking (count + last_used_at)
  - Enable/disable without deletion
  - Automatic cleanup of expired keys

- **Access Control Lists (ACL)**
  - Resource types: Queue, KV, Stream, PubSub, Admin
  - Rule-based access control
  - Public and authenticated rules
  - User and role-based restrictions
  - Wildcard pattern matching

- **Authentication Methods**
  - HTTP Basic Auth (Redis-style: `username:password@host`)
  - Bearer Token (API Key in Authorization header)
  - Query parameter API keys (`?api_key=sk_XXX`)
  - Client IP extraction and validation

- **Security Features**
  - Optional authentication (disabled by default)
  - Mandatory for 0.0.0.0 binding (production)
  - Multi-tenant isolation via permissions
  - Audit-ready (usage tracking, last login)
  - Production-ready security

#### 📦 Queue System (Phase 2 Week 1-3)
- **Core Queue Implementation**
  - FIFO with priority support (0-9, 9 = highest)
  - ACK/NACK mechanism for reliable delivery
  - Configurable retry logic (max_retries)
  - Dead Letter Queue (DLQ) for failed messages
  - Background deadline checker (1s interval)
  - Pending message tracking

- **9 REST API Endpoints**
  - POST `/queue/:name` - Create queue with custom config
  - POST `/queue/:name/publish` - Publish messages
  - GET `/queue/:name/consume/:consumer_id` - Consume messages
  - POST `/queue/:name/ack` - Acknowledge processing
  - POST `/queue/:name/nack` - Negative acknowledge (retry/DLQ)
  - GET `/queue/:name/stats` - Queue statistics
  - POST `/queue/:name/purge` - Clear all messages
  - DELETE `/queue/:name` - Delete queue
  - GET `/queue/list` - List all queues

- **Concurrency Protection (Zero Duplicates)**
  - Thread-safe RwLock implementation
  - Atomic message consumption (pop_front)
  - 5 comprehensive concurrency tests
  - Tested with 10-50 concurrent consumers
  - 100-1000 messages per test scenario
  - **ZERO duplicates** detected across all scenarios
  - Performance: ~7,500 msg/s with high concurrency

#### 🗜️ Compression System
- **LZ4 Compression** (fast, low CPU)
- **Zstandard (Zstd)** (better ratio, configurable level)
- Configurable minimum payload size
- Compression ratio tracking
- 6 comprehensive tests

#### 📊 Advanced Features
- **Advanced Logging** with tracing-subscriber
  - JSON format (structured logging for production)
  - Pretty format (colored output for development)
  - File/line number tracking
  - Thread ID and name tracking
  - Span context support

- **Configuration System**
  - YAML-based (Redis-compatible style)
  - Multiple config files (dev, prod, example)
  - CLI argument overrides
  - Environment variable support
  - Comprehensive inline documentation

- **Synap CLI** (Redis-compatible client)
  - Interactive REPL mode with rustyline
  - 18+ Redis-compatible commands
  - Colored output with timing
  - Command history and completion
  - Full documentation in docs/CLI_GUIDE.md

- **Extended KV Commands**
  - KEYS, DBSIZE, FLUSHDB/FLUSHALL
  - EXPIRE, TTL, PERSIST
  - SCAN with prefix matching

### Changed

- **Architecture**: Introduced `AppState` for shared resources (KVStore + QueueManager)
- **Router**: Updated to support multiple subsystems
- **Config**: Added queue, authentication, ACL, and rate_limit sections
- **Dependencies**: Added bcrypt, chrono, base64, rand for security
- **Edition**: Rust 2024 with nightly toolchain

### Tests

**Total: 96 tests passing** ✅
- 35 unit tests (21 KV + 14 Queue)
- 23 authentication tests (users, roles, API keys, ACL)
- 8 integration tests
- 10 S2S REST tests
- 20 S2S StreamableHTTP tests

**Coverage**: ~92% (comprehensive security and concurrency coverage)

### Documentation

- 📄 `docs/AUTHENTICATION.md` - Complete authentication guide
- 📄 `docs/QUEUE_CONCURRENCY_TESTS.md` - Concurrency test documentation
- 📄 `docs/BENCHMARK_RESULTS.md` - Performance benchmarks
- 📄 `docs/CLI_GUIDE.md` - CLI usage guide
- 📄 `docs/CONFIGURATION.md` - Configuration reference
- 📄 `docs/TESTING.md` - Testing strategy
- 📄 `docs/PHASE1_SUMMARY.md` - Phase 1 implementation summary

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

- In-memory only (persistence planned Phase 2 Week 10-12)
- No replication support (planned Phase 3)
- No WebSocket support (planned Phase 2)
- Rate limiting temporarily disabled (implementation in progress)
- TLS/SSL via reverse proxy only (nginx, Caddy)
- Single-node deployment (clustering planned Phase 5)

These limitations will be addressed in future phases.

---

## Future Releases

### [0.2.0-beta] - In Progress (Q4 2025)
- ✅ Queue System (FIFO with ACK/NACK, priorities, DLQ)
- ✅ Authentication & Authorization (users, roles, API keys, ACL)
- ✅ Compression (LZ4/Zstd)
- ✅ Queue REST API (9 endpoints)
- ✅ Concurrency protection (zero duplicates)
- 🔄 Event Streams (in progress)
- 🔄 Pub/Sub Router (planned)
- 🔄 Persistence Layer (planned)
- 🔄 WebSocket support (planned)

### [0.3.0-rc] - Planned Q1 2026
- Master-Slave Replication (auth structure ready)
- L1/L2 Cache System
- MCP Protocol Integration
- UMICP Protocol Integration
- TCP Protocol Support
- Rate Limiting (governor crate)

### [1.0.0] - Planned Q2 2026
- Production hardening
- ✅ Security features (Auth, TLS via proxy, RBAC)
- Distribution packages (MSI, DEB, Homebrew)
- GUI Dashboard
- Complete documentation
- Performance tuning
- Chaos engineering tests

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

