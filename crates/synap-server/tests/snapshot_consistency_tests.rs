//! phase23: snapshots paired with their WAL cutoff, same-key WAL order and
//! absolute expiry across restarts.
//!
//! Every test writes through `persistence::durable` (the path every transport
//! uses), restarts with `recover_all` and compares with what the server held
//! before the restart. Expiry tests control the clock by placing the recorded
//! times in the past (deadlines and WAL entry timestamps "written N seconds
//! ago") instead of sleeping, except where a key must expire between the
//! snapshot and the restart.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use synap_server::core::{
    BitmapStore, Expiry, HashStore, HyperLogLogStore, KVConfig, KVStore, ListStore, QueueConfig,
    SetOptions, SetStore, SortedSetStore, TransactionCommand, TransactionManager, ZAddOptions,
};
use synap_server::persistence::types::FsyncMode;
use synap_server::persistence::{
    Operation, PersistenceConfig, PersistenceLayer, RecoveredStores, SnapshotManager, StoreRefs,
    durable, recover_all, wal_frame,
};
use tokio::sync::oneshot;

fn config(name: &str, snapshots: bool) -> PersistenceConfig {
    let dir = format!("./target/phase23_{name}");
    let _ = std::fs::remove_dir_all(&dir);
    let mut config = PersistenceConfig::default();
    config.enabled = true;
    config.wal.enabled = true;
    config.wal.fsync_mode = FsyncMode::Periodic;
    config.wal.path = format!("{dir}/wal/synap.wal").into();
    config.snapshot.enabled = snapshots;
    config.snapshot.directory = format!("{dir}/snapshots").into();
    config.snapshot.interval_secs = 3600;
    config.snapshot.operation_threshold = usize::MAX;
    config
}

/// Every WAL-backed store, as the server holds them.
struct Stores {
    kv: Arc<KVStore>,
    hash: Arc<HashStore>,
    list: Arc<ListStore>,
    set: Arc<SetStore>,
    zset: Arc<SortedSetStore>,
    hll: Arc<HyperLogLogStore>,
    bitmap: Arc<BitmapStore>,
}

impl Stores {
    fn new() -> Self {
        Self {
            kv: Arc::new(KVStore::new(KVConfig::default())),
            hash: Arc::new(HashStore::new()),
            list: Arc::new(ListStore::new()),
            set: Arc::new(SetStore::new()),
            zset: Arc::new(SortedSetStore::new()),
            hll: Arc::new(HyperLogLogStore::new()),
            bitmap: Arc::new(BitmapStore::new()),
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
            hyperloglog_store: Some(&self.hll),
            bitmap_store: Some(&self.bitmap),
        }
    }

    fn transactions(&self) -> TransactionManager {
        TransactionManager::new(
            Arc::clone(&self.kv),
            Arc::clone(&self.hash),
            Arc::clone(&self.list),
            Arc::clone(&self.set),
            Arc::clone(&self.zset),
        )
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
        hyperloglog_store: Some(&r.hyperloglog_store),
        bitmap_store: Some(&r.bitmap_store),
    }
}

