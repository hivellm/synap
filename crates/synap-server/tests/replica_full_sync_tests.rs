//! phase31: a replica's full sync carries every replicated datatype, is
//! paired exactly with its replication offset, and replaces the replica's
//! state.
//!
//! The master writes through `persistence::durable` with a replication-only
//! persistence layer (no WAL), the path every transport uses, so its writes
//! take the same write-order stripes the full sync uses as its barrier.
//! Replicas connect over TCP, some through a proxy the test can cut or
//! point at another master. Convergence is polled with a deadline.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use synap_server::core::{
    BitmapStore, Expiry, GeospatialStore, HashStore, HyperLogLogStore, KVConfig, KVStore,
    ListStore, QueueConfig, QueueManager, SetOptions, SetStore, SortedSetStore, StreamConfig,
    StreamManager, ZAddOptions,
};
use synap_server::persistence::{PersistenceConfig, PersistenceLayer, StoreArcs, durable};
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::time::sleep;

/// Every datatype store a node holds.
struct Node {
    kv: Arc<KVStore>,
    hash: Arc<HashStore>,
    list: Arc<ListStore>,
    set: Arc<SetStore>,
    zset: Arc<SortedSetStore>,
    hll: Arc<HyperLogLogStore>,
    bitmap: Arc<BitmapStore>,
    queues: Arc<QueueManager>,
    streams: Arc<StreamManager>,
}

impl Node {
    fn new() -> Self {
        Self {
            kv: Arc::new(KVStore::new(KVConfig::default())),
            hash: Arc::new(HashStore::new()),
            list: Arc::new(ListStore::new()),
            set: Arc::new(SetStore::new()),
            zset: Arc::new(SortedSetStore::new()),
            hll: Arc::new(HyperLogLogStore::new()),
            bitmap: Arc::new(BitmapStore::new()),
            queues: Arc::new(QueueManager::new(QueueConfig::default())),
            streams: Arc::new(StreamManager::new(StreamConfig::default())),
        }
    }

    fn arcs(&self) -> StoreArcs {
        StoreArcs {
            kv_store: Arc::clone(&self.kv),
            hash_store: Some(Arc::clone(&self.hash)),
            list_store: Some(Arc::clone(&self.list)),
            set_store: Some(Arc::clone(&self.set)),
            sorted_set_store: Some(Arc::clone(&self.zset)),
            queue_manager: Some(Arc::clone(&self.queues)),
            stream_manager: Some(Arc::clone(&self.streams)),
            hyperloglog_store: Some(Arc::clone(&self.hll)),
            bitmap_store: Some(Arc::clone(&self.bitmap)),
        }
    }

    /// A comparable description of every value. Queue message ids and stream
    /// event ids are assigned anew on a replica, so payloads are compared.
    async fn fingerprint(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (key, value, expiry) in self.kv.dump_with_expiry().await.unwrap() {
            out.push(format!("kv:{key}={:?}@{expiry:?}", value.to_vec()));
        }
        for (key, fields) in self.hash.dump() {
            let fields: std::collections::BTreeMap<_, _> = fields.into_iter().collect();
            out.push(format!("hash:{key}={fields:?}"));
        }
        for (key, list) in self.list.dump() {
            out.push(format!("list:{key}={:?}", list.into_elements()));
        }
        for (key, set) in self.set.dump() {
            let mut members = set.into_members();
            members.sort();
            out.push(format!("set:{key}={members:?}"));
        }
        for (key, mut members) in self.zset.dump() {
            members.sort_by(|a, b| a.0.cmp(&b.0));
            out.push(format!("zset:{key}={members:?}"));
        }
        for (key, hll) in self.hll.dump() {
            out.push(format!("hll:{key}={:?}", hll.registers));
        }
        for (key, bitmap) in self.bitmap.dump() {
            out.push(format!("bitmap:{key}={:?}", bitmap.data));
        }
        for (queue, messages) in self.queues.dump().await.unwrap() {
            let mut payloads: Vec<_> = messages
                .iter()
                .map(|m| (m.payload.to_vec(), m.priority, m.max_retries))
                .collect();
            payloads.sort();
            out.push(format!("queue:{queue}={payloads:?}"));
        }
        for (room, events) in self.streams.get_all_events().await {
            let events: Vec<_> = events.iter().map(|e| (&e.event, &e.data)).collect();
            out.push(format!("room:{room}={events:?}"));
        }
        out.sort();
        out
    }
}

