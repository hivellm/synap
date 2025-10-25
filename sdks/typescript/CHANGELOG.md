# Changelog - @hivellm/synap

All notable changes to the Synap TypeScript SDK will be documented in this file.

## [0.2.0-beta.3] - 2025-10-24

### Fixed
- **CRITICAL**: Fixed pub/sub publish API contract - now sends `payload` field instead of incorrect `data` field
- **CRITICAL**: Fixed server queue consume to gracefully return `null` for non-existent queues instead of throwing error

### Added
- 8 new pub/sub unit tests covering API contract validation
- 10 new pub/sub S2S integration tests
- Tests for different payload types (string, number, object, array, null)
- Tests for edge cases (large payloads, rapid publishing, concurrent operations)

### Changed
- Total test count: 123 tests (was 115)
- All tests passing: 123/123 (100%)

### Impact
- Fixes compatibility issues with external projects (e.g., cmmv-queue)
- Prevents regression of pub/sub API contract
- Ensures queue operations handle missing queues gracefully

## [0.2.0-beta.2] - 2025-10-21

### Changed
- **BREAKING**: Package name changed from `@synap/client` to `@hivellm/synap`
- **BREAKING**: `mset()` now returns `boolean` instead of `number`
- Fixed `get()` to return `null` instead of `undefined` for non-existent keys
- Fixed `mset()`, `mget()`, `mdel()` batch operations to use correct server format
- Fixed `expire()` to use `ttl` parameter name instead of `seconds`
- Fixed `persist()` to correctly parse server response
- Fixed `mget()` to convert array response to object keyed by original keys

### Added
- **Full Queue System Support**: All queue operations now working via StreamableHTTP protocol
  - `createQueue()`, `deleteQueue()`, `listQueues()`
  - `publish()`, `consume()`, `publishString()`, `publishJSON()`, `consumeString()`, `consumeJSON()`
  - `ack()`, `nack()`
  - `stats()`, `purge()`
  - Priority-based message delivery
  - Message retry mechanism with configurable retries
- Full test coverage: **35 tests passing** (KV: 23, Queue: 12)

## [0.2.0-beta.1] - 2025-10-21

### Added

#### Core Features
- **StreamableHTTP Client** - Full protocol implementation
- **Key-Value Store** - Complete KV operations
  - GET, SET, DELETE, EXISTS
  - INCR, DECR (atomic operations)
  - MSET, MGET, MDEL (batch operations)
  - SCAN, KEYS (discovery)
  - EXPIRE, TTL, PERSIST (expiration management)
  - FLUSHDB, FLUSHALL, DBSIZE
  - Full statistics support

#### Queue System
- **Message Queue Client** - Production-ready queue operations
  - createQueue with custom configuration
  - publish/publishString/publishJSON
  - consume/consumeString/consumeJSON  
  - ACK/NACK support
  - Priority messaging (0-9)
  - Retry logic with Dead Letter Queue
  - Queue statistics
  - List, purge, delete operations

#### Authentication
- **Basic Auth** - Username/password authentication
- **API Key Auth** - Bearer token support
- Automatic header injection

#### Developer Experience
- **Full TypeScript** support with complete type definitions
- **ESM + CJS** dual package
- **Zero dependencies** (except uuid)
- **Error classes** (SynapError, NetworkError, ServerError, TimeoutError)
- **Debug mode** for request/response logging
- **Gzip compression** support (automatic)
- **Browser compatible** (ES2022+)

#### Testing
- Vitest test framework
- Coverage reporting
- Example tests for KV and Queue modules

#### Documentation
- Complete README with examples
- API reference
- Queue worker example
- TypeScript usage examples

### Technical Details

**Dependencies**:
- uuid: ^10.0.0 (only runtime dependency)

**Dev Dependencies**:
- TypeScript 5.7+
- Vitest 2.1+ (testing)
- tsup 8.3+ (bundler)
- ESLint + Prettier (code quality)

**Node.js**: 18+ required  
**TypeScript**: 5.0+ recommended

### Protocol

Uses **StreamableHTTP** protocol:
- Request/Response envelope pattern
- UUID request tracking
- JSON payload serialization
- Gzip compression support

### Package Format

- **CJS**: CommonJS for Node.js require()
- **ESM**: ES Modules for import/export
- **Types**: Full .d.ts type definitions
- **Source maps**: Available for debugging

---

## Future Releases

### [0.3.0] - Planned
- Event Streams support
- Pub/Sub operations
- WebSocket client
- Connection pooling
- Retry strategies
- Request interceptors

### [1.0.0] - Planned
- Production hardening
- Performance optimizations
- Complete test suite
- Advanced features

---

[0.2.0-beta.1]: https://github.com/hivellm/synap/releases/tag/typescript-v0.2.0-beta.1

