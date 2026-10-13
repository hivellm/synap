//! The built `synap-cli` binary against a real in-process server (review
//! findings E-04 and E-05).
//!
//! Health scripts read the process exit status and output streams, so these
//! tests run the executable and assert exactly those: `PING` exits 0 only when
//! the server answered, and a refused connection, a timeout or rejected
//! credentials exit non-zero with the diagnostic on stderr. Counter amounts are
//! applied as given on every transport, and a malformed amount is refused
//! before anything is sent.

use std::net::SocketAddr;
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::Duration;

use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::core::{
    BitmapStore, GeospatialStore, HashStore, HyperLogLogStore, ListStore, SetStore, SortedSetStore,
    TransactionManager,
};
use synap_server::monitoring::{ClientListManager, MonitoringManager};
use synap_server::protocol::resp3::server::spawn_resp3_listener;
use synap_server::protocol::synap_rpc::server::spawn_synap_rpc_listener;
use synap_server::server::router::create_router;
use synap_server::{AppState, KVConfig, KVStore, ScriptManager};
use thunder::server::ListenerHandle;

const USER: &str = "ops";
const PASSWORD: &str = "cli-exit-status-password";

fn app_state() -> AppState {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());
    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let transaction_manager = Arc::new(TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));

    AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(HyperLogLogStore::new()),
        bitmap_store: Arc::new(BitmapStore::new()),
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
        client_list_manager: Arc::new(ClientListManager::new()),
        cluster_topology: None,
        cluster_migration: None,
        hub_client: None,
        user_manager: None,
        require_auth: false,
        replication: None,
    }
}

/// A running server: one state behind HTTP, SynapRPC and RESP3 listeners.
struct Server {
    state: AppState,
    http: String,
    rpc: String,
    resp3: String,
    _rpc_handle: Arc<ListenerHandle>,
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral port addr")
        .port()
}

/// Start a server; with `require_auth` only `USER`/`PASSWORD` gets in.
async fn serve(require_auth: bool) -> Server {
    let users = Arc::new(UserManager::new());
    users
        .create_user(USER, PASSWORD, false)
        .expect("user is created");

    let mut state = app_state();
    if require_auth {
        state.user_manager = Some(Arc::clone(&users));
        state.require_auth = true;
    }

    let router = create_router(
        state.clone(),
        RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        McpConfig::default(),
        users,
        Arc::new(ApiKeyManager::new()),
        require_auth,
        require_auth,
    );
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("http binds");
    let http = format!("http://{}", http_listener.local_addr().expect("http addr"));
    tokio::spawn(async move {
        axum::serve(
            http_listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("http serves");
    });

    let resp3: SocketAddr = format!("127.0.0.1:{}", free_port())
        .parse()
        .expect("resp3 addr");
    spawn_resp3_listener(state.clone(), resp3, Duration::ZERO, 64)
        .await
        .expect("resp3 binds");

    let rpc_handle = spawn_synap_rpc_listener(
        state.clone(),
        "127.0.0.1:0".parse().expect("rpc addr"),
        Duration::ZERO,
        64,
    )
    .await
    .expect("rpc binds");

    tokio::time::sleep(Duration::from_millis(50)).await;
    Server {
        state,
        http,
        rpc: format!("synap://{}", rpc_handle.local_addr()),
        resp3: format!("resp3://{resp3}"),
        _rpc_handle: rpc_handle,
    }
}

impl Server {
    fn urls(&self) -> [&str; 3] {
        [&self.http, &self.rpc, &self.resp3]
    }

    async fn store(&self, key: &str, value: i64) {
        self.state
            .kv_store
            .set(key, value.to_string().into_bytes(), None)
            .await
            .expect("seed value");
    }

    async fn stored(&self, key: &str) -> i64 {
        let raw = self
            .state
            .kv_store
            .get(key)
            .await
            .expect("read stored value")
            .expect("key exists");
        String::from_utf8(raw)
            .expect("utf-8 counter")
            .parse()
            .expect("integer counter")
    }
}

/// The CLI's outcome: exit status and both output streams.
struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Run {
    fn describe(&self) -> String {
        format!(
            "exit {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code, self.stdout, self.stderr
        )
    }

    fn assert_success(&self) {
        assert_eq!(self.code, Some(0), "expected success: {}", self.describe());
        assert!(
            self.stderr.trim().is_empty(),
            "success writes nothing to stderr: {}",
            self.describe()
        );
    }

    fn assert_failure_mentioning(&self, needle: &str) {
        assert!(
            matches!(self.code, Some(code) if code != 0),
            "expected a non-zero exit: {}",
            self.describe()
        );
        assert!(
            self.stderr.contains(needle),
            "stderr must name the failure ({needle:?}): {}",
            self.describe()
        );
        assert!(
            !self.stdout.contains("PONG"),
            "a failure must not print PONG: {}",
            self.describe()
        );
    }
}

/// Run the built binary off the async runtime the server is serving on.
async fn cli(args: &[&str]) -> Run {
    let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let output: Output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_synap-cli"))
            .args(&args)
            .env("NO_COLOR", "1")
            .env_remove("RUST_LOG")
            .output()
            .expect("synap-cli runs")
    })
    .await
    .expect("cli task completes");
    Run {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// A listener that accepts connections and never answers.
fn silent_listener() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind silent listener");
    let port = listener.local_addr().expect("silent addr").port();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming().flatten() {
            held.push(stream);
        }
    });
    port
}

