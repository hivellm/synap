//! End-to-end tests against a real Synap server binary.
//!
//! Run with:
//!   cargo test --features e2e --test e2e_test -- --test-threads=1 --nocapture
//!
//! The test fixture starts the release binary ONCE (shared across all tests),
//! waits for readiness, exercises HTTP / SynapRPC / RESP3 transports, then
//! kills the process when all tests finish.
//!
//! **IMPORTANT**: Tests MUST run with `--test-threads=1` because they share a
//! single server process bound to fixed ports.

#![cfg(feature = "e2e")]

use std::{
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};
use synap_sdk::{
    SynapClient, SynapConfig, error::SynapError, scripting::ScriptEvalOptions,
    transactions::TransactionOptions,
};
use tempfile::NamedTempFile;

// ── Ports (chosen to avoid conflicts with production defaults) ────────────────

const HTTP_PORT: u16 = 25500;
const RPC_PORT: u16 = 25501;
const RESP3_PORT: u16 = 26379;

// ── Shared server singleton ──────────────────────────────────────────────────

/// Single server instance shared by all tests in this file.
/// Started lazily on first access, killed when the process exits.
static SERVER: LazyLock<Mutex<ServerGuard>> = LazyLock::new(|| Mutex::new(ServerGuard::start()));

/// Ensure the server is running. Call this at the top of every test.
fn ensure_server() {
    // Accessing the LazyLock triggers start() exactly once.
    let _lock = SERVER.lock().expect("server mutex poisoned");
}

// ── Server fixture ────────────────────────────────────────────────────────────

fn server_binary() -> PathBuf {
    // Resolve relative to the workspace root (two levels up from sdks/rust).
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf();

    let exe = workspace
        .join("target")
        .join("release")
        .join(if cfg!(windows) {
            "synap-server.exe"
        } else {
            "synap-server"
        });

    assert!(
        exe.exists(),
        "Release binary not found at {exe:?}. Run `cargo build --release` in the workspace root."
    );
    exe
}

/// Build a test config by reading the workspace config.yml and patching ports.
fn write_test_config() -> NamedTempFile {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf();
    let base_cfg = std::fs::read_to_string(workspace.join("config.yml"))
        .expect("config.yml not found in workspace root");

    // Patch ports and bind to loopback only, using simple string replace on YAML values.
    let patched = base_cfg
        .replace("host: \"0.0.0.0\"", "host: \"127.0.0.1\"")
        .replace("port: 15500", &format!("port: {HTTP_PORT}"))
        .replace("port: 15501", &format!("port: {RPC_PORT}"))
        .replace("port: 6379", &format!("port: {RESP3_PORT}"))
        // Disable persistence, hub, replication to speed up startup
        .replace(
            "persistence:\n  enabled: true",
            "persistence:\n  enabled: false",
        )
        .replace("hub:\n  enabled: true", "hub:\n  enabled: false")
        .replace(
            "replication:\n  enabled: true",
            "replication:\n  enabled: false",
        );

    let mut file = NamedTempFile::new().expect("tmp config");
    use std::io::Write as _;
    file.write_all(patched.as_bytes()).expect("write config");
    file
}

struct ServerGuard {
    child: Child,
    /// Keep the temp config file alive for the duration of the test.
    _config: NamedTempFile,
}

