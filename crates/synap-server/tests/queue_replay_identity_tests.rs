//! phase24: queue message identity across restarts and replicas (D-05).
//!
//! Every write goes through `persistence::durable` (the path every transport
//! uses). Restarts are `recover_all` on the same WAL and snapshot directory;
//! "deliverable" is checked by draining the recovered queue, the way a
//! consumer would see it.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use synap_server::core::{
    KVConfig, KVStore, QueueConfig, QueueManager, QueueMessage, StreamConfig, StreamManager,
};
use synap_server::persistence::types::FsyncMode;
use synap_server::persistence::{
    Operation, PersistenceConfig, PersistenceLayer, SnapshotManager, StoreArcs, StoreRefs,
    WriteAheadLog, durable, recover_all,
};
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use tokio::time::sleep;

fn config(name: &str) -> PersistenceConfig {
    let dir = format!("./target/phase24_{name}");
    let _ = std::fs::remove_dir_all(&dir);
    let mut config = PersistenceConfig::default();
    config.enabled = true;
    config.wal.enabled = true;
    config.wal.fsync_mode = FsyncMode::Periodic;
    config.wal.path = format!("{dir}/wal/synap.wal").into();
    config.snapshot.enabled = true;
    config.snapshot.directory = format!("{dir}/snapshots").into();
    config.snapshot.interval_secs = 3600;
    config.snapshot.operation_threshold = usize::MAX;
    config
}

/// The stores a queue test writes to: an empty KV store (required by every
/// persistence API) and the queue manager.
struct Server {
    kv: KVStore,
    queues: QueueManager,
}

impl Server {
    fn new() -> Self {
        Self {
            kv: KVStore::new(KVConfig::default()),
            queues: QueueManager::new(QueueConfig::default()),
        }
    }

    fn refs(&self) -> StoreRefs<'_> {
        StoreRefs {
            queue_manager: Some(&self.queues),
            ..StoreRefs::kv_only(&self.kv)
        }
    }
}

/// Restart: recover every store from the WAL and the latest snapshot.
async fn restart(config: &PersistenceConfig) -> QueueManager {
    recover_all(config, KVConfig::default(), QueueConfig::default())
        .await
        .expect("recovery must succeed")
        .queue_manager
        .expect("queues are recovered")
}

/// Everything a consumer can get from `queue` now, in delivery order. Each
/// delivery uses its own consumer so a prefetch limit cannot stop the drain.
async fn drain(qm: &QueueManager, queue: &str) -> Vec<QueueMessage> {
    let mut delivered = Vec::new();
    while let Some(message) = qm
        .consume(queue, &format!("drain-{}", delivered.len()))
        .await
        .unwrap()
    {
        delivered.push(message);
    }
    delivered
}

fn ids(messages: &[QueueMessage]) -> Vec<&str> {
    messages.iter().map(|m| m.id.as_str()).collect()
}

/// The fields recovery must keep, for whole-message comparisons.
fn identity(m: &QueueMessage) -> (String, Vec<u8>, u8, u32, u32, u32) {
    (
        m.id.clone(),
        m.payload.to_vec(),
        m.priority,
        m.retry_count,
        m.max_retries,
        m.created_at,
    )
}

async fn publish(
    log: &PersistenceLayer,
    qm: &QueueManager,
    queue: &str,
    body: &str,
) -> QueueMessage {
    durable::queue_publish(Some(log), qm, queue, body.as_bytes().to_vec(), None, None)
        .await
        .unwrap()
}

/// Publish, consume and ACK, then restart from the WAL alone: the
/// acknowledged message is not deliverable, the other keeps its ID.
#[tokio::test]
async fn wal_only_recovery_after_ack_leaves_no_deliverable_copy() {
    let config = config("wal_only_ack");
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let server = Server::new();
    let qm = &server.queues;
    durable::queue_create(Some(&layer), qm, "q", None)
        .await
        .unwrap();
    let acked = publish(&layer, qm, "q", "acked").await;
    let kept = publish(&layer, qm, "q", "kept").await;
    let delivered = qm.consume("q", "c").await.unwrap().unwrap();
    assert_eq!(delivered.id, acked.id);
    durable::queue_ack(Some(&layer), qm, "q", &acked.id)
        .await
        .unwrap();
    layer.shutdown().await.unwrap();

    let recovered = restart(&config).await;
    let deliverable = drain(&recovered, "q").await;
    assert_eq!(ids(&deliverable), vec![kept.id.as_str()]);
    assert_eq!(identity(&deliverable[0]), identity(&kept));
}