/// A master over `node`, plus the replication-only persistence layer its
/// writes go through.
async fn start_master(node: &Node) -> (Arc<MasterNode>, PersistenceLayer, SocketAddr) {
    let addr = free_addr();
    let config = ReplicationConfig {
        enabled: true,
        role: NodeRole::Master,
        replica_listen_address: Some(addr),
        heartbeat_interval_ms: 100,
        ..Default::default()
    };
    let master = Arc::new(MasterNode::with_stores(config, node.arcs()).await.unwrap());
    let persistence = PersistenceConfig {
        enabled: false,
        ..Default::default()
    };
    let layer = PersistenceLayer::new_with_replication(persistence, Some(Arc::clone(&master)))
        .await
        .unwrap();
    sleep(Duration::from_millis(100)).await;
    (master, layer, addr)
}

/// A replica of the master at `master_addr`, reconnecting every 100 ms.
async fn start_replica(node: &Node, master_addr: SocketAddr) -> Arc<ReplicaNode> {
    let config = ReplicationConfig {
        enabled: true,
        role: NodeRole::Replica,
        master_address: Some(master_addr.into()),
        auto_reconnect: true,
        reconnect_delay_ms: 100,
        ..Default::default()
    };
    ReplicaNode::new(config, node.arcs()).await.unwrap()
}

fn free_addr() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

fn deadline_in(ms: u64) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + ms
}