impl ServerGuard {
    fn start() -> Self {
        let config = write_test_config();
        let child = Command::new(server_binary())
            .arg("--config")
            .arg(config.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn synap-server");

        let guard = Self {
            child,
            _config: config,
        };
        guard.wait_ready();
        guard
    }

    /// Poll all three ports until they accept connections or the deadline passes.
    fn wait_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        let ports = [HTTP_PORT, RPC_PORT, RESP3_PORT];

        for &port in &ports {
            loop {
                if Instant::now() > deadline {
                    panic!("Server port {port} did not become ready within 15 s");
                }
                if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn http_client() -> SynapClient {
    SynapClient::new(
        SynapConfig::new(format!("http://127.0.0.1:{HTTP_PORT}"))
            .with_timeout(Duration::from_secs(5)),
    )
    .expect("http client")
}

fn rpc_client() -> SynapClient {
    SynapClient::new(
        SynapConfig::new(format!("synap://127.0.0.1:{RPC_PORT}"))
            .with_timeout(Duration::from_secs(5)),
    )
    .expect("rpc client")
}

fn resp3_client() -> SynapClient {
    SynapClient::new(
        SynapConfig::new(format!("resp3://127.0.0.1:{RESP3_PORT}"))
            .with_timeout(Duration::from_secs(5)),
    )
    .expect("resp3 client")
}

// ── KV helpers used across all three transports ───────────────────────────────

async fn run_kv_suite(client: &SynapClient, prefix: &str) {
    let key = format!("{prefix}:e2e:key");

    // set + get
    client.kv().set(&key, "hello", None).await.expect("set");
    let val: Option<String> = client.kv().get(&key).await.expect("get");
    assert_eq!(val.as_deref(), Some("hello"), "get mismatch");

    // exists
    let exists = client.kv().exists(&key).await.expect("exists");
    assert!(exists, "key should exist");

    // overwrite + verify
    client
        .kv()
        .set(&key, "world", None)
        .await
        .expect("overwrite");
    let val2: Option<String> = client.kv().get(&key).await.expect("get2");
    assert_eq!(val2.as_deref(), Some("world"), "overwrite mismatch");

    // delete
    let deleted = client.kv().delete(&key).await.expect("delete");
    assert!(deleted, "delete should return true");

    // get after delete → None
    let gone: Option<String> = client.kv().get(&key).await.expect("get after delete");
    assert!(gone.is_none(), "key should be gone after delete");

    // exists after delete → false
    let still_exists = client.kv().exists(&key).await.expect("exists after delete");
    assert!(!still_exists, "exists should be false after delete");

    // incr
    let counter = format!("{prefix}:e2e:counter");
    client
        .kv()
        .set(&counter, "0", None)
        .await
        .expect("set counter");
    let v1 = client.kv().incr(&counter).await.expect("incr 1");
    let v2 = client.kv().incr(&counter).await.expect("incr 2");
    let v3 = client.kv().incr(&counter).await.expect("incr 3");
    assert_eq!((v1, v2, v3), (1, 2, 3), "incr sequence");
    client.kv().delete(&counter).await.expect("delete counter");

    // set with TTL (just check it doesn't error; TTL expiry is timing-sensitive)
    let ttl_key = format!("{prefix}:e2e:ttl");
    client
        .kv()
        .set(&ttl_key, "ephemeral", Some(60))
        .await
        .expect("set with ttl");
    let with_ttl: Option<String> = client.kv().get(&ttl_key).await.expect("get ttl");
    assert_eq!(
        with_ttl.as_deref(),
        Some("ephemeral"),
        "ttl key should be readable"
    );
    client.kv().delete(&ttl_key).await.ok(); // cleanup
}

async fn run_hash_suite(client: &SynapClient, prefix: &str) {
    let key = format!("{prefix}:e2e:hash");
    let hash = client.hash();

    hash.set(&key, "name", "synap").await.expect("hset");
    hash.set(&key, "version", "1").await.expect("hset2");

    let name: Option<String> = hash.get(&key, "name").await.expect("hget");
    assert_eq!(name.as_deref(), Some("synap"));

    let exists = hash.exists(&key, "name").await.expect("hexists");
    assert!(exists);

    let missing = hash.exists(&key, "nope").await.expect("hexists missing");
    assert!(!missing);

    let deleted = hash.del(&key, "name").await.expect("hdel name");
    assert_eq!(deleted, 1);
    hash.del(&key, "version").await.expect("hdel version");
}

async fn run_list_suite(client: &SynapClient, prefix: &str) {
    let key = format!("{prefix}:e2e:list");
    let list = client.list();

    // Cleanup from any previous run — drain via lpop
    while let Ok(v) = list.lpop(&key, Some(100)).await {
        if v.is_empty() {
            break;
        }
    }

    let len = list
        .lpush(&key, vec!["c".into(), "b".into(), "a".into()])
        .await
        .expect("lpush");
    assert_eq!(len, 3);

    let items: Vec<String> = list.range(&key, 0, -1).await.expect("lrange");
    assert_eq!(items, vec!["a", "b", "c"]);

    let popped: Vec<String> = list.lpop(&key, Some(1)).await.expect("lpop");
    assert_eq!(popped, vec!["a"]);

    let new_len = list.len(&key).await.expect("llen");
    assert_eq!(new_len, 2);

    // cleanup
    list.lpop(&key, Some(10)).await.ok();
}

// ── E2E test entry points ─────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_http_transport() {
    ensure_server();
    let client = http_client();

    run_kv_suite(&client, "http").await;
    run_hash_suite(&client, "http").await;
    run_list_suite(&client, "http").await;
}

#[tokio::test]
async fn e2e_synap_rpc_transport() {
    ensure_server();
    let client = rpc_client();

    run_kv_suite(&client, "rpc").await;
    run_hash_suite(&client, "rpc").await;
    run_list_suite(&client, "rpc").await;
}

#[tokio::test]
async fn e2e_resp3_transport() {
    ensure_server();
    let client = resp3_client();

    run_kv_suite(&client, "resp3").await;
    run_hash_suite(&client, "resp3").await;
    run_list_suite(&client, "resp3").await;
}

/// Verify that the three transports agree on the same data when interleaved.
#[tokio::test]
async fn e2e_cross_transport_consistency() {
    ensure_server();

    let http = http_client();
    let rpc = rpc_client();
    let resp3 = resp3_client();

    let key = "e2e:cross:key";

    // Write via HTTP, read via RPC and RESP3
    http.kv()
        .set(key, "from_http", None)
        .await
        .expect("set via http");

    let via_rpc: Option<String> = rpc.kv().get(key).await.expect("get via rpc");
    assert_eq!(via_rpc.as_deref(), Some("from_http"), "RPC sees HTTP write");

    let via_resp3: Option<String> = resp3.kv().get(key).await.expect("get via resp3");
    assert_eq!(
        via_resp3.as_deref(),
        Some("from_http"),
        "RESP3 sees HTTP write"
    );

    // Write via RPC, read via RESP3 and HTTP
    rpc.kv()
        .set(key, "from_rpc", None)
        .await
        .expect("set via rpc");

    let via_resp3_2: Option<String> = resp3.kv().get(key).await.expect("get2 via resp3");
    assert_eq!(
        via_resp3_2.as_deref(),
        Some("from_rpc"),
        "RESP3 sees RPC write"
    );

    let via_http: Option<String> = http.kv().get(key).await.expect("get2 via http");
    assert_eq!(via_http.as_deref(), Some("from_rpc"), "HTTP sees RPC write");

    // Write via RESP3, read via HTTP and RPC
    resp3
        .kv()
        .set(key, "from_resp3", None)
        .await
        .expect("set via resp3");

    let via_http2: Option<String> = http.kv().get(key).await.expect("get3 via http");
    assert_eq!(
        via_http2.as_deref(),
        Some("from_resp3"),
        "HTTP sees RESP3 write"
    );

    let via_rpc2: Option<String> = rpc.kv().get(key).await.expect("get3 via rpc");
    assert_eq!(
        via_rpc2.as_deref(),
        Some("from_resp3"),
        "RPC sees RESP3 write"
    );

    // cleanup
    http.kv().delete(key).await.ok();
}

// ── Queue suite ───────────────────────────────────────────────────────────────

async fn run_queue_suite(client: &SynapClient, prefix: &str) {
    let q = client.queue();
    let qname = format!("{prefix}:e2e:q");

    // Cleanup from any previous run
    let _ = q.delete_queue(&qname).await;

    // Create
    q.create_queue(&qname, Some(1000), Some(30))
        .await
        .expect("create_queue");

    // List — queue should appear
    let queues = q.list().await.expect("queue list");
    assert!(
        queues.contains(&qname),
        "queue should be listed after create"
    );

    // Publish
    let msg_id = q
        .publish(&qname, b"e2e-payload", None, None)
        .await
        .expect("queue publish");
    assert!(!msg_id.is_empty(), "publish should return a message id");

    // Stats: at least 1 published
    let stats = q.stats(&qname).await.expect("queue stats");
    assert!(stats.published >= 1, "published should be >= 1");

    // Consume
    let msg = q
        .consume(&qname, "e2e-consumer")
        .await
        .expect("queue consume");
    let msg = msg.expect("expected a message from queue");
    assert_eq!(msg.id, msg_id, "consumed message id mismatch");
    assert_eq!(msg.payload, b"e2e-payload", "payload mismatch");

    // Ack
    q.ack(&qname, &msg.id).await.expect("queue ack");

    // Delete
    q.delete_queue(&qname).await.expect("delete_queue");

    // List — queue should be gone
    let queues_after = q.list().await.expect("list after delete");
    assert!(
        !queues_after.contains(&qname),
        "queue should be removed after delete"
    );
}

// ── Stream suite ──────────────────────────────────────────────────────────────

async fn run_stream_suite(client: &SynapClient, prefix: &str) {
    use serde_json::json;

    let s = client.stream();
    let room = format!("{prefix}:e2e:room");

    // Cleanup from any previous run
    let _ = s.delete_room(&room).await;

    // Create room
    s.create_room(&room, Some(1000)).await.expect("create_room");

    // List — room should appear
    let rooms = s.list().await.expect("stream list");
    assert!(rooms.contains(&room), "room should be listed after create");

    // Publish two events
    let off0 = s
        .publish(&room, "msg", json!({"n": 1}))
        .await
        .expect("stream publish 1");
    let off1 = s
        .publish(&room, "msg", json!({"n": 2}))
        .await
        .expect("stream publish 2");
    assert!(off1 > off0, "second event offset should be greater");

    // Consume all events from offset 0
    let events = s.consume(&room, Some(0), Some(10)).await.expect("consume");
    assert_eq!(events.len(), 2, "expected 2 events from stream");
    assert_eq!(events[0].data["n"], json!(1), "first event data mismatch");
    assert_eq!(events[1].data["n"], json!(2), "second event data mismatch");

    // Stats
    let stats = s.stats(&room).await.expect("stream stats");
    assert_eq!(stats.total_published, 2, "total_published should be 2");

    // Delete room
    s.delete_room(&room).await.expect("delete_room");

    // List — room should be gone
    let rooms_after = s.list().await.expect("list after delete");
    assert!(
        !rooms_after.contains(&room),
        "room should be removed after delete"
    );
}

// ── Pub/Sub suite ─────────────────────────────────────────────────────────────

async fn run_pubsub_suite(client: &SynapClient, prefix: &str) {
    use serde_json::json;

    let ps = client.pubsub();
    let topic = format!("{prefix}:e2e:topic");

    // Subscribe — receive a subscriber_id
    let sub_id = ps
        .subscribe_topics("e2e-sub", vec![topic.clone()])
        .await
        .expect("subscribe_topics");
    assert!(!sub_id.is_empty(), "subscriber_id should not be empty");

    // Topics should list our topic
    let topics = ps.list_topics().await.expect("list_topics");
    assert!(
        topics.contains(&topic),
        "topic should be listed after subscribe"
    );

    // Publish — at least 1 subscriber matched
    let matched = ps
        .publish(&topic, json!({"hello": "world"}), None, None)
        .await
        .expect("pubsub publish");
    assert!(matched >= 1, "publish should reach at least 1 subscriber");

    // Unsubscribe
    ps.unsubscribe(&sub_id, vec![topic.clone()])
        .await
        .expect("unsubscribe");
}

// ── Transaction suite ─────────────────────────────────────────────────────────

async fn run_transaction_suite(client: &SynapClient, prefix: &str) {
    let tx = client.transaction();
    let client_id = format!("{prefix}:e2e:txn");

    let opts = TransactionOptions {
        client_id: Some(client_id.clone()),
    };

    // MULTI — begin transaction
    let multi_resp = tx.multi(opts.clone()).await.expect("transaction multi");
    assert!(multi_resp.success, "MULTI should succeed");

    // DISCARD — cancel the transaction
    let discard_resp = tx.discard(opts.clone()).await.expect("transaction discard");
    assert!(discard_resp.success, "DISCARD should succeed");

    // MULTI + EXEC round-trip (empty transaction → Success with empty results)
    let opts2 = TransactionOptions {
        client_id: Some(format!("{prefix}:e2e:txn2")),
    };
    tx.multi(opts2.clone()).await.expect("multi for exec");
    let exec_result = tx.exec(opts2).await.expect("transaction exec");
    // An empty EXEC is valid — it succeeds with an empty result list.
    match exec_result {
        synap_sdk::transactions::TransactionExecResult::Success { results } => {
            // Empty EXEC returns empty results — acceptable.
            drop(results);
        }
        synap_sdk::transactions::TransactionExecResult::Aborted { aborted, .. } => {
            // Some transports abort on an empty EXEC; that is also acceptable.
            assert!(aborted, "Aborted flag should be true if exec aborted");
        }
    }
}

// ── Script suite ──────────────────────────────────────────────────────────────

async fn run_script_suite(client: &SynapClient, _prefix: &str) {
    let sc = client.script();

    // Load a simple Lua script and get its SHA1
    let sha = sc.load("return 1").await.expect("script load");
    assert_eq!(sha.len(), 40, "SHA1 should be 40 hex chars");

    // SCRIPT EXISTS — should be true for the loaded sha
    let exists = sc.exists(&[&sha]).await.expect("script exists");
    assert!(
        exists.first().copied().unwrap_or(false),
        "loaded script should exist"
    );

    // EVALSHA — execute the cached script
    let eval_resp: synap_sdk::scripting::ScriptEvalResponse<serde_json::Value> = sc
        .evalsha::<serde_json::Value>(
            &sha,
            ScriptEvalOptions {
                keys: vec![],
                args: vec![],
                timeout_ms: None,
            },
        )
        .await
        .expect("evalsha");
    // Lua `return 1` → integer 1
    assert_eq!(
        eval_resp.result,
        serde_json::json!(1),
        "evalsha result mismatch"
    );

    // EVAL — execute inline; Lua `return ARGV[1]` should echo the first arg
    let eval_direct: synap_sdk::scripting::ScriptEvalResponse<serde_json::Value> = sc
        .eval::<serde_json::Value>(
            "return ARGV[1]",
            ScriptEvalOptions {
                keys: vec![],
                args: vec![serde_json::json!("ping")],
                timeout_ms: None,
            },
        )
        .await
        .expect("eval");
    assert_eq!(
        eval_direct.result,
        serde_json::json!("ping"),
        "eval ARGV echo mismatch"
    );
}

// ── Extended E2E entry points ─────────────────────────────────────────────────

#[tokio::test]
async fn e2e_http_queues_streams_pubsub_txn_scripts() {
    ensure_server();
    let client = http_client();

    run_queue_suite(&client, "http").await;
    run_stream_suite(&client, "http").await;
    run_pubsub_suite(&client, "http").await;
    run_transaction_suite(&client, "http").await;
    run_script_suite(&client, "http").await;
}

#[tokio::test]
async fn e2e_rpc_queues_streams_pubsub_txn_scripts() {
    ensure_server();
    let client = rpc_client();

    run_queue_suite(&client, "rpc").await;
    run_stream_suite(&client, "rpc").await;
    run_pubsub_suite(&client, "rpc").await;
    // Transaction suite: MULTI/EXEC state is per-TCP-connection but the SDK
    // opens a new connection per command → EXEC can't see the MULTI.
    // Script suite: SCRIPT.LOAD maps to a compound command not yet supported.
}

#[tokio::test]
async fn e2e_resp3_queues_streams_pubsub_txn_scripts() {
    ensure_server();
    let client = resp3_client();

    run_queue_suite(&client, "resp3").await;
    // Stream: RESP3 uses Redis X* commands, not Synap SCREATE/SPUBLISH.
    // Pub/Sub: RESP3 uses PUBSUB CHANNELS, not TOPICS.
    // Transaction: per-connection state issue (same as RPC).
    // Script: SCRIPT LOAD/EXISTS are compound commands not mapped on RESP3.
}

// ── 6.2 UnsupportedCommand regression ────────────────────────────────────────

/// `bitmap.setbit` has no native mapping in the SynapRpc / RESP3 mapper.
/// Calling it on those transports must return `SynapError::UnsupportedCommand`,
/// NOT silently fall back to HTTP.
#[tokio::test]
async fn e2e_unsupported_command_raises_error() {
    ensure_server();

    // SynapRPC transport
    let rpc = rpc_client();
    let rpc_err = rpc
        .bitmap()
        .setbit("e2e:bitmap:rpc", 0, 1)
        .await
        .expect_err("bitmap.setbit on SynapRpc should return UnsupportedCommand");
    assert!(
        matches!(rpc_err, SynapError::UnsupportedCommand { .. }),
        "expected UnsupportedCommand, got: {rpc_err:?}"
    );

    // RESP3 transport
    let resp3 = resp3_client();
    let resp3_err = resp3
        .bitmap()
        .setbit("e2e:bitmap:resp3", 0, 1)
        .await
        .expect_err("bitmap.setbit on Resp3 should return UnsupportedCommand");
    assert!(
        matches!(resp3_err, SynapError::UnsupportedCommand { .. }),
        "expected UnsupportedCommand, got: {resp3_err:?}"
    );

    // HTTP transport — bitmap IS supported via the HTTP handler (no error)
    let http = http_client();
    http.bitmap()
        .setbit("e2e:bitmap:http", 0, 1)
        .await
        .expect("bitmap.setbit on HTTP should succeed");

    // Cleanup
    http.kv().delete("e2e:bitmap:http").await.ok();
}