/// A message captured in a snapshot and acknowledged after it, plus one
/// in flight at snapshot time and acknowledged after it: neither is
/// deliverable after restarting from the snapshot and the WAL tail.
#[tokio::test]
async fn snapshot_plus_wal_recovery_after_ack_leaves_no_deliverable_copy() {
    let config = config("snapshot_ack");
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let server = Server::new();
    let qm = &server.queues;
    durable::queue_create(Some(&layer), qm, "q", None)
        .await
        .unwrap();
    let in_flight = publish(&layer, qm, "q", "in-flight").await;
    let waiting = publish(&layer, qm, "q", "waiting").await;
    let kept = publish(&layer, qm, "q", "kept").await;
    assert_eq!(
        qm.consume("q", "c").await.unwrap().unwrap().id,
        in_flight.id
    );

    layer.snapshot(server.refs()).await.unwrap();

    durable::queue_ack(Some(&layer), qm, "q", &in_flight.id)
        .await
        .unwrap();
    assert_eq!(qm.consume("q", "c").await.unwrap().unwrap().id, waiting.id);
    durable::queue_ack(Some(&layer), qm, "q", &waiting.id)
        .await
        .unwrap();
    layer.shutdown().await.unwrap();

    let (snapshot, _) = SnapshotManager::new(config.snapshot.clone())
        .load_latest()
        .await
        .unwrap()
        .expect("a snapshot was written");
    assert_eq!(snapshot.version, 5);
    assert_eq!(snapshot.queue_data["q"].in_flight[0].id, in_flight.id);

    let recovered = restart(&config).await;
    let deliverable = drain(&recovered, "q").await;
    assert_eq!(ids(&deliverable), vec![kept.id.as_str()]);
    assert_eq!(identity(&deliverable[0]), identity(&kept));
}

/// A snapshot that already dropped an acknowledged message, with a WAL tail
/// that still holds the ACK: the replayed ACK finds nothing and recovery
/// succeeds.
#[tokio::test]
async fn ack_of_a_message_the_snapshot_already_removed_is_tolerated() {
    let config = config("snapshot_already_acked");
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let server = Server::new();
    let qm = &server.queues;
    durable::queue_create(Some(&layer), qm, "q", None)
        .await
        .unwrap();
    let acked = publish(&layer, qm, "q", "acked").await;
    let kept = publish(&layer, qm, "q", "kept").await;
    qm.consume("q", "c").await.unwrap();
    durable::queue_ack(Some(&layer), qm, "q", &acked.id)
        .await
        .unwrap();
    layer.shutdown().await.unwrap();

    // Cut the snapshot at the ACK record: its image (taken after the ACK)
    // no longer holds the message, and the replay starts with the ACK.
    let wal = WriteAheadLog::open(config.wal.clone()).await.unwrap();
    let ack_offset = wal
        .replay(0)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| matches!(entry.operation, Operation::QueueAck { .. }))
        .expect("the ACK was logged")
        .offset;
    drop(wal);
    SnapshotManager::new(config.snapshot.clone())
        .create_snapshot(server.refs(), ack_offset)
        .await
        .unwrap();

    let recovered = restart(&config).await;
    assert_eq!(ids(&drain(&recovered, "q").await), vec![kept.id.as_str()]);
}

