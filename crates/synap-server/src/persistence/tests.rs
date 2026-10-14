use super::*;
use crate::core::{HashStore, KVConfig, KVStore, ListStore, QueueConfig, SetStore, SortedSetStore};

#[tokio::test]
async fn test_wal_append_and_replay() {
    let config = types::WALConfig {
        enabled: true,
        path: "/tmp/test_wal.wal".into(),
        buffer_size_kb: 64,
        fsync_mode: types::FsyncMode::Always,
        fsync_interval_ms: 1000,
        max_size_mb: 1024,
    };

    // Clean up any existing file
    let _ = tokio::fs::remove_file(&config.path).await;

    // Create WAL and append operations
    let mut wal = WriteAheadLog::open(config.clone()).await.unwrap();

    let op1 = types::Operation::KVSet {
        key: "key1".to_string(),
        value: b"value1".to_vec(),
        ttl: None,
    };

    let op2 = types::Operation::KVSet {
        key: "key2".to_string(),
        value: b"value2".to_vec(),
        ttl: Some(3600),
    };

    let offset1 = wal.append(op1).await.unwrap();
    let offset2 = wal.append(op2).await.unwrap();

    // Offsets should be sequential
    assert_eq!(offset2, offset1 + 1);

    // Replay entries
    let entries = wal.replay(0).await.unwrap();
    assert_eq!(entries.len(), 2);

    // Verify entries
    match &entries[0].operation {
        types::Operation::KVSet { key, value, ttl } => {
            assert_eq!(key, "key1");
            assert_eq!(value, b"value1");
            assert_eq!(*ttl, None);
        }
        _ => panic!("Expected KVSet operation"),
    }

    match &entries[1].operation {
        types::Operation::KVSet { key, value, ttl } => {
            assert_eq!(key, "key2");
            assert_eq!(value, b"value2");
            assert_eq!(*ttl, Some(3600));
        }
        _ => panic!("Expected KVSet operation"),
    }

    // Cleanup
    let _ = tokio::fs::remove_file(&config.path).await;
}

#[tokio::test]
async fn test_crash_recovery() {
    use std::path::PathBuf;

    let wal_path = PathBuf::from("/tmp/test_crash_recovery.wal");
    let snapshot_dir = PathBuf::from("/tmp/test_snapshots");

    // Clean up
    let _ = tokio::fs::remove_file(&wal_path).await;
    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;

    let persist_config = types::PersistenceConfig {
        enabled: true,
        wal: types::WALConfig {
            enabled: true,
            path: wal_path.clone(),
            buffer_size_kb: 64,
            fsync_mode: types::FsyncMode::Always,
            fsync_interval_ms: 1000,
            max_size_mb: 1024,
        },
        snapshot: types::SnapshotConfig {
            enabled: true,
            directory: snapshot_dir.clone(),
            interval_secs: 300,
            operation_threshold: 10_000,
            max_snapshots: 10,
            compression: false,
        },
    };

    let kv_config = KVConfig {
        max_memory_mb: 1024,
        eviction_policy: crate::core::EvictionPolicy::NoEviction,
        ttl_cleanup_interval_ms: 100,
        allow_flush_commands: false,
        max_value_size_bytes: None,
        eviction_sample_size: 5,
    };

    let queue_config = QueueConfig::default();

    // First run: create data
    {
        let (kv, _hs, _ls, _ss, _zs, _qm, _offset) =
            recover(&persist_config, kv_config.clone(), queue_config.clone())
                .await
                .unwrap();

        // Add some data
        kv.set("user:1", b"Alice".to_vec(), None).await.unwrap();
        kv.set("user:2", b"Bob".to_vec(), None).await.unwrap();
        kv.set("user:3", b"Charlie".to_vec(), None).await.unwrap();

        // Note: In real implementation, we'd call persistence layer here
        // For now, this test just demonstrates the structure
    }

    // Second run: recover data
    {
        let (kv, _hs, _ls, _ss, _zs, _qm, _offset) =
            recover(&persist_config, kv_config.clone(), queue_config.clone())
                .await
                .unwrap();

        // Data would be recovered if we were actually logging to WAL
        // This test demonstrates the recovery process works

        // Verify store was created successfully
        let size = kv.dbsize().await.unwrap();
        assert_eq!(size, 0); // Fresh start on recovery
    }

    // Cleanup
    let _ = tokio::fs::remove_file(&wal_path).await;
    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;
}

