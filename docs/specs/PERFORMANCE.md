# Performance Specification

## Performance Targets

### Latency Goals (95th percentile)

| Operation | Target | Notes |
|-----------|--------|-------|
| KV GET | < 0.5ms | Single key lookup |
| KV SET | < 1ms | Including replication log |
| KV INCR | < 0.5ms | Atomic increment |
| KV SCAN | < 5ms | 100 keys |
| Queue PUBLISH | < 1ms | Add to queue |
| Queue CONSUME | < 0.5ms | Pop from queue |
| Queue ACK | < 0.5ms | Remove from pending |
| Stream PUBLISH | < 1ms | Broadcast to subscribers |
| Stream SUBSCRIBE | < 2ms | Initial subscription |
| PubSub PUBLISH | < 0.5ms | Topic routing |
| Replication Lag | < 10ms | Master to replica |

### Throughput Goals

| Operation | Target | Configuration |
|-----------|--------|---------------|
| KV Operations | 100K-500K ops/sec | Per core |
| Queue Messages | 50K-100K msgs/sec | Per queue |
| Event Broadcasts | 10K-50K events/sec | Per room |
| Pub/Sub Messages | 100K+ msgs/sec | All topics |
| Concurrent Connections | 100K+ | Per server instance |

### Resource Usage

| Resource | Target | Notes |
|----------|--------|-------|
| Memory Overhead | < 100 bytes/key | Radix tree efficiency |
| CPU Usage | < 50% | At 50K ops/sec |
| Network Bandwidth | < 100 Mbps | Typical workload |
| Connection Memory | < 10KB per connection | WebSocket overhead |

## Benchmark Scenarios

### Scenario 1: Key-Value Workload

**Profile**: 80% reads, 20% writes

```
Operations: 100K requests
├─ GET: 80K requests
└─ SET: 20K requests

Expected Results:
├─ Total Time: < 2s
├─ Avg Latency: < 0.5ms
├─ p95 Latency: < 1ms
└─ p99 Latency: < 2ms
```

**Benchmark Command**:
```bash
synap-benchmark kv \
  --operations 100000 \
  --read-ratio 0.8 \
  --key-size 20 \
  --value-size 100 \
  --connections 10
```

### Scenario 2: Queue Processing

**Profile**: High-throughput task queue

```
Operations: 50K messages
├─ PUBLISH: 50K messages
├─ CONSUME: 50K messages
└─ ACK: 50K messages

Expected Results:
├─ Publish Rate: 50K msgs/sec
├─ Consume Rate: 100K msgs/sec
├─ End-to-End: < 30s
└─ Message Loss: 0
```

**Benchmark Command**:
```bash
synap-benchmark queue \
  --messages 50000 \
  --priority-distribution uniform \
  --workers 5 \
  --ack-deadline 30
```

### Scenario 3: Event Broadcasting

**Profile**: Real-time chat room

```
Configuration:
├─ Rooms: 100
├─ Events per room: 1000
├─ Subscribers per room: 50
└─ Total broadcasts: 100K × 50 = 5M

Expected Results:
├─ Publish Latency: < 1ms p95
├─ Broadcast Fanout: < 2ms p95
├─ Subscriber Delivery: < 5ms p95
└─ Events/sec: 10K+
```

**Benchmark Command**:
```bash
synap-benchmark stream \
  --rooms 100 \
  --events-per-room 1000 \
  --subscribers-per-room 50 \
  --replay-history true
```

### Scenario 4: Pub/Sub Routing

**Profile**: Notification system

```
Configuration:
├─ Topics: 1000
├─ Messages: 100K
├─ Subscribers: 5000
├─ Wildcard Patterns: 20%
└─ Avg Fan-out: 3 subscribers/message

Expected Results:
├─ Routing Latency: < 0.5ms p95
├─ Delivery Latency: < 1ms p95
├─ Messages/sec: 100K+
└─ Total Deliveries: 300K
```

**Benchmark Command**:
```bash
synap-benchmark pubsub \
  --topics 1000 \
  --messages 100000 \
  --subscribers 5000 \
  --wildcard-ratio 0.2
```

### Scenario 5: Replication

**Profile**: Master-slave with 3 replicas

```
Configuration:
├─ Master: 1 node
├─ Replicas: 3 nodes
├─ Write Rate: 10K ops/sec
├─ Read Rate: 50K ops/sec (load balanced)
└─ Duration: 60 seconds

Expected Results:
├─ Replication Lag: < 10ms p95
├─ Replica Sync: 100% after writes
├─ Read Throughput: 150K ops/sec (3 × 50K)
└─ Write Throughput: 10K ops/sec (master)
```

**Benchmark Command**:
```bash
synap-benchmark replication \
  --replicas 3 \
  --write-rate 10000 \
  --read-rate 50000 \
  --duration 60
```

## Hardware Specifications

### Minimum Requirements