/// Write the same history once and check what a restart keeps: identity
/// and metadata of unacknowledged messages (retry counters included), the
/// queue configuration, in-flight messages ready again.
async fn unacknowledged_history(take_snapshot: bool, name: &str) {
    let config = config(name);
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let server = Server::new();
    let qm = &server.queues;
    let queue_config = QueueConfig {
        max_depth: 77,
        ack_deadline_secs: 45,
        default_max_retries: 6,
        default_priority: 2,
        prefetch_limit: 4,
    };
    durable::queue_create(Some(&layer), qm, "q", Some(queue_config))
        .await
        .unwrap();
    let mut retried =
        durable::queue_publish(Some(&layer), qm, "q", b"retried".to_vec(), Some(8), Some(3))
            .await
            .unwrap();
    let plain = publish(&layer, qm, "q", "plain").await;
    let in_flight = publish(&layer, qm, "q", "in-flight").await;
    assert_eq!(qm.consume("q", "c").await.unwrap().unwrap().id, retried.id);
    durable::queue_nack(Some(&layer), qm, "q", &retried.id, true)
        .await
        .unwrap();
    retried.retry_count = 1;
    // Delivered and never acknowledged: ready again after the restart.
    assert_eq!(qm.consume("q", "c").await.unwrap().unwrap().id, plain.id);
    assert_eq!(
        qm.consume("q", "c").await.unwrap().unwrap().id,
        in_flight.id
    );
    if take_snapshot {
        layer.snapshot(server.refs()).await.unwrap();
    }
    layer.shutdown().await.unwrap();

    let recovered = restart(&config).await;
    let state = recovered.dump_state().remove("q").expect("queue recovered");
    assert_eq!(state.config.max_depth, 77);
    assert_eq!(state.config.ack_deadline_secs, 45);
    assert_eq!(state.config.default_max_retries, 6);
    assert_eq!(state.config.default_priority, 2);
    assert_eq!(state.config.prefetch_limit, 4);
    let by_id: HashMap<String, QueueMessage> = drain(&recovered, "q")
        .await
        .into_iter()
        .map(|m| (m.id.clone(), m))
        .collect();
    assert_eq!(by_id.len(), 3, "{by_id:?}");
    for original in [&retried, &plain, &in_flight] {
        assert_eq!(identity(&by_id[&original.id]), identity(original));
    }
    assert_eq!(by_id[&retried.id].retry_count, 1);
    assert_eq!(by_id[&retried.id].priority, 8);
    assert_eq!(by_id[&retried.id].max_retries, 3);
}

#[tokio::test]
async fn unacknowledged_messages_keep_identity_and_metadata_from_the_wal() {
    unacknowledged_history(false, "identity_wal").await;
}

#[tokio::test]
async fn unacknowledged_messages_keep_identity_and_metadata_from_a_snapshot() {
    unacknowledged_history(true, "identity_snapshot").await;
}

/// QCREATE (with its configuration), QDELETE and QPURGE survive a restart,
/// from the WAL alone and from a snapshot taken midway. A purge removes only
/// what it removed: the message in flight at purge time stays.
async fn lifecycle_history(snapshot_midway: bool, name: &str) {
    let config = config(name);
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let server = Server::new();
    let qm = &server.queues;
    let log = Some(&layer);
    let custom = QueueConfig {
        max_depth: 12,
        ..QueueConfig::default()
    };
    durable::queue_create(log, qm, "keep", Some(custom))
        .await
        .unwrap();
    durable::queue_create(log, qm, "gone", None).await.unwrap();
    durable::queue_create(log, qm, "purged", None)
        .await
        .unwrap();
    publish(&layer, qm, "gone", "x").await;
    let in_flight = publish(&layer, qm, "purged", "in-flight").await;
    publish(&layer, qm, "purged", "purged-1").await;
    if snapshot_midway {
        layer.snapshot(server.refs()).await.unwrap();
    }
    assert_eq!(
        qm.consume("purged", "c").await.unwrap().unwrap().id,
        in_flight.id
    );
    assert_eq!(durable::queue_purge(log, qm, "purged").await.unwrap(), 1);
    assert!(durable::queue_delete(log, qm, "gone").await.unwrap());
    let after = publish(&layer, qm, "purged", "after-purge").await;
    layer.shutdown().await.unwrap();

    let recovered = restart(&config).await;
    let mut names = recovered.list_queues().await.unwrap();
    names.sort();
    assert_eq!(names, vec!["keep".to_string(), "purged".to_string()]);
    assert_eq!(recovered.dump_state()["keep"].config.max_depth, 12);
    let deliverable = drain(&recovered, "purged").await;
    let mut got = ids(&deliverable);
    got.sort_unstable();
    let mut want = vec![in_flight.id.as_str(), after.id.as_str()];
    want.sort_unstable();
    assert_eq!(got, want);
}

