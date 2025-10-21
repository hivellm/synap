use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use synap_server::{
    KVConfig, KVStore, MasterNode, NodeRole, ReplicaNode, ReplicationConfig, ReplicationLog,
};
use synap_server::persistence::types::Operation;
use tokio::runtime::Runtime;

fn bench_replication_log_append(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("replication_log");
    
    for size in [100, 1000, 10_000] {
        group.throughput(Throughput::Elements(size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("append", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let log = ReplicationLog::new(100_000);
                    
                    for i in 0..size {
                        let op = Operation::KVSet {
                            key: format!("key_{}", i),
                            value: vec![i as u8],
                            ttl: None,
                        };
                        black_box(log.append(op));
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn bench_replication_log_get_from_offset(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("replication_log_get");
    
    // Pre-populate log
    let log = ReplicationLog::new(100_000);
    for i in 0..10_000 {
        log.append(Operation::KVSet {
            key: format!("key_{}", i),
            value: vec![i as u8],
            ttl: None,
        });
    }
    
    for offset in [0, 5_000, 9_000] {
        group.bench_with_input(
            BenchmarkId::new("get_from_offset", offset),
            &offset,
            |b, &offset| {
                b.to_async(&rt).iter(|| async {
                    black_box(log.get_from_offset(offset).unwrap());
                });
            },
        );
    }
    
    group.finish();
}

fn bench_master_replication(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("master_replication");
    group.sample_size(20); // Slower test
    
    for batch_size in [100, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("replicate_operations", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    // Create master
                    let mut config = ReplicationConfig::default();
                    config.enabled = true;
                    config.role = NodeRole::Master;
                    config.replica_listen_address = Some("127.0.0.1:0".parse().unwrap());
                    
                    let kv = Arc::new(KVStore::new(KVConfig::default()));
                    let master = MasterNode::new(config, kv).await.unwrap();
                    
                    // Replicate operations
                    for i in 0..batch_size {
                        let op = Operation::KVSet {
                            key: format!("key_{}", i),
                            value: vec![i as u8],
                            ttl: None,
                        };
                        black_box(master.replicate(op));
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn bench_snapshot_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("snapshot");
    group.sample_size(10); // Slow operation
    
    for num_keys in [100, 1000] {
        group.throughput(Throughput::Elements(num_keys as u64));
        
        group.bench_with_input(
            BenchmarkId::new("create", num_keys),
            &num_keys,
            |b, &num_keys| {
                b.to_async(&rt).iter(|| async {
                    let kv = KVStore::new(KVConfig::default());
                    
                    // Populate
                    for i in 0..num_keys {
                        let key = format!("key_{}", i);
                        kv.set(
                            &key,
                            format!("value_{}", i).into_bytes(),
                            None,
                        )
                        .await
                        .unwrap();
                    }
                    
                    // Create snapshot
                    black_box(
                        synap_server::replication::sync::create_snapshot(&kv, 0)
                            .await
                            .unwrap()
                    );
                });
            },
        );
    }
    
    group.finish();
}

fn bench_snapshot_apply(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("snapshot_apply");
    group.sample_size(10);
    
    for num_keys in [100, 1000] {
        group.throughput(Throughput::Elements(num_keys as u64));
        
        group.bench_with_input(
            BenchmarkId::new("apply", num_keys),
            &num_keys,
            |b, &num_keys| {
                // Pre-create snapshot
                let snapshot = rt.block_on(async {
                    let kv = KVStore::new(KVConfig::default());
                    for i in 0..num_keys {
                        let key = format!("key_{}", i);
                        kv.set(&key, vec![i as u8], None).await.unwrap();
                    }
                    synap_server::replication::sync::create_snapshot(&kv, 0)
                        .await
                        .unwrap()
                });
                
                b.to_async(&rt).iter(|| async {
                    let kv = KVStore::new(KVConfig::default());
                    black_box(
                        synap_server::replication::sync::apply_snapshot(&kv, &snapshot)
                            .await
                            .unwrap()
                    );
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_replication_log_append,
    bench_replication_log_get_from_offset,
    bench_master_replication,
    bench_snapshot_creation,
    bench_snapshot_apply
);

criterion_main!(benches);