async fn restart(config: &PersistenceConfig) -> RecoveredStores {
    recover_all(config, KVConfig::default(), QueueConfig::default())
        .await
        .unwrap()
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
    for (key, hll) in stores.hyperloglog_store.unwrap().dump() {
        out.insert(format!("hll:{key}"), format!("{:?}", hll.registers));
    }
    for (key, bitmap) in stores.bitmap_store.unwrap().dump() {
        out.insert(format!("bitmap:{key}"), format!("{:?}", bitmap.data));
    }
    out
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// ── 1.2: the snapshot barrier ───────────────────────────────────────────────

/// A snapshot paused right after recording its cutoff: LPUSH and HINCRBY
/// issued meanwhile wait for the barrier, land after the cutoff and outside
/// the image, and recovery applies each exactly once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_paused_at_its_cutoff_applies_lpush_and_hincrby_once() {
    let config = config("paused_cutoff", true);
    let layer = Arc::new(PersistenceLayer::new(config.clone()).await.unwrap());
    let stores = Arc::new(Stores::new());
    durable::lpush(Some(&layer), &stores.list, "l", vec![b"a".to_vec()], false)
        .await
        .unwrap();
    durable::hset(Some(&layer), &stores.hash, "h", "f", b"0".to_vec())
        .await
        .unwrap();

    let (paused_tx, paused_rx) = oneshot::channel::<()>();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let pause = async move {
        paused_tx.send(()).unwrap();
        release_rx.await.unwrap();
    };
    let driver = async {
        paused_rx.await.unwrap();
        let writer = {
            let layer = Arc::clone(&layer);
            let stores = Arc::clone(&stores);
            tokio::spawn(async move {
                durable::lpush(Some(&layer), &stores.list, "l", vec![b"x".to_vec()], false)
                    .await
                    .unwrap();
                durable::hincrby(Some(&layer), &stores.hash, "h", "f", 1)
                    .await
                    .unwrap();
            })
        };
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(
            !writer.is_finished(),
            "a write completed while the snapshot held its barrier"
        );
        release_tx.send(()).unwrap();
        // The snapshot is polled alongside, so the writes complete once it
        // releases its barrier.
        writer.await.unwrap();
    };
    let (snapshot, ()) = tokio::join!(layer.snapshot_with_pause(stores.refs(), pause), driver);
    snapshot.unwrap();
    layer.shutdown().await.unwrap();

    // The image holds neither paused write; the cutoff is right after the
    // two writes made before the snapshot (a fresh WAL numbers entries from 1).
    let (image, _) = SnapshotManager::new(config.snapshot.clone())
        .load_latest()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(image.wal_offset, 3);
    assert_eq!(
        image.list_data["l"].clone().into_elements(),
        vec![b"a".to_vec()]
    );
    assert_eq!(image.hash_data["h"]["f"], b"0".to_vec());

    let r = restart(&config).await;
    assert_eq!(
        r.list_store.as_ref().unwrap().lrange("l", 0, -1).unwrap(),
        vec![b"x".to_vec(), b"a".to_vec()],
        "LPUSH applied twice or lost"
    );
    assert_eq!(
        r.hash_store.as_ref().unwrap().hget("h", "f").unwrap(),
        Some(b"1".to_vec()),
        "HINCRBY applied twice or lost"
    );
}

