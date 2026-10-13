//! phase32: snapshot files are published atomically, a damaged newest
//! snapshot falls back only when the WAL covers the gap, retention never
//! removes the only valid snapshot, and the WAL never falls behind a snapshot.
//!
//! Every test writes through `persistence::durable` (the path every
//! transport uses), takes snapshots through `PersistenceLayer::snapshot`,
//! simulates the crash or damage on disk, restarts with `recover_all` and
//! compares with the acknowledged state.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use synap_server::core::{
    HashStore, KVConfig, KVStore, ListStore, QueueConfig, SetStore, SortedSetStore, ZAddOptions,
};
use synap_server::persistence::types::{FsyncMode, PersistenceError};
use synap_server::persistence::{
    PersistenceConfig, PersistenceLayer, RecoveredStores, SnapshotManager, StoreRefs,
    WriteAheadLog, durable, recover_all, wal_frame,
};

fn config(name: &str, max_snapshots: usize) -> PersistenceConfig {
    let dir = format!("./target/phase32_{name}");
    let _ = std::fs::remove_dir_all(&dir);
    let mut config = PersistenceConfig::default();
    config.enabled = true;
    config.wal.enabled = true;
    config.wal.fsync_mode = FsyncMode::Always;
    config.wal.path = format!("{dir}/wal/synap.wal").into();
    config.snapshot.enabled = true;
    config.snapshot.directory = format!("{dir}/snapshots").into();
    config.snapshot.interval_secs = 3600;
    config.snapshot.operation_threshold = usize::MAX;
    config.snapshot.max_snapshots = max_snapshots;
    config
}

/// The WAL-backed stores these tests write to.
struct Stores {
    kv: KVStore,
    hash: HashStore,
    list: ListStore,
    set: SetStore,
    zset: SortedSetStore,
}

impl Stores {
    fn new() -> Self {
        Self {
            kv: KVStore::new(KVConfig::default()),
            hash: HashStore::new(),
            list: ListStore::new(),
            set: SetStore::new(),
            zset: SortedSetStore::new(),
        }
    }

    fn refs(&self) -> StoreRefs<'_> {
        StoreRefs {
            kv_store: &self.kv,
            hash_store: Some(&self.hash),
            list_store: Some(&self.list),
            set_store: Some(&self.set),
            sorted_set_store: Some(&self.zset),
            queue_manager: None,
            stream_manager: None,
            hyperloglog_store: None,
            bitmap_store: None,
        }
    }

    /// One round of acknowledged writes across the datatypes, tagged `round`
    /// so every round changes the state (non-idempotent LPUSH and HINCRBY
    /// included, so a replay from the wrong cutoff shows up).
    async fn write_round(&self, layer: &PersistenceLayer, round: u32) {
        let log = Some(layer);
        durable::kv_set(
            log,
            &self.kv,
            format!("k{round}"),
            format!("v{round}").into_bytes(),
            None,
        )
        .await
        .unwrap();
        durable::kv_set(
            log,
            &self.kv,
            "shared",
            format!("r{round}").into_bytes(),
            None,
        )
        .await
        .unwrap();
        durable::hincrby(log, &self.hash, "h", "n", 1)
            .await
            .unwrap();
        durable::lpush(
            log,
            &self.list,
            "l",
            vec![format!("e{round}").into_bytes()],
            false,
        )
        .await
        .unwrap();
        durable::sadd(log, &self.set, "s", vec![format!("m{round}").into_bytes()])
            .await
            .unwrap();
        durable::zadd(
            log,
            &self.zset,
            "z",
            format!("z{round}").into_bytes(),
            f64::from(round),
            &ZAddOptions::default(),
        )
        .await
        .unwrap();
        if round > 1 {
            durable::kv_delete(log, &self.kv, &format!("k{}", round - 1))
                .await
                .unwrap();
        }
    }
}