#[tokio::test]
async fn queue_lifecycle_survives_a_wal_only_restart() {
    lifecycle_history(false, "lifecycle_wal").await;
}

#[tokio::test]
async fn queue_lifecycle_survives_a_snapshot_plus_wal_restart() {
    lifecycle_history(true, "lifecycle_snapshot").await;
}

/// A delivery whose ACK deadline passed is requeued with its retry counter
/// raised, and the requeue is recorded, so the counter survives a restart.
#[tokio::test]
async fn ack_deadline_requeue_survives_a_restart() {
    let config = config("deadline_requeue");
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let server = Server::new();
    let qm = &server.queues;
    let immediate = QueueConfig {
        ack_deadline_secs: 0,
        ..QueueConfig::default()
    };
    durable::queue_create(Some(&layer), qm, "q", Some(immediate))
        .await
        .unwrap();
    let message = publish(&layer, qm, "q", "slow").await;
    qm.consume("q", "c").await.unwrap();
    assert_eq!(
        durable::queue_requeue_expired(Some(&layer), qm)
            .await
            .unwrap(),
        1
    );
    layer.shutdown().await.unwrap();

    let recovered = restart(&config).await;
    let deliverable = drain(&recovered, "q").await;
    assert_eq!(ids(&deliverable), vec![message.id.as_str()]);
    assert_eq!(deliverable[0].retry_count, 1);
}

// ── Master/replica parity ───────────────────────────────────────────────────

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral port addr")
        .port()
}

/// A master whose writes are recorded through a replication-only layer (no
/// WAL), plus a connected replica holding its own queue manager.
struct Pair {
    layer: PersistenceLayer,
    master_queues: QueueManager,
    replica_queues: Arc<QueueManager>,
    _replica: Arc<ReplicaNode>,
}

async fn pair() -> Pair {
    let addr: SocketAddr = format!("127.0.0.1:{}", free_port()).parse().unwrap();
    let master_config = ReplicationConfig {
        enabled: true,
        role: NodeRole::Master,
        replica_listen_address: Some(addr),
        heartbeat_interval_ms: 100,
        ..Default::default()
    };
    let master = Arc::new(
        MasterNode::new(
            master_config,
            Arc::new(KVStore::new(KVConfig::default())),
            Some(Arc::new(StreamManager::new(StreamConfig::default()))),
        )
        .await
        .unwrap(),
    );
    sleep(Duration::from_millis(100)).await;
    let mut persistence = PersistenceConfig::default();
    persistence.enabled = false;
    let layer = PersistenceLayer::new_with_replication(persistence, Some(master))
        .await
        .unwrap();

    let replica_queues = Arc::new(QueueManager::new(QueueConfig::default()));
    let replica = ReplicaNode::new(
        ReplicationConfig {
            enabled: true,
            role: NodeRole::Replica,
            master_address: Some(addr.into()),
            auto_reconnect: true,
            reconnect_delay_ms: 100,
            ..Default::default()
        },
        StoreArcs {
            queue_manager: Some(Arc::clone(&replica_queues)),
            stream_manager: Some(Arc::new(StreamManager::new(StreamConfig::default()))),
            ..StoreArcs::kv_only(Arc::new(KVStore::new(KVConfig::default())))
        },
    )
    .await
    .unwrap();
    // Let the initial (empty) full sync complete before the writes.
    sleep(Duration::from_millis(500)).await;
    Pair {
        layer,
        master_queues: QueueManager::new(QueueConfig::default()),
        replica_queues,
        _replica: replica,
    }
}

/// The deliverable view of every queue: per queue its ready messages (as
/// identity tuples) and its dead letters' IDs. In-flight messages are left
/// out: deliveries are not replicated, so on the replica they are ready.
type View = Vec<(
    String,
    Vec<(String, Vec<u8>, u8, u32, u32, u32)>,
    Vec<String>,
)>;

