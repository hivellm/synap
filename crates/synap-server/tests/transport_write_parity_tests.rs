//! Write durability parity across transports (phase21).
//!
//! Every mutating command must reach the WAL and the replicas whatever
//! transport carried it. Each test drives one transport (REST, the command
//! envelope, RESP3, SynapRPC, MCP) against a node whose persistence layer
//! writes an Always-mode WAL and feeds a live master→replica pair, then:
//!
//! 1. waits until the replica's stores hold exactly the node's state, and
//! 2. recovers a fresh state from the WAL alone and compares it too.
//!
//! The comparison is a fingerprint of every store (KV values and whether
//! they expire, hashes, lists, sets, sorted sets with exact score bits,
//! HyperLogLog cardinalities and bitmap bit counts), so a write that one
//! transport forgets to record, or records with the wrong value, shows up
//! as a difference instead of passing because the reply looked right.

mod test_helper;

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use synap_server::auth::OperationContext;
use synap_server::core::{
    BitmapStore, HashStore, HyperLogLogStore, ListStore, QueueConfig, SetStore, SortedSetStore,
    StreamConfig, StreamManager,
};
use synap_server::persistence::types::FsyncMode;
use synap_server::persistence::{PersistenceConfig, PersistenceLayer, StoreArcs, recover_all};
use synap_server::protocol::resp3::command::dispatch as resp3_dispatch;
use synap_server::protocol::resp3::parser::Resp3Value;
use synap_server::protocol::synap_rpc::dispatch::dispatch as rpc_dispatch;
use synap_server::protocol::synap_rpc::{Request as RpcRequest, SynapValue};
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use synap_server::{AppState, KVConfig, KVStore, handle_mcp_tool};
use tokio::time::sleep;

// ── Fixture ─────────────────────────────────────────────────────────────────

/// Borrowed view of every store a fingerprint reads.
struct Stores<'a> {
    kv: &'a KVStore,
    hash: &'a HashStore,
    list: &'a ListStore,
    set: &'a SetStore,
    zset: &'a SortedSetStore,
    hll: &'a HyperLogLogStore,
    bitmap: &'a BitmapStore,
}

/// Logical content of every store, with empty containers dropped.
#[derive(Debug, PartialEq)]
struct Fingerprint {
    kv: BTreeMap<String, (Vec<u8>, bool)>,
    hash: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
    list: BTreeMap<String, Vec<Vec<u8>>>,
    set: BTreeMap<String, BTreeSet<Vec<u8>>>,
    zset: BTreeMap<String, Vec<(Vec<u8>, u64)>>,
    hll: BTreeMap<String, u64>,
    bitmap: BTreeMap<String, usize>,
}

impl Fingerprint {
    fn key_count(&self) -> usize {
        self.kv.len()
            + self.hash.len()
            + self.list.len()
            + self.set.len()
            + self.zset.len()
            + self.hll.len()
            + self.bitmap.len()
    }
}

/// HyperLogLog and bitmap stores cannot be enumerated, so their keys are
/// probed by name. Every script below writes only these.
const HLL_KEYS: &[&str] = &[
    "r:hll", "r:hll2", "r:hlld", "p:hll", "p:hll2", "p:hlld", "h:hll", "e:hll",
];
const BITMAP_KEYS: &[&str] = &["r:bm", "p:bm", "h:bm", "e:bm"];

