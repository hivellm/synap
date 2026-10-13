//! Multi-key blocking pops across transports (phase33).
//!
//! BLPOP/BRPOP/BZPOPMIN/BZPOPMAX on several keys used to wait for each key's
//! notification in turn, so a push to the second key did not wake the client
//! until the first key was pushed too or the timeout expired. Each test runs
//! RESP3 and SynapRPC listeners over one shared `AppState` and blocks a client
//! on one transport while another transport pushes, once without persistence
//! and once through the durable path (Always-mode WAL), where every pop
//! attempt holds the keys' ordering stripes. The durable runs also recover a
//! fresh state from the WAL and check the pops were recorded.
//!
//! Clients are given a moment to block before the push; if one has not
//! blocked yet it pops on its first attempt, so the timing can make a test
//! less discriminating but never makes a correct server fail.

mod test_helper;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use synap_server::AppState;
use synap_server::KVConfig;
use synap_server::core::{ListStore, QueueConfig};
use synap_server::persistence::types::FsyncMode;
use synap_server::persistence::{PersistenceConfig, PersistenceLayer, recover_all};
use synap_server::protocol::resp3::server::spawn_resp3_listener;
use synap_server::protocol::synap_rpc::server::spawn_synap_rpc_listener;
use synap_server::protocol::synap_rpc::synap_config;
use thunder::Value;
use thunder::client::{Client, ClientConfig};
use thunder::server::ListenerHandle;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{Instant, sleep, timeout};

/// Time given to a client to block before the push that must wake it.
const SETTLE: Duration = Duration::from_millis(300);
/// A woken client must answer well within this; its own timeout is 10 s.
const WAKE_BOUND: Duration = Duration::from_secs(3);

// ── Deployment ──────────────────────────────────────────────────────────────

/// RESP3 and SynapRPC listeners over one state, optionally durable.
struct Deployment {
    state: AppState,
    resp3: SocketAddr,
    rpc: SocketAddr,
    persistence: Option<PersistenceConfig>,
    _rpc_handle: Arc<ListenerHandle>,
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral addr")
        .port()
}

