use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use synap_server::core::ListStore;
use tokio::runtime::Runtime;

/// Benchmark: List PUSH operations (LPUSH/RPUSH)
/// Target: <100µs p99 latency
fn bench_list_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_push");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();

    // LPUSH single element
    group.bench_function("lpush_single", |b| {
        let store = ListStore::new();
        b.to_async(&rt).iter(|| async {
            store
                .lpush(black_box("mylist"), vec![b"value".to_vec()], false)
                .unwrap();
        });
    });

    // RPUSH single element
    group.bench_function("rpush_single", |b| {
        let store = ListStore::new();
        b.to_async(&rt).iter(|| async {
            store
                .rpush(black_box("mylist"), vec![b"value".to_vec()], false)
                .unwrap();
        });
    });

    // LPUSH 10 elements at once
    group.bench_function("lpush_10_elements", |b| {
        let store = ListStore::new();
        let values: Vec<Vec<u8>> = (0..10)
            .map(|i| format!("value-{}", i).into_bytes())
            .collect();
        b.to_async(&rt).iter(|| async {
            store
                .lpush(black_box("mylist"), black_box(values.clone()), false)
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: List POP operations (LPOP/RPOP)
/// Target: <100µs p99 latency
fn bench_list_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_pop");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();

    // LPOP single element
    group.bench_function("lpop_single", |b| {
        b.iter_batched(
            || {
                let store = ListStore::new();
                rt.block_on(async {
                    // Pre-populate with 100 elements
                    let values: Vec<Vec<u8>> = (0..100)
                        .map(|i| format!("val-{}", i).into_bytes())
                        .collect();
                    store.rpush("mylist", values, false).unwrap();
                });
                store
            },
            |store| {
                rt.block_on(async {
                    let _ = store.lpop(black_box("mylist"), Some(1));
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // RPOP single element
    group.bench_function("rpop_single", |b| {
        b.iter_batched(
            || {
                let store = ListStore::new();
                rt.block_on(async {
                    let values: Vec<Vec<u8>> = (0..100)
                        .map(|i| format!("val-{}", i).into_bytes())
                        .collect();
                    store.rpush("mylist", values, false).unwrap();
                });
                store
            },
            |store| {
                rt.block_on(async {
                    let _ = store.rpop(black_box("mylist"), Some(1));
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark: List RANGE operation (LRANGE)
/// Target: <500µs for 100 elements
fn bench_list_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_range");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();

    for elem_count in [10, 50, 100, 500].iter() {
        let store = ListStore::new();

        // Pre-populate list
        rt.block_on(async {
            let values: Vec<Vec<u8>> = (0..*elem_count)
                .map(|i| format!("element-{}", i).into_bytes())
                .collect();
            store.rpush("mylist", values, false).unwrap();
        });

        group.throughput(Throughput::Elements(*elem_count));

        group.bench_with_input(
            BenchmarkId::from_parameter(elem_count),
            elem_count,
            |b, _elem_count| {
                b.to_async(&rt).iter(|| async {
                    let result = store
                        .lrange(black_box("mylist"), black_box(0), black_box(-1))
                        .unwrap();
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: List INDEX operation (LINDEX)
/// Target: <50µs p99 latency
fn bench_list_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_index");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();
    let store = ListStore::new();

    // Pre-populate list with 100 elements
    rt.block_on(async {
        let values: Vec<Vec<u8>> = (0..100)
            .map(|i| format!("val-{}", i).into_bytes())
            .collect();
        store.rpush("mylist", values, false).unwrap();
    });

    group.bench_function("lindex_middle", |b| {
        b.to_async(&rt).iter(|| async {
            let result = store.lindex(black_box("mylist"), black_box(50)).unwrap();
            black_box(result);
        });
    });

    group.bench_function("lindex_negative", |b| {
        b.to_async(&rt).iter(|| async {
            let result = store.lindex(black_box("mylist"), black_box(-1)).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: List SET operation (LSET)
/// Target: <100µs p99 latency
fn bench_list_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_set");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();
    let store = ListStore::new();

    // Pre-populate list
    rt.block_on(async {
        let values: Vec<Vec<u8>> = (0..100)
            .map(|i| format!("val-{}", i).into_bytes())
            .collect();
        store.rpush("mylist", values, false).unwrap();
    });

    group.bench_function("lset_middle", |b| {
        b.to_async(&rt).iter(|| async {
            store
                .lset(
                    black_box("mylist"),
                    black_box(50),
                    black_box(b"new".to_vec()),
                )
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: List TRIM operation (LTRIM)
/// Target: <200µs p99 latency
fn bench_list_trim(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_trim");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();

    for elem_count in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(elem_count),
            elem_count,
            |b, &elem_count| {
                b.iter_batched(
                    || {
                        let store = ListStore::new();
                        rt.block_on(async {
                            let values: Vec<Vec<u8>> = (0..elem_count)
                                .map(|i| format!("val-{}", i).into_bytes())
                                .collect();
                            store.rpush("mylist", values, false).unwrap();
                        });
                        store
                    },
                    |store| {
                        rt.block_on(async {
                            store
                                .ltrim(black_box("mylist"), black_box(10), black_box(90))
                                .unwrap();
                        });
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark: List REM operation (LREM)
/// Target: <300µs p99 latency
fn bench_list_rem(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_rem");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();

    group.bench_function("lrem_remove_2", |b| {
        b.iter_batched(
            || {
                let store = ListStore::new();
                rt.block_on(async {
                    let mut values = vec![];
                    for i in 0..100 {
                        if i % 10 == 0 {
                            values.push(b"target".to_vec());
                        } else {
                            values.push(format!("val-{}", i).into_bytes());
                        }
                    }
                    store.rpush("mylist", values, false).unwrap();
                });
                store
            },
            |store| {
                rt.block_on(async {
                    store
                        .lrem(
                            black_box("mylist"),
                            black_box(2),
                            black_box(b"target".to_vec()),
                        )
                        .unwrap();
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark: List INSERT operation (LINSERT)
/// Target: <200µs p99 latency
fn bench_list_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_insert");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();
    let store = ListStore::new();

    // Pre-populate list
    rt.block_on(async {
        let values: Vec<Vec<u8>> = (0..100)
            .map(|i| format!("val-{}", i).into_bytes())
            .collect();
        store.rpush("mylist", values, false).unwrap();
    });

    group.bench_function("linsert_before_middle", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = store.linsert(
                black_box("mylist"),
                black_box(true),
                black_box(b"val-50".to_vec()),
                black_box(b"inserted".to_vec()),
            );
        });
    });

    group.finish();
}

/// Benchmark: List RPOPLPUSH operation
/// Target: <150µs p99 latency
fn bench_list_rpoplpush(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_rpoplpush");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();

    group.bench_function("rpoplpush", |b| {
        b.iter_batched(
            || {
                let store = ListStore::new();
                rt.block_on(async {
                    let values: Vec<Vec<u8>> =
                        (0..10).map(|i| format!("val-{}", i).into_bytes()).collect();
                    store.rpush("source", values, false).unwrap();
                });
                store
            },
            |store| {
                rt.block_on(async {
                    let _ = store.rpoplpush(black_box("source"), black_box("dest"));
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark: List LEN operation (LLEN)
/// Target: <50µs p99 latency
fn bench_list_len(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_len");
    group.sample_size(1000);

    let rt = Runtime::new().unwrap();

    for elem_count in [10, 100, 1000].iter() {
        let store = ListStore::new();

        // Pre-populate list
        rt.block_on(async {
            let values: Vec<Vec<u8>> = (0..*elem_count)
                .map(|i| format!("val-{}", i).into_bytes())
                .collect();
            store.rpush("mylist", values, false).unwrap();
        });

        group.bench_with_input(
            BenchmarkId::from_parameter(elem_count),
            elem_count,
            |b, _elem_count| {
                b.to_async(&rt).iter(|| async {
                    let result = store.llen(black_box("mylist")).unwrap();
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Concurrent list access
fn bench_list_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_concurrent");
    group.sample_size(100);

    let rt = Runtime::new().unwrap();

    group.bench_function("concurrent_10_threads", |b| {
        b.to_async(&rt).iter(|| async {
            let store = std::sync::Arc::new(ListStore::new());
            let mut handles = vec![];

            for i in 0..10 {
                let store_clone = store.clone();
                let handle = tokio::spawn(async move {
                    for j in 0..100 {
                        let values = vec![format!("thread-{}-val-{}", i, j).into_bytes()];
                        let _ = store_clone.rpush("concurrent", values, false);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });

    group.finish();
}

/// Benchmark: List with large values
fn bench_list_large_values(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_large_values");
    group.sample_size(500);

    let rt = Runtime::new().unwrap();

    for size in [1024, 10240, 102400].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let store = ListStore::new();
            let large_value = vec![0u8; size];

            b.to_async(&rt).iter(|| async {
                store
                    .rpush(
                        black_box("mylist"),
                        vec![black_box(large_value.clone())],
                        false,
                    )
                    .unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_list_push,
    bench_list_pop,
    bench_list_range,
    bench_list_index,
    bench_list_set,
    bench_list_trim,
    bench_list_rem,
    bench_list_insert,
    bench_list_rpoplpush,
    bench_list_len,
    bench_list_concurrent,
    bench_list_large_values,
);
criterion_main!(benches);