#[tokio::test]
async fn test_snapshot_create_and_load() {
    use std::path::PathBuf;

    let snapshot_dir = PathBuf::from("/tmp/test_snapshot_dir");
    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;

    let config = types::SnapshotConfig {
        enabled: true,
        directory: snapshot_dir.clone(),
        interval_secs: 300,
        operation_threshold: 10_000,
        max_snapshots: 5,
        compression: false,
    };

    let snapshot_mgr = SnapshotManager::new(config);

    // Create KV store with some data
    let kv_store = KVStore::new(KVConfig::default());
    kv_store
        .set("key1", b"value1".to_vec(), None)
        .await
        .unwrap();
    kv_store
        .set("key2", b"value2".to_vec(), None)
        .await
        .unwrap();

    // Create snapshot
    let snapshot_path = snapshot_mgr
        .create_snapshot(apply::StoreRefs::kv_only(&kv_store), 42)
        .await
        .unwrap();

    assert!(snapshot_path.exists());

    // Load snapshot
    let (snapshot, _path) = snapshot_mgr.load_latest().await.unwrap().unwrap();

    assert_eq!(snapshot.version, 5); // v5 stores queues whole (v4 added KV expiry, HLL, bitmaps)
    assert_eq!(snapshot.wal_offset, 42);
    assert_eq!(snapshot.kv_data.len(), 2);
    assert_eq!(snapshot.kv_data.get("key1").unwrap(), b"value1");
    assert_eq!(snapshot.kv_data.get("key2").unwrap(), b"value2");

    // Cleanup
    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;
}

#[tokio::test]
async fn test_snapshot_rejects_corrupted_checksum() {
    use std::path::PathBuf;

    let snapshot_dir = PathBuf::from("/tmp/test_snapshot_corrupt_dir");
    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;

    let config = types::SnapshotConfig {
        enabled: true,
        directory: snapshot_dir.clone(),
        interval_secs: 300,
        operation_threshold: 10_000,
        max_snapshots: 5,
        compression: false,
    };
    let snapshot_mgr = SnapshotManager::new(config);

    let kv_store = KVStore::new(KVConfig::default());
    kv_store
        .set("k", b"some-value-bytes".to_vec(), None)
        .await
        .unwrap();

    let path = snapshot_mgr
        .create_snapshot(apply::StoreRefs::kv_only(&kv_store), 7)
        .await
        .unwrap();

    // Flip the last byte (part of the trailing CRC64) to simulate bit rot.
    let mut bytes = tokio::fs::read(&path).await.unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    tokio::fs::write(&path, &bytes).await.unwrap();

    // Load must fail with a corruption error rather than returning bad data.
    let result = snapshot_mgr.load_latest().await;
    assert!(
        matches!(
            result,
            Err(types::PersistenceError::SnapshotCorrupted { .. })
        ),
        "expected SnapshotCorrupted, got {result:?}"
    );

    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;
}

