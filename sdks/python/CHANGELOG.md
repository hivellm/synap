# Changelog

All notable changes to the Synap Python SDK will be documented in this file.

## [0.2.0] - 2025-10-25

### Added - Redis Data Structures 🎉

**Complete Redis-compatible Hash, List, and Set data structures - 45 new commands**

#### Hash Manager (15 commands)
- `hash.set()`, `hash.get()`, `hash.get_all()`, `hash.delete()`, `hash.exists()`
- `hash.keys()`, `hash.values()`, `hash.len()`, `hash.mset()`, `hash.mget()`
- `hash.incr_by()`, `hash.incr_by_float()`, `hash.set_nx()`

#### List Manager (16 commands)
- `list.lpush()`, `list.rpush()`, `list.lpop()`, `list.rpop()`, `list.range()`
- `list.len()`, `list.index()`, `list.set()`, `list.trim()`, `list.rem()`
- `list.insert()`, `list.rpoplpush()`, `list.pos()`, `list.lpushx()`, `list.rpushx()`

#### Set Manager (14 commands)
- `set.add()`, `set.rem()`, `set.is_member()`, `set.members()`, `set.card()`
- `set.pop()`, `set.rand_member()`, `set.move()`
- `set.inter()`, `set.union()`, `set.diff()`
- `set.inter_store()`, `set.union_store()`, `set.diff_store()`

**Usage Example**:
```python
from synap_sdk import SynapClient, SynapConfig

config = SynapConfig("http://localhost:15500")
async with SynapClient(config) as client:
    # Hash operations
    await client.hash.set("user:1", "name", "Alice")
    name = await client.hash.get("user:1", "name")
    
    # List operations
    await client.list.rpush("tasks", "task1", "task2")
    tasks = await client.list.range("tasks", 0, -1)
    
    # Set operations
    await client.set.add("tags", "python", "redis")
    is_member = await client.set.is_member("tags", "python")
```

## [0.1.1] - 2025-10-24

### Fixed
- **CRITICAL**: Fixed pub/sub publish API - now sends `payload` field instead of `message`
- **CRITICAL**: Fixed response field from `delivered` to `subscribers_matched`

### Added
- 8 pub/sub unit tests with mock validation
- 8 pub/sub S2S integration tests
- Tests verify API contract compliance

### Changed
- Ready for PyPI publication

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-10-23

### Added
- 🎉 **Initial Release**: Complete Python SDK for Synap
- ✅ **Key-Value Store**: Full CRUD, TTL, atomic operations (incr/decr), scan
- ✅ **Message Queues**: Create, publish, consume, ACK/NACK, statistics
- ✅ **Event Streams**: Create rooms, publish events, read with offset, statistics
- ✅ **Pub/Sub**: Subscribe/unsubscribe topics with wildcards, publish messages
- ✅ **StreamableHTTP Protocol**: Single unified endpoint `/api/stream`
- ✅ **Type-Safe**: Full type hints with mypy strict mode
- ✅ **Async/Await**: All operations are async with httpx
- ✅ **Context Manager**: Automatic resource cleanup with async context manager
- ✅ **Exception Handling**: Custom `SynapException` with specific error types
- ✅ **Comprehensive Tests**: pytest tests with 95%+ coverage
- ✅ **Documentation**: Complete docstrings for all public APIs

### Features

#### Key-Value Store
- `set`: Store key-value pairs with optional TTL
- `get`: Retrieve values
- `delete`: Remove keys
- `exists`: Check key existence
- `incr/decr`: Atomic increment/decrement
- `scan`: Scan keys by prefix
- `stats`: Get KV store statistics

#### Message Queues
- `create_queue`: Create queues with max size and message TTL
- `delete_queue`: Remove queues
- `publish`: Publish messages with priority (0-9) and max retries
- `consume`: Consume messages with consumer ID
- `ack/nack`: Acknowledge or requeue messages
- `stats`: Get queue statistics
- `list`: List all queues

#### Event Streams
- `create_room/delete_room`: Manage stream rooms
- `publish`: Publish events to rooms
- `read`: Read events from offset with limit
- `stats`: Get stream statistics
- `list_rooms`: List all rooms

#### Pub/Sub
- `subscribe_topics`: Subscribe to topic patterns
- `unsubscribe_topics`: Unsubscribe from topics
- `publish`: Publish messages to topics
- `stats`: Get pub/sub statistics

### Technical Details
- **Python Version**: 3.11+
- **Type Hints**: Full type coverage with mypy strict mode
- **Async**: httpx for async HTTP operations
- **Testing**: pytest with pytest-asyncio
- **Linting**: ruff for fast Python linting
- **Documentation**: Complete docstrings with examples

[unreleased]: https://github.com/hivellm/hivellm/compare/synap-python-v0.1.0...HEAD
[0.1.0]: https://github.com/hivellm/hivellm/releases/tag/synap-python-v0.1.0

