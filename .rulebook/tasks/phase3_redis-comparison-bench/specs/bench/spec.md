# Redis vs Synap Benchmark Specification

## ADDED Requirements

### Requirement: Real Redis Comparison Benchmark
The system MUST provide a benchmark that measures the same operations against a live Redis 7
instance and a live Synap instance using identical Criterion methodology.

#### Scenario: SET throughput comparison
Given both Redis (port 6379) and Synap (port 15500) are running
When Criterion runs the set_throughput benchmark group
Then latency measurements for Redis and Synap MUST use the same wall-clock timing
And results MUST be output as a Criterion comparison report

#### Scenario: Concurrent read parallelism
Given 8 threads simultaneously GET distinct keys
When the concurrent_reads benchmark runs
Then Synap throughput MUST scale with thread count (RwLock read path)
And Redis throughput MUST remain single-threaded (event loop bound)
And the report MUST show the per-thread throughput for both

### Requirement: S2S Feature Gate
The comparison benchmark MUST only compile its Redis-dependent code when the s2s Cargo
feature is active, so the default build does not require a Redis installation.

#### Scenario: Default build has no Redis dependency
Given the project is built without --features s2s
When cargo build or cargo test runs
Then the redis and reqwest crates MUST NOT be compiled into the binary
And all benchmark files MUST compile without errors

### Requirement: Reproducible Benchmark Script
The system MUST provide a shell script that automates the full comparison workflow.

#### Scenario: Script starts both servers and runs benchmarks
Given Docker or redis-server is available on the host
When scripts/bench-redis-comparison.sh is executed
Then Redis MUST be started automatically
And Synap MUST be started in release mode
And the benchmark MUST run and produce docs/benchmarks/latest-run.txt
And both servers MUST be cleanly stopped on exit