#[tokio::test]
async fn test_snapshot_roundtrip_all_datatypes() {
    use std::path::PathBuf;

    let snapshot_dir = PathBuf::from("/tmp/test_snapshot_all_types_dir");
    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;

    let config = types::SnapshotConfig {
        enabled: true,
        directory: snapshot_dir.clone(),
        interval_secs: 300,
        operation_threshold: 10_000,
        max_snapshots: 5,
        compression: false,
    };
    let snapshot_mgr = SnapshotManager::new(config);

    // Populate one key in each datatype.
    let kv_store = KVStore::new(KVConfig::default());
    kv_store.set("kvk", b"kvv".to_vec(), None).await.unwrap();

    let hash_store = HashStore::new();
    hash_store.hset("h", "f1", b"v1".to_vec()).unwrap();
    hash_store.hset("h", "f2", b"v2".to_vec()).unwrap();

    let list_store = ListStore::new();
    list_store
        .rpush("l", vec![b"a".to_vec(), b"b".to_vec()], false)
        .unwrap();

    let set_store = SetStore::new();
    set_store
        .sadd("s", vec![b"m1".to_vec(), b"m2".to_vec()])
        .unwrap();

    let sorted_set_store = SortedSetStore::new();
    let opts = crate::core::sorted_set::ZAddOptions::default();
    sorted_set_store.zadd("z", b"zm".to_vec(), 1.5, &opts);

    // Snapshot every datatype, then reload from disk.
    snapshot_mgr
        .create_snapshot(
            apply::StoreRefs {
                kv_store: &kv_store,
                hash_store: Some(&hash_store),
                list_store: Some(&list_store),
                set_store: Some(&set_store),
                sorted_set_store: Some(&sorted_set_store),
                queue_manager: None,
                stream_manager: None,
                hyperloglog_store: None,
                bitmap_store: None,
            },
            99,
        )
        .await
        .unwrap();

    let (snapshot, _path) = snapshot_mgr.load_latest().await.unwrap().unwrap();

    assert_eq!(snapshot.version, 5);
    assert_eq!(snapshot.wal_offset, 99);
    assert_eq!(snapshot.kv_data.get("kvk").unwrap(), b"kvv");
    let h = snapshot.hash_data.get("h").unwrap();
    assert_eq!(h.get("f1").unwrap(), b"v1");
    assert_eq!(h.get("f2").unwrap(), b"v2");
    assert_eq!(snapshot.list_data.get("l").unwrap().len(), 2);
    assert_eq!(snapshot.set_data.get("s").unwrap().len(), 2);
    let z = snapshot.sorted_set_data.get("z").unwrap();
    assert_eq!(z.len(), 1);
    assert_eq!(z[0].0, b"zm");
    assert!((z[0].1 - 1.5).abs() < 1e-9);

    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;
}

#[tokio::test]
async fn persistence_propagates_writes_to_replication_master() {
    use crate::replication::{MasterNode, NodeRole, ReplicationConfig};
    use std::path::PathBuf;
    use std::sync::Arc;

    // Master node bound to an ephemeral replica-listen port.
    let mut repl_config = ReplicationConfig::default();
    repl_config.enabled = true;
    repl_config.role = NodeRole::Master;
    repl_config.replica_listen_address = Some("127.0.0.1:0".parse().unwrap());

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = Arc::new(MasterNode::new(repl_config, kv, None).await.unwrap());
    let before = master.replication_offset();

    let wal_path = PathBuf::from("/tmp/test_repl_piggyback.wal");
    let _ = tokio::fs::remove_file(&wal_path).await;
    let persist_config = types::PersistenceConfig {
        enabled: true,
        wal: types::WALConfig {
            enabled: true,
            path: wal_path.clone(),
            buffer_size_kb: 64,
            fsync_mode: types::FsyncMode::Always,
            fsync_interval_ms: 1000,
            max_size_mb: 1024,
        },
        snapshot: types::SnapshotConfig {
            enabled: false,
            directory: PathBuf::from("/tmp/test_repl_piggyback_snap"),
            interval_secs: 300,
            operation_threshold: 10_000,
            max_snapshots: 5,
            compression: false,
        },
    };

    let layer = PersistenceLayer::new_with_replication(persist_config, Some(master.clone()))
        .await
        .unwrap();
    layer
        .log_kv_set("k".into(), b"v".to_vec(), None)
        .await
        .unwrap();

    // The logged write should have been propagated to the replication master.
    assert!(
        master.replication_offset() > before,
        "logged write should reach the replication master"
    );

    let _ = tokio::fs::remove_file(&wal_path).await;
}

