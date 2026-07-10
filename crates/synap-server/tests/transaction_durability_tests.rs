//! MULTI/EXEC durability, replication, and isolation (audit M-010, phase6k).
//!
//! phase6d made EXEC atomic against other transactions; phase6k makes a committed
//! transaction (1) durable — logged to the WAL as one batch and present after a
//! crash/recovery, (2) replicated — propagated to replicas through the shared
//! persistence hook, and (3) isolated — a non-transactional writer to a key the
//! EXEC touches is ordered entirely before or after it, never interleaved.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use synap_server::core::{
    CommittedWrite, HashStore, KVConfig, KVStore, ListStore, SetStore, SortedSetStore,
    StreamConfig, StreamManager, TransactionCommand, TransactionManager,
};
use synap_server::persistence::types::FsyncMode;
use synap_server::persistence::{PersistenceConfig, PersistenceLayer, recover};
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use tokio::time::sleep;

static PORT: AtomicU16 = AtomicU16::new(42000);

fn next_port() -> u16 {
    PORT.fetch_add(1, Ordering::SeqCst)
}

/// A committed EXEC is logged to the WAL as one batch and is fully present after
/// a crash + recovery (spec: "Committed transaction survives a crash").
#[tokio::test]
async fn committed_transaction_survives_crash() {
    let dir = "./target/txn_durability";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(format!("{dir}/snapshots")).unwrap();

    let mut config = PersistenceConfig::default();
    config.enabled = true;
    config.wal.enabled = true;
    config.wal.fsync_mode = FsyncMode::Always; // sync durability
    config.wal.path = PathBuf::from(format!("{dir}/test.wal"));
    config.snapshot.enabled = false;
    config.snapshot.directory = PathBuf::from(format!("{dir}/snapshots"));

    let kv_config = KVConfig::default();
    let queue_config = synap_server::core::QueueConfig::default();

    // --- Run 1: execute a transaction and log it durably, then "crash". ---
    {
        let kv = Arc::new(KVStore::new(kv_config.clone()));
        let hash = Arc::new(HashStore::new());
        let list = Arc::new(ListStore::new());
        let set = Arc::new(SetStore::new());
        let zset = Arc::new(SortedSetStore::new());
        let manager = TransactionManager::new(
            Arc::clone(&kv),
            Arc::clone(&hash),
            Arc::clone(&list),
            Arc::clone(&set),
            Arc::clone(&zset),
        );
        let persistence = PersistenceLayer::new(config.clone()).await.unwrap();

        let cid = "durability";
        manager.multi(cid.to_string()).unwrap();
        manager
            .queue_command_if_transaction(
                cid,
                TransactionCommand::KVSet {
                    key: "a".to_string(),
                    value: b"1".to_vec(),
                    ttl: None,
                },
            )
            .unwrap();
        manager
            .queue_command_if_transaction(
                cid,
                TransactionCommand::KVIncr {
                    key: "counter".to_string(),
                    delta: 5,
                },
            )
            .unwrap();
        manager
            .queue_command_if_transaction(
                cid,
                TransactionCommand::HashSet {
                    key: "h".to_string(),
                    field: "f".to_string(),
                    value: b"hv".to_vec(),
                },
            )
            .unwrap();
        manager
            .queue_command_if_transaction(
                cid,
                TransactionCommand::SetAdd {
                    key: "s".to_string(),
                    members: vec![b"m".to_vec()],
                },
            )
            .unwrap();

        let (_results, writes) = manager.exec(cid).await.unwrap().expect("EXEC committed");
        // Log the whole transaction as one atomic WAL batch (fsynced on return).
        persistence.log_transaction(&writes).await.unwrap();

        // Drop everything — simulate a crash after the commit was acknowledged.
        drop(persistence);
        sleep(Duration::from_millis(200)).await; // let the WAL file handle close
    }

    // --- Run 2: recover from the WAL and verify every write is present. ---
    {
        let (kv, hash, _list, set, _zset, _qm, _offset) =
            recover(&config, kv_config, queue_config).await.unwrap();

        assert_eq!(
            kv.get("a").await.unwrap(),
            Some(b"1".to_vec()),
            "KVSet not recovered"
        );
        assert_eq!(
            kv.get("counter").await.unwrap(),
            Some(b"5".to_vec()),
            "KVIncr effect (resulting SET) not recovered"
        );
        assert_eq!(
            hash.unwrap().hget("h", "f").unwrap(),
            Some(b"hv".to_vec()),
            "HashSet not recovered"
        );
        assert!(
            set.unwrap().sismember("s", b"m".to_vec()).unwrap(),
            "SetAdd not recovered"
        );
    }

    let _ = std::fs::remove_dir_all(dir);
}

