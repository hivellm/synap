use std::hint::black_box;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use synap_server::core::{QueueConfig, QueueManager};
use tokio::runtime::Runtime;

/// Benchmark: Queue message memory overhead with Arc sharing
fn bench_queue_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_memory");

    for message_size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*message_size as u64));

        group.bench_with_input(
            BenchmarkId::new("publish", message_size),
            message_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();

                b.iter_batched(
                    || {
                        // Setup: Fresh queue manager for each batch
                        let mut config = QueueConfig::default();
                        config.max_depth = 100000; // Allow large queue
                        let manager = QueueManager::new(config);
                        rt.block_on(async {
                            manager.create_queue("test_queue", None).await.unwrap();
                        });
                        manager
                    },
                    |manager| {
                        rt.block_on(async {
                            let payload = vec![0u8; size];
                            manager
                                .publish("test_queue", black_box(payload), None, None)
                                .await
                                .unwrap();
                        })
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark: Concurrent publish/consume operations
fn bench_concurrent_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_queue");

    for num_consumers in [1, 4, 16, 32].iter() {
        group.bench_with_input(
            BenchmarkId::new("pubsub", num_consumers),
            num_consumers,
            |b, &num_consumers| {
                let rt = Runtime::new().unwrap();
                let mut config = QueueConfig::default();
                config.max_depth = 100000; // Allow large queue
                let manager = QueueManager::new(config);

                rt.block_on(async {
                    manager
                        .create_queue("concurrent_queue", None)
                        .await
                        .unwrap();
                });

                b.to_async(&rt).iter(|| async {
                    // Publish 1000 messages
                    for i in 0..1000 {
                        let payload = format!("message_{}", i).into_bytes();
                        manager
                            .publish("concurrent_queue", payload, None, None)
                            .await
                            .unwrap();
                    }

                    // Consume with multiple consumers
                    let mut handles = Vec::new();
                    for consumer_id in 0..num_consumers {
                        let manager_clone = manager.clone();
                        let handle = tokio::spawn(async move {
                            let consumer_name = format!("consumer_{}", consumer_id);
                            let mut consumed = 0;

                            loop {
                                match manager_clone
                                    .consume("concurrent_queue", &consumer_name)
                                    .await
                                {
                                    Ok(Some(msg)) => {
                                        manager_clone
                                            .ack("concurrent_queue", &msg.id)
                                            .await
                                            .unwrap();
                                        consumed += 1;
                                    }
                                    Ok(None) => break,
                                    Err(_) => break}
                            }
                            consumed
                        });
                        handles.push(handle);
                    }

                    let mut total = 0;
                    for handle in handles {
                        total += handle.await.unwrap();
                    }

                    total
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Priority queue ordering performance
fn bench_priority_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_queue");

    for num_messages in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*num_messages as u64));

        group.bench_with_input(
            BenchmarkId::new("publish_mixed_priority", num_messages),
            num_messages,
            |b, &num_messages| {
                let rt = Runtime::new().unwrap();
                let mut config = QueueConfig::default();
                config.max_depth = 100000; // Allow large queue
                let manager = QueueManager::new(config);

                rt.block_on(async {
                    manager.create_queue("priority_queue", None).await.unwrap();
                });

                b.to_async(&rt).iter(|| async {
                    // Publish with random priorities
                    for i in 0..num_messages {
                        let payload = vec![0u8; 64];
                        let priority = (i % 10) as u8;
                        manager
                            .publish("priority_queue", payload, Some(priority), None)
                            .await
                            .unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("consume_ordered", num_messages),
            num_messages,
            |b, &num_messages| {
                let rt = Runtime::new().unwrap();
                let mut config = QueueConfig::default();
                config.max_depth = 100000; // Allow large queue
                let manager = QueueManager::new(config);

                rt.block_on(async {
                    manager.create_queue("priority_queue", None).await.unwrap();

                    // Pre-populate
                    for i in 0..num_messages {
                        let payload = vec![0u8; 64];
                        let priority = (i % 10) as u8;
                        manager
                            .publish("priority_queue", payload, Some(priority), None)
                            .await
                            .unwrap();
                    }
                });

                b.to_async(&rt).iter(|| async {
                    for _ in 0..num_messages {
                        if let Ok(Some(msg)) = manager.consume("priority_queue", "consumer").await {
                            let _ = manager.ack("priority_queue", &msg.id).await;
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Message pending and ACK overhead
fn bench_pending_messages(c: &mut Criterion) {
    let mut group = c.benchmark_group("pending_messages");

    group.bench_function("ack_throughput", |b| {
        let rt = Runtime::new().unwrap();
        let mut config = QueueConfig::default();
        config.max_depth = 100000; // Allow large queue
        let manager = QueueManager::new(config);

        rt.block_on(async {
            manager.create_queue("ack_queue", None).await.unwrap();
        });

        b.to_async(&rt).iter(|| async {
            // Publish and consume 100 messages
            let mut message_ids = Vec::new();

            for i in 0..100 {
                let payload = format!("msg_{}", i).into_bytes();
                manager
                    .publish("ack_queue", payload, None, None)
                    .await
                    .unwrap();
            }

            for _ in 0..100 {
                if let Ok(Some(msg)) = manager.consume("ack_queue", "consumer").await {
                    message_ids.push(msg.id);
                }
            }

            // ACK all
            for msg_id in message_ids {
                manager.ack("ack_queue", &msg_id).await.unwrap();
            }
        });
    });

    group.bench_function("nack_requeue", |b| {
        let rt = Runtime::new().unwrap();
        let mut config = QueueConfig::default();
        config.max_depth = 100000; // Allow large queue
        let manager = QueueManager::new(config);

        rt.block_on(async {
            manager.create_queue("nack_queue", None).await.unwrap();
        });

        b.to_async(&rt).iter(|| async {
            // Publish 100 messages
            for i in 0..100 {
                let payload = format!("msg_{}", i).into_bytes();
                manager
                    .publish("nack_queue", payload, None, None)
                    .await
                    .unwrap();
            }

            // Consume and NACK (requeue)
            for _ in 0..100 {
                if let Ok(Some(msg)) = manager.consume("nack_queue", "consumer").await {
                    manager.nack("nack_queue", &msg.id, true).await.unwrap();
                }
            }
        });
    });

    group.finish();
}

/// Benchmark: Queue depth and memory usage
fn bench_queue_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_depth");
    group.sample_size(20);

    for depth in [1000, 10000, 100000].iter() {
        group.throughput(Throughput::Elements(*depth as u64));

        group.bench_with_input(BenchmarkId::new("fill_queue", depth), depth, |b, &depth| {
            let rt = Runtime::new().unwrap();

            b.to_async(&rt).iter(|| async {
                let mut config = QueueConfig::default();
                config.max_depth = depth;
                let manager = QueueManager::new(config);

                manager.create_queue("deep_queue", None).await.unwrap();

                for i in 0..depth {
                    let payload = format!("msg_{}", i).into_bytes();
                    manager
                        .publish("deep_queue", payload, None, None)
                        .await
                        .unwrap();
                }

                manager.stats("deep_queue").await.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark: Deadline checker performance
fn bench_deadline_checker(c: &mut Criterion) {
    let mut group = c.benchmark_group("deadline_checker");

    group.bench_function("expired_messages", |b| {
        let rt = Runtime::new().unwrap();
        let mut config = QueueConfig::default();
        config.ack_deadline_secs = 1; // Short deadline
        let manager = QueueManager::new(config);

        rt.block_on(async {
            manager.create_queue("deadline_queue", None).await.unwrap();

            // Publish and consume 100 messages (they'll expire)
            for i in 0..100 {
                let payload = format!("msg_{}", i).into_bytes();
                manager
                    .publish("deadline_queue", payload, None, None)
                    .await
                    .unwrap();
            }

            for _ in 0..100 {
                let _ = manager.consume("deadline_queue", "consumer").await;
            }

            // Wait for expiration
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        });

        b.to_async(&rt).iter(|| async {
            // Trigger deadline check by trying to consume
            let _ = manager.consume("deadline_queue", "checker").await;
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_queue_memory,
    bench_concurrent_queue,
    bench_priority_queue,
    bench_pending_messages,
    bench_queue_depth,
    bench_deadline_checker,
);

criterion_main!(benches);