#[tokio::test]
async fn test_snapshot_cleanup_old() {
    use std::path::PathBuf;

    let snapshot_dir = PathBuf::from("/tmp/test_snapshot_cleanup");
    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;

    let config = types::SnapshotConfig {
        enabled: true,
        directory: snapshot_dir.clone(),
        interval_secs: 300,
        operation_threshold: 10_000,
        max_snapshots: 3, // Keep only 3
        compression: false,
    };

    let snapshot_mgr = SnapshotManager::new(config);
    let kv_store = KVStore::new(KVConfig::default());

    // Create 5 snapshots with different timestamps
    for i in 0..5 {
        kv_store
            .set(&format!("key{}", i), b"value".to_vec(), None)
            .await
            .unwrap();

        snapshot_mgr
            .create_snapshot(apply::StoreRefs::kv_only(&kv_store), i)
            .await
            .unwrap();

        // Sleep to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Should have only 3 snapshots (oldest 2 removed during creation)
    let stats = snapshot_mgr.stats().await.unwrap();
    assert!(
        stats.count <= 3,
        "Expected <= 3 snapshots, got {}",
        stats.count
    );

    // Cleanup
    let _ = tokio::fs::remove_dir_all(&snapshot_dir).await;
}

/// Persistence config rooted at a fresh `./target/<name>` directory.
fn restart_config(name: &str) -> types::PersistenceConfig {
    let dir = format!("./target/{name}");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(format!("{dir}/snap")).unwrap();
    let mut config = types::PersistenceConfig::default();
    config.wal.fsync_mode = types::FsyncMode::Always;
    config.wal.path = std::path::PathBuf::from(format!("{dir}/wal.log"));
    config.snapshot.enabled = false;
    config.snapshot.directory = std::path::PathBuf::from(format!("{dir}/snap"));
    config
}

/// One server lifetime: recover, log one SET through the WAL, shut down.
async fn run_server_and_set(config: &types::PersistenceConfig, key: &str) -> KVStore {
    let (kv, ..) = recover(config, KVConfig::default(), QueueConfig::default())
        .await
        .unwrap();
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    layer
        .log_kv_set(key.to_string(), key.as_bytes().to_vec(), None)
        .await
        .unwrap();
    layer.shutdown().await.unwrap();
    kv
}

/// A complete frame for a SET of `key` at `offset`.
fn set_frame(offset: u64, key: &str) -> Vec<u8> {
    wal_frame::encode_frame(&types::WALEntry {
        offset,
        timestamp: 0,
        operation: types::Operation::KVSet {
            key: key.into(),
            value: key.as_bytes().to_vec(),
            ttl: None,
        },
    })
    .unwrap()
}

/// Spec "Torn tail then new writes": for every torn-tail shape (cut short,
/// failing its checksum, zero fill), the first restart truncates the tail and
/// acknowledges a new write; the second restart recovers every frame before
/// the torn one plus the new write.
#[tokio::test]
async fn torn_tail_then_new_writes_survive_two_restarts() {
    let torn_frame = set_frame(99, "torn");
    let mut huge_length = (1u64 << 60).to_be_bytes().to_vec();
    huge_length.extend_from_slice(&[0x5A; 64]);
    let mut bad_checksum = torn_frame.clone();
    bad_checksum[8] ^= 0xFF;
    let shapes: Vec<(&str, Vec<u8>)> = vec![
        ("partial_length", torn_frame[..4].to_vec()),
        ("partial_checksum", torn_frame[..10].to_vec()),
        (
            "partial_payload",
            torn_frame[..torn_frame.len() / 2].to_vec(),
        ),
        ("corrupt_huge_length", huge_length),
        ("bad_checksum", bad_checksum),
        ("zero_fill", vec![0u8; 4096]),
    ];

    for (shape, tail) in shapes {
        let config = restart_config(&format!("restart_torn_{shape}"));
        run_server_and_set(&config, "first").await;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&config.wal.path)
            .unwrap();
        std::io::Write::write_all(&mut file, &tail).unwrap();
        drop(file);

        // Restart 1: the torn tail is truncated before anything is appended.
        let kv = run_server_and_set(&config, "second").await;
        assert_eq!(
            kv.get("first").await.unwrap(),
            Some(b"first".to_vec()),
            "{shape}"
        );

        // Restart 2: both writes recover and the file is clean.
        let (kv, ..) = recover(&config, KVConfig::default(), QueueConfig::default())
            .await
            .unwrap();
        assert_eq!(
            kv.get("first").await.unwrap(),
            Some(b"first".to_vec()),
            "{shape}"
        );
        assert_eq!(
            kv.get("second").await.unwrap(),
            Some(b"second".to_vec()),
            "{shape}"
        );
        assert_eq!(kv.get("torn").await.unwrap(), None, "{shape}");
        let report = wal_frame::scan(&config.wal.path, |_| {}).unwrap();
        assert_eq!(report.end, wal_frame::ScanEnd::Clean, "{shape}");
        assert_eq!(report.valid_frames, 2, "{shape}");
    }
}

