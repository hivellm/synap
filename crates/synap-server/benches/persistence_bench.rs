use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::path::PathBuf;
use synap_server::core::{KVConfig, KVStore};
use synap_server::persistence::{AsyncWAL, Operation, PersistenceConfig, SnapshotManager};
use tokio::runtime::Runtime;

/// Benchmark: AsyncWAL group commit performance
fn bench_wal_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_throughput");

    for batch_size in [10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("async_wal_writes", batch_size),
            batch_size,
            |b, &batch_size| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    let mut config = PersistenceConfig::default();
                    config.wal.path = PathBuf::from("./target/bench_wal_async.wal");
                    config.wal.fsync_mode = synap_server::persistence::types::FsyncMode::Periodic;

                    let wal = AsyncWAL::open(config.wal).await.unwrap();

                    // Write batch
                    for i in 0..batch_size {
                        let op = Operation::KVSet {
                            key: format!("key_{}", i),
                            value: vec![0u8; 64],
                            ttl: None,
                        };
                        wal.append(op).await.unwrap();
                    }
                });
            },
        );
    }

    // Cleanup
    let _ = std::fs::remove_file("./target/bench_wal_async.wal");

    group.finish();
}

/// Benchmark: Streaming snapshot vs traditional
fn bench_snapshot_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_memory");
    group.sample_size(10);

    for key_count in [1000, 10000, 100000].iter() {
        group.throughput(Throughput::Elements(*key_count as u64));

        group.bench_with_input(
            BenchmarkId::new("streaming_snapshot", key_count),
            key_count,
            |b, &key_count| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    // Create KV store with data
                    let mut kv_config = KVConfig::default();
                    kv_config.max_memory_mb = 4096;
                    let store = KVStore::new(kv_config);

                    // Populate
                    for i in 0..key_count {
                        let key = format!("key_{:08}", i);
                        store.set(&key, vec![0u8; 64], None).await.unwrap();
                    }

                    // Create snapshot
                    let mut snapshot_config =
                        synap_server::persistence::types::SnapshotConfig::default();
                    snapshot_config.directory = PathBuf::from("./target/bench_snapshots");
                    snapshot_config.enabled = true;

                    let snapshot_mgr = SnapshotManager::new(snapshot_config);
                    snapshot_mgr
                        .create_snapshot(&store, None, None, 0)
                        .await
                        .unwrap()
                });
            },
        );
    }

    // Cleanup
    let _ = std::fs::remove_dir_all("./target/bench_snapshots");

    group.finish();
}

/// Benchmark: Snapshot load performance
fn bench_snapshot_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_load");
    group.sample_size(20);

    // Setup: Create a snapshot to load
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let store = KVStore::new(KVConfig::default());
        for i in 0..10000 {
            let key = format!("key_{:08}", i);
            store.set(&key, vec![0u8; 64], None).await.unwrap();
        }

        let mut snapshot_config = synap_server::persistence::types::SnapshotConfig::default();
        snapshot_config.directory = PathBuf::from("./target/bench_load");
        snapshot_config.enabled = true;

        let snapshot_mgr = SnapshotManager::new(snapshot_config);
        snapshot_mgr
            .create_snapshot(&store, None, None, 0)
            .await
            .unwrap();
    });

    group.bench_function("load_snapshot", |b| {
        b.to_async(&rt).iter(|| async {
            let mut snapshot_config = synap_server::persistence::types::SnapshotConfig::default();
            snapshot_config.directory = PathBuf::from("./target/bench_load");

            let snapshot_mgr = SnapshotManager::new(snapshot_config);
            snapshot_mgr.load_latest().await.unwrap()
        });
    });

    // Cleanup
    let _ = std::fs::remove_dir_all("./target/bench_load");

    group.finish();
}

/// Benchmark: Recovery from WAL + Snapshot
fn bench_recovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("recovery");
    group.sample_size(10);

    // Setup: Create snapshot + WAL entries
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let store = KVStore::new(KVConfig::default());

        // Create initial snapshot
        for i in 0..5000 {
            let key = format!("key_{:08}", i);
            store.set(&key, vec![0u8; 64], None).await.unwrap();
        }

        let mut snapshot_config = synap_server::persistence::types::SnapshotConfig::default();
        snapshot_config.directory = PathBuf::from("./target/bench_recovery");
        let snapshot_mgr = SnapshotManager::new(snapshot_config);
        snapshot_mgr
            .create_snapshot(&store, None, None, 0)
            .await
            .unwrap();

        // Add more data to WAL
        let mut wal_config = synap_server::persistence::types::WALConfig::default();
        wal_config.path = PathBuf::from("./target/bench_recovery/recovery.wal");
        let wal = AsyncWAL::open(wal_config).await.unwrap();

        for i in 5000..10000 {
            let op = Operation::KVSet {
                key: format!("key_{:08}", i),
                value: vec![0u8; 64],
                ttl: None,
            };
            wal.append(op).await.unwrap();
        }
    });

    group.bench_function("full_recovery", |b| {
        b.to_async(&rt).iter(|| async {
            // Simulate recovery process
            let mut snapshot_config = synap_server::persistence::types::SnapshotConfig::default();
            snapshot_config.directory = PathBuf::from("./target/bench_recovery");

            let snapshot_mgr = SnapshotManager::new(snapshot_config);
            let snapshot = snapshot_mgr.load_latest().await.unwrap();

            // Load WAL entries
            let _wal_config = synap_server::persistence::types::WALConfig::default();
            let _path = PathBuf::from("./target/bench_recovery/recovery.wal");

            snapshot
        });
    });

    // Cleanup
    let _ = std::fs::remove_dir_all("./target/bench_recovery");

    group.finish();
}

/// Benchmark: Concurrent WAL writes
fn bench_concurrent_wal(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_wal");

    for num_writers in [1, 4, 16, 64].iter() {
        group.bench_with_input(
            BenchmarkId::new("parallel_writes", num_writers),
            num_writers,
            |b, &num_writers| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    let mut config = PersistenceConfig::default();
                    config.wal.path = PathBuf::from("./target/bench_concurrent_wal.wal");
                    config.wal.fsync_mode = synap_server::persistence::types::FsyncMode::Periodic;

                    let wal = AsyncWAL::open(config.wal).await.unwrap();

                    // Parallel writes
                    let mut handles = Vec::new();
                    for writer_id in 0..num_writers {
                        let wal_clone = wal.clone();
                        let handle = tokio::spawn(async move {
                            for i in 0..100 {
                                let op = Operation::KVSet {
                                    key: format!("writer_{}_key_{}", writer_id, i),
                                    value: vec![0u8; 64],
                                    ttl: None,
                                };
                                wal_clone.append(op).await.unwrap();
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

    // Cleanup
    let _ = std::fs::remove_file("./target/bench_concurrent_wal.wal");

    group.finish();
}

criterion_group!(
    benches,
    bench_wal_throughput,
    bench_snapshot_memory,
    bench_snapshot_load,
    bench_recovery,
    bench_concurrent_wal,
);

criterion_main!(benches);