fn view(qm: &QueueManager, skip: &[&str]) -> View {
    let mut queues: View = qm
        .dump_state()
        .into_iter()
        .map(|(name, state)| {
            let ready = state
                .ready
                .iter()
                .chain(state.in_flight.iter())
                .filter(|m| !skip.contains(&m.id.as_str()))
                .map(identity)
                .collect();
            let dead = state.dead_letter.iter().map(|m| m.id.clone()).collect();
            (name, ready, dead)
        })
        .collect();
    queues.sort();
    for (_, ready, _) in &mut queues {
        ready.sort();
    }
    queues
}

/// Poll until the replica's view equals `expected`, or fail after 10 s.
async fn replica_converges(pair: &Pair, expected: &View, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let got = view(&pair.replica_queues, &[]);
        if &got == expected {
            return;
        }
        if Instant::now() >= deadline {
            panic!("{what}: replica did not converge\nreplica: {got:?}\nmaster:  {expected:?}");
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// Master and replica hold the same message IDs and metadata through
/// publish, consume, ACK, NACK, purge and delete, and the same queue
/// configuration.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn master_and_replica_keep_identical_message_ids() {
    let pair = pair().await;
    let log = Some(&pair.layer);
    let qm = &pair.master_queues;
    let custom = QueueConfig {
        max_depth: 33,
        ..QueueConfig::default()
    };
    durable::queue_create(log, qm, "q", Some(custom))
        .await
        .unwrap();
    durable::queue_create(log, qm, "doomed", None)
        .await
        .unwrap();
    let mut published = Vec::new();
    for body in ["a", "b", "c"] {
        published.push(publish(&pair.layer, qm, "q", body).await);
    }
    replica_converges(&pair, &view(qm, &[]), "three publishes").await;
    assert_eq!(pair.replica_queues.dump_state()["q"].config.max_depth, 33);

    // Consume + ACK the first: gone on both.
    let first = qm.consume("q", "c").await.unwrap().unwrap();
    durable::queue_ack(log, qm, "q", &first.id).await.unwrap();
    replica_converges(&pair, &view(qm, &[]), "ACK").await;

    // Consume + NACK the second: requeued with retry_count 1 on both.
    let second = qm.consume("q", "c").await.unwrap().unwrap();
    durable::queue_nack(log, qm, "q", &second.id, true)
        .await
        .unwrap();
    replica_converges(&pair, &view(qm, &[]), "NACK").await;
    let replica_second = pair.replica_queues.dump_state()["q"]
        .ready
        .iter()
        .find(|m| m.id == second.id)
        .cloned()
        .expect("NACKed message requeued on the replica");
    assert_eq!(replica_second.retry_count, 1);

    // A delivered message stays ready on the replica (deliveries are not
    // replicated); purge then removes only the waiting ones on both.
    let delivered = qm.consume("q", "c").await.unwrap().unwrap();
    durable::queue_purge(log, qm, "q").await.unwrap();
    durable::queue_delete(log, qm, "doomed").await.unwrap();
    replica_converges(&pair, &view(qm, &[]), "purge and delete").await;
    let replica_ready = pair.replica_queues.dump_state()["q"].ready.clone();
    assert_eq!(ids(&replica_ready), vec![delivered.id.as_str()]);
    assert!(
        !pair
            .replica_queues
            .list_queues()
            .await
            .unwrap()
            .contains(&"doomed".to_string())
    );
}

/// A replica that connects after the master already holds queue state gets
/// it through a FULL sync: the same message IDs and metadata, the message in
/// flight on the master ready on the replica, dead letters and configuration
/// kept. A later streamed ACK then settles the in-flight message by ID.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn late_replica_full_sync_keeps_message_ids_and_queue_state() {
    let addr: SocketAddr = format!("127.0.0.1:{}", free_port()).parse().unwrap();
    let master_queues = Arc::new(QueueManager::new(QueueConfig::default()));
    let master = Arc::new(
        MasterNode::with_stores(
            ReplicationConfig {
                enabled: true,
                role: NodeRole::Master,
                replica_listen_address: Some(addr),
                heartbeat_interval_ms: 100,
                ..Default::default()
            },
            StoreArcs {
                queue_manager: Some(Arc::clone(&master_queues)),
                stream_manager: Some(Arc::new(StreamManager::new(StreamConfig::default()))),
                ..StoreArcs::kv_only(Arc::new(KVStore::new(KVConfig::default())))
            },
        )
        .await
        .unwrap(),
    );
    let mut persistence = PersistenceConfig::default();
    persistence.enabled = false;
    let layer = PersistenceLayer::new_with_replication(persistence, Some(master))
        .await
        .unwrap();
    let log = Some(&layer);
    let qm = master_queues.as_ref();
    let custom = QueueConfig {
        max_depth: 44,
        ..QueueConfig::default()
    };
    durable::queue_create(log, qm, "q", Some(custom))
        .await
        .unwrap();
    durable::queue_create(log, qm, "empty", None).await.unwrap();
    let dead = durable::queue_publish(log, qm, "q", b"dead".to_vec(), Some(9), Some(0))
        .await
        .unwrap();
    let retried = durable::queue_publish(log, qm, "q", b"retried".to_vec(), Some(8), None)
        .await
        .unwrap();
    let in_flight = durable::queue_publish(log, qm, "q", b"in-flight".to_vec(), Some(7), None)
        .await
        .unwrap();
    let waiting = publish(&layer, qm, "q", "waiting").await;
    assert_eq!(qm.consume("q", "c").await.unwrap().unwrap().id, dead.id);
    durable::queue_nack(log, qm, "q", &dead.id, false)
        .await
        .unwrap();
    assert_eq!(qm.consume("q", "c").await.unwrap().unwrap().id, retried.id);
    durable::queue_nack(log, qm, "q", &retried.id, true)
        .await
        .unwrap();
    assert_eq!(
        qm.consume("q", "c").await.unwrap().unwrap().id,
        in_flight.id
    );

    // The replica connects only now: everything above reaches it as the
    // full-sync image, not as streamed records.
    let replica_queues = Arc::new(QueueManager::new(QueueConfig::default()));
    let _replica = ReplicaNode::new(
        ReplicationConfig {
            enabled: true,
            role: NodeRole::Replica,
            master_address: Some(addr.into()),
            auto_reconnect: true,
            reconnect_delay_ms: 100,
            ..Default::default()
        },
        StoreArcs {
            queue_manager: Some(Arc::clone(&replica_queues)),
            stream_manager: Some(Arc::new(StreamManager::new(StreamConfig::default()))),
            ..StoreArcs::kv_only(Arc::new(KVStore::new(KVConfig::default())))
        },
    )
    .await
    .unwrap();
    let pair = Pair {
        layer,
        master_queues: QueueManager::new(QueueConfig::default()),
        replica_queues,
        _replica,
    };
    replica_converges(&pair, &view(qm, &[]), "full sync").await;

    let state = pair.replica_queues.dump_state().remove("q").unwrap();
    assert_eq!(state.config.max_depth, 44);
    assert_eq!(ids(&state.dead_letter), vec![dead.id.as_str()]);
    let ready: HashMap<&str, &QueueMessage> =
        state.ready.iter().map(|m| (m.id.as_str(), m)).collect();
    assert_eq!(ready.len(), 3);
    assert_eq!(ready[retried.id.as_str()].retry_count, 1);
    assert_eq!(identity(ready[in_flight.id.as_str()]), identity(&in_flight));
    assert_eq!(identity(ready[waiting.id.as_str()]), identity(&waiting));
    assert!(pair.replica_queues.dump_state().contains_key("empty"));

    // A streamed ACK after the full sync settles the in-flight message.
    durable::queue_ack(Some(&pair.layer), qm, "q", &in_flight.id)
        .await
        .unwrap();
    replica_converges(&pair, &view(qm, &[]), "ACK after full sync").await;
    assert!(
        !pair.replica_queues.dump_state()["q"]
            .ready
            .iter()
            .any(|m| m.id == in_flight.id)
    );
}
