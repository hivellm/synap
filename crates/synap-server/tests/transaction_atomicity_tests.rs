//! Crash and replication atomicity of MULTI/EXEC transactions (phase22).
//!
//! A committed transaction is ONE WAL record under one checksum and ONE
//! replication command, so a crash or a dropped replica connection exposes all
//! of its effects or none of them. Also covers the versioned WAL format: a WAL
//! written by Synap 1.3.2 still replays, and a WAL from a newer format refuses
//! to start.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use synap_server::core::{
    CommittedWrite, HashStore, KVConfig, KVStore, ListStore, QueueConfig, SetStore, SortedSetStore,
};
use synap_server::persistence::types::{FsyncMode, Operation, PersistenceConfig};
use synap_server::persistence::{
    PersistenceError, PersistenceLayer, StoreArcs, recover, wal_frame,
};
use synap_server::replication::types::ReplicationOperation;
use synap_server::replication::{
    MasterNode, NodeRole, ReplicaNode, ReplicationCommand, ReplicationConfig,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const WAIT: Duration = Duration::from_secs(10);

fn fresh_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("synap_txn_atomicity_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn persistence_config(dir: &Path, mode: FsyncMode) -> PersistenceConfig {
    let mut config = PersistenceConfig::default();
    config.enabled = true;
    config.wal.enabled = true;
    config.wal.fsync_mode = mode;
    config.wal.path = dir.join("wal").join("synap.wal");
    config.snapshot.enabled = false;
    config.snapshot.directory = dir.join("snapshots");
    config
}

/// A three-operation transaction touching three stores.
fn three_writes() -> Vec<CommittedWrite> {
    vec![
        CommittedWrite::KvSet {
            key: "tk".into(),
            value: b"tv".to_vec(),
            ttl: None,
        },
        CommittedWrite::HashSet {
            key: "th".into(),
            field: "f".into(),
            value: b"hv".to_vec(),
        },
        CommittedWrite::SetAdd {
            key: "ts".into(),
            members: vec![b"m".to_vec()],
        },
    ]
}

/// The persistence operations of [`three_writes`].
fn three_operations() -> Vec<Operation> {
    vec![
        Operation::KVSet {
            key: "tk".into(),
            value: b"tv".to_vec(),
            ttl: None,
        },
        Operation::HashSet {
            key: "th".into(),
            field: "f".into(),
            value: b"hv".to_vec(),
        },
        Operation::SetAdd {
            key: "ts".into(),
            members: vec![b"m".to_vec()],
        },
    ]
}

/// Which of the three transaction effects the stores hold. A missing hash or
/// set reads as `NotFound`, which means the effect is absent.
async fn effects(kv: &KVStore, hash: &HashStore, set: &SetStore) -> [bool; 3] {
    [
        kv.get("tk").await.unwrap() == Some(b"tv".to_vec()),
        matches!(hash.hget("th", "f"), Ok(Some(value)) if value == b"hv"),
        matches!(set.sismember("ts", b"m".to_vec()), Ok(true)),
    ]
}

/// Spec "Truncation at every byte": a WAL whose last record is a
/// three-operation transaction, cut at every byte inside that record, recovers
/// either all three effects or none of them.
#[tokio::test]
async fn transaction_cut_at_every_byte_recovers_all_or_nothing() {
    let dir = fresh_dir("every_byte");
    let config = persistence_config(&dir, FsyncMode::Always);
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    layer
        .log_kv_set("before".into(), b"1".to_vec(), None)
        .await
        .unwrap();
    let boundary = std::fs::metadata(&config.wal.path).unwrap().len() as usize;
    layer.log_transaction(&three_writes()).await.unwrap();
    layer.shutdown().await.unwrap();
    drop(layer);

    let full = std::fs::read(&config.wal.path).unwrap();
    assert!(full.len() > boundary + 40, "transaction record too small");

    for cut in boundary..=full.len() {
        std::fs::write(&config.wal.path, &full[..cut]).unwrap();
        let (kv, hash, _list, set, _zset, _queue, _offset) =
            recover(&config, KVConfig::default(), QueueConfig::default())
                .await
                .unwrap();
        assert_eq!(
            kv.get("before").await.unwrap(),
            Some(b"1".to_vec()),
            "cut {cut}"
        );
        let seen = effects(&kv, hash.as_ref().unwrap(), set.as_ref().unwrap()).await;
        if cut == full.len() {
            assert_eq!(seen, [true; 3], "complete record at {cut}");
        } else {
            assert_eq!(seen, [false; 3], "torn record cut at {cut}");
            // The torn record was discarded whole: the file ends on the
            // boundary again, ready for the next append.
            assert_eq!(
                std::fs::metadata(&config.wal.path).unwrap().len() as usize,
                boundary,
                "cut {cut}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Spec "Transaction larger than the buffer": a transaction whose record is
/// several times the writer buffer, acknowledged and then lost to a crash
/// (the WAL file as it was on disk at the acknowledgement, writer never shut
/// down), recovers every effect.
#[tokio::test]
async fn transaction_larger_than_the_writer_buffer_survives_a_crash_after_ack() {
    for (name, mode) in [
        ("always", FsyncMode::Always),
        ("periodic", FsyncMode::Periodic),
    ] {
        let dir = fresh_dir(&format!("large_{name}"));
        let config = persistence_config(&dir, mode);
        let buffer_bytes = config.wal.buffer_size_kb * 1024;
        let values: Vec<Vec<u8>> = (0..3u8)
            .map(|i| (0..buffer_bytes + 7919).map(|b| (b as u8) ^ i).collect())
            .collect();
        let writes: Vec<CommittedWrite> = values
            .iter()
            .enumerate()
            .map(|(i, value)| CommittedWrite::KvSet {
                key: format!("big{i}"),
                value: value.clone(),
                ttl: None,
            })
            .collect();

        let layer = PersistenceLayer::new(config.clone()).await.unwrap();
        layer.log_transaction(&writes).await.unwrap();
        // Crash right after the acknowledgement: keep exactly what the OS has.
        let image = std::fs::read(&config.wal.path).unwrap();
        assert!(image.len() > 3 * buffer_bytes, "{name}: {}", image.len());

        let crashed = persistence_config(&dir.join("crashed"), mode);
        std::fs::create_dir_all(crashed.wal.path.parent().unwrap()).unwrap();
        std::fs::write(&crashed.wal.path, &image).unwrap();
        let (kv, ..) = recover(&crashed, KVConfig::default(), QueueConfig::default())
            .await
            .unwrap();
        for (i, value) in values.iter().enumerate() {
            assert_eq!(
                kv.get(&format!("big{i}")).await.unwrap().as_ref(),
                Some(value),
                "{name}: big{i}"
            );
        }
        drop(layer);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Spec "Legacy WAL": a WAL in the 1.3.2 on-disk format (headerless frames,
/// tests/fixtures/wal_1_3_2.wal, see make_wal_1_3_2.py) replays every entry,
/// is upgraded in place to the current format without changing its frames,
/// and keeps working with new transaction records appended after it.
#[tokio::test]
async fn wal_written_by_1_3_2_replays_and_upgrades() {
    let fixture =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wal_1_3_2.wal"))
            .unwrap();
    let dir = fresh_dir("legacy");
    let config = persistence_config(&dir, FsyncMode::Always);
    std::fs::create_dir_all(config.wal.path.parent().unwrap()).unwrap();
    std::fs::write(&config.wal.path, &fixture).unwrap();

    let (kv, hash, list, set, zset, _queue, _offset) =
        recover(&config, KVConfig::default(), QueueConfig::default())
            .await
            .unwrap();
    assert_legacy_state(
        &kv,
        &hash.unwrap(),
        &list.unwrap(),
        &set.unwrap(),
        &zset.unwrap(),
    )
    .await;

    // Upgraded: the current header, then the 1.3.2 frames byte for byte.
    let upgraded = std::fs::read(&config.wal.path).unwrap();
    assert_eq!(&upgraded[..12], &wal_frame::encode_file_header());
    assert_eq!(&upgraded[12..], &fixture[..]);

    // New records continue the 1.3.2 offsets (1..=8) and replay after them.
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    layer.log_transaction(&three_writes()).await.unwrap();
    layer.shutdown().await.unwrap();
    drop(layer);
    let mut offsets = Vec::new();
    wal_frame::scan(&config.wal.path, |entry| offsets.push(entry.offset)).unwrap();
    assert_eq!(offsets, (1..=9).collect::<Vec<u64>>());

    let (kv, hash, list, set, zset, _queue, _offset) =
        recover(&config, KVConfig::default(), QueueConfig::default())
            .await
            .unwrap();
    let (hash, set) = (hash.unwrap(), set.unwrap());
    assert_legacy_state(&kv, &hash, &list.unwrap(), &set, &zset.unwrap()).await;
    assert_eq!(effects(&kv, &hash, &set).await, [true; 3]);
    let _ = std::fs::remove_dir_all(&dir);
}

async fn assert_legacy_state(
    kv: &KVStore,
    hash: &HashStore,
    list: &ListStore,
    set: &SetStore,
    zset: &SortedSetStore,
) {
    assert_eq!(kv.get("legacy:a").await.unwrap(), Some(b"alpha".to_vec()));
    assert_eq!(kv.get("legacy:gone").await.unwrap(), None);
    assert_eq!(hash.hget("legacy:h", "f").unwrap(), Some(b"hv".to_vec()));
    assert_eq!(hash.hget("legacy:h", "n").unwrap(), Some(b"7".to_vec()));
    assert_eq!(
        list.lrange("legacy:l", 0, -1).unwrap(),
        vec![b"one".to_vec(), b"two".to_vec()]
    );
    let mut members = set.smembers("legacy:s").unwrap();
    members.sort();
    assert_eq!(members, vec![b"m1".to_vec(), b"m2".to_vec()]);
    assert_eq!(zset.zscore("legacy:z", b"zm"), Some(2.5));
}

/// Spec "Unknown version": a WAL whose header declares a newer format fails
/// recovery and the WAL open with an error naming both versions, and the file
/// is left untouched.
#[tokio::test]
async fn newer_wal_format_refuses_to_start() {
    let dir = fresh_dir("newer");
    let config = persistence_config(&dir, FsyncMode::Always);
    std::fs::create_dir_all(config.wal.path.parent().unwrap()).unwrap();
    let mut bytes = wal_frame::WAL_MAGIC.to_vec();
    bytes.extend_from_slice(&(wal_frame::WAL_FORMAT_VERSION + 1).to_be_bytes());
    std::fs::write(&config.wal.path, &bytes).unwrap();

    let err = recover(&config, KVConfig::default(), QueueConfig::default())
        .await
        .err()
        .expect("a newer WAL format must not replay");
    assert!(
        matches!(
            err,
            PersistenceError::WALUnsupportedVersion {
                found: 3,
                supported: 2,
                ..
            }
        ),
        "{err:?}"
    );
    assert!(err.refuses_startup());
    let message = err.to_string();
    assert!(
        message.contains("format version 3") && message.contains("up to 2"),
        "{message}"
    );
    let err = PersistenceLayer::new(config.clone()).await.err().unwrap();
    assert!(err.refuses_startup(), "{err}");
    assert_eq!(std::fs::read(&config.wal.path).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The stores a test replica applies to.
struct ReplicaStores {
    kv: Arc<KVStore>,
    hash: Arc<HashStore>,
    set: Arc<SetStore>,
}

impl ReplicaStores {
    fn new() -> Self {
        Self {
            kv: Arc::new(KVStore::new(KVConfig::default())),
            hash: Arc::new(HashStore::new()),
            set: Arc::new(SetStore::new()),
        }
    }

    fn arcs(&self) -> StoreArcs {
        let mut arcs = StoreArcs::kv_only(Arc::clone(&self.kv));
        arcs.hash_store = Some(Arc::clone(&self.hash));
        arcs.set_store = Some(Arc::clone(&self.set));
        arcs
    }

    async fn effects(&self) -> [bool; 3] {
        effects(&self.kv, &self.hash, &self.set).await
    }
}

fn replica_config(master: std::net::SocketAddr) -> ReplicationConfig {
    ReplicationConfig {
        enabled: true,
        role: NodeRole::Replica,
        master_address: Some(master.into()),
        auto_reconnect: true,
        reconnect_delay_ms: 50,
        ..Default::default()
    }
}

/// A replication command framed the way the master sends it.
fn command_frame(cmd: &ReplicationCommand) -> Vec<u8> {
    let data = bincode::serde::encode_to_vec(cmd, bincode::config::legacy()).unwrap();
    let mut frame = (data.len() as u32).to_be_bytes().to_vec();
    frame.extend(data);
    frame
}

/// Accept the replica's next connection and return it with the offset its
/// handshake requests.
async fn accept_replica(listener: &TcpListener) -> (TcpStream, u64) {
    let (mut stream, _) = tokio::time::timeout(WAIT, listener.accept())
        .await
        .expect("replica did not reconnect")
        .unwrap();
    let mut handshake = [0u8; 8];
    stream.read_exact(&mut handshake).await.unwrap();
    let (offset, _) =
        bincode::serde::decode_from_slice::<u64, _>(&handshake, bincode::config::legacy()).unwrap();
    (stream, offset)
}

async fn wait_until<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + WAIT;
    while !check().await {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Spec "Replica disconnect mid-transaction": the master connection drops
/// while a transaction command is partly sent (inside the length prefix, right
/// after it, mid-payload, one byte short). The replica holds none of the
/// transaction's effects after each drop, asks to resume at the transaction's
/// offset, and applies it whole once the resync delivers it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replica_disconnect_mid_transaction_applies_nothing_until_resync() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let stores = ReplicaStores::new();
    let replica = ReplicaNode::new(
        replica_config(listener.local_addr().unwrap()),
        stores.arcs(),
    )
    .await
    .unwrap();

    // First connection: sync, one plain write at offset 0.
    let (mut conn, offset) = accept_replica(&listener).await;
    assert_eq!(offset, 0);
    let sync = ReplicationCommand::PartialSync {
        from_offset: 0,
        operations: vec![],
    };
    conn.write_all(&command_frame(&sync)).await.unwrap();
    let before = ReplicationCommand::Operation(ReplicationOperation {
        offset: 0,
        timestamp: 0,
        operation: Operation::KVSet {
            key: "before".into(),
            value: b"1".to_vec(),
            ttl: None,
        },
    });
    conn.write_all(&command_frame(&before)).await.unwrap();

    let transaction = ReplicationOperation {
        offset: 1,
        timestamp: 0,
        operation: Operation::Transaction {
            operations: three_operations(),
        },
    };
    let frame = command_frame(&ReplicationCommand::Operation(transaction.clone()));
    let cuts = [2, 4, frame.len() / 2, frame.len() - 1];

    for (round, cut) in cuts.into_iter().enumerate() {
        conn.write_all(&frame[..cut]).await.unwrap();
        conn.flush().await.unwrap();
        drop(conn);

        // The replica gives up on the partial command and reconnects, asking
        // to resume at the transaction's offset.
        let (next, offset) = accept_replica(&listener).await;
        assert_eq!(offset, 1, "cut {cut}: resume point");
        assert_eq!(stores.kv.get("before").await.unwrap(), Some(b"1".to_vec()));
        assert_eq!(stores.effects().await, [false; 3], "cut {cut}");
        assert_eq!(replica.current_offset(), 1, "cut {cut}");
        conn = next;
        if round + 1 < cuts.len() {
            let resync = ReplicationCommand::PartialSync {
                from_offset: 1,
                operations: vec![],
            };
            conn.write_all(&command_frame(&resync)).await.unwrap();
        }
    }

    // Resync delivers the whole record.
    let resync = ReplicationCommand::PartialSync {
        from_offset: 1,
        operations: vec![transaction],
    };
    conn.write_all(&command_frame(&resync)).await.unwrap();
    wait_until("the resynced transaction", || async {
        stores.effects().await == [true; 3]
    })
    .await;
    assert_eq!(replica.current_offset(), 2);
}

/// End to end through a real master: `log_transaction` on the master reaches a
/// connected replica as one unit and is applied whole.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn master_ships_a_transaction_to_a_replica_as_one_unit() {
    let addr: std::net::SocketAddr = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        probe.local_addr().unwrap()
    };
    let mut master_config = ReplicationConfig::default();
    master_config.enabled = true;
    master_config.role = NodeRole::Master;
    master_config.replica_listen_address = Some(addr);
    master_config.heartbeat_interval_ms = 100;
    let master_kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = Arc::new(
        MasterNode::new(master_config, master_kv, None)
            .await
            .unwrap(),
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stores = ReplicaStores::new();
    let replica = ReplicaNode::new(replica_config(addr), stores.arcs())
        .await
        .unwrap();
    wait_until("the replica to connect", || async {
        !master.list_replicas().is_empty() && replica.is_connected()
    })
    .await;

    let mut config = PersistenceConfig::default();
    config.enabled = false;
    let layer = PersistenceLayer::new_with_replication(config, Some(Arc::clone(&master)))
        .await
        .unwrap();
    let start = master.replication_offset();
    layer.log_transaction(&three_writes()).await.unwrap();
    assert_eq!(
        master.replication_offset(),
        start + 1,
        "one replication unit"
    );

    wait_until("the replicated transaction", || async {
        stores.effects().await == [true; 3]
    })
    .await;
    assert_eq!(replica.current_offset(), start + 1);
}