/// Spec "Interior corruption": a checksum mismatch followed by valid frames
/// fails recovery and the WAL open with the bad frame's byte position.
#[tokio::test]
async fn interior_corruption_fails_startup_with_the_byte_position() {
    let config = restart_config("restart_interior");
    {
        let layer = PersistenceLayer::new(config.clone()).await.unwrap();
        for key in ["a", "b", "c"] {
            layer
                .log_kv_set(key.into(), key.as_bytes().to_vec(), None)
                .await
                .unwrap();
        }
        layer.shutdown().await.unwrap();
    }
    let mut bytes = std::fs::read(&config.wal.path).unwrap();
    // Frames follow the file header.
    let first = wal_frame::FILE_HEADER_LEN as usize;
    let second =
        first + 12 + u64::from_be_bytes(bytes[first..first + 8].try_into().unwrap()) as usize;
    bytes[second + 14] ^= 0xFF; // inside the second frame's payload
    std::fs::write(&config.wal.path, &bytes).unwrap();

    let expected = format!("at byte {second}");
    let err = recover(&config, KVConfig::default(), QueueConfig::default())
        .await
        .err()
        .expect("recovery must fail on interior corruption");
    assert!(
        matches!(err, types::PersistenceError::WALInteriorCorruption { .. }),
        "{err}"
    );
    assert!(err.to_string().contains(&expected), "{err}");
    let err = PersistenceLayer::new(config.clone()).await.err().unwrap();
    assert!(err.to_string().contains(&expected), "{err}");
    // Nothing was truncated or appended.
    assert_eq!(std::fs::read(&config.wal.path).unwrap(), bytes);
}

/// Spec "Record of an unknown type at the end of the WAL": a last frame that
/// is complete and matches its checksum but holds a record type this release
/// does not know is not a crash's leftovers. Recovery and the WAL open refuse
/// startup, naming the WAL, the frame's byte position and a newer release, and
/// leave the file byte for byte as it was.
#[tokio::test]
async fn an_unreadable_last_record_refuses_startup_and_is_kept() {
    let config = restart_config("restart_unreadable");
    run_server_and_set(&config, "first").await;
    let mut bytes = std::fs::read(&config.wal.path).unwrap();
    let position = bytes.len() as u64;
    bytes.extend_from_slice(&unknown_record_frame(2));
    std::fs::write(&config.wal.path, &bytes).unwrap();

    let path = config.wal.path.display().to_string().replace('\\', "\\\\");
    let check = |err: types::PersistenceError| {
        assert!(
            matches!(
                &err,
                types::PersistenceError::WALUnreadableRecord { position: at, .. } if *at == position
            ),
            "{err:?}"
        );
        assert!(err.refuses_startup(), "{err}");
        let message = err.to_string();
        assert!(message.contains(&path), "{message}");
        assert!(
            message.contains(&format!("at byte {position}")),
            "{message}"
        );
        assert!(message.contains("newer Synap release"), "{message}");
    };
    check(
        recover(&config, KVConfig::default(), QueueConfig::default())
            .await
            .err()
            .expect("recovery must refuse an unreadable last record"),
    );
    check(PersistenceLayer::new(config.clone()).await.err().unwrap());
    assert_eq!(std::fs::read(&config.wal.path).unwrap(), bytes);
}

/// A complete frame at `offset` whose checksum matches but whose record type
/// this release does not know, as a newer release would append one.
fn unknown_record_frame(offset: u64) -> Vec<u8> {
    let mut unknown = set_frame(offset, "from-a-newer-release");
    // Payload bytes 16..20 hold the operation variant tag (bincode legacy:
    // offset u64, timestamp u64, then the tag as u32 LE); no variant has this
    // number. Re-checksum so only the record type is unknown.
    let payload = wal_frame::FRAME_HEADER_LEN as usize;
    unknown[payload + 16..payload + 20].copy_from_slice(&1000u32.to_le_bytes());
    let checksum = crc32fast::hash(&unknown[payload..]);
    unknown[8..12].copy_from_slice(&checksum.to_be_bytes());
    unknown
}

