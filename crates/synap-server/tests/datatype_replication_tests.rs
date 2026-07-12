//! Master→replica convergence for non-KV datatypes (phase6j item 2.2).
//!
//! phase6j extended a replica to apply EVERY `Operation` variant, not just KV +
//! stream, through a shared applier. These tests drive a live master→replica
//! pair over TCP and assert that hash, list, set, and sorted-set writes fanned
//! out by the master converge on the replica's stores.
//!
//! The master's datatype writes are propagated with `MasterNode::replicate`
//! (the same call the persistence/propagate hook makes for every logged op); the
//! replica applies them via `apply::apply_operation`. Convergence is polled with
//! a deadline because propagation is fire-and-forget over a heartbeat-driven
//! channel and would otherwise be racy on a loaded CI runner.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use synap_server::core::{
    HashStore, ListStore, SetStore, SortedSetStore, StreamConfig, StreamManager,
};
use synap_server::persistence::types::Operation;
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use synap_server::{KVConfig, KVStore};
use tokio::time::sleep;

/// Ask the OS for a free ephemeral port. A static counter is NOT safe here:
/// nextest runs every test in its own process, so each process would restart
/// the counter at the same base and all tests would race for one port.
fn next_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral port addr")
        .port()
}

async fn create_master() -> (Arc<MasterNode>, SocketAddr) {
    let port = next_port();
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    let config = ReplicationConfig {
        enabled: true,
        role: NodeRole::Master,
        replica_listen_address: Some(addr),
        heartbeat_interval_ms: 100,
        ..Default::default()
    };
    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let stream = Arc::new(StreamManager::new(StreamConfig::default()));
    let master = Arc::new(MasterNode::new(config, kv, Some(stream)).await.unwrap());
    sleep(Duration::from_millis(100)).await;
    (master, addr)
}

/// A connected replica plus handles to every datatype store it converges.
struct ReplicaStores {
    _replica: Arc<ReplicaNode>,
    hash: Arc<HashStore>,
    list: Arc<ListStore>,
    set: Arc<SetStore>,
    zset: Arc<SortedSetStore>,
}

async fn create_replica(master_addr: SocketAddr) -> ReplicaStores {
    let config = ReplicationConfig {
        enabled: true,
        role: NodeRole::Replica,
        master_address: Some(master_addr),
        auto_reconnect: true,
        reconnect_delay_ms: 100,
        ..Default::default()
    };
    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let stream = Arc::new(StreamManager::new(StreamConfig::default()));
    let hash = Arc::new(HashStore::new());
    let list = Arc::new(ListStore::new());
    let set = Arc::new(SetStore::new());
    let zset = Arc::new(SortedSetStore::new());
    let replica = ReplicaNode::new(
        config,
        kv,
        Some(stream),
        Some(hash.clone()),
        Some(list.clone()),
        Some(set.clone()),
        Some(zset.clone()),
        None,
    )
    .await
    .unwrap();
    // Let the initial (empty) full sync complete before incremental writes.
    sleep(Duration::from_millis(500)).await;
    ReplicaStores {
        _replica: replica,
        hash,
        list,
        set,
        zset,
    }
}

/// Poll `check` until it returns true or a 10s deadline elapses.
async fn poll_until<F: Fn() -> bool>(check: F, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if check() {
            return;
        }
        if Instant::now() >= deadline {
            panic!("{what} did not converge on the replica within deadline");
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hash_write_converges_on_replica() {
    let (master, addr) = create_master().await;
    let stores = create_replica(addr).await;

    master.replicate(Operation::HashSet {
        key: "h1".to_string(),
        field: "f1".to_string(),
        value: b"v1".to_vec(),
    });

    poll_until(
        || stores.hash.hget("h1", "f1").ok().flatten() == Some(b"v1".to_vec()),
        "hash field",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn list_write_converges_on_replica() {
    let (master, addr) = create_master().await;
    let stores = create_replica(addr).await;

    master.replicate(Operation::ListPush {
        key: "l1".to_string(),
        values: vec![b"a".to_vec(), b"b".to_vec()],
        left: false,
    });

    poll_until(
        || {
            stores.list.lrange("l1", 0, -1).unwrap_or_default()
                == vec![b"a".to_vec(), b"b".to_vec()]
        },
        "list elements",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn set_write_converges_on_replica() {
    let (master, addr) = create_master().await;
    let stores = create_replica(addr).await;

    master.replicate(Operation::SetAdd {
        key: "s1".to_string(),
        members: vec![b"m1".to_vec()],
    });

    poll_until(
        || stores.set.sismember("s1", b"m1".to_vec()).unwrap_or(false),
        "set member",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sorted_set_write_converges_on_replica() {
    let (master, addr) = create_master().await;
    let stores = create_replica(addr).await;

    master.replicate(Operation::ZAdd {
        key: "z1".to_string(),
        member: b"m1".to_vec(),
        score: 1.5,
        nx: false,
        xx: false,
        gt: false,
        lt: false,
    });

    poll_until(
        || stores.zset.zscore("z1", b"m1") == Some(1.5),
        "sorted-set score",
    )
    .await;
}
