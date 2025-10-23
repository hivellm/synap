//! KV Store Benchmark - With and Without Replication
//!
//! Compares performance of KV operations:
//! - Baseline (no replication)
//! - With 1 replica
//! - With 3 replicas

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use synap_server::core::{KVConfig, KVStore};
use synap_server::persistence::types::Operation;
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use tokio::runtime::Runtime;

static BENCH_PORT: AtomicU16 = AtomicU16::new(40000);

fn next_port() -> u16 {
    BENCH_PORT.fetch_add(1, Ordering::SeqCst)
}

// Helper: Create KV store without replication (baseline)
fn create_kv_baseline() -> Arc<KVStore> {
    Arc::new(KVStore::new(KVConfig::default()))
}

// Helper: Create KV store with master node
async fn create_kv_with_master() -> (Arc<MasterNode>, Arc<KVStore>, std::net::SocketAddr) {
    let port = next_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Master;
    config.replica_listen_address = Some(addr);
    config.heartbeat_interval_ms = 100;

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = Arc::new(MasterNode::new(config, Arc::clone(&kv), None).await.unwrap());

    tokio::time::sleep(Duration::from_millis(50)).await;

    (master, kv, addr)
}

// Helper: Create replica
async fn create_replica(master_addr: std::net::SocketAddr) -> Arc<ReplicaNode> {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Replica;
    config.master_address = Some(master_addr);
    config.auto_reconnect = true;
    config.reconnect_delay_ms = 100;

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let replica = ReplicaNode::new(config, kv, None).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    replica
}

fn bench_kv_set_baseline(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let kv = create_kv_baseline();

    let mut group = c.benchmark_group("kv_set_baseline");
    group.throughput(Throughput::Elements(1));

    group.bench_function("set_single_key", |b| {
        b.to_async(&rt).iter(|| async {
            kv.set(
                black_box("benchmark_key"),
                black_box(b"benchmark_value".to_vec()),
                None,
            )
            .await
            .unwrap();
        });
    });

    group.finish();
}

fn bench_kv_set_with_1_replica(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (master, kv, master_addr) = rt.block_on(async { create_kv_with_master().await });

    let _replica1 = rt.block_on(async { create_replica(master_addr).await });

    rt.block_on(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
    });

    let mut group = c.benchmark_group("kv_set_1_replica");
    group.throughput(Throughput::Elements(1));

    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    group.bench_function("set_with_replication", |b| {
        b.to_async(&rt).iter(|| {
            let counter = counter.clone();
            let master = master.clone();
            let kv = kv.clone();
            async move {
                let id = counter.fetch_add(1, Ordering::SeqCst);
                let key = format!("bench_key_{}", id);
                let value = b"benchmark_value".to_vec();

                kv.set(black_box(&key), black_box(value.clone()), None)
                    .await
                    .unwrap();

                master.replicate(Operation::KVSet {
                    key,
                    value,
                    ttl: None});
            }
        });
    });

    group.finish();
}

fn bench_kv_set_with_3_replicas(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (master, kv, master_addr) = rt.block_on(async { create_kv_with_master().await });

    let _replica1 = rt.block_on(async { create_replica(master_addr).await });
    let _replica2 = rt.block_on(async { create_replica(master_addr).await });
    let _replica3 = rt.block_on(async { create_replica(master_addr).await });

    rt.block_on(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
    });

    let mut group = c.benchmark_group("kv_set_3_replicas");
    group.throughput(Throughput::Elements(1));

    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    group.bench_function("set_with_3_replicas", |b| {
        b.to_async(&rt).iter(|| {
            let counter = counter.clone();
            let master = master.clone();
            let kv = kv.clone();
            async move {
                let id = counter.fetch_add(1, Ordering::SeqCst);
                let key = format!("bench_key_{}", id);
                let value = b"benchmark_value".to_vec();

                kv.set(black_box(&key), black_box(value.clone()), None)
                    .await
                    .unwrap();

                master.replicate(Operation::KVSet {
                    key,
                    value,
                    ttl: None});
            }
        });
    });

    group.finish();
}

fn bench_kv_get_baseline(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let kv = create_kv_baseline();

    // Pre-populate
    rt.block_on(async {
        for i in 0..1000 {
            kv.set(
                &format!("key_{}", i),
                format!("value_{}", i).into_bytes(),
                None,
            )
            .await
            .unwrap();
        }
    });

    let mut group = c.benchmark_group("kv_get_baseline");
    group.throughput(Throughput::Elements(1));

    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    group.bench_function("get_existing_key", |b| {
        b.to_async(&rt).iter(|| {
            let counter = counter.clone();
            let kv = kv.clone();
            async move {
                let key_num = counter.fetch_add(1, Ordering::SeqCst) % 1000;
                kv.get(black_box(&format!("key_{}", key_num)))
                    .await
                    .unwrap();
            }
        });
    });

    group.finish();
}

