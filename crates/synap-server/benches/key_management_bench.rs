use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;
use synap_server::core::{
    HashStore, KVConfig, KVStore, KeyManager, ListStore, SetStore, SortedSetStore, ZAddOptions,
};
use tokio::runtime::Runtime;

/// Benchmark: EXISTS operation performance
fn bench_exists(c: &mut Criterion) {
    let mut group = c.benchmark_group("exists");

    let rt = Runtime::new().unwrap();
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let manager = KeyManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    );

    // Pre-populate with different types
    rt.block_on(async {
        kv_store
            .set("kv_key", b"value".to_vec(), None)
            .await
            .unwrap();
        hash_store
            .hset("hash_key", "field", b"value".to_vec())
            .unwrap();
        list_store
            .lpush("list_key", vec![b"value".to_vec()], false)
            .unwrap();
        set_store.sadd("set_key", vec![b"member".to_vec()]).unwrap();
        let _ = sorted_set_store.zadd("zset_key", b"member".to_vec(), 1.0, &ZAddOptions::default());
    });

    group.bench_function("exists_kv", |b| {
        b.to_async(&rt).iter(|| async {
            manager.exists(black_box("kv_key")).await.unwrap();
        });
    });

    group.bench_function("exists_hash", |b| {
        b.to_async(&rt).iter(|| async {
            manager.exists(black_box("hash_key")).await.unwrap();
        });
    });

    group.bench_function("exists_list", |b| {
        b.to_async(&rt).iter(|| async {
            manager.exists(black_box("list_key")).await.unwrap();
        });
    });

    group.bench_function("exists_nonexistent", |b| {
        b.to_async(&rt).iter(|| async {
            manager.exists(black_box("nonexistent_key")).await.unwrap();
        });
    });

    group.finish();
}