```yaml
CPU: 2 cores (4 threads)
RAM: 4 GB
Network: 100 Mbps
Disk: 20 GB (for logs)

Expected Performance:
├─ KV ops: 50K ops/sec
├─ Queue: 25K msgs/sec
├─ Connections: 10K concurrent
└─ Memory: 1M keys
```

### Recommended (Production)

```yaml
CPU: 8 cores (16 threads)
RAM: 32 GB
Network: 1 Gbps
Disk: 100 GB SSD

Expected Performance:
├─ KV ops: 200K ops/sec
├─ Queue: 100K msgs/sec
├─ Connections: 100K concurrent
└─ Memory: 10M+ keys
```

### High-Performance

```yaml
CPU: 16+ cores (32+ threads)
RAM: 128 GB
Network: 10 Gbps
Disk: 500 GB NVMe SSD

Expected Performance:
├─ KV ops: 500K+ ops/sec
├─ Queue: 200K+ msgs/sec
├─ Connections: 500K+ concurrent
└─ Memory: 100M+ keys
```

## Performance Tuning

### Tokio Runtime

```yaml
# Environment variables
TOKIO_WORKER_THREADS=16
RUST_LOG=info  # Reduce to 'warn' for production
```

### Connection Pool

```yaml
server:
  max_connections: 100000
  connection_timeout_secs: 60
  idle_timeout_secs: 300
```

### Memory Allocation

```rust
// Use jemalloc for better memory performance
[dependencies]
jemallocator = "0.5"

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;
```

### HTTP/2 Settings

```yaml
protocol:
  http2: true
  http2_max_concurrent_streams: 1000
  http2_initial_window_size: 1048576  # 1MB
```

## Memory Profile

### Key-Value Store

```
Memory per key-value pair:
├─ Key: ~20 bytes (avg)
├─ Value: variable
├─ Metadata: ~50 bytes
├─ Radix tree overhead: ~30 bytes
└─ Total: ~100 bytes + value size

Examples:
├─ 1M keys (100 byte values): ~200 MB
├─ 10M keys (100 byte values): ~2 GB
└─ 100M keys (100 byte values): ~20 GB
```

### Queue System

```
Memory per message:
├─ Payload: variable
├─ Headers: ~100 bytes
├─ Metadata: ~50 bytes
└─ Total: ~150 bytes + payload

10K queued messages (1KB each): ~11 MB
```

### Event Stream

```
Memory per room:
├─ Ring buffer: capacity × event_size
├─ Subscribers: ~50 bytes per subscriber
└─ Overhead: ~1 KB

100 rooms (10K events, 50 subscribers each):
├─ Events: 100 × 10K × 200 bytes = 200 MB
├─ Subscribers: 100 × 50 × 50 bytes = 250 KB
└─ Total: ~200 MB
```

## Profiling

### CPU Profiling

```bash
# Build with symbols
cargo build --release --features profiling

# Run with perf
perf record -g ./target/release/synap-server

# Generate flamegraph
perf script | stackcollapse-perf.pl | flamegraph.pl > synap-flame.svg
```

### Memory Profiling

```bash
# Use valgrind
valgrind --tool=massif --massif-out-file=massif.out \
  ./target/release/synap-server

# Analyze
ms_print massif.out
```

### Async Profiling

```rust
// Enable tokio-console
#[tokio::main]
#[console_subscriber::main]
async fn main() {
    // Server code
}

// Run tokio-console
tokio-console http://localhost:6669
```

## Load Testing

### wrk Benchmark

```bash
# HTTP throughput test
wrk -t12 -c400 -d30s \
  -s scripts/kv-set.lua \
  http://localhost:15500/api/v1/command

# Expected: 100K+ requests/sec
```

### Custom Load Test

```python
import asyncio
import time
from synap import AsyncSynapClient

async def load_test(duration_secs: int, target_rps: int):
    client = AsyncSynapClient(url='http://localhost:15500')
    
    start_time = time.time()
    request_count = 0
    
    interval = 1.0 / target_rps
    
    while time.time() - start_time < duration_secs:
        tasks = []
        
        # Generate batch of requests
        for _ in range(target_rps):
            task = client.kv.set(
                f'key-{request_count}',
                f'value-{request_count}'
            )
            tasks.append(task)
            request_count += 1
        
        # Execute batch
        await asyncio.gather(*tasks)
        
        # Wait for next second
        await asyncio.sleep(1)
    
    elapsed = time.time() - start_time
    rps = request_count / elapsed
    
    print(f'Completed {request_count} requests in {elapsed:.2f}s')
    print(f'Actual RPS: {rps:.0f}')

# Run load test
asyncio.run(load_test(duration_secs=60, target_rps=10000))
```

## Optimization Techniques

### 1. Batch Operations

```typescript
// Instead of individual requests
for (const key of keys) {
  await client.kv.get(key);  // N network round-trips
}

// Use batch
const results = await client.batch(
  keys.map(key => ({ command: 'kv.get', payload: { key } }))
);  // 1 network round-trip
```

### 2. Connection Pooling