/// Writers on many keys across shards (KV, lists, hashes, sets, sorted sets,
/// multi-key MSET and MULTI/EXEC) race a loop of snapshots. The restart from
/// the latest snapshot plus the WAL after its cutoff reproduces the live
/// state exactly: nothing duplicated, nothing lost.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn writes_racing_snapshots_recover_to_the_live_state() {
    let config = config("racing_snapshots", true);
    let layer = Arc::new(PersistenceLayer::new(config.clone()).await.unwrap());
    let stores = Arc::new(Stores::new());
    let transactions = Arc::new(stores.transactions());
    let done = Arc::new(AtomicBool::new(false));
    let taken = Arc::new(AtomicUsize::new(0));

    let snapshotter = {
        let layer = Arc::clone(&layer);
        let stores = Arc::clone(&stores);
        let done = Arc::clone(&done);
        let taken = Arc::clone(&taken);
        tokio::spawn(async move {
            while !done.load(SeqCst) {
                layer.snapshot(stores.refs()).await.unwrap();
                taken.fetch_add(1, SeqCst);
                tokio::time::sleep(Duration::from_millis(3)).await;
            }
        })
    };

    let writers: Vec<_> = (0..8u8)
        .map(|t| {
            let layer = Arc::clone(&layer);
            let stores = Arc::clone(&stores);
            let transactions = Arc::clone(&transactions);
            let taken = Arc::clone(&taken);
            tokio::spawn(async move {
                let log = Some(layer.as_ref());
                // Keep writing until several snapshots have raced the writes.
                let mut i = 0u32;
                while i < 250 || taken.load(SeqCst) < 3 {
                    i += 1;
                    let k = (i as usize * 7 + t as usize) % 32;
                    let member = format!("m{}", i % 5).into_bytes();
                    durable::kv_incr(log, &stores.kv, &format!("c{k}"), 1)
                        .await
                        .unwrap();
                    durable::kv_append(log, &stores.kv, &format!("a{k}"), vec![b'a' + t])
                        .await
                        .unwrap();
                    durable::lpush(log, &stores.list, &format!("l{k}"), vec![vec![t]], false)
                        .await
                        .unwrap();
                    durable::hincrby(log, &stores.hash, &format!("h{k}"), "f", 1)
                        .await
                        .unwrap();
                    durable::sadd(log, &stores.set, &format!("s{k}"), vec![member.clone()])
                        .await
                        .unwrap();
                    durable::zincrby(log, &stores.zset, &format!("z{k}"), member, 1.0)
                        .await
                        .unwrap();
                    durable::kv_mset(
                        log,
                        &stores.kv,
                        vec![
                            (format!("m{k}"), i.to_string().into_bytes()),
                            (format!("m{}", (k + 1) % 32), i.to_string().into_bytes()),
                        ],
                    )
                    .await
                    .unwrap();
                    if i.is_multiple_of(10) {
                        let client = format!("tx{t}");
                        transactions.multi(client.clone()).unwrap();
                        for command in [
                            TransactionCommand::KVIncr {
                                key: format!("c{k}"),
                                delta: 10,
                            },
                            TransactionCommand::ListLPush {
                                key: format!("l{k}"),
                                values: vec![b"tx".to_vec()],
                            },
                            TransactionCommand::HashIncrBy {
                                key: format!("h{k}"),
                                field: "tx".into(),
                                delta: 1,
                            },
                        ] {
                            transactions
                                .queue_command_if_transaction(&client, command)
                                .unwrap();
                        }
                        let outcome = durable::exec(log, &transactions, &client).await.unwrap();
                        assert!(outcome.is_some());
                    }
                    tokio::task::yield_now().await;
                }
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap();
    }
    done.store(true, SeqCst);
    snapshotter.await.unwrap();
    assert!(taken.load(SeqCst) >= 3);

    let live = fingerprint(stores.refs()).await;
    layer.shutdown().await.unwrap();

    let (image, _) = SnapshotManager::new(config.snapshot.clone())
        .load_latest()
        .await
        .unwrap()
        .unwrap();
    assert!(image.wal_offset > 0, "recovery would not use the snapshot");

    let r = restart(&config).await;
    let recovered = fingerprint(recovered_refs(&r)).await;
    assert_eq!(live.len(), recovered.len());
    for (key, value) in &live {
        assert_eq!(
            Some(value),
            recovered.get(key),
            "{key} differs after restart"
        );
    }
}

// ── 1.3: same-key WAL order ─────────────────────────────────────────────────

/// 8 clients append distinct single bytes to one key 1000 times each. APPEND
/// is recorded as the resulting value, so the WAL must hold those records in
/// memory order for the replay to end on the value the key held.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_append_on_one_key_recovers_the_same_value() {
    let config = config("concurrent_append", false);
    let layer = Arc::new(PersistenceLayer::new(config.clone()).await.unwrap());
    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let writers: Vec<_> = (0..8u8)
        .map(|t| {
            let layer = Arc::clone(&layer);
            let kv = Arc::clone(&kv);
            tokio::spawn(async move {
                for _ in 0..1000 {
                    durable::kv_append(Some(&layer), &kv, "k", vec![b'a' + t])
                        .await
                        .unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap();
    }
    let before = kv.get("k").await.unwrap().unwrap();
    assert_eq!(before.len(), 8000);
    layer.shutdown().await.unwrap();

    // Every record extends the previous one by exactly one byte: the WAL
    // holds the 8000 resulting values in memory order, not just a lucky last.
    let mut lengths = Vec::new();
    wal_frame::scan(&config.wal.path, |entry| {
        if let Operation::KVSet { key, value, .. } = entry.operation
            && key == "k"
        {
            lengths.push(value.len());
        }
    })
    .unwrap();
    assert_eq!(lengths.len(), 8000);
    if let Some(i) = (0..lengths.len()).find(|&i| lengths[i] != i + 1) {
        panic!(
            "WAL record {i} holds {} bytes, expected {}: records out of memory order",
            lengths[i],
            i + 1
        );
    }

    let r = restart(&config).await;
    assert_eq!(r.kv_store.get("k").await.unwrap(), Some(before));
}

// ── 1.4–1.5: absolute expiry across restarts ────────────────────────────────

/// Snapshot-only restart: a key written 30 s ago with 100 s to live comes
/// back with at most 70 s left, a persistent key stays persistent, and a key
/// whose deadline passes before the restart is gone. HyperLogLog and bitmap
/// data written before the snapshot survive it (v4 sections).
#[tokio::test]
async fn snapshot_restart_keeps_absolute_expiry() {
    let config = config("snapshot_expiry", true);
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let stores = Stores::new();
    let written_30s_ago = now_ms() - 30_000;
    stores
        .kv
        .set_with_opts(
            "k",
            b"v".to_vec(),
            Some(Expiry::UnixMilliseconds(written_30s_ago + 100_000)),
            SetOptions::default(),
        )
        .await
        .unwrap();
    stores.kv.set("p", b"v".to_vec(), None).await.unwrap();
    stores
        .kv
        .set_with_opts(
            "short",
            b"v".to_vec(),
            Some(Expiry::Milliseconds(700)),
            SetOptions::default(),
        )
        .await
        .unwrap();
    stores
        .hll
        .pfadd("visitors", vec![b"u1".to_vec(), b"u2".to_vec()], Some(100))
        .unwrap();
    stores.bitmap.setbit("bits", 9, 1).unwrap();

    layer.snapshot(stores.refs()).await.unwrap();
    layer.shutdown().await.unwrap();
    tokio::time::sleep(Duration::from_millis(900)).await;

    let r = restart(&config).await;
    let ttl = r.kv_store.ttl("k").await.unwrap().unwrap();
    assert!((65..=70).contains(&ttl), "k has {ttl}s to live");
    assert_eq!(r.kv_store.ttl("p").await.unwrap(), None);
    assert!(
        !r.kv_store.exists("short").await.unwrap(),
        "a key that expired before the restart came back"
    );
    assert_eq!(r.hyperloglog_store.pfcount("visitors").unwrap(), 2);
    let hll = &r.hyperloglog_store.dump()["visitors"];
    let deadline_ms = hll.expires_at_ms().unwrap();
    assert!(deadline_ms > now_ms() + 95_000 && deadline_ms <= now_ms() + 101_000);
    assert_eq!(r.bitmap_store.getbit("bits", 9).unwrap(), 1);
}

/// WAL-only restart: SET EX, EXPIRE and PXAT are recorded as absolute
/// deadlines, so a key written 30 s ago with 100 s to live has at most 70 s
/// left, and a key whose deadline is already past is not resurrected.
#[tokio::test]
async fn wal_only_restart_keeps_absolute_expiry() {
    let config = config("wal_expiry", false);
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let kv = KVStore::new(KVConfig::default());
    let hll = HyperLogLogStore::new();
    let log = Some(&layer);
    let now = now_ms();

    durable::kv_set_with_opts(
        log,
        &kv,
        "k",
        b"v".to_vec(),
        Some(Expiry::UnixMilliseconds(now - 30_000 + 100_000)),
        SetOptions::default(),
    )
    .await
    .unwrap();
    durable::kv_set(log, &kv, "gone", b"v".to_vec(), None)
        .await
        .unwrap();
    // Written 60 s ago with 10 s to live: already expired.
    durable::kv_set_with_opts(
        log,
        &kv,
        "gone",
        b"v2".to_vec(),
        Some(Expiry::UnixMilliseconds(now - 60_000 + 10_000)),
        SetOptions::default(),
    )
    .await
    .unwrap();
    durable::kv_set(log, &kv, "e", b"v".to_vec(), None)
        .await
        .unwrap();
    assert!(durable::kv_expire(log, &kv, "e", 50).await.unwrap());
    durable::kv_set(log, &kv, "ex", b"v".to_vec(), Some(100))
        .await
        .unwrap();
    durable::kv_append(log, &kv, "ex", b"w".to_vec())
        .await
        .unwrap();
    durable::pfadd(log, &hll, "visitors", vec![b"u1".to_vec()], Some(40))
        .await
        .unwrap();
    layer.shutdown().await.unwrap();

    let r = restart(&config).await;
    let ttl = r.kv_store.ttl("k").await.unwrap().unwrap();
    assert!((65..=70).contains(&ttl), "k has {ttl}s to live");
    assert!(!r.kv_store.exists("gone").await.unwrap());
    let ttl = r.kv_store.ttl("e").await.unwrap().unwrap();
    assert!((45..=50).contains(&ttl), "e has {ttl}s to live");
    assert_eq!(r.kv_store.get("ex").await.unwrap(), Some(b"vw".to_vec()));
    let ttl = r.kv_store.ttl("ex").await.unwrap().unwrap();
    assert!((95..=100).contains(&ttl), "ex has {ttl}s to live");
    let deadline_ms = r.hyperloglog_store.dump()["visitors"]
        .expires_at_ms()
        .unwrap();
    assert!(deadline_ms > now_ms() + 35_000 && deadline_ms <= now_ms() + 41_000);
}

/// Snapshot + WAL restart: deadlines from the image are kept, and expiry
/// changes recorded after the snapshot's cutoff (EXPIRE shortening a key, a
/// PXAT in the past, PFADD with a TTL) are replayed as absolute deadlines.
#[tokio::test]
async fn snapshot_and_wal_restart_keeps_absolute_expiry() {
    let config = config("snapshot_wal_expiry", true);
    let layer = PersistenceLayer::new(config.clone()).await.unwrap();
    let stores = Stores::new();
    let log = Some(&layer);
    let now = now_ms();

    durable::kv_set_with_opts(
        log,
        &stores.kv,
        "k",
        b"v".to_vec(),
        Some(Expiry::UnixMilliseconds(now + 70_000)),
        SetOptions::default(),
    )
    .await
    .unwrap();
    durable::kv_set(log, &stores.kv, "j", b"v".to_vec(), None)
        .await
        .unwrap();
    durable::kv_set(log, &stores.kv, "keep", b"v".to_vec(), Some(100))
        .await
        .unwrap();
    layer.snapshot(stores.refs()).await.unwrap();

    assert!(durable::kv_expire(log, &stores.kv, "k", 20).await.unwrap());
    durable::kv_set_with_opts(
        log,
        &stores.kv,
        "j",
        b"v2".to_vec(),
        Some(Expiry::UnixMilliseconds(now - 50_000)),
        SetOptions::default(),
    )
    .await
    .unwrap();
    durable::pfadd(log, &stores.hll, "h", vec![b"x".to_vec()], Some(30))
        .await
        .unwrap();
    layer.shutdown().await.unwrap();

    let r = restart(&config).await;
    let ttl = r.kv_store.ttl("k").await.unwrap().unwrap();
    assert!((15..=20).contains(&ttl), "k has {ttl}s to live");
    assert!(!r.kv_store.exists("j").await.unwrap());
    let ttl = r.kv_store.ttl("keep").await.unwrap().unwrap();
    assert!((95..=100).contains(&ttl), "keep has {ttl}s to live");
    assert_eq!(r.hyperloglog_store.pfcount("h").unwrap(), 1);
}

/// One frame of a 1.3.2 WAL written by hand (an encoder independent of the
/// server's serde types): `KVSet { key, value, ttl }` at `timestamp`.
fn legacy_kv_set_frame(offset: u64, timestamp: u64, key: &str, ttl: Option<u64>) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&offset.to_le_bytes());
    payload.extend_from_slice(&timestamp.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes()); // Operation::KVSet
    payload.extend_from_slice(&(key.len() as u64).to_le_bytes());
    payload.extend_from_slice(key.as_bytes());
    payload.extend_from_slice(&1u64.to_le_bytes());
    payload.push(b'v');
    match ttl {
        Some(ttl) => {
            payload.push(1);
            payload.extend_from_slice(&ttl.to_le_bytes());
        }
        None => payload.push(0),
    }
    let mut frame = Vec::new();
    frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    frame.extend_from_slice(&crc32fast::hash(&payload).to_be_bytes());
    frame.extend_from_slice(&payload);
    frame
}

/// A 1.3.2 WAL record setting `k` with a 100 s TTL at timestamp T, recovered
/// at T + 40 s, leaves `k` at most 60 s to live; one with 10 s to live
/// written 60 s ago is not recovered; a persistent one stays persistent.
#[tokio::test]
async fn legacy_wal_records_expire_from_their_entry_timestamp() {
    let config = config("legacy_expiry", false);
    let now_secs = now_ms() / 1000;
    std::fs::create_dir_all(config.wal.path.parent().unwrap()).unwrap();
    let mut wal = Vec::new();
    wal.extend(legacy_kv_set_frame(0, now_secs - 40, "k", Some(100)));
    wal.extend(legacy_kv_set_frame(1, now_secs - 60, "gone", Some(10)));
    wal.extend(legacy_kv_set_frame(2, now_secs - 60, "p", None));
    std::fs::write(&config.wal.path, wal).unwrap();

    let r = restart(&config).await;
    let ttl = r.kv_store.ttl("k").await.unwrap().unwrap();
    assert!((55..=60).contains(&ttl), "k has {ttl}s to live");
    assert!(!r.kv_store.exists("gone").await.unwrap());
    assert_eq!(r.kv_store.get("p").await.unwrap(), Some(b"v".to_vec()));
    assert_eq!(r.kv_store.ttl("p").await.unwrap(), None);
}

/// Blocking pops through the layer are ordered with the pushes they race,
/// so the restart rebuilds the same lists and sorted sets.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blocking_pops_replay_in_order() {
    let config = config("blocking_pops", false);
    let layer = Arc::new(PersistenceLayer::new(config.clone()).await.unwrap());
    let stores = Arc::new(Stores::new());

    let waiter = {
        let layer = Arc::clone(&layer);
        let stores = Arc::clone(&stores);
        tokio::spawn(async move {
            durable::blpop(Some(&layer), &stores.list, vec!["q".into()], Some(5))
                .await
                .unwrap()
        })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    let log = Some(layer.as_ref());
    durable::rpush(log, &stores.list, "q", vec![b"1".to_vec()], false)
        .await
        .unwrap();
    assert_eq!(waiter.await.unwrap(), ("q".to_string(), b"1".to_vec()));
    for value in [b"2", b"3", b"4"] {
        durable::lpush(log, &stores.list, "q", vec![value.to_vec()], false)
            .await
            .unwrap();
    }
    durable::brpop(log, &stores.list, vec!["q".into()], Some(1))
        .await
        .unwrap();
    durable::brpoplpush(log, &stores.list, "q", "dst", Some(1))
        .await
        .unwrap();
    for (member, score) in [(b"a", 1.0), (b"b", 2.0), (b"c", 3.0)] {
        durable::zadd(
            log,
            &stores.zset,
            "z",
            member.to_vec(),
            score,
            &ZAddOptions::default(),
        )
        .await
        .unwrap();
    }
    durable::bzpopmin(log, &stores.zset, vec!["z".into()], Some(1))
        .await
        .unwrap();
    durable::bzpopmax(log, &stores.zset, vec!["z".into()], Some(1))
        .await
        .unwrap();
    let live = fingerprint(stores.refs()).await;
    layer.shutdown().await.unwrap();

    let r = restart(&config).await;
    assert_eq!(fingerprint(recovered_refs(&r)).await, live);
    assert_eq!(
        r.list_store.as_ref().unwrap().lrange("q", 0, -1).unwrap(),
        vec![b"4".to_vec()]
    );
    assert_eq!(
        r.sorted_set_store.as_ref().unwrap().zscore("z", b"b"),
        Some(2.0)
    );
}
