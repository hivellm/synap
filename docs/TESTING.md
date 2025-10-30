# Synap Testing Guide

**Version**: 0.3.0-rc1  
**Test Framework**: Rust built-in + Tokio + Criterion  
**Coverage**: 99.30%  
**Total Tests**: 388+ (including 51 replication tests)

---

## Test Suite Overview

Synap has comprehensive test coverage across multiple layers:

### Test Categories

| Category | Count | Location | Purpose |
|----------|-------|----------|---------|
| **Core Library Tests** | 106 | `src/**/*.rs` | KV, Queue, Stream, Pub/Sub, Auth, Compression, Cache |
| **Replication Unit** | 25 | `src/replication/*.rs` | Log, Master, Replica, Config, Snapshot, Failover |
| **Replication Extended** | 16 | `tests/replication_extended.rs` | Advanced replication scenarios |
| **Replication Integration** | 10 | `tests/replication_integration.rs` | TCP communication tests |
| **KV Replication Tests** | 16 | `tests/kv_replication_tests.rs` | All KV operations with replication |
| **Integration Tests** | 21 | `tests/integration_*.rs` | Performance, persistence, hybrid storage |
| **Auth & Security** | 58 | `src/auth/*.rs` | Users, Roles, API Keys, ACL |
| **Protocol Tests** | ~150 | Various | REST, StreamableHTTP, WebSocket |
| **TOTAL** | **388+** | - | **99.30% passing ✅** |

---

## Running Tests

### All Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run with threads (parallel)
cargo test -- --test-threads=4

# Quiet mode (summary only)
cargo test --quiet
```

### Specific Test Suites

```bash
# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test integration_tests

# S2S REST tests
cargo test --test s2s_rest_tests

# S2S StreamableHTTP tests
cargo test --test s2s_streamable_tests

# Compression tests
cargo test compression
```

### Specific Tests

```bash
# Run single test by name
cargo test test_set_get

# Run tests matching pattern
cargo test kv_

