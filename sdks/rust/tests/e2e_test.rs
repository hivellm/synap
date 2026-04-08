//! End-to-end tests against a real Synap server binary.
//!
//! Run with:
//!   cargo test --features e2e --test e2e_test -- --nocapture
//!
//! The test fixture starts the release binary, waits for readiness,
//! exercises HTTP / SynapRPC / RESP3 transports, then kills the process.

#![cfg(feature = "e2e")]

use std::{
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use synap_sdk::{SynapClient, SynapConfig};
use tempfile::NamedTempFile;

// ── Ports (chosen to avoid conflicts with production defaults) ────────────────

const HTTP_PORT: u16 = 25500;
const RPC_PORT: u16 = 25501;
const RESP3_PORT: u16 = 26379;

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
    let cfg = SynapConfig::new(format!("http://127.0.0.1:{HTTP_PORT}"))
        .with_http_transport()
        .with_timeout(Duration::from_secs(5));
    SynapClient::new(cfg).expect("http client")
}

fn rpc_client() -> SynapClient {
    let cfg = SynapConfig::new(format!("http://127.0.0.1:{HTTP_PORT}"))
        .with_synap_rpc_transport()
        .with_rpc_addr("127.0.0.1", RPC_PORT)
        .with_timeout(Duration::from_secs(5));
    SynapClient::new(cfg).expect("rpc client")
}

fn resp3_client() -> SynapClient {
    let cfg = SynapConfig::new(format!("http://127.0.0.1:{HTTP_PORT}"))
        .with_resp3_transport()
        .with_resp3_addr("127.0.0.1", RESP3_PORT)
        .with_timeout(Duration::from_secs(5));
    SynapClient::new(cfg).expect("resp3 client")
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
    let _server = ServerGuard::start();
    let client = http_client();

    run_kv_suite(&client, "http").await;
    run_hash_suite(&client, "http").await;
    run_list_suite(&client, "http").await;
}

#[tokio::test]
async fn e2e_synap_rpc_transport() {
    let _server = ServerGuard::start();
    let client = rpc_client();

    run_kv_suite(&client, "rpc").await;
    run_hash_suite(&client, "rpc").await;
    run_list_suite(&client, "rpc").await;
}

#[tokio::test]
async fn e2e_resp3_transport() {
    let _server = ServerGuard::start();
    let client = resp3_client();

    run_kv_suite(&client, "resp3").await;
    run_hash_suite(&client, "resp3").await;
    run_list_suite(&client, "resp3").await;
}

/// Verify that the three transports agree on the same data when interleaved.
#[tokio::test]
async fn e2e_cross_transport_consistency() {
    let _server = ServerGuard::start();

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