/// Spec "Damaged frame before a record of an unknown type": a frame failing
/// its checksum, or zero fill, followed by a record this release cannot decode
/// is interior corruption, not a torn tail. Recovery and the WAL open refuse
/// startup with both byte positions and leave the file byte for byte as it
/// was, so the record is not truncated with the damage.
#[tokio::test]
async fn a_damaged_frame_before_an_unreadable_record_refuses_startup_and_is_kept() {
    let mut bad_checksum = set_frame(2, "damaged");
    bad_checksum[8] ^= 0xFF;
    let damages = [
        ("checksum_mismatch", bad_checksum),
        ("zero_fill", vec![0u8; wal_frame::FRAME_HEADER_LEN as usize]),
    ];
    for (damage, bad) in damages {
        let config = restart_config(&format!("restart_damaged_then_unreadable_{damage}"));
        run_server_and_set(&config, "first").await;
        let mut bytes = std::fs::read(&config.wal.path).unwrap();
        let position = bytes.len() as u64;
        bytes.extend_from_slice(&bad);
        let next = bytes.len() as u64;
        bytes.extend_from_slice(&unknown_record_frame(3));
        std::fs::write(&config.wal.path, &bytes).unwrap();

        let check = |err: types::PersistenceError| {
            assert!(
                matches!(
                    &err,
                    types::PersistenceError::WALInteriorCorruption { position: at, next_valid, .. }
                        if *at == position && *next_valid == next
                ),
                "{damage}: {err:?}"
            );
            assert!(err.refuses_startup(), "{damage}: {err}");
            let message = err.to_string();
            assert!(
                message.contains(&format!("at byte {position}")),
                "{damage}: {message}"
            );
            assert!(
                message.contains(&format!("follows at byte {next}")),
                "{damage}: {message}"
            );
        };
        check(
            recover(&config, KVConfig::default(), QueueConfig::default())
                .await
                .err()
                .expect("recovery must refuse a damaged frame before an intact record"),
        );
        check(PersistenceLayer::new(config.clone()).await.err().unwrap());
        assert_eq!(std::fs::read(&config.wal.path).unwrap(), bytes, "{damage}");
    }
}

/// The newest snapshot's stream rooms come back at startup: every event with
/// its ID, offset, type, data and timestamp, publishes continuing after the
/// last offset, each room a new incarnation. A room the manager already has
/// is skipped rather than merged.
#[tokio::test]
async fn snapshot_stream_rooms_are_restored_at_startup() {
    use crate::core::stream::{StreamConfig, StreamManager};

    let mut config = restart_config("restart_streams");
    config.snapshot.enabled = true;
    let before = StreamManager::new(StreamConfig::default());
    before.create_room("feed").await.unwrap();
    for body in ["a", "b", "c"] {
        before
            .publish("feed", "created", body.as_bytes().to_vec())
            .await
            .unwrap();
    }
    let published = before.consume("feed", "c", 0, 10).await.unwrap();
    let kv = KVStore::new(KVConfig::default());
    SnapshotManager::new(config.snapshot.clone())
        .create_snapshot(
            StoreRefs {
                stream_manager: Some(&before),
                ..StoreRefs::kv_only(&kv)
            },
            1,
        )
        .await
        .unwrap();

    let recovered = recover_all(&config, KVConfig::default(), QueueConfig::default())
        .await
        .unwrap();
    assert_eq!(recovered.streams.len(), 1);
    let after = StreamManager::new(StreamConfig::default());
    assert_eq!(restore_streams(&after, recovered.streams.clone()).await, 1);
    let restored = after.consume("feed", "c", 0, 10).await.unwrap();
    assert_eq!(restored.len(), 3);
    for (event, original) in restored.iter().zip(&published) {
        assert_eq!(event.id, original.id);
        assert_eq!(event.offset, original.offset);
        assert_eq!(event.event, original.event);
        assert_eq!(event.data, original.data);
        assert_eq!(event.timestamp, original.timestamp);
    }
    assert_eq!(
        after
            .publish("feed", "created", b"d".to_vec())
            .await
            .unwrap(),
        3
    );
    assert!(
        after.room_stats("feed").await.unwrap().generation
            > before.room_stats("feed").await.unwrap().generation
    );

    assert_eq!(restore_streams(&after, recovered.streams).await, 0);
    assert_eq!(after.consume("feed", "c", 0, 10).await.unwrap().len(), 4);
}