```rust
// Reuse connections
let pool = ConnectionPool::new(20);  // 20 connections

// Concurrent requests share pool
let tasks: Vec<_> = (0..100)
    .map(|i| client.kv_get(&format!("key-{}", i)))
    .collect();

futures::future::join_all(tasks).await;
```

### 3. Compression

```yaml
protocol:
  compression: true
  compression_threshold: 1024  # Compress if > 1KB
```

Benefits:
- 50-70% size reduction for JSON
- Lower network bandwidth
- Slight CPU overhead (<5%)

### 4. MessagePack

```typescript
const client = new SynapClient({
  format: 'msgpack'  // Binary serialization
});
```

Benefits:
- 30-50% smaller than JSON
- Faster serialization
- Better for large payloads

### 5. Read from Replicas

```typescript
// Write to master
await masterClient.kv.set('key', 'value');

// Read from replicas (load balanced)
const value1 = await replicaClient1.kv.get('key');
const value2 = await replicaClient2.kv.get('key');
```

## Performance Monitoring

### Metrics to Track

```yaml
# Prometheus metrics

# Latency histograms
synap_operation_duration_seconds{component="kv", operation="get"}
synap_operation_duration_seconds{component="queue", operation="publish"}

# Throughput counters
synap_operations_total{component="kv", operation="get"}
synap_operations_total{component="queue", operation="consume"}

# Resource usage
synap_memory_bytes
synap_cpu_usage_percent
synap_connections_active

# Queue metrics
synap_queue_depth{queue="tasks"}
synap_queue_consumers{queue="tasks"}

# Replication metrics
synap_replication_lag_ms{replica="replica-1"}
synap_replication_log_size
```

### Grafana Dashboard

```json
{
  "dashboard": "Synap Performance",
  "panels": [
    {
      "title": "Operations per Second",
      "query": "rate(synap_operations_total[1m])"
    },
    {
      "title": "p95 Latency",
      "query": "histogram_quantile(0.95, synap_operation_duration_seconds)"
    },
    {
      "title": "Queue Depth",
      "query": "synap_queue_depth"
    },
    {
      "title": "Replication Lag",
      "query": "synap_replication_lag_ms"
    }
  ]
}
```

## Benchmarking Tools

### synap-benchmark CLI

```bash
# Full benchmark suite
synap-benchmark all --duration 60 --report benchmark-report.json

# Specific component
synap-benchmark kv --operations 1000000
synap-benchmark queue --messages 500000 --workers 10
synap-benchmark stream --rooms 50 --events 10000

# Stress test
synap-benchmark stress \
  --duration 3600 \
  --kv-rate 100000 \
  --queue-rate 50000 \
  --connections 50000
```

### Custom Benchmark

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use synap_client::SynapClient;

fn benchmark_kv_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = rt.block_on(async {
        SynapClient::connect("http://localhost:15500").await.unwrap()
    });
    
    c.bench_function("kv_set", |b| {
        b.iter(|| {
            rt.block_on(async {
                client.kv_set(
                    black_box("bench-key"),
                    black_box("bench-value"),
                    None
                ).await.unwrap()
            })
        })
    });
    
    c.bench_function("kv_get", |b| {
        b.iter(|| {
            rt.block_on(async {
                client.kv_get::<String>(black_box("bench-key"))
                    .await.unwrap()
            })
        })
    });
}

criterion_group!(benches, benchmark_kv_operations);
criterion_main!(benches);
```

## Scalability Limits

### Single Node Limits

```yaml
Maximum Capacity (32GB RAM):
├─ Keys: 100M+ (100 byte values)
├─ Queue Messages: 100M+ (1KB each)
├─ Event Rooms: 100K
├─ Pub/Sub Topics: Unlimited (radix tree)
├─ Concurrent Connections: 100K+
└─ Operations/sec: 500K+
```

### Clustered Limits

```yaml
Master + 5 Replicas:
├─ Write Throughput: 500K ops/sec (master)
├─ Read Throughput: 2.5M ops/sec (5 × 500K)
├─ Total Connections: 600K (6 × 100K)
└─ Replication Lag: < 10ms p95
```

## Performance Regression Testing

### CI/CD Integration

```yaml
# .github/workflows/performance.yml
name: Performance Tests

on: [pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run benchmarks
        run: |
          cargo bench --features benchmarks > bench-results.txt
      
      - name: Compare with baseline
        run: |
          python scripts/compare-benchmarks.py \
            bench-results.txt \
            baseline-bench.txt \
            --threshold 5%  # Fail if 5% regression
```

### Automated Alerts

```python
def check_performance_regression(current: dict, baseline: dict):
    for metric, value in current.items():
        baseline_value = baseline.get(metric)
        
        if baseline_value:
            regression = (value - baseline_value) / baseline_value
            
            if regression > 0.05:  # 5% regression
                alert(f'Performance regression in {metric}: {regression * 100:.1f}%')
```

## See Also

- [OPTIMIZATION.md](OPTIMIZATION.md) - Optimization strategies
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture
- [DEPLOYMENT.md](DEPLOYMENT.md) - Production deployment