async fn deploy(name: &str, durable: bool) -> Deployment {
    let mut state = test_helper::create_test_app_state();
    let persistence = if durable {
        let dir = PathBuf::from(format!("./target/phase33_blocking/{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut config = PersistenceConfig::default();
        config.enabled = true;
        config.wal.enabled = true;
        config.wal.fsync_mode = FsyncMode::Always;
        config.wal.path = dir.join("wal.log");
        config.snapshot.enabled = false;
        config.snapshot.directory = dir.join("snapshots");
        let layer = PersistenceLayer::new(config.clone())
            .await
            .expect("persistence layer opens");
        state.persistence = Some(Arc::new(layer));
        Some(config)
    } else {
        None
    };

    let resp3: SocketAddr = format!("127.0.0.1:{}", free_port())
        .parse()
        .expect("resp3 addr");
    spawn_resp3_listener(state.clone(), resp3, Duration::ZERO, 256)
        .await
        .expect("resp3 binds");
    let rpc_handle = spawn_synap_rpc_listener(
        state.clone(),
        "127.0.0.1:0".parse().expect("rpc addr"),
        Duration::ZERO,
        256,
    )
    .await
    .expect("rpc binds");
    let rpc = rpc_handle.local_addr();

    Deployment {
        state,
        resp3,
        rpc,
        persistence,
        _rpc_handle: rpc_handle,
    }
}

impl Deployment {
    /// For a durable deployment, drain the WAL and return the list store
    /// recovered from it alone.
    async fn recovered_lists(&self) -> Option<ListStore> {
        let config = self.persistence.as_ref()?;
        let layer = self.state.persistence.clone().expect("layer");
        layer.shutdown().await.expect("WAL drains");
        let recovered = recover_all(config, KVConfig::default(), QueueConfig::default())
            .await
            .expect("recovery succeeds");
        Some(recovered.list_store.expect("list store recovered"))
    }

    async fn resp3(&self) -> Resp3 {
        Resp3::connect(self.resp3).await
    }

    async fn rpc(&self) -> Rpc {
        Rpc::connect(self.rpc).await
    }
}

// ── Clients, both rendering replies the same way ────────────────────────────

/// Minimal RESP client. Replies render as `nil`, `[a, b]`, plain text for
/// bulk/simple strings and integers, `ERR ...` for errors.
struct Resp3 {
    stream: BufReader<TcpStream>,
}

impl Resp3 {
    async fn connect(addr: SocketAddr) -> Self {
        let stream = TcpStream::connect(addr).await.expect("resp3 connects");
        Self {
            stream: BufReader::new(stream),
        }
    }

    async fn call(&mut self, args: &[&str]) -> String {
        let mut frame = format!("*{}\r\n", args.len());
        for arg in args {
            frame.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
        }
        self.stream
            .get_mut()
            .write_all(frame.as_bytes())
            .await
            .expect("resp3 write");
        self.reply().await
    }

    async fn reply(&mut self) -> String {
        let mut line = String::new();
        self.stream.read_line(&mut line).await.expect("resp3 read");
        let line = line.trim_end().to_owned();
        let (kind, rest) = line.split_at(1);
        match kind {
            "_" => "nil".into(),
            "$" => {
                let len: i64 = rest.parse().expect("bulk length");
                if len < 0 {
                    return "nil".into();
                }
                let mut buf = vec![0u8; len as usize + 2];
                self.stream.read_exact(&mut buf).await.expect("bulk body");
                String::from_utf8_lossy(&buf[..len as usize]).into_owned()
            }
            "*" => {
                let count: i64 = rest.parse().expect("array length");
                if count < 0 {
                    return "nil".into();
                }
                let mut parts = Vec::new();
                for _ in 0..count {
                    parts.push(Box::pin(self.reply()).await);
                }
                format!("[{}]", parts.join(", "))
            }
            "+" | ":" | "," => rest.to_owned(),
            _ => line,
        }
    }
}

/// SynapRPC client over the real listener.
struct Rpc {
    client: Client,
}

impl Rpc {
    async fn connect(addr: SocketAddr) -> Self {
        let client = Client::connect_with(&format!("synap://{addr}"), synap_config(), {
            ClientConfig::new().call_timeout(Duration::from_secs(30))
        })
        .await
        .expect("rpc connects");
        Self { client }
    }

    /// `args` are strings except a trailing blocking timeout, sent as an int.
    async fn call(&self, command: &str, args: &[&str]) -> String {
        let blocking = command.starts_with('B');
        let last = args.len().saturating_sub(1);
        let args = args
            .iter()
            .enumerate()
            .map(|(i, a)| match a.parse::<i64>() {
                Ok(n) if blocking && i == last => Value::Int(n),
                _ => Value::Str((*a).to_owned()),
            })
            .collect();
        match self.client.call(command, args).await {
            Ok(value) => render(&value),
            Err(e) => format!("ERR {e}"),
        }
    }
}

fn render(value: &Value) -> String {
    match value {
        Value::Null => "nil".into(),
        Value::Str(s) => s.clone(),
        Value::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(render).collect();
            format!("[{}]", parts.join(", "))
        }
        other => format!("{other:?}"),
    }
}

/// Run `pop` (a blocking command), push with `push` once it has had time to
/// block, and return the pop's reply, failing if it took longer than
/// [`WAKE_BOUND`] after the push.
async fn woken_by<P, Q>(what: &str, pop: P, push: Q) -> String
where
    P: Future<Output = String> + Send + 'static,
    Q: Future<Output = String>,
{
    let blocked = tokio::spawn(pop);
    sleep(SETTLE).await;
    assert!(!blocked.is_finished(), "{what}: returned before the push");
    let pushed = push.await;
    assert!(!pushed.starts_with("ERR"), "{what}: push failed: {pushed}");
    let pushed_at = Instant::now();
    let reply = timeout(WAKE_BOUND, blocked)
        .await
        .unwrap_or_else(|_| panic!("{what}: the push did not wake the blocked client"))
        .expect("blocked client task");
    assert!(pushed_at.elapsed() < WAKE_BOUND);
    reply
}

// ── Scenarios ───────────────────────────────────────────────────────────────

async fn list_pops_wake_on_a_later_key(durable: bool) {
    let d = deploy(&format!("lists-{durable}"), durable).await;

    // RESP3 client blocked on [a, b]; SynapRPC pushes to b.
    let mut blocker = d.resp3().await;
    let pusher = d.rpc().await;
    let reply = woken_by(
        "RESP3 BLPOP / SynapRPC RPUSH",
        async move { blocker.call(&["BLPOP", "a", "b", "10"]).await },
        pusher.call("RPUSH", &["b", "from-rpc"]),
    )
    .await;
    assert_eq!(reply, "[b, from-rpc]");

    // SynapRPC client blocked on [c, d, e]; RESP3 pushes to e.
    let blocker = d.rpc().await;
    let mut pusher = d.resp3().await;
    let reply = woken_by(
        "SynapRPC BRPOP / RESP3 LPUSH",
        async move { blocker.call("BRPOP", &["c", "d", "e", "10"]).await },
        pusher.call(&["LPUSH", "e", "first", "second"]),
    )
    .await;
    assert_eq!(reply, "[e, first]", "BRPOP pops the tail");

    // Keys that already hold data: the first one in argument order wins.
    let mut client = d.resp3().await;
    client.call(&["RPUSH", "f", "from-f"]).await;
    assert_eq!(
        client.call(&["BLPOP", "a", "f", "e", "10"]).await,
        "[f, from-f]"
    );

    if let Some(lists) = d.recovered_lists().await {
        assert!(!lists.exists("b"), "the BLPOP was recorded");
        assert_eq!(
            lists.lrange("e", 0, -1).expect("e survives"),
            vec![b"second".to_vec()],
            "the BRPOP was recorded"
        );
        assert!(!lists.exists("f"));
    }
}

async fn sorted_set_pops_wake_on_a_later_key(durable: bool) {
    let d = deploy(&format!("zsets-{durable}"), durable).await;

    let mut blocker = d.resp3().await;
    let pusher = d.rpc().await;
    let reply = woken_by(
        "RESP3 BZPOPMIN / SynapRPC ZADD",
        async move { blocker.call(&["BZPOPMIN", "z1", "z2", "10"]).await },
        pusher.call("ZADD", &["z2", "1", "m"]),
    )
    .await;
    assert_eq!(reply, "[z2, m, 1]");

    let blocker = d.rpc().await;
    let mut pusher = d.resp3().await;
    let reply = woken_by(
        "SynapRPC BZPOPMAX / RESP3 ZADD",
        async move { blocker.call("BZPOPMAX", &["z3", "z4", "10"]).await },
        pusher.call(&["ZADD", "z4", "2.5", "n"]),
    )
    .await;
    assert_eq!(reply, "[z4, n, 2.5]");
    assert_eq!(d.state.sorted_set_store.zcard("z2"), 0);
    assert_eq!(d.state.sorted_set_store.zcard("z4"), 0);
}

async fn timeouts_return_nil(durable: bool) {
    let d = deploy(&format!("timeouts-{durable}"), durable).await;
    let mut resp3 = d.resp3().await;
    let rpc = d.rpc().await;

    let started = Instant::now();
    assert_eq!(resp3.call(&["BLPOP", "none1", "none2", "1"]).await, "nil");
    assert_eq!(rpc.call("BRPOP", &["none1", "none2", "1"]).await, "nil");
    assert_eq!(resp3.call(&["BZPOPMIN", "nz1", "nz2", "1"]).await, "nil");
    assert_eq!(rpc.call("BZPOPMAX", &["nz1", "nz2", "1"]).await, "nil");
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(3900),
        "each pop waited for its timeout: {elapsed:?}"
    );
}