/// Benchmark: TYPE operation performance
fn bench_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("type");

    let rt = Runtime::new().unwrap();
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let manager = KeyManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    );

    // Pre-populate with different types
    rt.block_on(async {
        kv_store
            .set("kv_key", b"value".to_vec(), None)
            .await
            .unwrap();
        hash_store
            .hset("hash_key", "field", b"value".to_vec())
            .unwrap();
        list_store
            .lpush("list_key", vec![b"value".to_vec()], false)
            .unwrap();
        set_store.sadd("set_key", vec![b"member".to_vec()]).unwrap();
        let _ = sorted_set_store.zadd("zset_key", b"member".to_vec(), 1.0, &ZAddOptions::default());
    });

    group.bench_function("type_kv", |b| {
        b.to_async(&rt).iter(|| async {
            manager.key_type(black_box("kv_key")).await.unwrap();
        });
    });

    group.bench_function("type_hash", |b| {
        b.to_async(&rt).iter(|| async {
            manager.key_type(black_box("hash_key")).await.unwrap();
        });
    });

    group.bench_function("type_list", |b| {
        b.to_async(&rt).iter(|| async {
            manager.key_type(black_box("list_key")).await.unwrap();
        });
    });

    group.bench_function("type_set", |b| {
        b.to_async(&rt).iter(|| async {
            manager.key_type(black_box("set_key")).await.unwrap();
        });
    });

    group.bench_function("type_zset", |b| {
        b.to_async(&rt).iter(|| async {
            manager.key_type(black_box("zset_key")).await.unwrap();
        });
    });

    group.bench_function("type_nonexistent", |b| {
        b.to_async(&rt).iter(|| async {
            manager
                .key_type(black_box("nonexistent_key"))
                .await
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: RENAME operation performance
fn bench_rename(c: &mut Criterion) {
    let mut group = c.benchmark_group("rename");

    for data_type in ["kv", "hash", "list"].iter() {
        let data_type = *data_type;
        group.bench_with_input(
            BenchmarkId::new("rename", data_type),
            &data_type,
            |b, data_type| {
                let rt = Runtime::new().unwrap();
                let kv_store = Arc::new(KVStore::new(KVConfig::default()));
                let hash_store = Arc::new(HashStore::new());
                let list_store = Arc::new(ListStore::new());
                let set_store = Arc::new(SetStore::new());
                let sorted_set_store = Arc::new(SortedSetStore::new());

                let manager = KeyManager::new(
                    kv_store.clone(),
                    hash_store.clone(),
                    list_store.clone(),
                    set_store.clone(),
                    sorted_set_store.clone(),
                );

                // Pre-populate based on type
                rt.block_on(async {
                    match *data_type {
                        "kv" => {
                            kv_store
                                .set("source_key", b"value".to_vec(), None)
                                .await
                                .unwrap();
                        }
                        "hash" => {
                            hash_store
                                .hset("source_key", "field", b"value".to_vec())
                                .unwrap();
                        }
                        "list" => {
                            list_store
                                .lpush("source_key", vec![b"value".to_vec()], false)
                                .unwrap();
                        }
                        _ => {}
                    }
                });

                b.to_async(&rt).iter(|| async {
                    // Clean up destination before each iteration
                    let _ = manager.exists("dest_key").await;
                    manager
                        .rename(black_box("source_key"), black_box("dest_key"))
                        .await
                        .unwrap();
                    // Rename back for next iteration
                    manager.rename("dest_key", "source_key").await.unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: RENAMENX operation performance
fn bench_renamenx(c: &mut Criterion) {
    let mut group = c.benchmark_group("renamenx");

    let rt = Runtime::new().unwrap();
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let manager = KeyManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    );

    // Pre-populate
    rt.block_on(async {
        kv_store
            .set("source_key", b"value".to_vec(), None)
            .await
            .unwrap();
    });

    group.bench_function("renamenx_success", |b| {
        b.to_async(&rt).iter(|| async {
            // Ensure destination doesn't exist
            if manager.exists("dest_key").await.unwrap() {
                kv_store.mdel(&["dest_key".to_string()]).await.unwrap();
            }
            manager
                .renamenx(black_box("source_key"), black_box("dest_key"))
                .await
                .unwrap();
            // Rename back for next iteration
            manager.rename("dest_key", "source_key").await.unwrap();
        });
    });

    group.bench_function("renamenx_fail_dest_exists", |b| {
        rt.block_on(async {
            kv_store
                .set("dest_key", b"value2".to_vec(), None)
                .await
                .unwrap();
        });

        b.to_async(&rt).iter(|| async {
            manager
                .renamenx(black_box("source_key"), black_box("dest_key"))
                .await
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: COPY operation performance
fn bench_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("copy");

    for data_type in ["kv", "hash", "list"].iter() {
        let data_type = *data_type;
        group.bench_with_input(
            BenchmarkId::new("copy", data_type),
            &data_type,
            |b, data_type| {
                let rt = Runtime::new().unwrap();
                let kv_store = Arc::new(KVStore::new(KVConfig::default()));
                let hash_store = Arc::new(HashStore::new());
                let list_store = Arc::new(ListStore::new());
                let set_store = Arc::new(SetStore::new());
                let sorted_set_store = Arc::new(SortedSetStore::new());

                let manager = KeyManager::new(
                    kv_store.clone(),
                    hash_store.clone(),
                    list_store.clone(),
                    set_store.clone(),
                    sorted_set_store.clone(),
                );

                // Pre-populate based on type
                rt.block_on(async {
                    match *data_type {
                        "kv" => {
                            kv_store
                                .set("source_key", b"value".to_vec(), None)
                                .await
                                .unwrap();
                        }
                        "hash" => {
                            hash_store
                                .hset("source_key", "field", b"value".to_vec())
                                .unwrap();
                        }
                        "list" => {
                            list_store
                                .lpush("source_key", vec![b"value".to_vec()], false)
                                .unwrap();
                        }
                        _ => {}
                    }
                });

                b.to_async(&rt).iter(|| async {
                    // Clean up destination before each iteration
                    if manager.exists("dest_key").await.unwrap() {
                        kv_store.mdel(&["dest_key".to_string()]).await.unwrap();
                        hash_store.hdel("dest_key", &[]).unwrap_or(0);
                        let _ = list_store.delete("dest_key");
                    }
                    manager
                        .copy(
                            black_box("source_key"),
                            black_box("dest_key"),
                            black_box(true),
                        )
                        .await
                        .unwrap();
                    // Clean up destination for next iteration
                    if manager.exists("dest_key").await.unwrap() {
                        kv_store.mdel(&["dest_key".to_string()]).await.unwrap();
                        hash_store.hdel("dest_key", &[]).unwrap_or(0);
                        let _ = list_store.delete("dest_key");
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: RANDOMKEY operation performance
fn bench_randomkey(c: &mut Criterion) {
    let mut group = c.benchmark_group("randomkey");

    for num_keys in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("randomkey", num_keys),
            num_keys,
            |b, &num_keys| {
                let rt = Runtime::new().unwrap();
                let kv_store = Arc::new(KVStore::new(KVConfig::default()));
                let hash_store = Arc::new(HashStore::new());
                let list_store = Arc::new(ListStore::new());
                let set_store = Arc::new(SetStore::new());
                let sorted_set_store = Arc::new(SortedSetStore::new());

                let manager = KeyManager::new(
                    kv_store.clone(),
                    hash_store.clone(),
                    list_store.clone(),
                    set_store.clone(),
                    sorted_set_store.clone(),
                );

                // Pre-populate with keys
                rt.block_on(async {
                    for i in 0..num_keys {
                        let key = format!("key_{}", i);
                        kv_store.set(&key, b"value".to_vec(), None).await.unwrap();
                    }
                });

                b.to_async(&rt).iter(|| async {
                    manager.randomkey().await.unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_exists,
    bench_type,
    bench_rename,
    bench_renamenx,
    bench_copy,
    bench_randomkey
);
criterion_main!(benches);
