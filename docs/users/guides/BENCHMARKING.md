---
title: Benchmarking Guide
module: guides
id: benchmarking
order: 12
description: How to benchmark and measure Synap performance
tags: [guides, benchmarking, performance, testing]
---

# Benchmarking Guide

Learn how to benchmark Synap and measure performance accurately.

## Overview

Benchmarking helps you:

- **Measure performance** - Understand actual throughput and latency
- **Compare configurations** - Find optimal settings
- **Identify bottlenecks** - Discover performance issues
- **Validate improvements** - Measure impact of optimizations

## Preparation

### System Requirements

**Minimum:**
- 4 CPU cores
- 8GB RAM
- SSD storage
- Dedicated machine (no other workloads)

**Recommended:**
- 8+ CPU cores
- 16GB+ RAM
- NVMe SSD
- Isolated network

### Environment Setup

**1. Isolate System:**
```bash
# Disable unnecessary services
sudo systemctl stop docker
sudo systemctl stop postgresql

# Set CPU governor to performance
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

**2. Configure Synap:**
```yaml
# config.yml - Benchmark configuration
server:
  host: "127.0.0.1"
  port: 15500

kv_store:
  max_memory_mb: 4096

persistence:
  enabled: false  # Disable for pure performance test

logging:
  level: "warn"  # Reduce logging overhead
```

**3. Warm Up:**
```bash
# Run warm-up before benchmarking
for i in {1..10000}; do
  curl -X POST http://localhost:15500/kv/set \
    -H "Content-Type: application/json" \
    -d "{\"key\":\"warmup:$i\",\"value\":\"data\"}"
done
```

## Benchmarking Tools

### 1. Built-in Benchmarks

**Using Synap Benchmarks:**
```bash
# Run KV store benchmarks
cargo bench --bench kv_bench

# Run queue benchmarks
cargo bench --bench queue_bench

# Run stream benchmarks
cargo bench --bench stream_bench
```

### 2. Custom Benchmarks

**Python Script:**
```python
import asyncio
import time
import aiohttp
from statistics import mean, stdev

async def benchmark_set(session, key, value):
    start = time.perf_counter()
    async with session.post(
        'http://localhost:15500/kv/set',
        json={'key': key, 'value': value}
    ) as resp:
        await resp.json()
    return (time.perf_counter() - start) * 1000  # ms

async def run_benchmark(operations=10000, concurrency=100):
    async with aiohttp.ClientSession() as session:
        tasks = []
        for i in range(operations):
            task = benchmark_set(
                session,
                f'key:{i}',
                f'value:{i}' * 100  # 100 bytes
            )
            tasks.append(task)
            
            if len(tasks) >= concurrency:
                results = await asyncio.gather(*tasks)
                tasks = []
                yield results
        
        if tasks:
            results = await asyncio.gather(*tasks)
            yield results

async def main():
    latencies = []
    start_time = time.time()
    
    async for batch in run_benchmark(operations=100000, concurrency=100):
        latencies.extend(batch)
    
    duration = time.time() - start_time
    ops_per_sec = len(latencies) / duration
    
    print(f"Operations: {len(latencies)}")
    print(f"Duration: {duration:.2f}s")
    print(f"Throughput: {ops_per_sec:,.0f} ops/sec")
    print(f"Latency p50: {sorted(latencies)[len(latencies)//2]:.2f}ms")
    print(f"Latency p95: {sorted(latencies)[int(len(latencies)*0.95)]:.2f}ms")
    print(f"Latency p99: {sorted(latencies)[int(len(latencies)*0.99)]:.2f}ms")

asyncio.run(main())
```

**Rust Benchmark:**
```rust
use std::time::Instant;
use tokio::time;

#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();
    let operations = 100_000;
    let concurrency = 100;
    
    let start = Instant::now();
    let mut handles = Vec::new();
    
    for i in 0..operations {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let start = Instant::now();
            client
                .post("http://localhost:15500/kv/set")
                .json(&serde_json::json!({
                    "key": format!("key:{}", i),
                    "value": format!("value:{}", i)
                }))
                .send()
                .await
                .unwrap();
            start.elapsed().as_micros() as f64 / 1000.0
        });
        
        handles.push(handle);
        
        if handles.len() >= concurrency {
            let results: Vec<f64> = futures::future::join_all(handles)
                .await
                .into_iter()
                .map(|r| r.unwrap())
                .collect();
            handles.clear();
        }
    }
    
    let duration = start.elapsed();
    println!("Throughput: {:.0} ops/sec", operations as f64 / duration.as_secs_f64());
}
```

### 3. Using Redis Benchmark Tools

**redis-benchmark (for comparison):**
```bash
# Install redis-benchmark
sudo apt-get install redis-tools

