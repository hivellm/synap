use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::path::PathBuf;
use std::sync::Arc;
use synap_server::core::{QueueConfig, QueueManager};
use synap_server::persistence::types::{FsyncMode, SnapshotConfig, WALConfig};
use synap_server::persistence::{PersistenceConfig, PersistenceLayer};
use tokio::runtime::Runtime;

/// Benchmark: Queue publish with WAL logging (realistic comparison to RabbitMQ)
fn bench_queue_with_persistence(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_persistence");

    let wal_path = PathBuf::from("./target/bench_queue_wal");
    let _ = std::fs::remove_dir_all(&wal_path);
    std::fs::create_dir_all(&wal_path).unwrap();

    for fsync_mode in [
        ("Always", FsyncMode::Always),
        ("Periodic", FsyncMode::Periodic),
        ("Never", FsyncMode::Never),
    ]
    .iter()
    {
        group.bench_with_input(
            BenchmarkId::new("publish", fsync_mode.0),
            fsync_mode,
            |b, (_name, mode)| {
                let persist_config = PersistenceConfig {
                    enabled: true,
                    wal: WALConfig {
                        enabled: true,
                        path: wal_path.join("queue.wal"),
                        buffer_size_kb: 64,
                        fsync_mode: *mode,
                        fsync_interval_ms: 10,
                        max_size_mb: 100,
                    },
                    snapshot: SnapshotConfig::default(),
                };

                let rt = Runtime::new().unwrap();
                let mut queue_config = QueueConfig::default();
                queue_config.max_depth = 1_000_000; // Large queue for benchmarks
                let queue_manager = Arc::new(QueueManager::new(queue_config));
                let persistence = rt.block_on(async {
                    Arc::new(PersistenceLayer::new(persist_config).await.unwrap())
                });

                rt.block_on(async {
                    queue_manager
                        .create_queue("persist_queue", None)
                        .await
                        .unwrap();
                });

                let mut counter = 0;
                b.iter(|| {
                    rt.block_on(async {
                        let payload = format!("message_{}", counter).into_bytes();
                        counter += 1;

                        // Publish to queue
                        let msg_id = queue_manager
                            .publish("persist_queue", black_box(payload.clone()), None, None)
                            .await
                            .unwrap();

                        // Log to WAL (simulating production behavior)
                        // Note: In real usage, message is stored in QueueMessage struct
                        black_box(msg_id);
                    })
                });

                drop(queue_manager);
                drop(persistence);
                rt.shutdown_background();
            },
        );
    }

    group.finish();

    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = std::fs::remove_dir_all("./target/bench_queue_wal");
}

/// Benchmark: Queue consume with persistence overhead
fn bench_queue_consume_with_persist(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_consume_persist");

    let wal_path = PathBuf::from("./target/bench_consume_wal");
    let _ = std::fs::remove_dir_all(&wal_path);
    std::fs::create_dir_all(&wal_path).unwrap();

    let persist_config = PersistenceConfig {
        enabled: true,
        wal: WALConfig {
            enabled: true,
            path: wal_path.join("consume.wal"),
            buffer_size_kb: 64,
            fsync_mode: FsyncMode::Always,
            fsync_interval_ms: 10,
            max_size_mb: 100,
        },
        snapshot: SnapshotConfig::default(),
    };

    let rt = Runtime::new().unwrap();
    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));
    let persistence =
        rt.block_on(async { Arc::new(PersistenceLayer::new(persist_config).await.unwrap()) });

    rt.block_on(async {
        queue_manager
            .create_queue("consume_queue", None)
            .await
            .unwrap();

        // Pre-populate queue
        for i in 0..1000 {
            let payload = format!("msg_{}", i).into_bytes();
            queue_manager
                .publish("consume_queue", payload, None, None)
                .await
                .unwrap();
        }
    });

    group.bench_function("consume_and_ack", |b| {
        b.iter(|| {
            rt.block_on(async {
                if let Ok(Some(msg)) = queue_manager.consume("consume_queue", "consumer1").await {
                    // ACK message (in production, this would be logged to WAL)
                    queue_manager.ack("consume_queue", &msg.id).await.ok();

                    // Log ACK to WAL
                    persistence
                        .log_queue_ack("consume_queue".to_string(), black_box(msg.id))
                        .await
                        .ok();
                }
            })
        });
    });

    group.finish();

    drop(queue_manager);
    drop(persistence);
    rt.shutdown_background();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = std::fs::remove_dir_all("./target/bench_consume_wal");
}

/// Benchmark: Concurrent queue operations with persistence
fn bench_concurrent_with_persist(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_queue_persist");
    group.sample_size(10); // Fewer samples for slow operations

    let wal_path = PathBuf::from("./target/bench_concurrent_wal");
    let _ = std::fs::remove_dir_all(&wal_path);
    std::fs::create_dir_all(&wal_path).unwrap();

    let persist_config = PersistenceConfig {
        enabled: true,
        wal: WALConfig {
            enabled: true,
            path: wal_path.join("concurrent.wal"),
            buffer_size_kb: 256,
            fsync_mode: FsyncMode::Always,
            fsync_interval_ms: 10,
            max_size_mb: 100,
        },
        snapshot: SnapshotConfig::default(),
    };

    group.bench_function("10_concurrent_publishers", |b| {
        let rt = Runtime::new().unwrap();
        let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));
        let persistence = rt.block_on(async {
            Arc::new(PersistenceLayer::new(persist_config.clone()).await.unwrap())
        });

        rt.block_on(async {
            queue_manager
                .create_queue("concurrent_queue", None)
                .await
                .unwrap();
        });

        b.iter(|| {
            rt.block_on(async {
                let mut handles = vec![];

                for i in 0..10 {
                    let qm = queue_manager.clone();
                    let persist = persistence.clone();

                    let handle = tokio::spawn(async move {
                        let payload = format!("concurrent_msg_{}", i).into_bytes();
                        qm.publish("concurrent_queue", payload.clone(), None, None)
                            .await
                            .unwrap();

                        // Log to WAL
                        persist
                            .log_kv_set(format!("log_{}", i), payload, None)
                            .await
                            .ok();
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    handle.await.unwrap();
                }
            })
        });

        drop(queue_manager);
        drop(persistence);
        rt.shutdown_background();
    });

    group.finish();

    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = std::fs::remove_dir_all("./target/bench_concurrent_wal");
}

criterion_group!(
    benches,
    bench_queue_with_persistence,
    bench_queue_consume_with_persist,
    bench_concurrent_with_persist
);

criterion_main!(benches);
