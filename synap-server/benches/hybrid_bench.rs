use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use synap_server::core::{KVConfig, KVStore};
use tokio::runtime::Runtime;

/// Benchmark: HashMap vs RadixTrie for small datasets
fn bench_small_dataset_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_small_dataset");

    for key_count in [100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*key_count as u64));

        group.bench_with_input(
            BenchmarkId::new("insert_small", key_count),
            key_count,
            |b, &key_count| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    let store = KVStore::new(KVConfig::default());

                    // Small dataset (uses HashMap)
                    for i in 0..key_count {
                        let key = format!("key_{}", i);
                        store.set(&key, vec![0u8; 64], None).await.unwrap();
                    }

                    store
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("read_small", key_count),
            key_count,
            |b, &key_count| {
                let rt = Runtime::new().unwrap();
                let store = KVStore::new(KVConfig::default());

                // Pre-populate
                rt.block_on(async {
                    for i in 0..key_count {
                        let key = format!("key_{}", i);
                        store.set(&key, vec![0u8; 64], None).await.unwrap();
                    }
                });

                b.to_async(&rt).iter(|| async {
                    for i in 0..key_count {
                        let key = format!("key_{}", i);
                        let _ = store.get(&key).await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Performance across the upgrade threshold
fn bench_upgrade_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_upgrade");
    group.sample_size(10);

    // Test performance at different scales
    for key_count in [5000, 10000, 20000].iter() {
        group.bench_with_input(
            BenchmarkId::new("cross_threshold", key_count),
            key_count,
            |b, &key_count| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    let store = KVStore::new(KVConfig::default());

                    // Insert across threshold
                    for i in 0..key_count {
                        let key = format!("key_{:08}", i);
                        store.set(&key, vec![0u8; 64], None).await.unwrap();
                    }

                    // Verify random access
                    let mid = key_count / 2;
                    let key = format!("key_{:08}", mid);
                    let _ = store.get(&key).await.unwrap();

                    store
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Prefix search in HashMap vs RadixTrie
fn bench_prefix_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_prefix_search");

    // Small dataset (HashMap)
    group.bench_function("prefix_hashmap_1k", |b| {
        let rt = Runtime::new().unwrap();
        let store = KVStore::new(KVConfig::default());

        rt.block_on(async {
            for i in 0..1000 {
                let key = format!("user:{}", i);
                store.set(&key, vec![0], None).await.unwrap();
            }
        });

        b.to_async(&rt).iter(|| async {
            let _ = store.scan(Some("user:"), 2000).await.unwrap();
        });
    });

    // Large dataset (RadixTrie after upgrade)
    group.bench_function("prefix_radixtrie_100k", |b| {
        let rt = Runtime::new().unwrap();
        let store = KVStore::new(KVConfig::default());

        rt.block_on(async {
            for i in 0..100_000 {
                let key = format!("user:{:08}", i);
                store.set(&key, vec![0], None).await.unwrap();
            }
        });

        b.to_async(&rt).iter(|| async {
            let _ = store.scan(Some("user:"), 2000).await.unwrap();
        });
    });

    group.finish();
}

/// Benchmark: Random access patterns
fn bench_random_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_random_access");

    for key_count in [1000, 10000, 50000].iter() {
        group.bench_with_input(
            BenchmarkId::new("random_reads", key_count),
            key_count,
            |b, &key_count| {
                let rt = Runtime::new().unwrap();
                let store = KVStore::new(KVConfig::default());

                // Pre-populate
                rt.block_on(async {
                    for i in 0..key_count {
                        let key = format!("key_{:08}", i);
                        store.set(&key, vec![0u8; 64], None).await.unwrap();
                    }
                });

                b.to_async(&rt).iter(|| async {
                    // Random access pattern
                    for i in (0..key_count).step_by(100) {
                        let key = format!("key_{:08}", i);
                        let _ = store.get(&key).await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Mixed operations (SET/GET/DELETE)
fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_mixed_ops");
    group.sample_size(20);

    for key_count in [1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("mixed", key_count),
            key_count,
            |b, &key_count| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    let store = KVStore::new(KVConfig::default());

                    // Mix of operations
                    for i in 0..key_count {
                        let key = format!("key_{}", i);

                        // SET
                        store.set(&key, vec![i as u8], None).await.unwrap();

                        // GET
                        let _ = store.get(&key).await.unwrap();

                        // DELETE every 10th key
                        if i % 10 == 0 {
                            store.delete(&key).await.unwrap();
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_small_dataset_performance,
    bench_upgrade_threshold,
    bench_prefix_search,
    bench_random_access,
    bench_mixed_operations,
);

criterion_main!(benches);