async fn fingerprint(s: Stores<'_>) -> Fingerprint {
    let mut kv = BTreeMap::new();
    for (key, value) in s.kv.dump().await.expect("kv dump") {
        let expires = matches!(s.kv.ttl(&key).await, Ok(Some(_)));
        kv.insert(key, (value, expires));
    }
    let hash = s
        .hash
        .dump()
        .into_iter()
        .filter(|(_, fields)| !fields.is_empty())
        .map(|(k, fields)| (k, fields.into_iter().collect()))
        .collect();
    let list = s
        .list
        .dump()
        .into_iter()
        .map(|(k, v)| (k, v.into_elements()))
        .filter(|(_, v)| !v.is_empty())
        .collect();
    let set = s
        .set
        .dump()
        .into_iter()
        .map(|(k, v)| (k, v.into_members().into_iter().collect::<BTreeSet<_>>()))
        .filter(|(_, v)| !v.is_empty())
        .collect();
    let zset = s
        .zset
        .dump()
        .into_iter()
        .filter(|(_, members)| !members.is_empty())
        .map(|(k, members)| {
            let mut members: Vec<(Vec<u8>, u64)> = members
                .into_iter()
                .map(|(m, sc)| (m, sc.to_bits()))
                .collect();
            members.sort();
            (k, members)
        })
        .collect();
    let hll = HLL_KEYS
        .iter()
        .filter_map(|k| s.hll.pfcount(k).ok().map(|c| ((*k).to_owned(), c)))
        .collect();
    let bitmap = BITMAP_KEYS
        .iter()
        .filter_map(|k| {
            s.bitmap
                .bitcount(k, None, None)
                .ok()
                .filter(|c| *c > 0)
                .map(|c| ((*k).to_owned(), c))
        })
        .collect();
    Fingerprint {
        kv,
        hash,
        list,
        set,
        zset,
        hll,
        bitmap,
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral addr")
        .port()
}

/// A node under test: an AppState whose persistence layer writes an
/// Always-mode WAL and replicates to a connected replica.
struct Node {
    name: &'static str,
    state: AppState,
    config: PersistenceConfig,
    _master: Arc<MasterNode>,
    _replica: Arc<ReplicaNode>,
    replica: StoreArcs,
}

impl Node {
    async fn start(name: &'static str) -> Self {
        let dir = PathBuf::from(format!("./target/phase21_parity/{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut config = PersistenceConfig::default();
        config.enabled = true;
        config.wal.enabled = true;
        config.wal.fsync_mode = FsyncMode::Always;
        config.wal.path = dir.join("wal.log");
        config.snapshot.enabled = false;
        config.snapshot.directory = dir.join("snapshots");

        let mut state = test_helper::create_test_app_state();

        let master_addr: SocketAddr = format!("127.0.0.1:{}", free_port()).parse().unwrap();
        let master = Arc::new(
            MasterNode::new(
                ReplicationConfig {
                    enabled: true,
                    role: NodeRole::Master,
                    replica_listen_address: Some(master_addr),
                    heartbeat_interval_ms: 100,
                    ..Default::default()
                },
                state.kv_store.clone(),
                None,
            )
            .await
            .expect("master starts"),
        );
        sleep(Duration::from_millis(100)).await;

        let replica = StoreArcs {
            kv_store: Arc::new(KVStore::new(KVConfig::default())),
            hash_store: Some(Arc::new(HashStore::new())),
            list_store: Some(Arc::new(ListStore::new())),
            set_store: Some(Arc::new(SetStore::new())),
            sorted_set_store: Some(Arc::new(SortedSetStore::new())),
            queue_manager: None,
            stream_manager: Some(Arc::new(StreamManager::new(StreamConfig::default()))),
            hyperloglog_store: Some(Arc::new(HyperLogLogStore::new())),
            bitmap_store: Some(Arc::new(BitmapStore::new())),
        };
        let replica_node = ReplicaNode::new(
            ReplicationConfig {
                enabled: true,
                role: NodeRole::Replica,
                master_address: Some(master_addr.into()),
                auto_reconnect: true,
                reconnect_delay_ms: 100,
                ..Default::default()
            },
            replica.clone(),
        )
        .await
        .expect("replica starts");
        // Let the initial (empty) full sync finish before any write.
        sleep(Duration::from_millis(500)).await;

        let layer = PersistenceLayer::new_with_replication(config.clone(), Some(master.clone()))
            .await
            .expect("persistence layer opens");
        state.persistence = Some(Arc::new(layer));

        Self {
            name,
            state,
            config,
            _master: master,
            _replica: replica_node,
            replica,
        }
    }

    fn stores(&self) -> Stores<'_> {
        Stores {
            kv: &self.state.kv_store,
            hash: &self.state.hash_store,
            list: &self.state.list_store,
            set: &self.state.set_store,
            zset: &self.state.sorted_set_store,
            hll: &self.state.hyperloglog_store,
            bitmap: &self.state.bitmap_store,
        }
    }

    fn replica_stores(&self) -> Stores<'_> {
        let r = &self.replica;
        Stores {
            kv: &r.kv_store,
            hash: r.hash_store.as_deref().unwrap(),
            list: r.list_store.as_deref().unwrap(),
            set: r.set_store.as_deref().unwrap(),
            zset: r.sorted_set_store.as_deref().unwrap(),
            hll: r.hyperloglog_store.as_deref().unwrap(),
            bitmap: r.bitmap_store.as_deref().unwrap(),
        }
    }

    /// Compare the node's state with its replica and with a WAL-only restart,
    /// returning the recovered stores for scenario-specific assertions.
    async fn assert_parity(self, min_keys: usize) -> synap_server::persistence::RecoveredStores {
        let before = fingerprint(self.stores()).await;
        assert!(
            before.key_count() >= min_keys,
            "{}: the script wrote only {} keys: {before:?}",
            self.name,
            before.key_count()
        );

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let on_replica = fingerprint(self.replica_stores()).await;
            if on_replica == before {
                break;
            }
            if Instant::now() >= deadline {
                panic!(
                    "{}: replica did not converge\nnode:    {before:?}\nreplica: {on_replica:?}",
                    self.name
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        let layer = self.state.persistence.clone().expect("layer");
        layer.shutdown().await.expect("WAL drains");
        drop(layer);

        let recovered = recover_all(&self.config, KVConfig::default(), QueueConfig::default())
            .await
            .expect("recovery succeeds");
        let after = fingerprint(Stores {
            kv: &recovered.kv_store,
            hash: recovered.hash_store.as_ref().unwrap(),
            list: recovered.list_store.as_ref().unwrap(),
            set: recovered.set_store.as_ref().unwrap(),
            zset: recovered.sorted_set_store.as_ref().unwrap(),
            hll: &recovered.hyperloglog_store,
            bitmap: &recovered.bitmap_store,
        })
        .await;
        assert_eq!(
            before, after,
            "{}: state recovered from the WAL differs from the state before restart",
            self.name
        );
        recovered
    }
}

// ── RESP3 ───────────────────────────────────────────────────────────────────

async fn resp3(state: &AppState, parts: &[&str]) -> Resp3Value {
    let args: Vec<Resp3Value> = parts
        .iter()
        .map(|p| Resp3Value::BulkString(p.as_bytes().to_vec()))
        .collect();
    let reply = resp3_dispatch(state, &OperationContext::unrestricted(), &args).await;
    assert!(
        !matches!(reply, Resp3Value::Error(_)),
        "RESP3 {parts:?} failed: {reply:?}"
    );
    reply
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resp3_writes_survive_restart_and_reach_the_replica() {
    let node = Node::start("resp3").await;
    let s = &node.state;
    for cmd in [
        &["SET", "r:s", "v1"][..],
        &["SET", "r:ttl", "ab", "EX", "300"],
        &["APPEND", "r:ttl", "cd"],
        &["SETRANGE", "r:pad", "3", "x"],
        &["INCR", "r:n"],
        &["INCRBY", "r:n", "5"],
        &["DECRBY", "r:n", "2"],
        &["DECR", "r:n"],
        &["MSET", "r:m1", "a", "r:m2", "b"],
        &["MSETNX", "r:m3", "c"],
        &["GETSET", "r:s", "v2"],
        &["SET", "r:gone", "x"],
        &["DEL", "r:gone"],
        &["SET", "r:e", "y"],
        &["EXPIRE", "r:e", "500"],
        &["SET", "r:p", "z", "EX", "100"],
        &["PERSIST", "r:p"],
        &["HSET", "r:h", "f1", "a", "f2", "b"],
        &["HDEL", "r:h", "f2"],
        &["HINCRBY", "r:h", "n", "3"],
        &["HINCRBYFLOAT", "r:h", "fl", "1.5"],
        &["HMSET", "r:h2", "x", "1", "y", "2"],
        &["LPUSH", "r:l", "a", "b", "c"],
        &["RPUSH", "r:l", "d"],
        &["LPOP", "r:l"],
        &["RPOP", "r:l"],
        &["SADD", "r:set", "a", "b", "c"],
        &["SREM", "r:set", "b"],
        &["ZADD", "r:z", "1", "a"],
        &["ZADD", "r:z", "2.5", "b"],
        &["ZREM", "r:z", "a"],
        &["PFADD", "r:hll", "x", "y", "z"],
        &["PFADD", "r:hll2", "w"],
        &["PFMERGE", "r:hlld", "r:hll", "r:hll2"],
        &["SETBIT", "r:bm", "7", "1"],
        &["SETBIT", "r:bm", "100", "1"],
        &["GEOADD", "r:geo", "13.36", "38.11", "palermo"],
        &[
            "EVAL",
            "redis.call('SET', KEYS[1], ARGV[1]) redis.call('RPUSH', KEYS[2], ARGV[1]) return 1",
            "2",
            "r:lua",
            "r:lual",
            "v",
        ],
    ] {
        resp3(s, cmd).await;
    }
    let recovered = node.assert_parity(15).await;
    assert_eq!(
        recovered.kv_store.get("r:ttl").await.unwrap(),
        Some(b"abcd".to_vec())
    );
    assert!(matches!(recovered.kv_store.ttl("r:ttl").await, Ok(Some(t)) if t > 0));
}

// ── SynapRPC ────────────────────────────────────────────────────────────────

fn s(v: &str) -> SynapValue {
    SynapValue::Str(v.to_owned())
}

fn i(v: i64) -> SynapValue {
    SynapValue::Int(v)
}

async fn rpc(state: &AppState, command: &str, args: Vec<SynapValue>) -> SynapValue {
    let reply = rpc_dispatch(
        state,
        &OperationContext::unrestricted(),
        RpcRequest {
            id: 1,
            command: command.to_owned(),
            args,
        },
    )
    .await;
    match reply.result {
        Ok(v) => v,
        Err(e) => panic!("SynapRPC {command} failed: {e}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn synap_rpc_writes_survive_restart_and_reach_the_replica() {
    let node = Node::start("rpc").await;
    let st = &node.state;
    rpc(st, "SET", vec![s("p:s"), s("v1")]).await;
    rpc(st, "SET", vec![s("p:ttl"), s("ab"), i(300)]).await;
    rpc(st, "APPEND", vec![s("p:ttl"), s("cd")]).await;
    rpc(st, "SETRANGE", vec![s("p:pad"), i(3), s("x")]).await;
    rpc(st, "INCR", vec![s("p:n")]).await;
    rpc(st, "INCRBY", vec![s("p:n"), i(5)]).await;
    rpc(st, "DECRBY", vec![s("p:n"), i(2)]).await;
    rpc(st, "MSET", vec![s("p:m1"), s("a"), s("p:m2"), s("b")]).await;
    rpc(st, "MSETNX", vec![s("p:m3"), s("c")]).await;
    rpc(st, "GETSET", vec![s("p:s"), s("v2")]).await;
    rpc(st, "SET", vec![s("p:gone"), s("x")]).await;
    rpc(st, "DEL", vec![s("p:gone")]).await;
    rpc(st, "SET", vec![s("p:e"), s("y")]).await;
    rpc(st, "EXPIRE", vec![s("p:e"), i(500)]).await;
    rpc(st, "SET", vec![s("p:p"), s("z"), i(100)]).await;
    rpc(st, "PERSIST", vec![s("p:p")]).await;
    // SynapRPC writes reach the replica (spec scenario).
    rpc(st, "HSET", vec![s("p:h"), s("f"), s("v")]).await;
    rpc(st, "HINCRBY", vec![s("p:h"), s("n"), i(3)]).await;
    rpc(st, "HINCRBYFLOAT", vec![s("p:h"), s("fl"), s("1.5")]).await;
    rpc(st, "HMSET", vec![s("p:h2"), s("x"), s("1"), s("y"), s("2")]).await;
    rpc(st, "HDEL", vec![s("p:h2"), s("y")]).await;
    rpc(st, "LPUSH", vec![s("p:l"), s("a"), s("b"), s("c")]).await;
    rpc(st, "RPUSH", vec![s("p:l"), s("d")]).await;
    rpc(st, "LPOP", vec![s("p:l")]).await;
    rpc(st, "RPOP", vec![s("p:l")]).await;
    rpc(st, "SADD", vec![s("p:set"), s("a"), s("b"), s("c")]).await;
    rpc(st, "SREM", vec![s("p:set"), s("b")]).await;
    rpc(st, "ZADD", vec![s("p:z"), s("1"), s("a")]).await;
    rpc(st, "ZADD", vec![s("p:z"), s("2.5"), s("b")]).await;
    rpc(st, "ZREM", vec![s("p:z"), s("a")]).await;
    rpc(st, "PFADD", vec![s("p:hll"), s("x"), s("y")]).await;
    rpc(st, "PFADD", vec![s("p:hll2"), s("w")]).await;
    rpc(st, "PFMERGE", vec![s("p:hlld"), s("p:hll"), s("p:hll2")]).await;
    rpc(st, "SETBIT", vec![s("p:bm"), i(9), i(1)]).await;
    rpc(
        st,
        "GEOADD",
        vec![s("p:geo"), s("38.11"), s("13.36"), s("palermo")],
    )
    .await;

    let replica_hash = node.replica.hash_store.clone().unwrap();
    let recovered = node.assert_parity(15).await;
    assert_eq!(replica_hash.hget("p:h", "f").unwrap(), Some(b"v".to_vec()));
    assert_eq!(
        recovered.kv_store.get("p:ttl").await.unwrap(),
        Some(b"abcd".to_vec())
    );
}

// ── REST and the command envelope ───────────────────────────────────────────

async fn serve(state: AppState) -> String {
    let router = test_helper::create_test_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    sleep(Duration::from_millis(50)).await;
    base
}

async fn post(client: &reqwest::Client, url: String, body: serde_json::Value) {
    let resp = client.post(&url).json(&body).send().await.unwrap();
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    assert!(status.is_success(), "POST {url} {body} -> {status}: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rest_writes_survive_restart_and_reach_the_replica() {
    let node = Node::start("rest").await;
    let base = serve(node.state.clone()).await;
    let c = reqwest::Client::new();

    // APPEND keeps bytes and expiry (spec scenario).
    post(
        &c,
        format!("{base}/kv/set"),
        json!({"key": "h:ttl", "value": "ab", "ttl": 300}),
    )
    .await;
    post(
        &c,
        format!("{base}/kv/h:ttl/append"),
        json!({"value": "cd"}),
    )
    .await;
    post(
        &c,
        format!("{base}/kv/h:pad/setrange"),
        json!({"offset": 3, "value": "x"}),
    )
    .await;
    post(
        &c,
        format!("{base}/kv/set"),
        json!({"key": "h:s", "value": "v1"}),
    )
    .await;
    post(&c, format!("{base}/kv/h:s/getset"), json!({"value": "v2"})).await;
    post(
        &c,
        format!("{base}/kv/msetnx"),
        json!({"pairs": [["h:m1", "a"]]}),
    )
    .await;
    post(
        &c,
        format!("{base}/kv/set"),
        json!({"key": "h:gone", "value": "x"}),
    )
    .await;
    let resp = c
        .delete(format!("{base}/kv/del/h:gone"))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    post(
        &c,
        format!("{base}/key/h:s/rename"),
        json!({"destination": "h:renamed"}),
    )
    .await;
    post(
        &c,
        format!("{base}/key/h:renamed/copy"),
        json!({"destination": "h:copy"}),
    )
    .await;
    post(
        &c,
        format!("{base}/hash/h:h/set"),
        json!({"field": "f", "value": "a"}),
    )
    .await;
    post(
        &c,
        format!("{base}/hash/h:h/incrby"),
        json!({"field": "n", "increment": 4}),
    )
    .await;
    // A hash renamed across types must be renamed on replay too.
    post(
        &c,
        format!("{base}/key/h:h/rename"),
        json!({"destination": "h:h2"}),
    )
    .await;
    post(
        &c,
        format!("{base}/list/h:l/rpush"),
        json!({"values": ["a", "b", "c"]}),
    )
    .await;
    post(&c, format!("{base}/list/h:l/lpop"), json!({"count": 1})).await;
    post(
        &c,
        format!("{base}/set/h:set/add"),
        json!({"members": ["a", "b"]}),
    )
    .await;
    post(
        &c,
        format!("{base}/set/h:set/rem"),
        json!({"members": ["a"]}),
    )
    .await;
    // REST sorted sets never reached the WAL before phase21.
    post(
        &c,
        format!("{base}/sortedset/h:z/zadd"),
        json!({"member": "a", "score": 1.5}),
    )
    .await;
    post(
        &c,
        format!("{base}/sortedset/h:z/zadd"),
        json!({"member": "b", "score": 2.5}),
    )
    .await;
    post(
        &c,
        format!("{base}/sortedset/h:z/zrem"),
        json!({"members": ["a"]}),
    )
    .await;
    post(
        &c,
        format!("{base}/hyperloglog/h:hll/pfadd"),
        json!({"elements": ["x", "y"]}),
    )
    .await;
    post(
        &c,
        format!("{base}/bitmap/h:bm/setbit"),
        json!({"offset": 5, "value": 1}),
    )
    .await;
    post(
        &c,
        format!("{base}/geospatial/h:geo/geoadd"),
        json!({"locations": [{"lat": 38.11, "lon": 13.36, "member": "palermo"}]}),
    )
    .await;

    let recovered = node.assert_parity(12).await;
    assert_eq!(
        recovered.kv_store.get("h:ttl").await.unwrap(),
        Some(b"abcd".to_vec())
    );
    assert!(matches!(recovered.kv_store.ttl("h:ttl").await, Ok(Some(t)) if t > 0 && t <= 300));
    assert!(recovered.hash_store.as_ref().unwrap().hlen("h:h2").unwrap() > 0);
}

async fn command(client: &reqwest::Client, base: &str, cmd: &str, payload: serde_json::Value) {
    let resp = client
        .post(format!("{base}/api/v1/command"))
        .json(&json!({"command": cmd, "request_id": "parity", "payload": payload}))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    assert!(
        status.is_success() && body["success"] == json!(true),
        "envelope {cmd} -> {status}: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn envelope_writes_survive_restart_and_reach_the_replica() {
    let node = Node::start("envelope").await;
    let base = serve(node.state.clone()).await;
    let c = reqwest::Client::new();

    command(
        &c,
        &base,
        "kv.set",
        json!({"key": "e:ttl", "value": "ab", "ttl": 300}),
    )
    .await;
    command(
        &c,
        &base,
        "kv.append",
        json!({"key": "e:ttl", "value": "cd"}),
    )
    .await;
    // SETRANGE with zero padding (spec scenario).
    command(
        &c,
        &base,
        "kv.setrange",
        json!({"key": "e:pad", "offset": 3, "value": "x"}),
    )
    .await;
    command(&c, &base, "kv.incr", json!({"key": "e:n"})).await;
    command(&c, &base, "kv.decr", json!({"key": "e:n"})).await;
    command(&c, &base, "kv.set", json!({"key": "e:e", "value": "y"})).await;
    command(&c, &base, "kv.expire", json!({"key": "e:e", "ttl": 500})).await;
    command(
        &c,
        &base,
        "kv.set",
        json!({"key": "e:p", "value": "z", "ttl": 100}),
    )
    .await;
    command(&c, &base, "kv.persist", json!({"key": "e:p"})).await;
    command(
        &c,
        &base,
        "kv.getset",
        json!({"key": "e:g", "value": "new"}),
    )
    .await;
    command(
        &c,
        &base,
        "key.copy",
        json!({"source": "e:g", "destination": "e:g2"}),
    )
    .await;
    command(
        &c,
        &base,
        "hash.set",
        json!({"key": "e:h", "field": "f", "value": "a"}),
    )
    .await;
    command(
        &c,
        &base,
        "hash.incrby",
        json!({"key": "e:h", "field": "n", "increment": 2}),
    )
    .await;
    command(
        &c,
        &base,
        "list.rpush",
        json!({"key": "e:l", "values": ["a", "b"]}),
    )
    .await;
    command(&c, &base, "list.lpop", json!({"key": "e:l", "count": 1})).await;
    command(
        &c,
        &base,
        "set.add",
        json!({"key": "e:set", "members": ["a", "b"]}),
    )
    .await;
    command(
        &c,
        &base,
        "sortedset.zadd",
        json!({"key": "e:z", "member": "a", "score": 3.5}),
    )
    .await;
    command(
        &c,
        &base,
        "hyperloglog.pfadd",
        json!({"key": "e:hll", "elements": ["x"]}),
    )
    .await;
    command(
        &c,
        &base,
        "bitmap.setbit",
        json!({"key": "e:bm", "offset": 3, "value": 1}),
    )
    .await;
    command(
        &c,
        &base,
        "geospatial.geoadd",
        json!({"key": "e:geo", "locations": [{"lat": 38.11, "lon": 13.36, "member": "palermo"}]}),
    )
    .await;

    let recovered = node.assert_parity(12).await;
    assert_eq!(
        recovered.kv_store.get("e:pad").await.unwrap(),
        Some(b"\0\0\0x".to_vec())
    );
}

// ── MCP ─────────────────────────────────────────────────────────────────────

async fn mcp(state: &Arc<AppState>, tool: &'static str, args: serde_json::Value) {
    let request = rmcp::model::CallToolRequestParams::new(tool)
        .with_arguments(args.as_object().cloned().unwrap());
    if let Err(e) = handle_mcp_tool(request, state.clone(), &OperationContext::unrestricted()).await
    {
        panic!("MCP {tool} failed: {e:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_writes_survive_restart_and_reach_the_replica() {
    let node = Node::start("mcp").await;
    let st = Arc::new(node.state.clone());
    mcp(
        &st,
        "synap_kv_set",
        json!({"key": "m:k", "value": "ab", "ttl": 300}),
    )
    .await;
    // APPEND used to log the operand as the whole value.
    mcp(&st, "synap_kv_append", json!({"key": "m:k", "value": "cd"})).await;
    mcp(&st, "synap_kv_set", json!({"key": "m:gone", "value": "x"})).await;
    mcp(&st, "synap_kv_delete", json!({"key": "m:gone"})).await;
    mcp(&st, "synap_kv_set", json!({"key": "m:old", "value": "r"})).await;
    mcp(
        &st,
        "synap_key_rename",
        json!({"source": "m:old", "destination": "m:new"}),
    )
    .await;
    mcp(
        &st,
        "synap_hash_set",
        json!({"key": "m:h", "field": "f", "value": "a"}),
    )
    .await;
    mcp(
        &st,
        "synap_list_push",
        json!({"key": "m:l", "values": ["a", "b", "c"]}),
    )
    .await;
    mcp(&st, "synap_list_pop", json!({"key": "m:l"})).await;
    mcp(
        &st,
        "synap_set_add",
        json!({"key": "m:set", "members": ["a", "b"]}),
    )
    .await;
    mcp(
        &st,
        "synap_sortedset_zadd",
        json!({"key": "m:z", "member": "a", "score": 4.5}),
    )
    .await;
    drop(st);

    let recovered = node.assert_parity(6).await;
    assert_eq!(
        recovered.kv_store.get("m:k").await.unwrap(),
        Some(b"abcd".to_vec())
    );
}
