# Synap Benchmarks

This directory contains comprehensive benchmark results and performance analysis for Synap.

## Available Benchmarks

### [1.4.0 Performance Baseline](BASELINE_1_4_0.md)
Client-side throughput, p50/p95/p99 latency and server memory of the 1.4.0
release next to 1.3.2, recorded with `scripts/bench/perf_baseline.py`:
- RESP3 and SynapRPC, GET / SET / list / pipelined SET
- Every durability mode (`off`, `never`, `periodic`, `always`)
- What the cost of logging every write is, and which part of it is avoidable
- Raw reports in [`reports/`](reports/)

### [Benchmark Results Extended](BENCHMARK_RESULTS_EXTENDED.md)
Extended performance benchmarks covering all major components:
- Key-Value Store operations
- Queue System throughput
- Event Stream performance
- Pub/Sub messaging
- Replication overhead
- Memory usage patterns

### [Persistence Benchmarks](PERSISTENCE_BENCHMARKS.md)
Write-Ahead Log (WAL) and snapshot system performance:
- WAL write throughput
- Snapshot creation and restoration
- Recovery time analysis
- Durability mode comparisons
- Disk I/O patterns

### [Queue Concurrency Tests](QUEUE_CONCURRENCY_TESTS.md)
Queue system concurrency and parallelism analysis:
- Multi-producer scenarios
- Multi-consumer patterns
- Acknowledgment overhead
- Message ordering guarantees
- Throughput under load

## Benchmark Environment

The 1.4.0 baseline states its own machine and method. The other benchmarks
were conducted under these conditions:
- **Hardware**: AMD Ryzen 9 5950X, 64GB RAM, NVMe SSD
- **OS**: Linux (Ubuntu 22.04 LTS)
- **Rust**: 1.85+ nightly (edition 2024)
- **Methodology**: Average of 10 runs with warmup

## Performance Targets

See [Performance Specification](../specs/PERFORMANCE.md) for detailed targets and requirements.

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark suite
cargo bench --bench kv_store
cargo bench --bench queue_system
cargo bench --bench event_stream

# With detailed output
cargo bench -- --verbose
```

## Interpreting Results

- **Throughput**: Operations per second (ops/s)
- **Latency**: Measured at p50, p95, p99 percentiles
- **Memory**: RSS (Resident Set Size) in MB
- **CPU**: User + System time percentage

## Contributing

When adding new benchmarks:
1. Follow existing benchmark structure
2. Document test scenarios clearly
3. Include hardware/environment details
4. Run multiple iterations for accuracy
5. Update this README with new benchmark descriptions

