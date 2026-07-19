# Changelog - @hivehub/synap

All notable changes to the Synap TypeScript SDK will be documented in this file.

## [Unreleased]

## [1.1.0] - 2026-07-19

### Changed
- **The `synap://` transport now runs on [Thunder](https://github.com/hivellm/thunder)**
  (`@hivehub/thunder` 0.2.1), the family's shared binary RPC client — the same
  protocol implementation the Synap server runs, so the two ends of the wire
  cannot drift.

### Added
- Credentials travel in the RPC handshake. The previous transport never sent
  `AUTH`, so it could not reach a `require_auth` server on 15501 at all.

### Fixed
- **Binary values survive the round trip.** `Bytes` were decoded as UTF-8
  unconditionally, so every invalid sequence became U+FFFD and `deadbeef` came
  back as `adfdfd` — corrupted and unrecoverable. Bytes that are valid UTF-8
  still decode to a string; anything else stays a `Buffer`.

## [1.0.0] - 2026-07-11

### Fixed
- **`kv.set` leaked non-string values as `"[object Object]"`.** The command
  mapper coerced objects with `String()`; non-string values are now
  JSON-encoded on set, matching `kv.get`'s documented auto-parse — objects,
  arrays, numbers and booleans round-trip transparently.
- Auth rejection tests probe whether the target server enforces auth and skip
  against auth-disabled dev servers.

### Changed
- Version aligned with the Synap server 1.0.0 release. SynapRPC (`synap://host:15501`) is the default transport; RESP3 and HTTP remain available via URL scheme. Test suite verified against the official `hivehub/synap:1.0.0` image.
- Dev dependencies refreshed (`typescript-eslint` 8.63, vitest/eslint/prettier
  minors); stale `@types/uuid` removed (uuid ships its own types).
  TypeScript stays on 6.x (7.x is the new native-compiler major).

## [0.11.0] - 2026-04-09

### Added

- **URL-scheme transport selection**: `SynapClient` constructor now accepts a
  plain URL string whose scheme determines the transport:
  - `synap://host:port` → SynapRPC (recommended default)
  - `resp3://host:port` → RESP3
  - `http://` / `https://` → HTTP/REST
- **Full command parity on SynapRPC + RESP3**: queue, stream, pub/sub,
  transaction, script, geospatial, and HyperLogLog commands mapped in
  `mapCommand` / `mapResponse`; 364 unit tests pass.
- **`UnsupportedCommandError`**: thrown for commands not mapped on the active
  native transport. Exported from `@hivehub/synap` index.
- **Reactive pub/sub over SynapRPC**: `SynapRpcTransport.subscribePush()`
  opens a dedicated TCP socket and relays push frames (`id == 0xFFFFFFFF`)
  to the subscriber callback.
- **E2E suites extended**: `runQueueSuite`, `runStreamSuite`,
  `runPubSubSuite`, `runTransactionSuite`, `runScriptSuite` added;
  `UnsupportedCommandError` regression (3 cases: RPC/RESP3 raise, HTTP
  succeeds).

### Changed

- **Constructor options deprecated**: `transport`, `rpcHost`, `rpcPort`,
  `resp3Host`, `resp3Port` options on `SynapClient` / `Synap` are marked
  `@deprecated`. Migrate to URL-scheme construction. Will be removed in
  v0.12.0.

## [0.10.0] - 2026-04-08

### Added

- **Multi-transport support**: `Synap` / `SynapClient` now accept a
  `transport` option taking one of `'synaprpc'` (default), `'resp3'` or
  `'http'`. SynapRPC opens a persistent TCP connection, frames requests
  with MessagePack, and preserves numeric/boolean/byte types that HTTP
  would otherwise stringify. RESP3 speaks the Redis wire protocol.
  Unmapped commands (queues, streams, pub/sub, scripting, transactions…)
  fall back to HTTP automatically.
- **New client options**: `rpcHost`, `rpcPort`, `resp3Host`, `resp3Port`
  for overriding binary listener endpoints.
- **E2E test suite** (`src/__tests__/e2e.test.ts`, gated behind `RUN_E2E=true`):
  spawns the release binary and exercises all three transports plus
  cross-transport consistency.

### Fixed

- **SynapRPC `Bytes` decoding**: `fromWireValue` returned raw
  `Uint8Array` for string values, breaking `kv.get()`. Now decoded as UTF-8.
- **RESP3 framing**: separate line/binary buffers lost residual bytes when
  switching between header lines and bulk payloads, causing `parseValue`
  to hang on multi-chunk responses. Unified into a single `Buffer`.
- **`asInt` boolean coercion**: SynapRPC returns `EXISTS` as `Bool(true)`,
  which `parseInt('true')` turned into `NaN`, so `kv.exists()` always
  returned `false`. Now handles booleans correctly.

## [0.9.x] Previously under Unreleased

### Added - Sorted Set Support 🎉 (October 25, 2025)

**New Module: sorted-set.ts with 18 operations**

#### Core Operations (15 methods)
- `add()`, `rem()`, `score()`, `card()`, `incrBy()`
- `range()`, `revRange()`, `rank()`, `revRank()`, `count()`
- `rangeByScore()`, `popMin()`, `popMax()`
- `remRangeByRank()`, `remRangeByScore()`

#### Set Operations (3 methods)
- `interStore()` (with weights & aggregation)
- `unionStore()` (with weights & aggregation)
- `diffStore()`

#### Types
- `ScoredMember` interface
- `SortedSetStats` interface

#### Client Integration
- `synap.sortedSet` property

#### Tests
- 18 comprehensive unit tests (100% passing ✅)

### Added - Redis Data Structures (v0.3.0) 🎉

**Complete Redis-compatible Hash, List, and Set data structures - 45 new commands**

#### Hash Manager (15 commands)
- `hash.set()`, `hash.get()`, `hash.getAll()`, `hash.del()`, `hash.exists()`
- `hash.keys()`, `hash.values()`, `hash.len()`, `hash.mset()`, `hash.mget()`
- `hash.incrBy()`, `hash.incrByFloat()`, `hash.setNX()`

#### List Manager (16 commands)
- `list.lpush()`, `list.rpush()`, `list.lpop()`, `list.rpop()`, `list.range()`
- `list.len()`, `list.index()`, `list.set()`, `list.trim()`, `list.rem()`
- `list.insert()`, `list.rpoplpush()`, `list.pos()`, `list.lpushx()`, `list.rpushx()`

#### Set Manager (14 commands)
- `set.add()`, `set.rem()`, `set.isMember()`, `set.members()`, `set.card()`
- `set.pop()`, `set.randMember()`, `set.move()`
- `set.inter()`, `set.union()`, `set.diff()`
- `set.interStore()`, `set.unionStore()`, `set.diffStore()`

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
- **BREAKING**: Package name changed from `@synap/client` to `@hivellm/synap` (now `@hivehub/synap` in v0.8.1+)
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

