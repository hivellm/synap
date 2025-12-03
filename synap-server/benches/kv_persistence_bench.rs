use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use synap_server::core::KVStore;
use synap_server::persistence::types::{FsyncMode, SnapshotConfig, WALConfig};
use synap_server::persistence::{PersistenceConfig, PersistenceLayer};
use synap_server::{EvictionPolicy, KVConfig};
use tokio::runtime::Runtime;

/// Benchmark: KV Store with persistence enabled (realistic comparison to Redis)
fn bench_kv_with_persistence(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_persistence");

    // Setup persistence configuration
    let wal_path = PathBuf::from("./target/bench_persist_wal");
    let snapshot_path = PathBuf::from("./target/bench_persist_snapshot");

    // Clean up previous benchmarks
    let _ = std::fs::remove_dir_all(&wal_path);
    let _ = std::fs::remove_dir_all(&snapshot_path);
    std::fs::create_dir_all(&wal_path).unwrap();
    std::fs::create_dir_all(&snapshot_path).unwrap();

    let persist_config = PersistenceConfig {
        enabled: true,
        wal: WALConfig {
            enabled: true,
            path: wal_path.join("test.wal"),
            buffer_size_kb: 64,
            fsync_mode: FsyncMode::Always,
            fsync_interval_ms: 10,
            max_size_mb: 100,
        },
        snapshot: SnapshotConfig {
            enabled: true,
            directory: snapshot_path,
            interval_secs: 3600,
            operation_threshold: 10000,
            max_snapshots: 3,
            compression: false,
        },
    };

    let rt = Runtime::new().unwrap();

    // Initialize persistence layer and KV store
    let (kv_store, persistence) = rt.block_on(async {
        let kv = Arc::new(KVStore::new(KVConfig {
            max_memory_mb: 1024,
            eviction_policy: EvictionPolicy::Lru,
            ttl_cleanup_interval_ms: 1000,
            allow_flush_commands: false,
        }));

        let persist = Arc::new(PersistenceLayer::new(persist_config.clone()).await.unwrap());

        (kv, persist)
    });

    group.bench_function("set_with_wal", |b| {
        let mut counter = 0;
        b.iter(|| {
            rt.block_on(async {
                let key = format!("key_{}", counter);
                let value = b"test_value_with_persistence".to_vec();
                counter += 1;

                // Set in KV store
                kv_store
                    .set(&key, black_box(value.clone()), None)
                    .await
                    .unwrap();

                // Log to WAL (simulating what happens in production)
                persistence
                    .log_kv_set(black_box(key), black_box(value), None)
                    .await
                    .ok();
            })
        });
    });

    group.bench_function("set_with_wal_batch", |b| {
        let mut counter = 0;
        b.iter(|| {
            rt.block_on(async {
                // Batch of 10 operations
                for _ in 0..10 {
                    let key = format!("batch_key_{}", counter);
                    let value = b"batch_value".to_vec();
                    counter += 1;

                    kv_store.set(&key, value.clone(), None).await.unwrap();
                    persistence
                        .log_kv_set(black_box(key), black_box(value), None)
                        .await
                        .ok();
                }
            })
        });
    });

    group.bench_function("get_with_persistence", |b| {
        // Pre-populate data
        rt.block_on(async {
            for i in 0..1000 {
                let key = format!("read_key_{}", i);
                let value = b"read_value".to_vec();
                kv_store.set(&key, value, None).await.unwrap();
            }
        });

        let mut counter = 0;
        b.iter(|| {
            rt.block_on(async {
                let key = format!("read_key_{}", counter % 1000);
                counter += 1;
                kv_store.get(black_box(&key)).await.unwrap();
            })
        });
    });

    group.finish();

    // Cleanup
    drop(kv_store);
    drop(persistence);
    rt.shutdown_background();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = std::fs::remove_dir_all("./target/bench_persist_wal");
    let _ = std::fs::remove_dir_all("./target/bench_persist_snapshot");
}

/// Benchmark: Write throughput with different fsync modes
fn bench_fsync_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("fsync_modes");

    for fsync_mode in [
        ("EveryFlush", FsyncMode::Always),
        ("Periodic", FsyncMode::Periodic),
        ("Never", FsyncMode::Never),
    ]
    .iter()
    {
        group.bench_with_input(
            BenchmarkId::new("write", fsync_mode.0),
            fsync_mode,
            |b, (name, mode)| {
                let wal_path = PathBuf::from(format!("./target/bench_fsync_{}", name));
                let _ = std::fs::remove_dir_all(&wal_path);
                std::fs::create_dir_all(&wal_path).unwrap();

                let persist_config = PersistenceConfig {
                    enabled: true,
                    wal: WALConfig {
                        enabled: true,
                        path: wal_path.join("test.wal"),
                        buffer_size_kb: 64,
                        fsync_mode: *mode,
                        fsync_interval_ms: 10,
                        max_size_mb: 100,
                    },
                    snapshot: SnapshotConfig::default(),
                };

                let rt = Runtime::new().unwrap();
                let kv_store = Arc::new(KVStore::new(KVConfig::default()));
                let persistence = rt.block_on(async {
                    Arc::new(PersistenceLayer::new(persist_config).await.unwrap())
                });

                let mut counter = 0;
                b.iter(|| {
                    rt.block_on(async {
                        let key = format!("fsync_key_{}", counter);
                        let value = b"fsync_value".to_vec();
                        counter += 1;

                        kv_store.set(&key, value.clone(), None).await.unwrap();
                        persistence
                            .log_kv_set(black_box(key), black_box(value), None)
                            .await
                            .ok();
                    })
                });

                drop(kv_store);
                drop(persistence);
                rt.shutdown_background();
                std::thread::sleep(std::time::Duration::from_millis(100));
                let _ = std::fs::remove_dir_all(wal_path);
            },
        );
    }

    group.finish();
}

