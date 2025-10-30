use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::collections::HashMap;
use std::hint::black_box;
use synap_server::core::HashStore;
use tokio::runtime::Runtime;

/// Benchmark: Hash SET operation (HSET)
/// Target: <100µs p99 latency
fn bench_hash_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_set");
    group.sample_size(1000);

    // Test different field value sizes
    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let rt = Runtime::new().unwrap();
            let store = HashStore::new();
            let value = vec![0u8; size];

            b.to_async(&rt).iter(|| async {
                store
                    .hset(
                        black_box("user:1000"),
                        black_box("name"),
                        black_box(value.clone()),
                    )
                    .unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark: Hash GET operation (HGET)
/// Target: <50µs p99 latency
fn bench_hash_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_get");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();
    let store = HashStore::new();

    // Pre-populate hash
    rt.block_on(async {
        store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();
        store.hset("user:1000", "age", b"30".to_vec()).unwrap();
        store
            .hset("user:1000", "email", b"alice@example.com".to_vec())
            .unwrap();
    });

    group.bench_function("hget_existing_field", |b| {
        b.to_async(&rt).iter(|| async {
            let result = store
                .hget(black_box("user:1000"), black_box("name"))
                .unwrap();
            black_box(result);
        });
    });

    group.bench_function("hget_nonexistent_field", |b| {
        b.to_async(&rt).iter(|| async {
            let result = store
                .hget(black_box("user:1000"), black_box("nonexistent"))
                .unwrap();
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: Hash GETALL operation (HGETALL)
/// Target: <500µs for 100 fields
fn bench_hash_getall(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_getall");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();

    // Test different field counts
    for field_count in [10, 50, 100, 500].iter() {
        let store = HashStore::new();

        // Pre-populate hash with N fields
        rt.block_on(async {
            for i in 0..*field_count {
                store
                    .hset(
                        "user:1000",
                        &format!("field_{}", i),
                        format!("value_{}", i).into_bytes(),
                    )
                    .unwrap();
            }
        });

        group.throughput(Throughput::Elements(*field_count as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(field_count),
            field_count,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let all = store.hgetall(black_box("user:1000")).unwrap();
                    black_box(all);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Hash DELETE operation (HDEL)
fn bench_hash_del(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_del");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();

    // Single field delete
    group.bench_function("hdel_single_field", |b| {
        b.iter_batched(
            || {
                let store = HashStore::new();
                rt.block_on(async {
                    store.hset("user:1000", "temp", b"value".to_vec()).unwrap();
                });
                store
            },
            |store| {
                rt.block_on(async {
                    store
                        .hdel(black_box("user:1000"), &[String::from("temp")])
                        .unwrap();
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Multi-field delete
    group.bench_function("hdel_10_fields", |b| {
        b.iter_batched(
            || {
                let store = HashStore::new();
                rt.block_on(async {
                    for i in 0..10 {
                        store
                            .hset("user:1000", &format!("field_{}", i), b"value".to_vec())
                            .unwrap();
                    }
                });
                store
            },
            |store| {
                rt.block_on(async {
                    let fields: Vec<String> = (0..10).map(|i| format!("field_{}", i)).collect();
                    store.hdel(black_box("user:1000"), &fields).unwrap();
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark: Hash INCR operations (HINCRBY, HINCRBYFLOAT)
fn bench_hash_incr(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_incr");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();
    let store = HashStore::new();

    group.bench_function("hincrby", |b| {
        b.to_async(&rt).iter(|| async {
            store
                .hincrby(
                    black_box("stats:user:1000"),
                    black_box("counter"),
                    black_box(1),
                )
                .unwrap();
        });
    });

    group.bench_function("hincrbyfloat", |b| {
        b.to_async(&rt).iter(|| async {
            store
                .hincrbyfloat(
                    black_box("stats:user:1000"),
                    black_box("score"),
                    black_box(1.5),
                )
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: Hash bulk operations (HMSET, HMGET)
fn bench_hash_bulk(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_bulk");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();

    // HMSET with different field counts
    for field_count in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*field_count as u64));

        group.bench_with_input(
            BenchmarkId::new("hmset", field_count),
            field_count,
            |b, &count| {
                let store = HashStore::new();
                let mut fields = HashMap::new();
                for i in 0..count {
                    fields.insert(format!("field_{}", i), format!("value_{}", i).into_bytes());
                }

                b.to_async(&rt).iter(|| async {
                    store
                        .hmset(black_box("user:1000"), black_box(fields.clone()))
                        .unwrap();
                });
            },
        );
    }

    // HMGET with different field counts
    for field_count in [10, 50, 100].iter() {
        let store = HashStore::new();

        // Pre-populate
        rt.block_on(async {
            let mut fields = HashMap::new();
            for i in 0..*field_count {
                fields.insert(format!("field_{}", i), format!("value_{}", i).into_bytes());
            }
            store.hmset("user:1000", fields).unwrap();
        });

        group.throughput(Throughput::Elements(*field_count as u64));

        group.bench_with_input(
            BenchmarkId::new("hmget", field_count),
            field_count,
            |b, &count| {
                let fields: Vec<String> = (0..count).map(|i| format!("field_{}", i)).collect();

                b.to_async(&rt).iter(|| async {
                    let result = store
                        .hmget(black_box("user:1000"), black_box(&fields))
                        .unwrap();
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Concurrent hash operations
/// Target: Linear scaling with thread count
fn bench_hash_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_concurrent");
    group.sample_size(100);

    let rt = Runtime::new().unwrap();

    for num_tasks in [1, 4, 16, 64].iter() {
        group.throughput(Throughput::Elements((num_tasks * 100) as u64));

        group.bench_with_input(
            BenchmarkId::new("concurrent_hset", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                let store = HashStore::new();

                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();

                    for i in 0..num_tasks {
                        let store_clone = store.clone();
                        let handle = tokio::spawn(async move {
                            for j in 0..100 {
                                let key = format!("user:{}", i);
                                let field = format!("field_{}", j);
                                let value = format!("value_{}_{}", i, j).into_bytes();
                                store_clone.hset(&key, &field, value).unwrap();
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Hash operations with sharding
fn bench_hash_sharding(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_sharding");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();
    let store = HashStore::new();

    // Test operations across different shards (64 shards total)
    group.bench_function("hset_across_shards", |b| {
        b.to_async(&rt).iter(|| async {
            for i in 0..64 {
                let key = format!("user:{}", i); // Each goes to different shard
                store.hset(&key, "name", b"Alice".to_vec()).unwrap();
            }
        });
    });

    group.bench_function("hget_across_shards", |b| {
        // Pre-populate
        rt.block_on(async {
            for i in 0..64 {
                let key = format!("user:{}", i);
                store.hset(&key, "name", b"Alice".to_vec()).unwrap();
            }
        });

        b.to_async(&rt).iter(|| async {
            for i in 0..64 {
                let key = format!("user:{}", i);
                let result = store.hget(&key, "name").unwrap();
                black_box(result);
            }
        });
    });

    group.finish();
}

/// Benchmark summary: Compare all basic operations
fn bench_hash_operations_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_operations_summary");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();
    let store = HashStore::new();

    // Pre-populate for GET tests
    rt.block_on(async {
        for i in 0..100 {
            store
                .hset("user:1000", &format!("field_{}", i), b"value".to_vec())
                .unwrap();
        }
    });

    // HSET
    group.bench_function("hset", |b| {
        b.to_async(&rt).iter(|| async {
            store
                .hset(
                    black_box("user:2000"),
                    black_box("name"),
                    black_box(b"Alice".to_vec()),
                )
                .unwrap();
        });
    });

    // HGET
    group.bench_function("hget", |b| {
        b.to_async(&rt).iter(|| async {
            let result = store
                .hget(black_box("user:1000"), black_box("field_0"))
                .unwrap();
            black_box(result);
        });
    });

    // HDEL
    group.bench_function("hdel", |b| {
        b.iter_batched(
            || {
                rt.block_on(async {
                    store
                        .hset("temp:key", "temp_field", b"value".to_vec())
                        .unwrap();
                });
            },
            |_| {
                rt.block_on(async {
                    store
                        .hdel(black_box("temp:key"), &[String::from("temp_field")])
                        .unwrap();
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // HEXISTS
    group.bench_function("hexists", |b| {
        b.to_async(&rt).iter(|| async {
            let result = store
                .hexists(black_box("user:1000"), black_box("field_0"))
                .unwrap();
            black_box(result);
        });
    });

    // HLEN
    group.bench_function("hlen", |b| {
        b.to_async(&rt).iter(|| async {
            let len = store.hlen(black_box("user:1000")).unwrap();
            black_box(len);
        });
    });

    // HGETALL (100 fields)
    group.bench_function("hgetall_100_fields", |b| {
        b.to_async(&rt).iter(|| async {
            let all = store.hgetall(black_box("user:1000")).unwrap();
            black_box(all);
        });
    });

    // HINCRBY
    group.bench_function("hincrby", |b| {
        b.to_async(&rt).iter(|| async {
            store
                .hincrby(black_box("stats:1000"), black_box("counter"), black_box(1))
                .unwrap();
        });
    });

    // HINCRBYFLOAT
    group.bench_function("hincrbyfloat", |b| {
        b.to_async(&rt).iter(|| async {
            store
                .hincrbyfloat(black_box("stats:1000"), black_box("score"), black_box(1.5))
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: Hash vs Redis comparison baseline
/// Compare with Redis published benchmarks
fn bench_hash_vs_redis(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_vs_redis");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();
    let store = HashStore::new();

    // Redis HSET benchmark baseline: ~100K ops/sec = 10µs per op
    // Synap target: <100µs (10x slower is acceptable for Rust safety)
    group.bench_function("synap_hset_vs_redis_baseline", |b| {
        b.to_async(&rt).iter(|| async {
            store
                .hset(
                    black_box("user:1000"),
                    black_box("name"),
                    black_box(b"Alice".to_vec()),
                )
                .unwrap();
        });
    });

    // Redis HGET benchmark baseline: ~200K ops/sec = 5µs per op
    // Synap target: <50µs (10x slower is acceptable)
    rt.block_on(async {
        store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();
    });

    group.bench_function("synap_hget_vs_redis_baseline", |b| {
        b.to_async(&rt).iter(|| async {
            let result = store
                .hget(black_box("user:1000"), black_box("name"))
                .unwrap();
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: Memory efficiency
fn bench_hash_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_memory");
    group.sample_size(50);

    let rt = Runtime::new().unwrap();

    // Measure memory for different hash sizes
    for num_hashes in [100, 1_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::new("populate_hashes", num_hashes),
            num_hashes,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let store = HashStore::new();

                    for i in 0..count {
                        let key = format!("user:{}", i);
                        store.hset(&key, "name", b"Alice".to_vec()).unwrap();
                        store.hset(&key, "age", b"30".to_vec()).unwrap();
                        store
                            .hset(&key, "email", b"alice@example.com".to_vec())
                            .unwrap();
                    }

                    black_box(store);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_hash_set,
    bench_hash_get,
    bench_hash_getall,
    bench_hash_del,
    bench_hash_incr,
    bench_hash_bulk,
    bench_hash_concurrent,
    bench_hash_sharding,
    bench_hash_operations_summary,
    bench_hash_vs_redis,
    bench_hash_memory
);

criterion_main!(benches);
