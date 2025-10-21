use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use synap_server::{KVConfig, KVStore};

fn bench_kv_set(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(KVStore::new(KVConfig::default()));

    c.bench_function("kv_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = black_box("test_key");
            let value = black_box(b"test_value".to_vec());
            store.set(key, value, None).await.unwrap();
        });
    });
}

fn bench_kv_get(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(KVStore::new(KVConfig::default()));

    // Pre-populate
    rt.block_on(async {
        store
            .set("test_key", b"test_value".to_vec(), None)
            .await
            .unwrap();
    });

    c.bench_function("kv_get", |b| {
        b.to_async(&rt).iter(|| async {
            let key = black_box("test_key");
            store.get(key).await.unwrap();
        });
    });
}

fn bench_kv_delete(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("kv_delete", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let store = Arc::new(KVStore::new(KVConfig::default()));
                rt.block_on(async {
                    store
                        .set("test_key", b"test_value".to_vec(), None)
                        .await
                        .unwrap();
                });
                store
            },
            |store| async move {
                let key = black_box("test_key");
                store.delete(key).await.unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_kv_incr(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(KVStore::new(KVConfig::default()));

    c.bench_function("kv_incr", |b| {
        b.to_async(&rt).iter(|| async {
            let key = black_box("counter");
            store.incr(key, 1).await.unwrap();
        });
    });
}

fn bench_kv_mset(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(KVStore::new(KVConfig::default()));

    let mut group = c.benchmark_group("kv_mset");

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let pairs: Vec<_> = (0..size)
                    .map(|i| (format!("key_{}", i), format!("value_{}", i).into_bytes()))
                    .collect();
                store.mset(black_box(pairs)).await.unwrap();
            });
        });
    }
    group.finish();
}

fn bench_kv_mget(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(KVStore::new(KVConfig::default()));

    // Pre-populate
    rt.block_on(async {
        for i in 0..1000 {
            store
                .set(
                    &format!("key_{}", i),
                    format!("value_{}", i).into_bytes(),
                    None,
                )
                .await
                .unwrap();
        }
    });

    let mut group = c.benchmark_group("kv_mget");

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let keys: Vec<_> = (0..size).map(|i| format!("key_{}", i)).collect();
                store.mget(black_box(&keys)).await.unwrap();
            });
        });
    }
    group.finish();
}

fn bench_kv_scan(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(KVStore::new(KVConfig::default()));

    // Pre-populate with prefixed keys
    rt.block_on(async {
        for i in 0..1000 {
            store
                .set(&format!("user:{}", i), b"data".to_vec(), None)
                .await
                .unwrap();
        }
    });

    c.bench_function("kv_scan", |b| {
        b.to_async(&rt).iter(|| async {
            store
                .scan(black_box(Some("user:")), black_box(100))
                .await
                .unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_kv_set,
    bench_kv_get,
    bench_kv_delete,
    bench_kv_incr,
    bench_kv_mset,
    bench_kv_mget,
    bench_kv_scan
);
criterion_main!(benches);
