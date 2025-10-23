# Changelog

All notable changes to the Synap Python SDK will be documented in this file.

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