/// Benchmark: Recovery performance
fn bench_recovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistence_recovery");

    let wal_path = PathBuf::from("./target/bench_recovery_wal");
    let snapshot_path = PathBuf::from("./target/bench_recovery_snapshot");

    // Prepare data
    let _ = std::fs::remove_dir_all(&wal_path);
    let _ = std::fs::remove_dir_all(&snapshot_path);
    std::fs::create_dir_all(&wal_path).unwrap();
    std::fs::create_dir_all(&snapshot_path).unwrap();

    let persist_config = PersistenceConfig {
        enabled: true,
        wal: WALConfig {
            enabled: true,
            path: wal_path.join("test.wal"),
            buffer_size_kb: 64,
            fsync_mode: FsyncMode::Always,
            fsync_interval_ms: 10,
            max_size_mb: 100,
        },
        snapshot: SnapshotConfig {
            enabled: true,
            directory: snapshot_path.clone(),
            interval_secs: 3600,
            operation_threshold: 10000,
            max_snapshots: 3,
            compression: false,
        },
    };

    let rt = Runtime::new().unwrap();

    // Pre-populate WAL with data
    rt.block_on(async {
        let kv = Arc::new(KVStore::new(KVConfig::default()));
        let persist = Arc::new(PersistenceLayer::new(persist_config.clone()).await.unwrap());

        // Write 1000 operations
        for i in 0..1000 {
            let key = format!("recovery_key_{}", i);
            let value = format!("recovery_value_{}", i).into_bytes();
            kv.set(&key, value.clone(), None).await.unwrap();
            persist.log_kv_set(key, value, None).await.ok();
        }

        // Flush WAL
        std::thread::sleep(std::time::Duration::from_millis(50));
    });

    group.bench_function("recover_1000_operations", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _result = synap_server::persistence::recover(
                    &persist_config,
                    KVConfig::default(),
                    synap_server::QueueConfig::default(),
                )
                .await;
            })
        });
    });

    group.finish();

    // Cleanup
    rt.shutdown_background();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = std::fs::remove_dir_all("./target/bench_recovery_wal");
    let _ = std::fs::remove_dir_all("./target/bench_recovery_snapshot");
}

/// Benchmark: Snapshot creation performance
fn bench_snapshot_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_creation");

    for size in [100, 1000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("create", size),
            size,
            |b, &dataset_size| {
                let snapshot_path = PathBuf::from("./target/bench_snapshot");
                let _ = std::fs::remove_dir_all(&snapshot_path);
                std::fs::create_dir_all(&snapshot_path).unwrap();

                let persist_config = PersistenceConfig {
                    enabled: true,
                    wal: WALConfig::default(),
                    snapshot: SnapshotConfig {
                        enabled: true,
                        directory: snapshot_path.clone(),
                        interval_secs: 3600,
                        operation_threshold: 10000,
                        max_snapshots: 3,
                        compression: false,
                    },
                };

                let rt = Runtime::new().unwrap();
                let kv = Arc::new(KVStore::new(KVConfig::default()));
                let persistence = rt.block_on(async {
                    Arc::new(PersistenceLayer::new(persist_config).await.unwrap())
                });

                // Pre-populate data
                rt.block_on(async {
                    for i in 0..dataset_size {
                        let key = format!("snap_key_{}", i);
                        let value = format!("snap_value_{}", i).into_bytes();
                        kv.set(&key, value, None).await.unwrap();
                    }
                });

                b.iter(|| {
                    rt.block_on(async {
                        persistence
                            .maybe_snapshot(black_box(&*kv), None, None)
                            .await
                            .ok();
                    })
                });

                drop(kv);
                drop(persistence);
                rt.shutdown_background();
                std::thread::sleep(std::time::Duration::from_millis(100));
                let _ = std::fs::remove_dir_all(snapshot_path);
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_kv_with_persistence,
    bench_fsync_modes,
    bench_recovery,
    bench_snapshot_creation
);

criterion_main!(benches);
