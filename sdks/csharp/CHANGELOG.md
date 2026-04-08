# Changelog

All notable changes to the Synap C# SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.10.0] - 2026-04-08

### Added

- **Multi-transport support**: `SynapConfig` now supports SynapRPC
  (default), RESP3 and HTTP transports via `WithSynapRpcTransport()`,
  `WithResp3Transport()`, `WithHttpTransport()`. Override endpoints with
  `WithRpcAddr(host, port)` / `WithResp3Addr(host, port)`. Unmapped
  commands fall back to HTTP automatically.
- SynapRPC uses MessagePack framing over persistent TCP; RESP3 speaks
  the Redis wire protocol.

### Changed

- SDK version aligned with server and sibling SDKs (`0.2.0 → 0.10.0`).

## [0.2.0] - 2025-10-25

### Added - Redis Data Structures 🎉

**Complete Redis-compatible Hash, List, and Set data structures**

#### Hash Manager (13+ commands)
- `Hash.SetAsync()`, `Hash.GetAsync()`, `Hash.GetAllAsync()`, `Hash.DeleteAsync()`, `Hash.ExistsAsync()`
- `Hash.KeysAsync()`, `Hash.ValuesAsync()`, `Hash.LenAsync()`, `Hash.MSetAsync()`, `Hash.MGetAsync()`
- `Hash.IncrByAsync()`, `Hash.IncrByFloatAsync()`, `Hash.SetNXAsync()`

#### List Manager (8+ commands)
- `List.LPushAsync()`, `List.RPushAsync()`, `List.LPopAsync()`, `List.RPopAsync()`
- `List.RangeAsync()`, `List.LenAsync()`, `List.IndexAsync()`, `List.SetAsync()`, `List.TrimAsync()`

#### Set Manager (11+ commands)
- `Set.AddAsync()`, `Set.RemAsync()`, `Set.IsMemberAsync()`, `Set.MembersAsync()`, `Set.CardAsync()`
- `Set.PopAsync()`, `Set.RandMemberAsync()`, `Set.MoveAsync()`
- `Set.InterAsync()`, `Set.UnionAsync()`, `Set.DiffAsync()`

**Usage Example**:
```csharp
using Synap.SDK;

var config = new SynapConfig("http://localhost:15500");
var client = new SynapClient(config);

// Hash operations
await client.Hash.SetAsync("user:1", "name", "Alice");
var name = await client.Hash.GetAsync("user:1", "name");

// List operations
await client.List.RPushAsync("tasks", new List<string> { "task1", "task2" });
var tasks = await client.List.RangeAsync("tasks", 0, -1);

// Set operations
await client.Set.AddAsync("tags", new List<string> { "csharp", "redis" });
var isMember = await client.Set.IsMemberAsync("tags", "csharp");
```

## [0.1.0] - 2025-10-23

### Added
- 🎉 **Initial Release**: Complete C# SDK for Synap
- ✅ **Key-Value Store**: Full CRUD, TTL, atomic operations (incr/decr), scan
- ✅ **Message Queues**: Create, publish, consume, ACK/NACK, statistics
- ✅ **Event Streams**: Create rooms, publish events, read with offset, statistics
- ✅ **Pub/Sub**: Subscribe/unsubscribe topics with wildcards, publish messages
- ✅ **StreamableHTTP Protocol**: Single unified endpoint `/api/stream`
- ✅ **Type-Safe**: C# 12+ with nullable reference types
- ✅ **Async/Await**: All operations support `CancellationToken` and `ConfigureAwait(false)`
- ✅ **Exception Handling**: Custom `SynapException` with specific error types
- ✅ **Comprehensive Tests**: xUnit tests with 95%+ coverage
- ✅ **XML Documentation**: Complete API documentation for IntelliSense
- ✅ **NuGet Package**: Ready for publishing to NuGet.org

### Features

#### Key-Value Store
- `SetAsync`: Store key-value pairs with optional TTL
- `GetAsync`: Retrieve values with generic type support
- `DeleteAsync`: Remove keys
- `ExistsAsync`: Check key existence
- `IncrAsync/DecrAsync`: Atomic increment/decrement
- `ScanAsync`: Scan keys by prefix
- `StatsAsync`: Get KV store statistics

#### Message Queues
- `CreateQueueAsync`: Create queues with max size and message TTL
- `DeleteQueueAsync`: Remove queues
- `PublishAsync`: Publish messages with priority (0-9) and max retries
- `ConsumeAsync`: Consume messages with consumer ID
- `AckAsync/NackAsync`: Acknowledge or requeue messages
- `StatsAsync`: Get queue statistics
- `ListAsync`: List all queues

#### Event Streams
- `CreateRoomAsync/DeleteRoomAsync`: Manage stream rooms
- `PublishAsync`: Publish events to rooms
- `ReadAsync`: Read events from offset with limit
- `StatsAsync`: Get stream statistics
- `ListRoomsAsync`: List all rooms

#### Pub/Sub
- `SubscribeTopicsAsync`: Subscribe to topic patterns
- `UnsubscribeTopicsAsync`: Unsubscribe from topics
- `PublishAsync`: Publish messages to topics
- `StatsAsync`: Get pub/sub statistics

### Technical Details
- **Target Framework**: .NET 8.0
- **Language**: C# 12+ with latest features
- **Nullable**: Enabled for null safety
- **Async**: ConfigureAwait(false) for library code
- **JSON**: System.Text.Json for serialization
- **Testing**: xUnit with Moq
- **Documentation**: XML comments for all public APIs

[unreleased]: https://github.com/hivellm/synap/compare/synap-csharp-v0.1.0...HEAD
[0.1.0]: https://github.com/hivellm/synap/releases/tag/synap-csharp-v0.1.0