# Benchmark SET operations
redis-benchmark -h localhost -p 15500 -t set -n 100000 -c 100

# Benchmark GET operations
redis-benchmark -h localhost -p 15500 -t get -n 100000 -c 100
```

## Benchmark Scenarios

### 1. Throughput Benchmark

**Goal:** Measure maximum operations per second.

```python
# Measure SET throughput
async def throughput_benchmark():
    operations = 1_000_000
    concurrency = 1000
    
    # Run benchmark
    # Calculate: ops/sec
```

**Expected Results:**
- SET: 40K-50K ops/sec (with persistence)
- GET: 10M+ ops/sec (in-memory)

### 2. Latency Benchmark

**Goal:** Measure operation latency.

```python
# Measure latency percentiles
async def latency_benchmark():
    operations = 100_000
    concurrency = 1  # Sequential
    
    # Run benchmark
    # Calculate: p50, p95, p99
```

**Expected Results:**
- GET: <1ms p95
- SET: <5ms p95 (with persistence)

### 3. Concurrent Clients Benchmark

**Goal:** Measure performance under load.

```python
# Test with different concurrency levels
for concurrency in [1, 10, 100, 1000]:
    results = await benchmark(concurrency=concurrency)
    print(f"Concurrency {concurrency}: {results}")
```

### 4. Memory Benchmark

**Goal:** Measure memory usage.

```bash
# Monitor memory during benchmark
watch -n 1 'curl -s http://localhost:15500/info | jq .memory'
```

### 5. Persistence Impact Benchmark

**Goal:** Compare with/without persistence.

```yaml
# Test 1: No persistence
persistence:
  enabled: false

# Test 2: With persistence
persistence:
  enabled: true
  wal:
    enabled: true
```

## Benchmarking Best Practices

### 1. Run Multiple Iterations

```python
results = []
for i in range(5):
    result = await run_benchmark()
    results.append(result)

# Use median or average
median_result = sorted(results)[len(results)//2]
```

### 2. Measure Steady State

**Warm-up Period:**
- Run 10% of operations as warm-up
- Discard warm-up results
- Measure steady-state performance

### 3. Control Variables

**Keep Constant:**
- System configuration
- Network conditions
- Data size
- Operation type

**Vary:**
- Concurrency level
- Persistence settings
- Compression settings

### 4. Measure Multiple Metrics

**Key Metrics:**
- Throughput (ops/sec)
- Latency (p50, p95, p99)
- Memory usage
- CPU usage
- Error rate

### 5. Document Results

**Include:**
- System specifications
- Configuration
- Benchmark parameters
- Results (with units)
- Date and time

## Example Results

### KV Store Benchmarks

**SET Operations:**
```
Operations: 1,000,000
Concurrency: 100
Duration: 25.3s
Throughput: 39,525 ops/sec
Latency p50: 2.1ms
Latency p95: 4.8ms
Latency p99: 8.2ms
```

**GET Operations:**
```
Operations: 10,000,000
Concurrency: 1000
Duration: 0.8s
Throughput: 12,500,000 ops/sec
Latency p50: 0.08ms
Latency p95: 0.15ms
Latency p99: 0.25ms
```

### Queue Benchmarks

**Publish:**
```
Operations: 100,000
Throughput: 35,000 ops/sec
Latency p95: 3.2ms
```

**Consume:**
```
Operations: 100,000
Throughput: 40,000 ops/sec
Latency p95: 2.8ms
```

## Troubleshooting

### Low Throughput

**Possible Causes:**
- CPU bottleneck
- Network bottleneck
- Persistence overhead
- Lock contention

**Solutions:**
- Increase concurrency
- Disable persistence for test
- Use faster network
- Check CPU usage

### High Latency

**Possible Causes:**
- Slow disk I/O
- Network latency
- Lock contention
- Memory pressure

**Solutions:**
- Use SSD
- Reduce network latency
- Tune concurrency
- Increase memory

## Related Topics

- [Performance Guide](./PERFORMANCE.md) - Performance optimization
- [Performance Tuning](../configuration/PERFORMANCE_TUNING.md) - Configuration tuning
- [Monitoring](../operations/MONITORING.md) - Production monitoring