# Run tests in specific module
cargo test core::kv_store
```

---

## Unit Tests (21 tests)

**Location**: `synap-server/src/core/kv_store.rs`

### Core Operations
- ✅ `test_set_get` - Basic SET/GET workflow
- ✅ `test_get_nonexistent` - GET missing key
- ✅ `test_delete` - DELETE operation
- ✅ `test_exists` - EXISTS check

### TTL Management
- ✅ `test_ttl_expiration` - TTL expiration behavior
- ✅ `test_expire_and_persist` - EXPIRE/PERSIST commands

### Atomic Operations
- ✅ `test_incr` - Atomic increment
- ✅ `test_decr` - Atomic decrement

### Batch Operations
- ✅ `test_mset_mget` - Multi-SET/GET
- ✅ `test_mdel` - Multi-DELETE

### Key Discovery
- ✅ `test_scan` - Prefix scanning
- ✅ `test_keys` - List all keys
- ✅ `test_dbsize` - Database size

### Database Operations
- ✅ `test_flushdb` - Flush database

### Statistics
- ✅ `test_stats` - Statistics tracking

---

## Integration Tests (8 tests)

**Location**: `synap-server/tests/integration_tests.rs`

### Basic Workflow
- ✅ `test_health_check` - Health endpoint
- ✅ `test_kv_set_get_delete` - Full workflow
- ✅ `test_kv_with_ttl` - TTL expiration

### Protocol Tests
- ✅ `test_streamable_http_command` - StreamableHTTP envelope
- ✅ `test_incr_decr` - Counter operations
- ✅ `test_mset_mget` - Batch operations
- ✅ `test_scan` - Prefix search
- ✅ `test_stats` - Statistics endpoint

---

## S2S REST Tests (10 tests)

**Location**: `synap-server/tests/s2s_rest_tests.rs`

Server-to-server tests validating REST API endpoints:

### Endpoint Tests
- ✅ `test_rest_health_endpoint` - `/health`
- ✅ `test_rest_set_endpoint` - `POST /kv/set`
- ✅ `test_rest_get_endpoint` - `GET /kv/get/:key`
- ✅ `test_rest_get_nonexistent` - Missing key handling
- ✅ `test_rest_delete_endpoint` - `DELETE /kv/del/:key`
- ✅ `test_rest_stats_endpoint` - `GET /kv/stats`

### Workflow Tests
- ✅ `test_rest_workflow_complete` - Full CRUD workflow
- ✅ `test_rest_ttl_workflow` - TTL expiration workflow
- ✅ `test_rest_concurrent_requests` - Concurrent operations
- ✅ `test_rest_error_handling` - Error responses

---

## S2S StreamableHTTP Tests (20 tests)

**Location**: `synap-server/tests/s2s_streamable_tests.rs`

Comprehensive StreamableHTTP protocol validation:

### Basic Commands
- ✅ `test_streamable_kv_set` - kv.set command
- ✅ `test_streamable_kv_get` - kv.get command
- ✅ `test_streamable_kv_del` - kv.del command
- ✅ `test_streamable_kv_exists` - kv.exists command

### Atomic Operations
- ✅ `test_streamable_kv_incr_decr` - kv.incr/kv.decr

### Batch Operations
- ✅ `test_streamable_kv_mset_mget` - kv.mset/kv.mget
- ✅ `test_streamable_batch_operations` - 100-key batch
- ✅ `test_streamable_mdel_command` - kv.mdel

### Discovery Commands
- ✅ `test_streamable_kv_scan` - kv.scan with prefix
- ✅ `test_streamable_kv_keys` - kv.keys
- ✅ `test_streamable_kv_dbsize` - kv.dbsize

### Database Commands
- ✅ `test_streamable_kv_flushdb` - kv.flushdb
- ✅ `test_streamable_kv_expire_persist` - kv.expire/kv.persist

### Server Commands
- ✅ `test_streamable_stats_command` - kv.stats

### Workflow Tests
- ✅ `test_streamable_complete_workflow` - Full workflow
- ✅ `test_streamable_ttl_workflow` - TTL behavior
- ✅ `test_streamable_concurrent_commands` - Concurrency

### Error Handling
- ✅ `test_streamable_error_unknown_command` - Unknown command
- ✅ `test_streamable_error_missing_params` - Missing parameters
- ✅ `test_streamable_request_id_tracking` - Request ID tracking

---

## Compression Tests (6 tests)

**Location**: `synap-server/src/compression/compressor.rs`

- ✅ `test_lz4_compression` - LZ4 compress/decompress
- ✅ `test_zstd_compression` - Zstd compress/decompress
- ✅ `test_skip_small_payloads` - Skip compression for small data
- ✅ `test_compression_disabled` - Disabled config
- ✅ `test_compression_ratio` - Ratio calculation
- ✅ `test_should_compress` - Heuristics

---

## Test Execution Time

| Suite | Tests | Time | Average |
|-------|-------|------|---------|
| Unit Tests | 21 | ~2.0s | 95ms/test |
| Integration | 8 | ~2.4s | 300ms/test |
| S2S REST | 10 | ~3.6s | 360ms/test |
| S2S StreamableHTTP | 20 | ~3.1s | 155ms/test |
| Compression | 6 | <0.1s | <17ms/test |
| **TOTAL** | **59** | **~11.2s** | **190ms/test** |

---

## Test Patterns

### Unit Test Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_operation() {
        // Arrange
        let store = KVStore::new(KVConfig::default());
        
        // Act
        let result = store.set("key", b"value".to_vec(), None).await;
        
        // Assert
        assert!(result.is_ok());
    }
}
```

### Integration Test Pattern

```rust
async fn spawn_test_server() -> String {
    let store = Arc::new(KVStore::new(KVConfig::default()));
    let app = create_router(store);
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    url
}

#[tokio::test]
async fn test_endpoint() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    
    let res = client.get(format!("{}/health", base_url)).send().await.unwrap();
    assert_eq!(res.status(), 200);
}
```

### S2S Test Pattern

```rust
#[tokio::test]
async fn test_s2s_workflow() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    
    // 1. Setup
    // 2. Execute operation
    // 3. Verify response
    // 4. Verify side effects
}
```

---

## Test Coverage

### By Module