/// Wait until the replica holds exactly the master's data.
async fn converge(master: &Node, replica: &Node, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let expected = master.fingerprint().await;
        let actual = replica.fingerprint().await;
        if expected == actual {
            return;
        }
        if Instant::now() >= deadline {
            panic!("{what}: replica did not converge\nmaster:  {expected:?}\nreplica: {actual:?}");
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// One value of every replicated datatype, written through `durable`.
async fn write_every_datatype(node: &Node, layer: &PersistenceLayer, deadline_ms: u64) {
    let log = Some(layer);
    durable::kv_set_with_opts(
        log,
        &node.kv,
        "kv:expiring",
        b"v".to_vec(),
        Some(Expiry::UnixMilliseconds(deadline_ms)),
        SetOptions::default(),
    )
    .await
    .unwrap();
    durable::kv_set(log, &node.kv, "kv:plain", b"p".to_vec(), None)
        .await
        .unwrap();
    durable::hset(log, &node.hash, "hash", "f", b"hv".to_vec())
        .await
        .unwrap();
    durable::rpush(
        log,
        &node.list,
        "list",
        vec![b"a".to_vec(), b"b".to_vec()],
        false,
    )
    .await
    .unwrap();
    durable::sadd(log, &node.set, "set", vec![b"m1".to_vec(), b"m2".to_vec()])
        .await
        .unwrap();
    durable::zadd(
        log,
        &node.zset,
        "zset",
        b"z".to_vec(),
        2.5,
        &ZAddOptions::default(),
    )
    .await
    .unwrap();
    let geo = GeospatialStore::new(Arc::clone(&node.zset));
    durable::geoadd(
        log,
        &geo,
        "geo",
        vec![(48.8566, 2.3522, b"paris".to_vec())],
        false,
        false,
        false,
    )
    .await
    .unwrap();
    durable::pfadd(
        log,
        &node.hll,
        "hll",
        vec![b"x".to_vec(), b"y".to_vec()],
        Some(3600),
    )
    .await
    .unwrap();
    durable::setbit(log, &node.bitmap, "bitmap", 7, 1)
        .await
        .unwrap();
    node.queues.create_queue("queue", None).await.unwrap();
    durable::queue_publish(
        log,
        &node.queues,
        "queue",
        b"msg".to_vec(),
        Some(3),
        Some(2),
    )
    .await
    .unwrap();
    node.streams.create_room("room").await.unwrap();
    node.streams
        .publish("room", "evt", b"data".to_vec())
        .await
        .unwrap();
}

/// A replica that connects after the master already holds every datatype
/// receives all of them, KV with the master's absolute deadline.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn late_replica_receives_every_datatype() {
    let master = Node::new();
    let (_node, layer, addr) = start_master(&master).await;
    let deadline = deadline_in(3_600_000);
    write_every_datatype(&master, &layer, deadline).await;
    assert_eq!(master.fingerprint().await.len(), 11);

    let replica = Node::new();
    let _replica = start_replica(&replica, addr).await;
    converge(&master, &replica, "late replica").await;

    let expiry = replica
        .kv
        .dump_with_expiry()
        .await
        .unwrap()
        .into_iter()
        .find(|(k, _, _)| k == "kv:expiring")
        .map(|(_, _, e)| e);
    assert_eq!(expiry, Some(Some(deadline)));
    assert!(replica.zset.zscore("geo", b"paris").is_some());
}

/// Writes issued while the full sync holds the barrier wait for it and are
/// streamed after the image: each takes effect on the replica exactly once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn writes_during_full_sync_apply_once() {
    let master = Node::new();
    let (master_node, layer, addr) = start_master(&master).await;
    let layer = Arc::new(layer);
    durable::lpush(Some(&layer), &master.list, "l", vec![b"a".to_vec()], false)
        .await
        .unwrap();
    durable::hincrby(Some(&layer), &master.hash, "h", "f", 1)
        .await
        .unwrap();
    durable::kv_incr(Some(&layer), &master.kv, "c", 1)
        .await
        .unwrap();

    let (reached, release) = master_node.pause_next_full_sync();
    let replica = Node::new();
    let replica_node = start_replica(&replica, addr).await;
    let offset = tokio::time::timeout(Duration::from_secs(10), reached)
        .await
        .expect("full sync never reached the barrier")
        .unwrap();
    assert_eq!(offset, 3, "the image is taken after the three writes");

    let writers = {
        let (layer, list, hash, kv) = (
            Arc::clone(&layer),
            Arc::clone(&master.list),
            Arc::clone(&master.hash),
            Arc::clone(&master.kv),
        );
        tokio::spawn(async move {
            durable::lpush(Some(&layer), &list, "l", vec![b"x".to_vec()], false)
                .await
                .unwrap();
            durable::hincrby(Some(&layer), &hash, "h", "f", 1)
                .await
                .unwrap();
            durable::kv_incr(Some(&layer), &kv, "c", 1).await.unwrap();
        })
    };
    sleep(Duration::from_millis(200)).await;
    assert!(
        !writers.is_finished(),
        "a write completed while the full sync held the barrier"
    );
    assert_eq!(master_node.replication_offset(), 3);
    release.send(()).unwrap();
    writers.await.unwrap();

    converge(&master, &replica, "writes during full sync").await;
    assert_eq!(
        replica.list.lrange("l", 0, -1).unwrap(),
        vec![b"x".to_vec(), b"a".to_vec()]
    );
    assert_eq!(replica.hash.hget("h", "f").unwrap(), Some(b"2".to_vec()));
    assert_eq!(replica.kv.get("c").await.unwrap(), Some(b"2".to_vec()));
    assert_eq!(replica_node.current_offset(), 6);
}

/// Writers running non-idempotent commands on many keys while a replica
/// connects: once they stop, the replica equals the master.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_during_full_sync_converge() {
    let master = Node::new();
    let (_node, layer, addr) = start_master(&master).await;
    let layer = Arc::new(layer);
    let stop = Arc::new(AtomicBool::new(false));
    let writers: Vec<_> = (0..8)
        .map(|w| {
            let (layer, list, hash, kv, stop) = (
                Arc::clone(&layer),
                Arc::clone(&master.list),
                Arc::clone(&master.hash),
                Arc::clone(&master.kv),
                Arc::clone(&stop),
            );
            tokio::spawn(async move {
                let mut i = 0u64;
                while !stop.load(Ordering::Relaxed) || i < 200 {
                    let key = format!("w{w}_{}", i % 16);
                    let log = Some(layer.as_ref());
                    durable::lpush(log, &list, &key, vec![i.to_be_bytes().to_vec()], false)
                        .await
                        .unwrap();
                    durable::hincrby(log, &hash, &key, "n", 1).await.unwrap();
                    durable::kv_incr(log, &kv, &key, 1).await.unwrap();
                    durable::lpush(log, &list, "shared", vec![vec![w as u8]], false)
                        .await
                        .unwrap();
                    i += 1;
                    if i.is_multiple_of(32) {
                        tokio::task::yield_now().await;
                    }
                }
            })
        })
        .collect();

    sleep(Duration::from_millis(100)).await;
    let replica = Node::new();
    let _replica = start_replica(&replica, addr).await;
    sleep(Duration::from_millis(500)).await;
    stop.store(true, Ordering::Relaxed);
    for writer in writers {
        writer.await.unwrap();
    }
    converge(&master, &replica, "concurrent writers").await;
}

