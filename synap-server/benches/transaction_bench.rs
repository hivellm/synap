use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;
use synap_server::core::{
    HashStore, KVConfig, KVStore, ListStore, SetStore, SortedSetStore, TransactionManager,
};
use tokio::runtime::Runtime;

/// Benchmark: Transaction overhead (MULTI/EXEC)
fn bench_transaction_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_overhead");

    for num_commands in [1, 5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*num_commands as u64));

        group.bench_with_input(
            BenchmarkId::new("multi_exec", num_commands),
            num_commands,
            |b, &num_commands| {
                let rt = Runtime::new().unwrap();
                let kv_store = Arc::new(KVStore::new(KVConfig::default()));
                let hash_store = Arc::new(HashStore::new());
                let list_store = Arc::new(ListStore::new());
                let set_store = Arc::new(SetStore::new());
                let sorted_set_store = Arc::new(SortedSetStore::new());

                let tx_manager = Arc::new(TransactionManager::new(
                    kv_store.clone(),
                    hash_store.clone(),
                    list_store.clone(),
                    set_store.clone(),
                    sorted_set_store.clone(),
                ));

                let client_id = format!("bench_client_{}", num_commands);

                b.to_async(&rt).iter(|| async {
                    // Start transaction
                    tx_manager.multi(client_id.clone()).unwrap();

                    // Execute empty transaction (measures MULTI/EXEC overhead)
                    // In real usage, commands would be queued via handlers with client_id
                    let _results = tx_manager.exec(&client_id).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: WATCH operation performance
fn bench_watch_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("watch_performance");

    for num_keys in [1, 5, 10, 20, 50].iter() {
        group.throughput(Throughput::Elements(*num_keys as u64));

        group.bench_with_input(
            BenchmarkId::new("watch_keys", num_keys),
            num_keys,
            |b, &num_keys| {
                let rt = Runtime::new().unwrap();
                let kv_store = Arc::new(KVStore::new(KVConfig::default()));
                let hash_store = Arc::new(HashStore::new());
                let list_store = Arc::new(ListStore::new());
                let set_store = Arc::new(SetStore::new());
                let sorted_set_store = Arc::new(SortedSetStore::new());

                let tx_manager = Arc::new(TransactionManager::new(
                    kv_store.clone(),
                    hash_store.clone(),
                    list_store.clone(),
                    set_store.clone(),
                    sorted_set_store.clone(),
                ));

                // Pre-populate keys
                rt.block_on(async {
                    for i in 0..num_keys {
                        let key = format!("watch:key_{}", i);
                        kv_store
                            .set(&key, format!("value_{}", i).as_bytes().to_vec(), None)
                            .await
                            .unwrap();
                    }
                });

                let client_id = "bench_client";
                let keys: Vec<String> = (0..num_keys).map(|i| format!("watch:key_{}", i)).collect();

                b.to_async(&rt).iter(|| async {
                    tx_manager.multi(client_id.to_string()).unwrap();
                    tx_manager
                        .watch(client_id, black_box(keys.clone()))
                        .unwrap();
                    let _ = tx_manager.discard(client_id);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Transaction conflict detection (WATCH + EXEC)
fn bench_conflict_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("conflict_detection");

    for num_watched_keys in [1, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("watch_and_exec", num_watched_keys),
            num_watched_keys,
            |b, &num_watched_keys| {
                let rt = Runtime::new().unwrap();
                let kv_store = Arc::new(KVStore::new(KVConfig::default()));
                let hash_store = Arc::new(HashStore::new());
                let list_store = Arc::new(ListStore::new());
                let set_store = Arc::new(SetStore::new());
                let sorted_set_store = Arc::new(SortedSetStore::new());

                let tx_manager = Arc::new(TransactionManager::new(
                    kv_store.clone(),
                    hash_store.clone(),
                    list_store.clone(),
                    set_store.clone(),
                    sorted_set_store.clone(),
                ));

                // Pre-populate keys
                rt.block_on(async {
                    for i in 0..num_watched_keys {
                        let key = format!("conflict:key_{}", i);
                        kv_store
                            .set(&key, format!("value_{}", i).as_bytes().to_vec(), None)
                            .await
                            .unwrap();
                    }
                });

                let client_id = "bench_client";
                let keys: Vec<String> = (0..num_watched_keys)
                    .map(|i| format!("conflict:key_{}", i))
                    .collect();

                b.to_async(&rt).iter(|| async {
                    tx_manager.multi(client_id.to_string()).unwrap();
                    tx_manager.watch(client_id, keys.clone()).unwrap();

                    // Modify a watched key (simulate conflict)
                    kv_store
                        .set("conflict:key_0", "modified".as_bytes().to_vec(), None)
                        .await
                        .unwrap();

                    // Try to execute (should abort)
                    let _result = tx_manager.exec(client_id).await;
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Transaction with multiple command types
fn bench_mixed_commands(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_commands");

    group.bench_function("kv_hash_list_set", |b| {
        let rt = Runtime::new().unwrap();
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let tx_manager = Arc::new(TransactionManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        ));

        let client_id = "bench_client";

        b.to_async(&rt).iter(|| async {
            tx_manager.multi(client_id.to_string()).unwrap();

            // Execute empty transaction (measures overhead)
            // In real usage, commands would be queued via handlers
            let _results = tx_manager.exec(client_id).await.unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_transaction_overhead,
    bench_watch_performance,
    bench_conflict_detection,
    bench_mixed_commands
);
criterion_main!(benches);
