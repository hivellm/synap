# Changelog

All notable changes to the Synap PHP SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.0] - 2026-04-08

### Added

- **Multi-transport support**: `SynapConfig` now supports SynapRPC
  (default), RESP3 and HTTP transports via `withSynapRpcTransport()`,
  `withResp3Transport()`, `withHttpTransport()`. Override endpoints with
  `withRpcAddr(host, port)` / `withResp3Addr(host, port)`. Unmapped
  commands fall back to HTTP automatically.
- SynapRPC uses MessagePack framing over persistent TCP; RESP3 speaks
  the Redis wire protocol.

### Changed

- SDK version aligned with server and sibling SDKs (`0.2.0 → 0.10.0`).

## [0.2.0] - 2025-10-25

### Added - Redis Data Structures 🎉

**Complete Redis-compatible Hash, List, and Set data structures**

#### Hash Manager (13 commands)
- `hash()->set()`, `hash()->get()`, `hash()->getAll()`, `hash()->delete()`, `hash()->exists()`
- `hash()->keys()`, `hash()->values()`, `hash()->len()`, `hash()->mset()`, `hash()->mget()`
- `hash()->incrBy()`, `hash()->incrByFloat()`, `hash()->setNX()`

#### List Manager (9 commands)
- `list()->lpush()`, `list()->rpush()`, `list()->lpop()`, `list()->rpop()`, `list()->range()`
- `list()->len()`, `list()->index()`, `list()->set()`, `list()->trim()`

#### Set Manager (11 commands)
- `set()->add()`, `set()->rem()`, `set()->isMember()`, `set()->members()`, `set()->card()`
- `set()->pop()`, `set()->randMember()`, `set()->move()`
- `set()->inter()`, `set()->union()`, `set()->diff()`

**Usage Example**:
```php
<?php
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

$client = new SynapClient(new SynapConfig('http://localhost:15500'));

// Hash operations
$client->hash()->set('user:1', 'name', 'Alice');
$name = $client->hash()->get('user:1', 'name');

// List operations
$client->list()->rpush('tasks', ['task1', 'task2']);
$tasks = $client->list()->range('tasks', 0, -1);

// Set operations
$client->set()->add('tags', ['php', 'redis']);
$isMember = $client->set()->isMember('tags', 'php');
```

## [0.1.0] - 2025-10-23

### Added

**Core Features:**
- ✅ Complete Key-Value Store API with TTL support
- ✅ Message Queue API with ACK/NACK and priority
- ✅ Event Stream API with offset tracking
- ✅ Pub/Sub API with wildcard topic support
- ✅ StreamableHTTP protocol implementation

**Type Safety:**
- ✅ PHP 8.2+ strict types
- ✅ Full type hints for all parameters and returns
- ✅ Readonly classes for immutable data types
- ✅ Named arguments support

**Developer Experience:**
- ✅ PSR-4 autoloading
- ✅ PSR-12 code style compliance
- ✅ Comprehensive error handling
- ✅ Complete API documentation
- ✅ Working examples

**Quality Assurance:**
- ✅ PHPStan max level static analysis
- ✅ PHP-CS-Fixer for code style
- ✅ PHPUnit for testing
- ✅ Composer scripts for quality checks

### Dependencies

**Core:**
- php: ^8.2
- guzzlehttp/guzzle: ^7.8 (HTTP client)

**Development:**
- phpunit/phpunit: ^11.0 (testing)
- phpstan/phpstan: ^2.0 (static analysis)
- squizlabs/php_codesniffer: ^3.10 (code style)
- friendsofphp/php-cs-fixer: ^3.64 (formatting)

### Documentation

- Complete README with API reference
- Changelog (this file)
- Examples demonstrating all features
- Inline PHPDoc for all public methods

### Compatibility

- PHP: 8.2+
- Synap Server: v1.0.0+
- Platforms: Linux, macOS, Windows

[0.1.0]: https://github.com/hivellm/synap/releases/tag/php-sdk-v0.1.0