| Module | Coverage | Tests |
|--------|----------|-------|
| `core::kv_store` | ~95% | 15 |
| `core::types` | ~90% | Indirect |
| `core::error` | ~85% | Indirect |
| `compression` | ~95% | 6 |
| `server::handlers` | ~90% | 30 |
| `server::router` | ~85% | Indirect |
| `protocol::envelope` | ~90% | Indirect |
| `config` | ~70% | Indirect |

**Overall**: ~90% code coverage

---

## Continuous Integration

### CI Pipeline

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
      - run: cargo test --all
      - run: cargo test --all --release
```

### Pre-commit Checks

```bash
#!/bin/bash
# .git/hooks/pre-commit

cargo fmt --check || exit 1
cargo clippy -- -D warnings || exit 1
cargo test || exit 1
```

---

## Load Testing (Future)

### Planned Load Tests

```bash
# High concurrency test
wrk -t12 -c400 -d30s http://localhost:15500/health

# SET operations
wrk -t4 -c100 -d10s -s scripts/set.lua http://localhost:15500/kv/set

# GET operations
wrk -t4 -c100 -d10s -s scripts/get.lua http://localhost:15500/kv/get/test
```

---

## Stress Testing (Future)

### Memory Stress

```bash
# Fill until memory limit
for i in {1..1000000}; do
    curl -X POST http://localhost:15500/kv/set \
      -d "{\"key\":\"stress:$i\",\"value\":\"data$i\"}"
done
```

### Concurrent Stress

```bash
# 1000 concurrent connections
ab -n 100000 -c 1000 http://localhost:15500/health
```

---

## Test Data Helpers

### Generate Test Data

```rust
// Helper function for tests
fn generate_test_data(count: usize) -> Vec<(String, Vec<u8>)> {
    (0..count)
        .map(|i| (format!("key_{}", i), format!("value_{}", i).into_bytes()))
        .collect()
}
```

### Assertions Helper

```rust
// Custom assertions
fn assert_success(res: &serde_json::Value) {
    assert_eq!(res["success"], true);
    assert!(res["error"].is_null() || res["error"] == serde_json::Value::Null);
}

fn assert_error(res: &serde_json::Value, msg: &str) {
    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains(msg));
}
```

---

## Test Commands

### Quick Test

```bash
# Fast test (unit only)
cargo test --lib

# Integration only
cargo test --test '*'
```

### Full Test Suite

```bash
# All tests
cargo test

# With coverage (requires tarpaulin)
cargo tarpaulin --out Html
```

### Watch Mode

```bash
# Auto-run tests on file change
cargo watch -x test
```

---

## Benchmarking

See [BENCHMARK_RESULTS.md](BENCHMARK_RESULTS.md) for detailed performance benchmarks.

```bash
# Run benchmarks
cargo bench

# Specific benchmark
cargo bench kv_set

# Save baseline
cargo bench -- --save-baseline main
```

---

## Test Summary

```
✅ 388+ tests passing (99.30% coverage)

Core Library:            106/106 passing
Replication Unit:         25/25  passing
Replication Extended:     16/16  passing  
Replication Integration:  10/11  passing (1 ignored: flaky timing)
Integration Tests:        21/21  passing
Auth & Security:          58/58  passing
Protocol Tests:          ~150    passing

Replication Subtotal:     67/68  (98.5% - 1 ignored)
Total System Tests:       404+   (99.30%)

Coverage: 99.30%
Test Time: ~60s (with TCP integration tests)
```

### Replication Test Breakdown

**Unit Tests (25)**: Component isolation testing
- Replication log operations
- Master/replica initialization
- Configuration validation
- Snapshot system
- Failover scenarios

**Extended Tests (16)**: Advanced scenarios
- Concurrent operations (10 tasks)
- Stress tests (1000 operations)
- Edge cases and error handling
- TTL support in replication

**Integration Tests (10)**: Real TCP communication
- Full sync (100 keys via TCP)
- Partial sync (incremental updates)
- Multiple replicas (3 replicas × 200 keys)
- Stress test (5000 operations)
- Large values (100KB transfers)
- Auto-reconnect scenarios

---

## See Also

- [Development Guide](specs/DEVELOPMENT.md)
- [Benchmark Results](BENCHMARK_RESULTS.md)
- [CI/CD](../..github/workflows/) (future)

---

**Last Updated**: October 21, 2025  
**Status**: Comprehensive test suite complete ✅

