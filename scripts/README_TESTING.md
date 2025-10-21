# Synap Performance Testing Guide

This directory contains scripts for testing and benchmarking the Redis-level performance optimizations implemented in Synap.

## Quick Start

### Run All Tests and Benchmarks

**PowerShell (Windows):**
```powershell
.\scripts\test-performance.ps1
```

**Bash (Linux/Mac):**
```bash
chmod +x ./scripts/test-performance.sh
./scripts/test-performance.sh
```

## Individual Benchmarks

### KV Store Benchmarks
Tests the 64-way sharded storage, Compact StoredValue, and Adaptive TTL Cleanup:

```bash
cargo bench --bench kv_bench
```

**Tests:**
- `stored_value_memory` - Memory overhead of new enum-based StoredValue
- `concurrent_operations` - Sharded concurrent SET/GET performance
- `write_throughput` - Sequential write performance
- `read_latency` - P99 latency measurements
- `ttl_cleanup` - Adaptive TTL cleanup efficiency
- `memory_footprint` - Memory usage with 1M keys
- `shard_distribution` - Uniformity of key distribution across shards

### Queue Benchmarks
Tests Arc-shared message payloads and concurrent queue operations:

```bash
cargo bench --bench queue_bench
```

**Tests:**
- `queue_memory` - Memory overhead with Arc sharing
- `concurrent_queue` - Multi-consumer performance
- `priority_queue` - Priority ordering efficiency
- `pending_messages` - ACK/NACK overhead
- `queue_depth` - Performance with deep queues
- `deadline_checker` - Message expiration handling

### Persistence Benchmarks
Tests AsyncWAL group commit and streaming snapshots:

```bash
cargo bench --bench persistence_bench
```

**Tests:**
- `wal_throughput` - AsyncWAL batched write performance
- `snapshot_memory` - Streaming snapshot memory efficiency
- `snapshot_load` - Snapshot loading performance
- `recovery` - Full recovery from WAL + Snapshot
- `concurrent_wal` - Parallel WAL write performance

## Running Specific Tests

### Run single benchmark
```bash
cargo bench --bench kv_bench -- stored_value_memory
```

### Run with baseline comparison
```bash
# Save baseline
cargo bench --bench kv_bench --save-baseline before-optimization

# After changes, compare
cargo bench --bench kv_bench --baseline before-optimization
```

### Run tests only (no benchmarks)
```bash
cargo test --release
```

## Viewing Results

### Criterion HTML Reports
After running benchmarks, open:
```
target/criterion/<benchmark_name>/report/index.html
```

### Command Line Output
Benchmarks print:
- Execution time (mean, median, std dev)
- Throughput (elements/sec or bytes/sec)
- Comparison vs previous run

## Expected Results

Based on the optimizations implemented:

### Memory Efficiency
- **StoredValue overhead**: 72 bytes → 24-32 bytes (40% reduction)
- **1M keys memory**: ~200MB → ~120MB
- **Queue pending**: 50-70% reduction with Arc sharing

### Write Performance
- **Throughput**: 50K ops/s → 150K+ ops/s (3x improvement)
- **Group commit**: 10-100x better with batching
- **Concurrent writes**: Near-linear scaling up to 64 threads

### Read Performance
- **P99 latency**: 2-5ms → <0.5ms (4-10x faster)
- **Concurrent reads**: 64x parallelism with sharding
- **Hit rate**: No degradation

### TTL Cleanup
- **CPU usage**: 10-100x reduction with sampling
- **Cleanup time**: O(n) → O(1) with probabilistic approach

## Troubleshooting

### Benchmarks too slow
```bash
# Reduce sample size
cargo bench --bench kv_bench -- --sample-size 10
```

### Out of memory
```bash
# Skip memory-intensive tests
cargo bench --bench kv_bench -- --skip memory_footprint
```

### Clean benchmark data
```bash
rm -rf target/criterion
```

## CI/CD Integration

Add to your CI pipeline:

```yaml
# .github/workflows/benchmarks.yml
- name: Run benchmarks
  run: cargo bench --all

- name: Archive benchmark results
  uses: actions/upload-artifact@v2
  with:
    name: benchmark-results
    path: target/criterion
```

## Performance Regression Detection

Track performance over time:

```bash
# Run on each commit
cargo bench -- --save-baseline main

# Compare branches
git checkout feature-branch
cargo bench -- --baseline main
```

## Additional Resources

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Synap Performance Optimizations Guide](../docs/PERFORMANCE_OPTIMIZATIONS.md)
- [CHANGELOG.md](../CHANGELOG.md) - Optimization details

## Contributing

When adding new optimizations:
1. Add corresponding benchmark
2. Run full test suite
3. Document expected improvements
4. Update this README

---

**Note**: Benchmarks require significant CPU and may take 10-30 minutes to complete.
For quick validation, use `--sample-size 10` flag.