fn recovered_refs(r: &RecoveredStores) -> StoreRefs<'_> {
    StoreRefs {
        kv_store: &r.kv_store,
        hash_store: r.hash_store.as_ref(),
        list_store: r.list_store.as_ref(),
        set_store: r.set_store.as_ref(),
        sorted_set_store: r.sorted_set_store.as_ref(),
        queue_manager: None,
        stream_manager: None,
        hyperloglog_store: None,
        bitmap_store: None,
    }
}

/// A comparable description of every key in every store.
async fn fingerprint(stores: StoreRefs<'_>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in stores.kv_store.dump().await.unwrap() {
        out.insert(format!("kv:{key}"), format!("{value:?}"));
    }
    for (key, fields) in stores.hash_store.unwrap().dump() {
        let fields: BTreeMap<_, _> = fields.into_iter().collect();
        out.insert(format!("hash:{key}"), format!("{fields:?}"));
    }
    for (key, list) in stores.list_store.unwrap().dump() {
        out.insert(format!("list:{key}"), format!("{:?}", list.into_elements()));
    }
    for (key, set) in stores.set_store.unwrap().dump() {
        let mut members = set.into_members();
        members.sort();
        out.insert(format!("set:{key}"), format!("{members:?}"));
    }
    for (key, mut members) in stores.sorted_set_store.unwrap().dump() {
        members.sort_by(|a, b| a.0.cmp(&b.0));
        out.insert(format!("zset:{key}"), format!("{members:?}"));
    }
    out
}

async fn recover(config: &PersistenceConfig) -> Result<RecoveredStores, PersistenceError> {
    recover_all(config, KVConfig::default(), QueueConfig::default()).await
}

async fn recovered_state(config: &PersistenceConfig) -> BTreeMap<String, String> {
    let recovered = recover(config).await.unwrap();
    fingerprint(recovered_refs(&recovered)).await
}

/// Run a server lifetime: rounds 1-2, snapshot S1, rounds 3-4, snapshot S2,
/// rounds 5-6, then shut down. Returns the acknowledged state and S1, S2.
async fn two_snapshots_then_writes(
    config: &PersistenceConfig,
) -> (BTreeMap<String, String>, PathBuf, PathBuf) {
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let stores = Stores::new();
    for round in 1..=2 {
        stores.write_round(&layer, round).await;
    }
    let s1 = layer.snapshot(stores.refs()).await.unwrap();
    for round in 3..=4 {
        stores.write_round(&layer, round).await;
    }
    let s2 = layer.snapshot(stores.refs()).await.unwrap();
    for round in 5..=6 {
        stores.write_round(&layer, round).await;
    }
    layer.shutdown().await.unwrap();
    (fingerprint(stores.refs()).await, s1, s2)
}