// ── PING ─────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn ping_exits_zero_when_the_server_answers() {
    let server = serve(false).await;
    for url in server.urls() {
        let run = cli(&["--url", url, "PING"]).await;
        run.assert_success();
        assert!(run.stdout.contains("PONG"), "{url}: {}", run.describe());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ping_with_accepted_credentials_exits_zero() {
    let server = serve(true).await;
    for url in [&server.http, &server.rpc] {
        let run = cli(&["--url", url, "--user", USER, "--password", PASSWORD, "PING"]).await;
        run.assert_success();
        assert!(run.stdout.contains("PONG"), "{url}: {}", run.describe());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ping_exits_non_zero_when_the_connection_is_refused() {
    let port = free_port(); // bound and released: nothing listens there now
    for scheme in ["http", "synap", "resp3"] {
        let url = format!("{scheme}://127.0.0.1:{port}");
        let run = cli(&["--url", &url, "--timeout", "10", "PING"]).await;
        run.assert_failure_mentioning("PING failed");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ping_exits_non_zero_when_the_server_does_not_answer() {
    let port = silent_listener();
    for scheme in ["http", "synap", "resp3"] {
        let url = format!("{scheme}://127.0.0.1:{port}");
        let run = cli(&["--url", &url, "--timeout", "1", "PING"]).await;
        run.assert_failure_mentioning("PING failed");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ping_exits_non_zero_when_credentials_are_denied() {
    let server = serve(true).await;

    let wrong = ["--user", USER, "--password", "not-the-password", "PING"];
    let run = cli(&[&["--url", server.http.as_str()][..], &wrong[..]].concat()).await;
    run.assert_failure_mentioning("401");
    let run = cli(&[&["--url", server.rpc.as_str()][..], &wrong[..]].concat()).await;
    run.assert_failure_mentioning("Unauthorized");

    // Without credentials the authenticated transports refuse the ping too.
    let run = cli(&["--url", &server.http, "PING"]).await;
    run.assert_failure_mentioning("401");
    let run = cli(&["--url", &server.resp3, "PING"]).await;
    run.assert_failure_mentioning("NOAUTH");
}

#[tokio::test(flavor = "multi_thread")]
async fn credentials_on_resp3_are_refused_instead_of_ignored() {
    let server = serve(true).await;
    let run = cli(&[
        "--url",
        &server.resp3,
        "--user",
        USER,
        "--password",
        PASSWORD,
        "PING",
    ])
    .await;
    run.assert_failure_mentioning("not supported on resp3://");
}

// ── INCR / DECR amounts ──────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn incr_by_ten_adds_ten_on_every_transport() {
    let server = serve(false).await;
    server.store("c", 0).await;

    for (i, url) in server.urls().into_iter().enumerate() {
        let run = cli(&["--url", url, "INCR", "c", "10"]).await;
        run.assert_success();
        let expected = 10 * (i as i64 + 1);
        assert!(
            run.stdout.contains(&format!("(integer) {expected}")),
            "{url}: {}",
            run.describe()
        );
        assert_eq!(server.stored("c").await, expected, "{url}");
    }
    assert_eq!(server.stored("c").await, 30);
}

#[tokio::test(flavor = "multi_thread")]
async fn counter_amounts_apply_as_given_on_every_transport() {
    let server = serve(false).await;
    for url in server.urls() {
        server.store("n", 5).await;
        cli(&["--url", url, "INCR", "n"]).await.assert_success();
        assert_eq!(server.stored("n").await, 6, "{url}: INCR without amount");
        cli(&["--url", url, "INCR", "n", "0"])
            .await
            .assert_success();
        assert_eq!(server.stored("n").await, 6, "{url}: INCR by 0");
        cli(&["--url", url, "INCR", "n", "-4"])
            .await
            .assert_success();
        assert_eq!(server.stored("n").await, 2, "{url}: INCR by -4");
        cli(&["--url", url, "DECR", "n"]).await.assert_success();
        assert_eq!(server.stored("n").await, 1, "{url}: DECR without amount");
        cli(&["--url", url, "DECR", "n", "-9"])
            .await
            .assert_success();
        assert_eq!(server.stored("n").await, 10, "{url}: DECR by -9");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn counter_overflow_exits_non_zero_without_mutation() {
    let server = serve(false).await;
    let max = i64::MAX.to_string();
    for url in server.urls() {
        server.store("big", i64::MAX).await;
        let run = cli(&["--url", url, "INCR", "big", "1"]).await;
        run.assert_failure_mentioning("overflow");
        assert_eq!(server.stored("big").await, i64::MAX, "{url}");

        server.store("small", -2).await;
        let run = cli(&["--url", url, "DECR", "small", &max]).await;
        run.assert_failure_mentioning("overflow");
        assert_eq!(server.stored("small").await, -2, "{url}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn malformed_amount_exits_non_zero_without_mutation() {
    let server = serve(false).await;
    server.store("c", 5).await;
    for url in server.urls() {
        for (cmd, amount) in [
            ("INCR", "ten"),
            ("DECR", "1.5"),
            ("INCR", "9223372036854775808"),
        ] {
            let run = cli(&["--url", url, cmd, "c", amount]).await;
            run.assert_failure_mentioning(&format!("invalid amount '{amount}'"));
        }
        assert_eq!(server.stored("c").await, 5, "{url}: counter untouched");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn malformed_amount_is_refused_before_connecting() {
    // Nothing listens on this port: had the CLI sent anything, the error
    // would be a connection failure, not the amount.
    let port = free_port();
    for scheme in ["http", "synap", "resp3"] {
        let url = format!("{scheme}://127.0.0.1:{port}");
        let run = cli(&["--url", &url, "INCR", "c", "ten"]).await;
        run.assert_failure_mentioning("invalid amount 'ten'");
        assert!(
            !run.stderr.contains("PING") && !run.stderr.to_lowercase().contains("connect"),
            "{url}: {}",
            run.describe()
        );
    }
}