/// A TCP proxy in front of a master: the test can cut every connection and
/// choose the master new connections reach.
struct Proxy {
    addr: SocketAddr,
    upstream: Arc<parking_lot::Mutex<SocketAddr>>,
    cut: watch::Sender<u64>,
}

impl Proxy {
    async fn start(upstream: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let upstream = Arc::new(parking_lot::Mutex::new(upstream));
        let (cut, _) = watch::channel(0u64);
        let (target, cuts) = (Arc::clone(&upstream), cut.clone());
        tokio::spawn(async move {
            while let Ok((mut client, _)) = listener.accept().await {
                let to = *target.lock();
                let mut cut_rx = cuts.subscribe();
                tokio::spawn(async move {
                    let Ok(mut server) = TcpStream::connect(to).await else {
                        return;
                    };
                    tokio::select! {
                        _ = tokio::io::copy_bidirectional(&mut client, &mut server) => {}
                        _ = cut_rx.changed() => {}
                    }
                });
            }
        });
        Self {
            addr,
            upstream,
            cut,
        }
    }

    /// Close every open connection; new ones reach `upstream`.
    fn cut_to(&self, upstream: SocketAddr) {
        *self.upstream.lock() = upstream;
        self.cut.send_modify(|n| *n += 1);
    }
}

/// A replica that loses its connection catches up from the master's
/// replication log when it reconnects.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resync_after_disconnect_catches_up() {
    let master = Node::new();
    let (master_node, layer, addr) = start_master(&master).await;
    durable::rpush(Some(&layer), &master.list, "l", vec![b"a".to_vec()], false)
        .await
        .unwrap();
    let proxy = Proxy::start(addr).await;
    let replica = Node::new();
    let replica_node = start_replica(&replica, proxy.addr).await;
    converge(&master, &replica, "first sync").await;

    proxy.cut_to(addr);
    durable::rpush(Some(&layer), &master.list, "l", vec![b"b".to_vec()], false)
        .await
        .unwrap();
    durable::kv_delete(Some(&layer), &master.kv, "missing")
        .await
        .unwrap();
    durable::hset(Some(&layer), &master.hash, "h", "f", b"v".to_vec())
        .await
        .unwrap();
    converge(&master, &replica, "after reconnect").await;
    assert_eq!(
        replica_node.current_offset(),
        master_node.replication_offset()
    );
}

/// A replica that followed an earlier run of the master (its offset is
/// beyond the new master's log) gets a full sync on reconnect, and keys the
/// new master does not hold are gone from it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resync_after_master_restart_replaces_stale_state() {
    let first = Node::new();
    let (_first_node, first_layer, first_addr) = start_master(&first).await;
    write_every_datatype(&first, &first_layer, deadline_in(3_600_000)).await;
    durable::kv_set(Some(&first_layer), &first.kv, "gone", b"x".to_vec(), None)
        .await
        .unwrap();
    let proxy = Proxy::start(first_addr).await;
    let replica = Node::new();
    let replica_node = start_replica(&replica, proxy.addr).await;
    converge(&first, &replica, "first master").await;
    durable::hset(Some(&first_layer), &first.hash, "h2", "f", b"1".to_vec())
        .await
        .unwrap();
    converge(&first, &replica, "streamed write").await;
    assert!(replica_node.current_offset() > 0);

    // "Restart": a new master whose stores hold a different dataset and
    // whose replication log starts again at 0.
    let second = Node::new();
    let (second_node, second_layer, second_addr) = start_master(&second).await;
    durable::kv_set(Some(&second_layer), &second.kv, "kept", b"k".to_vec(), None)
        .await
        .unwrap();
    durable::sadd(Some(&second_layer), &second.set, "s", vec![b"m".to_vec()])
        .await
        .unwrap();
    assert!(second_node.replication_offset() < replica_node.current_offset());

    proxy.cut_to(second_addr);
    converge(&second, &replica, "after master restart").await;
    assert_eq!(replica.kv.get("gone").await.unwrap(), None);
    assert_eq!(replica_node.current_offset(), 2);
}