async fn every_blocked_client_gets_one_element(durable: bool) {
    let d = deploy(&format!("fanout-{durable}"), durable).await;

    let mut blocked = Vec::new();
    for i in 0..16 {
        if i % 2 == 0 {
            let mut client = d.resp3().await;
            blocked.push(tokio::spawn(async move {
                client.call(&["BLPOP", "other", "q", "10"]).await
            }));
        } else {
            let client = d.rpc().await;
            blocked.push(tokio::spawn(async move {
                client.call("BLPOP", &["other", "q", "10"]).await
            }));
        }
    }
    sleep(SETTLE).await;

    let mut pushers = Vec::new();
    for p in 0..4 {
        if p % 2 == 0 {
            let mut client = d.resp3().await;
            pushers.push(tokio::spawn(async move {
                for i in 0..4 {
                    let value = format!("v{p}-{i}");
                    client.call(&["RPUSH", "q", &value]).await;
                }
            }));
        } else {
            let client = d.rpc().await;
            pushers.push(tokio::spawn(async move {
                for i in 0..4 {
                    let value = format!("v{p}-{i}");
                    client.call("RPUSH", &["q", &value]).await;
                }
            }));
        }
    }
    for pusher in pushers {
        pusher.await.expect("pusher task");
    }

    let mut got = BTreeSet::new();
    for client in blocked {
        let reply = timeout(WAKE_BOUND, client)
            .await
            .expect("every blocked client is served")
            .expect("blocked client task");
        assert!(reply.starts_with("[q, v"), "unexpected reply {reply}");
        assert!(got.insert(reply), "an element was delivered twice");
    }
    assert_eq!(got.len(), 16);
    assert!(!d.state.list_store.exists("q"));
    if let Some(lists) = d.recovered_lists().await {
        assert!(!lists.exists("q"), "all 16 pops were recorded");
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn list_pops_wake_on_a_later_key_in_memory() {
    list_pops_wake_on_a_later_key(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn list_pops_wake_on_a_later_key_durable() {
    list_pops_wake_on_a_later_key(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sorted_set_pops_wake_on_a_later_key_in_memory() {
    sorted_set_pops_wake_on_a_later_key(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sorted_set_pops_wake_on_a_later_key_durable() {
    sorted_set_pops_wake_on_a_later_key(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn timeouts_return_nil_in_memory() {
    timeouts_return_nil(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn timeouts_return_nil_durable() {
    timeouts_return_nil(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_blocked_client_gets_one_element_in_memory() {
    every_blocked_client_gets_one_element(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_blocked_client_gets_one_element_durable() {
    every_blocked_client_gets_one_element(true).await;
}
