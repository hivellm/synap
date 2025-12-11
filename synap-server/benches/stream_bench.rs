use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use synap_server::core::{StreamConfig, StreamManager};
use tokio::runtime::Runtime;

/// Benchmark: Stream publish throughput
fn bench_stream_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_publish");

    for message_size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*message_size as u64));

        group.bench_with_input(
            BenchmarkId::new("publish", message_size),
            message_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();
                let manager = StreamManager::new(StreamConfig::default());

                rt.block_on(async {
                    manager.create_room("bench_room").await.unwrap();
                });

                b.iter(|| {
                    rt.block_on(async {
                        let payload = vec![0u8; size];
                        manager
                            .publish("bench_room", "test_event", black_box(payload))
                            .await
                            .unwrap();
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Stream consume throughput
fn bench_stream_consume(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_consume");

    for count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(
            BenchmarkId::new("consume", count),
            count,
            |b, &message_count| {
                let rt = Runtime::new().unwrap();
                let manager = StreamManager::new(StreamConfig::default());

                rt.block_on(async {
                    manager.create_room("bench_room").await.unwrap();

                    // Pre-populate stream
                    for i in 0..message_count {
                        let payload = format!("message_{}", i).into_bytes();
                        manager
                            .publish("bench_room", "event", payload)
                            .await
                            .unwrap();
                    }
                });

                b.iter(|| {
                    rt.block_on(async {
                        let messages = manager
                            .consume("bench_room", "consumer1", 0, message_count)
                            .await
                            .unwrap();
                        black_box(messages);
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Ring buffer overflow handling
fn bench_stream_overflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_overflow");

    group.bench_function("overflow_10k", |b| {
        let rt = Runtime::new().unwrap();
        let mut config = StreamConfig::default();
        config.max_buffer_size = 1000; // Small buffer
        let manager = StreamManager::new(config);

        rt.block_on(async {
            manager.create_room("overflow_room").await.unwrap();
        });

        b.iter(|| {
            rt.block_on(async {
                // Publish beyond capacity
                for i in 0..10_000 {
                    let payload = format!("msg_{}", i).into_bytes();
                    manager
                        .publish("overflow_room", "event", black_box(payload))
                        .await
                        .unwrap();
                }
            })
        });
    });

    group.finish();
}

/// Benchmark: Multiple subscribers consuming from same stream
fn bench_stream_multi_subscriber(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_multi_subscriber");

    for subscriber_count in [1, 5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*subscriber_count as u64));

        group.bench_with_input(
            BenchmarkId::new("subscribers", subscriber_count),
            subscriber_count,
            |b, &count| {
                let rt = Runtime::new().unwrap();
                let manager = StreamManager::new(StreamConfig::default());

                rt.block_on(async {
                    manager.create_room("multi_room").await.unwrap();

                    // Pre-populate with 1000 messages
                    for i in 0..1000 {
                        let payload = format!("msg_{}", i).into_bytes();
                        manager
                            .publish("multi_room", "event", payload)
                            .await
                            .unwrap();
                    }
                });

                b.iter(|| {
                    rt.block_on(async {
                        // Multiple subscribers read concurrently
                        let mut handles = vec![];
                        for sub_id in 0..count {
                            let manager = manager.clone();
                            let handle = tokio::spawn(async move {
                                manager
                                    .consume("multi_room", &format!("sub_{}", sub_id), 0, 100)
                                    .await
                                    .unwrap()
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            black_box(handle.await.unwrap());
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Offset-based consumption patterns
fn bench_stream_offset_consumption(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_offset");

    let rt = Runtime::new().unwrap();
    let manager = StreamManager::new(StreamConfig::default());

    rt.block_on(async {
        manager.create_room("offset_room").await.unwrap();

        // Pre-populate with 10K messages
        for i in 0..10_000 {
            let payload = format!("message_{}", i).into_bytes();
            manager
                .publish("offset_room", "event", payload)
                .await
                .unwrap();
        }
    });

    group.bench_function("sequential_batches", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Consume in batches of 100
                for offset in (0..10_000).step_by(100) {
                    let messages = manager
                        .consume("offset_room", "consumer", offset, 100)
                        .await
                        .unwrap();
                    black_box(messages);
                }
            })
        });
    });

    group.bench_function("random_access", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Random offset access
                for offset in [0, 1000, 5000, 9000].iter() {
                    let messages = manager
                        .consume("offset_room", "consumer", *offset, 50)
                        .await
                        .unwrap();
                    black_box(messages);
                }
            })
        });
    });

    group.finish();
}

/// Benchmark: Room statistics retrieval
fn bench_stream_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_stats");

    let rt = Runtime::new().unwrap();
    let manager = StreamManager::new(StreamConfig::default());

    rt.block_on(async {
        // Create multiple rooms with data
        for room_id in 0..10 {
            let room_name = format!("room_{}", room_id);
            manager.create_room(&room_name).await.unwrap();

            for i in 0..100 {
                let payload = format!("msg_{}", i).into_bytes();
                manager.publish(&room_name, "event", payload).await.unwrap();
            }
        }
    });

    group.bench_function("single_room_stats", |b| {
        b.iter(|| {
            rt.block_on(async {
                let stats = manager.room_stats("room_0").await.unwrap();
                black_box(stats);
            })
        });
    });

    group.bench_function("all_rooms_list", |b| {
        b.iter(|| {
            rt.block_on(async {
                let rooms = manager.list_rooms().await;
                black_box(rooms);
            })
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_stream_publish,
    bench_stream_consume,
    bench_stream_overflow,
    bench_stream_multi_subscriber,
    bench_stream_offset_consumption,
    bench_stream_stats
);

criterion_main!(benches);
