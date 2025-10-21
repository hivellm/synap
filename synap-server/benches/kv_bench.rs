use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use synap_server::core::{KVConfig, KVStore};
use tokio::runtime::Runtime;

/// Benchmark: Memory overhead of StoredValue
fn bench_stored_value_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("stored_value_memory");
    
    // Test different value sizes
    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(BenchmarkId::new("set_persistent", size), size, |b, &size| {
            let rt = Runtime::new().unwrap();
            let store = KVStore::new(KVConfig::default());
            let value = vec![0u8; size];
            
            b.to_async(&rt).iter(|| async {
                store.set("test_key", black_box(value.clone()), None).await.unwrap();
            });
        });
        
        group.bench_with_input(BenchmarkId::new("set_expiring", size), size, |b, &size| {
            let rt = Runtime::new().unwrap();
            let store = KVStore::new(KVConfig::default());
            let value = vec![0u8; size];
            
            b.to_async(&rt).iter(|| async {
                store.set("test_key", black_box(value.clone()), Some(3600)).await.unwrap();
            });
        });
    }
    
    group.finish();
}

/// Benchmark: Sharded vs non-sharded concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_operations");
    
    for num_tasks in [1, 4, 16, 64].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_set", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                let rt = Runtime::new().unwrap();
                let store = KVStore::new(KVConfig::default());
                
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for i in 0..num_tasks {
                        let store_clone = store.clone();
                        let handle = tokio::spawn(async move {
                            for j in 0..100 {
                                let key = format!("key_{}_{}", i, j);
                                store_clone.set(&key, vec![1, 2, 3], None).await.unwrap();
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
        
        group.bench_with_input(
            BenchmarkId::new("concurrent_get", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                let rt = Runtime::new().unwrap();
                let store = KVStore::new(KVConfig::default());
                
                // Pre-populate
                rt.block_on(async {
                    for i in 0..1000 {
                        let key = format!("key_{}", i);
                        store.set(&key, vec![1, 2, 3], None).await.unwrap();
                    }
                });
                
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for i in 0..num_tasks {
                        let store_clone = store.clone();
                        let handle = tokio::spawn(async move {
                            for j in 0..100 {
                                let key = format!("key_{}", (i * 100 + j) % 1000);
                                let _ = store_clone.get(&key).await;
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

/// Benchmark: Write throughput with different batch sizes
fn bench_write_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_throughput");
    group.sample_size(50);
    
    for batch_size in [10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("sequential_writes", batch_size),
            batch_size,
            |b, &batch_size| {
                let rt = Runtime::new().unwrap();
                let store = KVStore::new(KVConfig::default());
                
                b.to_async(&rt).iter(|| async {
                    for i in 0..batch_size {
                        let key = format!("key_{}", i);
                        store.set(&key, vec![0u8; 64], None).await.unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark: Read latency percentiles
fn bench_read_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_latency");
    group.sample_size(1000);
    
    let rt = Runtime::new().unwrap();
    let store = KVStore::new(KVConfig::default());
    
    // Pre-populate with 10K keys
    rt.block_on(async {
        for i in 0..10000 {
            let key = format!("key_{}", i);
            store.set(&key, vec![0u8; 64], None).await.unwrap();
        }
    });
    
    group.bench_function("single_get", |b| {
        b.to_async(&rt).iter(|| async {
            let key = format!("key_{}", black_box(5000));
            let _ = store.get(&key).await;
        });
    });
    
    group.bench_function("batch_get_100", |b| {
        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let key = format!("key_{}", i);
                let _ = store.get(&key).await;
            }
        });
    });
    
    group.finish();
}

/// Benchmark: TTL cleanup performance
fn bench_ttl_cleanup(c: &mut Criterion) {
    let mut group = c.benchmark_group("ttl_cleanup");
    group.sample_size(20);
    
    for key_count in [1000, 10000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::new("adaptive_cleanup", key_count),
            key_count,
            |b, &key_count| {
                let rt = Runtime::new().unwrap();
                let mut config = KVConfig::default();
                config.max_memory_mb = 1024;
                let store = KVStore::new(config);
                
                // Populate with 50% expired keys
                rt.block_on(async {
                    for i in 0..key_count {
                        let key = format!("key_{}", i);
                        let ttl = if i % 2 == 0 { Some(1) } else { None };
                        store.set(&key, vec![0u8; 64], ttl).await.unwrap();
                    }
                    
                    // Wait for expiration
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                });
                
                b.to_async(&rt).iter(|| async {
                    // Call internal cleanup (via set to trigger it)
                    store.set("trigger", vec![1], None).await.unwrap();
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark: Memory footprint for 1M keys
fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_footprint");
    group.sample_size(10);
    
    group.bench_function("load_1m_keys", |b| {
        let rt = Runtime::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let mut config = KVConfig::default();
            config.max_memory_mb = 4096;
            let store = KVStore::new(config);
            
            // Load 1M keys
            for i in 0..1_000_000 {
                let key = format!("key_{:08}", i);
                store.set(&key, vec![0u8; 64], None).await.unwrap();
                
                // Sample progress
                if i % 100_000 == 0 {
                    let stats = store.stats().await;
                    println!("Loaded {} keys, memory: {}MB", 
                        stats.total_keys, 
                        stats.total_memory_bytes / 1024 / 1024
                    );
                }
            }
            
            store
        });
    });
    
    group.finish();
}

/// Benchmark: Shard distribution uniformity
fn bench_shard_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("shard_distribution");
    
    group.bench_function("check_uniformity", |b| {
        let rt = Runtime::new().unwrap();
        let store = KVStore::new(KVConfig::default());
        
        b.to_async(&rt).iter(|| async {
            // Insert 10K keys and check distribution
            for i in 0..10000 {
                let key = format!("key_{}", i);
                store.set(&key, vec![1], None).await.unwrap();
            }
            
            store.dbsize().await.unwrap()
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_stored_value_memory,
    bench_concurrent_operations,
    bench_write_throughput,
    bench_read_latency,
    bench_ttl_cleanup,
    bench_memory_footprint,
    bench_shard_distribution,
);

criterion_main!(benches);