fn bench_kv_get_with_1_replica(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (master, kv, master_addr) = rt.block_on(async { create_kv_with_master().await });

    let _replica1 = rt.block_on(async { create_replica(master_addr).await });

    // Pre-populate
    rt.block_on(async {
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let value = format!("value_{}", i).into_bytes();
            kv.set(&key, value.clone(), None).await.unwrap();
            master.replicate(Operation::KVSet {
                key,
                value,
                ttl: None});
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    });

    let mut group = c.benchmark_group("kv_get_1_replica");
    group.throughput(Throughput::Elements(1));

    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    group.bench_function("get_from_master", |b| {
        b.to_async(&rt).iter(|| {
            let counter = counter.clone();
            let kv = kv.clone();
            async move {
                let key_num = counter.fetch_add(1, Ordering::SeqCst) % 1000;
                kv.get(black_box(&format!("key_{}", key_num)))
                    .await
                    .unwrap();
            }
        });
    });

    group.finish();
}

fn bench_kv_batch_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("kv_batch_operations");

    for size in [10, 100, 1000].iter() {
        // Baseline
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("baseline", size), size, |b, &size| {
            let kv = create_kv_baseline();
            b.to_async(&rt).iter(|| async {
                for i in 0..size {
                    kv.set(
                        &format!("batch_key_{}", i),
                        format!("batch_value_{}", i).into_bytes(),
                        None,
                    )
                    .await
                    .unwrap();
                }
            });
        });

        // With 1 replica
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("1_replica", size), size, |b, &size| {
            let (master, kv, master_addr) = rt.block_on(async { create_kv_with_master().await });
            let _replica1 = rt.block_on(async { create_replica(master_addr).await });
            rt.block_on(async { tokio::time::sleep(Duration::from_millis(500)).await });

            b.to_async(&rt).iter(|| async {
                for i in 0..size {
                    let key = format!("batch_key_{}", i);
                    let value = format!("batch_value_{}", i).into_bytes();
                    kv.set(&key, value.clone(), None).await.unwrap();
                    master.replicate(Operation::KVSet {
                        key,
                        value,
                        ttl: None});
                }
            });
        });
    }

    group.finish();
}

fn bench_kv_mixed_workload(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("kv_mixed_workload");
    group.throughput(Throughput::Elements(100));

    // Baseline: 70% reads, 30% writes
    group.bench_function("baseline_70r_30w", |b| {
        let kv = create_kv_baseline();

        // Pre-populate
        rt.block_on(async {
            for i in 0..1000 {
                kv.set(
                    &format!("key_{}", i),
                    format!("value_{}", i).into_bytes(),
                    None,
                )
                .await
                .unwrap();
            }
        });

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        b.to_async(&rt).iter(|| {
            let counter = counter.clone();
            let kv = kv.clone();
            async move {
                for _ in 0..100 {
                    let rand_num = counter.fetch_add(1, Ordering::SeqCst);
                    if rand_num % 100 < 70 {
                        // Read
                        let key_num = rand_num % 1000;
                        kv.get(&format!("key_{}", key_num)).await.unwrap();
                    } else {
                        // Write
                        let key_num = rand_num % 1000;
                        kv.set(
                            &format!("key_{}", key_num),
                            format!("new_value_{}", rand_num).into_bytes(),
                            None,
                        )
                        .await
                        .unwrap();
                    }
                }
            }
        });
    });

    // With 1 replica: 70% reads, 30% writes
    group.bench_function("1_replica_70r_30w", |b| {
        let (master, kv, master_addr) = rt.block_on(async { create_kv_with_master().await });
        let _replica1 = rt.block_on(async { create_replica(master_addr).await });

        // Pre-populate
        rt.block_on(async {
            for i in 0..1000 {
                let key = format!("key_{}", i);
                let value = format!("value_{}", i).into_bytes();
                kv.set(&key, value.clone(), None).await.unwrap();
                master.replicate(Operation::KVSet {
                    key,
                    value,
                    ttl: None});
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        });

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        b.to_async(&rt).iter(|| {
            let counter = counter.clone();
            let master = master.clone();
            let kv = kv.clone();
            async move {
                for _ in 0..100 {
                    let rand_num = counter.fetch_add(1, Ordering::SeqCst);
                    if rand_num % 100 < 70 {
                        // Read
                        let key_num = rand_num % 1000;
                        kv.get(&format!("key_{}", key_num)).await.unwrap();
                    } else {
                        // Write
                        let key = format!("key_{}", rand_num % 1000);
                        let value = format!("new_value_{}", rand_num).into_bytes();
                        kv.set(&key, value.clone(), None).await.unwrap();
                        master.replicate(Operation::KVSet {
                            key,
                            value,
                            ttl: None});
                    }
                }
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_kv_set_baseline,
    bench_kv_set_with_1_replica,
    bench_kv_set_with_3_replicas,
    bench_kv_get_baseline,
    bench_kv_get_with_1_replica,
    bench_kv_batch_operations,
    bench_kv_mixed_workload,
);

criterion_main!(benches);