/// A committed EXEC is propagated to a connected replica through the persistence
/// hook (spec: "Committed transaction reaches a replica").
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn committed_transaction_reaches_replica() {
    let addr: SocketAddr = format!("127.0.0.1:{}", next_port()).parse().unwrap();

    // Master + a replication-only persistence layer (persistence disabled → no
    // WAL file, but transactions are still replicated).
    let master_cfg = ReplicationConfig {
        enabled: true,
        role: NodeRole::Master,
        replica_listen_address: Some(addr),
        heartbeat_interval_ms: 100,
        ..Default::default()
    };
    let master_kv = Arc::new(KVStore::new(KVConfig::default()));
    let master_stream = Arc::new(StreamManager::new(StreamConfig::default()));
    let master = Arc::new(
        MasterNode::new(master_cfg, master_kv, Some(master_stream))
            .await
            .unwrap(),
    );
    sleep(Duration::from_millis(100)).await;

    let mut persist_cfg = PersistenceConfig::default();
    persist_cfg.enabled = false;
    let persistence =
        PersistenceLayer::new_with_replication(persist_cfg, Some(Arc::clone(&master)))
            .await
            .unwrap();

    // Replica holding every datatype store.
    let replica_cfg = ReplicationConfig {
        enabled: true,
        role: NodeRole::Replica,
        master_address: Some(addr),
        auto_reconnect: true,
        reconnect_delay_ms: 100,
        ..Default::default()
    };
    let r_kv = Arc::new(KVStore::new(KVConfig::default()));
    let r_stream = Arc::new(StreamManager::new(StreamConfig::default()));
    let r_hash = Arc::new(HashStore::new());
    let r_list = Arc::new(ListStore::new());
    let r_set = Arc::new(SetStore::new());
    let r_zset = Arc::new(SortedSetStore::new());
    let _replica = ReplicaNode::new(
        replica_cfg,
        Arc::clone(&r_kv),
        Some(Arc::clone(&r_stream)),
        Some(Arc::clone(&r_hash)),
        Some(Arc::clone(&r_list)),
        Some(Arc::clone(&r_set)),
        Some(Arc::clone(&r_zset)),
        None,
    )
    .await
    .unwrap();
    sleep(Duration::from_millis(500)).await;

    // Commit a transaction's worth of writes through the persistence hook.
    let writes = vec![
        CommittedWrite::KvSet {
            key: "tk".to_string(),
            value: b"tv".to_vec(),
            ttl: None,
        },
        CommittedWrite::HashSet {
            key: "th".to_string(),
            field: "f".to_string(),
            value: b"hv".to_vec(),
        },
    ];
    persistence.log_transaction(&writes).await.unwrap();

    // Poll the replica for convergence (propagation is heartbeat-driven).
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let kv_ok = r_kv.get("tk").await.unwrap() == Some(b"tv".to_vec());
        let hash_ok = r_hash.hget("th", "f").ok().flatten() == Some(b"hv".to_vec());
        if kv_ok && hash_ok {
            break;
        }
        if Instant::now() >= deadline {
            panic!("committed transaction did not reach the replica within deadline");
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// A non-transactional write to a key an EXEC holds is ordered entirely after the
/// EXEC — never interleaved (spec: "Concurrent plain write does not interleave").
///
/// The EXEC holds the per-key **write** lock for the union of its keys across all
/// commands; this test holds that same lock explicitly (as `exec` does, via
/// `write_keys`) and shows a plain `SET` on the key — which takes the shared
/// **read** side — blocks until the write lock is released, then applies strictly
/// after (phase12 read/write split preserves M-010 isolation).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_write_is_isolated_from_held_key_lock() {
    let kv = Arc::new(KVStore::new(KVConfig::default()));
    kv.set("k", b"before".to_vec(), None).await.unwrap();

    // Simulate an EXEC holding key `k`'s write lock across its commands.
    let keys: std::collections::BTreeSet<String> = ["k".to_string()].into_iter().collect();
    let guard = kv.key_locks().write_keys(&keys).await;

    let kv2 = Arc::clone(&kv);
    let writer = tokio::spawn(async move {
        kv2.set("k", b"after".to_vec(), None).await.unwrap();
    });

    // While the lock is held, the plain SET cannot proceed and the value is
    // unchanged — the write has not interleaved into the critical section.
    sleep(Duration::from_millis(150)).await;
    assert!(
        !writer.is_finished(),
        "plain SET interleaved while the EXEC key lock was held"
    );
    assert_eq!(kv.get("k").await.unwrap(), Some(b"before".to_vec()));

    // Releasing the lock lets the plain SET complete, ordered strictly after.
    drop(guard);
    writer.await.unwrap();
    assert_eq!(kv.get("k").await.unwrap(), Some(b"after".to_vec()));
}