fn temp_name(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// A crash at any byte of a snapshot write: the new protocol leaves only a
/// partial temporary file, which recovery ignores, so the state comes from
/// S1 plus the WAL. A file damaged in place (as earlier releases left it, or
/// by the disk) at any byte falls back to S1 because the WAL still holds
/// every record after S1's cutoff. Both equal the acknowledged state.
#[tokio::test]
async fn crash_at_any_byte_of_the_newest_snapshot_loses_nothing() {
    let config = config("crash_any_byte", 10);
    let (expected, _s1, s2) = two_snapshots_then_writes(&config).await;
    assert_eq!(recovered_state(&config).await, expected, "intact files");

    let bytes = std::fs::read(&s2).unwrap();
    let temp = temp_name(&s2);
    std::fs::remove_file(&s2).unwrap();
    for len in 0..=bytes.len() {
        std::fs::write(&temp, &bytes[..len]).unwrap();
        assert_eq!(
            recovered_state(&config).await,
            expected,
            "crash after {len} of {} bytes of the temporary file",
            bytes.len()
        );
    }
    std::fs::remove_file(&temp).unwrap();

    for len in 0..bytes.len() {
        std::fs::write(&s2, &bytes[..len]).unwrap();
        assert_eq!(
            recovered_state(&config).await,
            expected,
            "newest snapshot cut at {len} of {} bytes",
            bytes.len()
        );
    }

    // A flipped byte anywhere fails the checksum (or the decode) the same way.
    for position in (0..bytes.len()).step_by(7) {
        let mut damaged = bytes.clone();
        damaged[position] ^= 0x5A;
        std::fs::write(&s2, &damaged).unwrap();
        assert_eq!(
            recovered_state(&config).await,
            expected,
            "byte {position} flipped"
        );
    }
}

/// With every snapshot damaged, recovery replays the whole WAL, which still
/// starts at offset 1.
#[tokio::test]
async fn every_snapshot_damaged_replays_the_whole_wal() {
    let config = config("all_damaged", 10);
    let (expected, s1, s2) = two_snapshots_then_writes(&config).await;
    for path in [&s1, &s2] {
        let bytes = std::fs::read(path).unwrap();
        std::fs::write(path, &bytes[..bytes.len() / 3]).unwrap();
    }
    assert_eq!(recovered_state(&config).await, expected);
}

/// When the WAL was cut after the damaged snapshot, falling back to the older
/// one would lose the records in between: recovery refuses startup and names
/// the damaged file instead of starting empty.
#[tokio::test]
async fn damaged_newest_with_the_wal_cut_after_it_refuses_startup() {
    let config = config("wal_cut", 10);
    let (expected, _s1, s2) = two_snapshots_then_writes(&config).await;
    let s2_cutoff = SnapshotManager::new(config.snapshot.clone())
        .load(&s2)
        .await
        .unwrap()
        .wal_offset;
    WriteAheadLog::open(config.wal.clone())
        .await
        .unwrap()
        .truncate(s2_cutoff)
        .await
        .unwrap();
    // The intact S2 plus the cut WAL is still complete.
    assert_eq!(recovered_state(&config).await, expected);

    let bytes = std::fs::read(&s2).unwrap();
    std::fs::write(&s2, &bytes[..bytes.len() - 1]).unwrap();
    let err = recover(&config)
        .await
        .err()
        .expect("recovery must refuse to fall back past a cut WAL");
    assert!(
        matches!(err, PersistenceError::SnapshotUnrecoverable(_)),
        "{err}"
    );
    assert!(err.refuses_startup(), "{err}");
    let name = s2.file_name().unwrap().to_string_lossy().into_owned();
    assert!(err.to_string().contains(&name), "{err}");
    assert!(err.to_string().contains("WAL starts at offset"), "{err}");
}

/// A WAL that was removed, or disabled, cannot vouch for anything after the
/// older snapshot either.
#[tokio::test]
async fn damaged_newest_without_a_wal_refuses_startup() {
    let config = config("wal_gone", 10);
    let (_expected, _s1, s2) = two_snapshots_then_writes(&config).await;
    let bytes = std::fs::read(&s2).unwrap();
    std::fs::write(&s2, &bytes[..bytes.len() / 2]).unwrap();

    std::fs::remove_file(&config.wal.path).unwrap();
    let err = recover(&config).await.err().expect("WAL removed");
    assert!(err.refuses_startup(), "{err}");
    assert!(err.to_string().contains("holds no records"), "{err}");

    let mut disabled = config.clone();
    disabled.wal.enabled = false;
    let err = recover(&disabled).await.err().expect("WAL disabled");
    assert!(err.refuses_startup(), "{err}");
    assert!(err.to_string().contains("WAL is disabled"), "{err}");
}

/// Two snapshots taken back to back (well within one second) get two files;
/// both load and the second, with the later cutoff, is the newest.
#[tokio::test]
async fn two_snapshots_in_the_same_second_both_survive() {
    let config = config("same_second", 10);
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let stores = Stores::new();
    stores.write_round(&layer, 1).await;
    let first = layer.snapshot(stores.refs()).await.unwrap();
    stores.write_round(&layer, 2).await;
    let second = layer.snapshot(stores.refs()).await.unwrap();
    let expected = fingerprint(stores.refs()).await;
    layer.shutdown().await.unwrap();

    assert_ne!(first, second);
    let mgr = SnapshotManager::new(config.snapshot.clone());
    assert_eq!(
        mgr.list_snapshots().await.unwrap(),
        vec![first.clone(), second.clone()]
    );
    let first_cutoff = mgr.load(&first).await.unwrap().wal_offset;
    let second_cutoff = mgr.load(&second).await.unwrap().wal_offset;
    assert!(first_cutoff < second_cutoff);
    assert_eq!(mgr.load_latest().await.unwrap().unwrap().1, second);
    assert_eq!(recovered_state(&config).await, expected);
}

/// Retention keeps the snapshot it just wrote even when configured to keep
/// none, and removes older files, damaged ones included.
#[tokio::test]
async fn retention_never_deletes_the_only_valid_snapshot() {
    let config = config("retention", 0);
    std::fs::create_dir_all(&config.snapshot.directory).unwrap();
    let legacy = config.snapshot.directory.join("snapshot-v4-1700000000.bin");
    std::fs::write(&legacy, b"SYNAP004 damaged by an earlier crash").unwrap();

    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let stores = Stores::new();
    let mut last = PathBuf::new();
    for round in 1..=3 {
        stores.write_round(&layer, round).await;
        last = layer.snapshot(stores.refs()).await.unwrap();
    }
    let expected = fingerprint(stores.refs()).await;
    layer.shutdown().await.unwrap();

    let mgr = SnapshotManager::new(config.snapshot.clone());
    assert_eq!(mgr.list_snapshots().await.unwrap(), vec![last.clone()]);
    assert!(!legacy.exists());
    mgr.load(&last).await.unwrap();
    assert_eq!(recovered_state(&config).await, expected);
}

/// Byte length of the WAL file header plus its first `frames` frames.
fn wal_prefix_len(path: &Path, frames: usize) -> u64 {
    let mut len = wal_frame::FILE_HEADER_LEN;
    let mut seen = 0;
    wal_frame::scan(path, |entry| {
        if seen < frames {
            len += wal_frame::encode_frame(&entry).unwrap().len() as u64;
        }
        seen += 1;
    })
    .unwrap();
    len
}

/// A WAL that lost records the snapshot holds (power loss right after a
/// snapshot taken by an earlier release, which did not fsync the WAL first):
/// writes acknowledged after the restart continue at the snapshot's cutoff,
/// so the next recovery from that snapshot replays them.
#[tokio::test]
async fn wal_behind_a_snapshot_does_not_lose_later_writes() {
    let config = config("wal_behind", 10);
    let stores = Stores::new();
    {
        let layer = PersistenceLayer::new(config.clone()).await.unwrap();
        stores.write_round(&layer, 1).await;
        layer.snapshot(stores.refs()).await.unwrap();
        layer.shutdown().await.unwrap();
    }
    // Drop the last two records below the cutoff, as an unsynced WAL tail.
    let frames = {
        let mut count = 0;
        wal_frame::scan(&config.wal.path, |_| count += 1).unwrap();
        count
    };
    let keep = wal_prefix_len(&config.wal.path, frames - 2);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&config.wal.path)
        .unwrap();
    file.set_len(keep).unwrap();
    drop(file);

    // Restart: the snapshot holds everything; new writes continue at its cutoff.
    let recovered = recover(&config).await.unwrap();
    assert_eq!(
        fingerprint(recovered_refs(&recovered)).await,
        fingerprint(stores.refs()).await
    );
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    layer.continue_wal_at(recovered.wal_offset);
    let restarted = Stores {
        kv: recovered.kv_store,
        hash: recovered.hash_store.unwrap(),
        list: recovered.list_store.unwrap(),
        set: recovered.set_store.unwrap(),
        zset: recovered.sorted_set_store.unwrap(),
    };
    restarted.write_round(&layer, 2).await;
    let expected = fingerprint(restarted.refs()).await;
    layer.shutdown().await.unwrap();

    assert_eq!(recovered_state(&config).await, expected);
}
