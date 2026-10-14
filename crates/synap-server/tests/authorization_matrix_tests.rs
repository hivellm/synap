//! One authorization contract on every transport (review findings S-02, S-05).
//!
//! Each test starts a real deployment with authentication required — HTTP
//! (REST and the `/api/v1/command` envelope), the RESP3 listener and the
//! SynapRPC listener, all over one shared `AppState` — and checks that a
//! caller gets the same decision whichever transport it uses.

mod test_helper;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use base64::{Engine as _, engine::general_purpose};
use reqwest::StatusCode;
use serde_json::json;
use synap_server::AppState;
use synap_server::auth::{Action, ApiKeyManager, Permission, Role, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::protocol::resp3::server::spawn_resp3_listener;
use synap_server::protocol::synap_rpc::server::spawn_synap_rpc_listener;
use synap_server::protocol::synap_rpc::synap_config;
use synap_server::server::router::create_router;
use thunder::Value;
use thunder::client::{Client, ClientConfig};
use thunder::server::ListenerHandle;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const PASSWORD: &str = "matrix-password-1";

/// A running deployment: one state, three listeners.
struct Deployment {
    state: AppState,
    http: String,
    resp3: SocketAddr,
    rpc: SocketAddr,
    _rpc_handle: Arc<ListenerHandle>,
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral port addr")
        .port()
}

/// Users: `noroles` (nothing), `reader` (built-in read-only role), `scoped`
/// (read and write on `kv:orders:*` and `sortedset:a*`), `boss` (admin).
async fn deploy(require_auth: bool) -> Deployment {
    let users = UserManager::new();
    users
        .create_role(Role::custom(
            "orders",
            vec![
                Permission::new("kv:orders:*", Action::Read),
                Permission::new("kv:orders:*", Action::Write),
                Permission::new("sortedset:a*", Action::Read),
                Permission::new("sortedset:a*", Action::Write),
            ],
        ))
        .expect("role is created");
    for (name, admin) in [
        ("noroles", false),
        ("reader", false),
        ("scoped", false),
        ("boss", true),
    ] {
        users
            .create_user(name, PASSWORD, admin)
            .expect("user is created");
    }
    users
        .add_user_role("reader", "readonly")
        .expect("role is granted");
    users
        .add_user_role("scoped", "orders")
        .expect("role is granted");
    let users = Arc::new(users);

    let mut state = test_helper::create_test_app_state();
    state.user_manager = Some(Arc::clone(&users));
    state.require_auth = require_auth;

    let router = create_router(
        state.clone(),
        RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        McpConfig::default(),
        Arc::clone(&users),
        Arc::new(ApiKeyManager::new()),
        true,
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
    let rpc = rpc_handle.local_addr();

    tokio::time::sleep(Duration::from_millis(50)).await;
    Deployment {
        state,
        http,
        resp3,
        rpc,
        _rpc_handle: rpc_handle,
    }
}

fn basic(user: &str) -> String {
    let encoded = general_purpose::STANDARD.encode(format!("{user}:{PASSWORD}"));
    format!("Basic {encoded}")
}

// ── Minimal RESP client ───────────────────────────────────────────────────────

struct Resp3 {
    stream: BufReader<TcpStream>,
}

impl Resp3 {
    async fn connect(addr: SocketAddr, user: Option<&str>) -> Self {
        let stream = TcpStream::connect(addr).await.expect("resp3 connects");
        let mut client = Self {
            stream: BufReader::new(stream),
        };
        if let Some(user) = user {
            let reply = client.call(&["AUTH", user, PASSWORD]).await;
            assert_eq!(reply, "+OK", "AUTH as {user}");
        }
        client
    }

    /// Send one command and render its reply as text: `+OK`, `-ERR ...`,
    /// `:1`, `$value`, `$nil`, or `*[a, b]`.
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
        let line = line.trim_end().to_string();
        let (kind, rest) = line.split_at(1);
        match kind {
            "$" => {
                let len: i64 = rest.parse().expect("bulk length");
                if len < 0 {
                    return "$nil".into();
                }
                let mut buf = vec![0u8; len as usize + 2];
                self.stream.read_exact(&mut buf).await.expect("bulk body");
                format!("${}", String::from_utf8_lossy(&buf[..len as usize]))
            }
            "*" | "~" | "%" => {
                let count: i64 = rest.parse().expect("aggregate length");
                let items = if kind == "%" { count * 2 } else { count };
                let mut parts = Vec::new();
                for _ in 0..items.max(0) {
                    parts.push(Box::pin(self.reply()).await);
                }
                format!("*[{}]", parts.join(", "))
            }
            _ => line,
        }
    }
}

async fn rpc_client(addr: SocketAddr, user: &str) -> Client {
    Client::connect_with(
        &format!("synap://{addr}"),
        synap_config(),
        ClientConfig::new().user_pass(user, PASSWORD),
    )
    .await
    .expect("rpc connects")
}

// ── One operation on each transport, reduced to allowed / denied ─────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Get(&'static str),
    Set(&'static str),
    /// The admin-only command of each transport: `FLUSHALL` natively and on
    /// the envelope, `POST /script/flush` on REST (which has no flush route).
    AdminOnly,
}

async fn over_rest(d: &Deployment, user: &str, op: Op) -> Outcome {
    let http = reqwest::Client::new();
    let response = match op {
        Op::Get(key) => http.get(format!("{}/kv/get/{key}", d.http)),
        Op::Set(key) => http
            .post(format!("{}/kv/set", d.http))
            .json(&json!({"key": key, "value": "from-rest"})),
        Op::AdminOnly => http.post(format!("{}/script/flush", d.http)),
    }
    .header("Authorization", basic(user))
    .send()
    .await
    .expect("rest responds");
    match response.status() {
        StatusCode::FORBIDDEN => Outcome::Denied,
        status if status.is_success() || status == StatusCode::NOT_FOUND => Outcome::Allowed,
        status => panic!("REST {op:?} as {user}: unexpected {status}"),
    }
}

async fn over_envelope(d: &Deployment, user: &str, op: Op) -> Outcome {
    let (command, payload) = match op {
        Op::Get(key) => ("kv.get", json!({"key": key})),
        Op::Set(key) => ("kv.set", json!({"key": key, "value": "from-envelope"})),
        Op::AdminOnly => ("kv.flushall", json!({})),
    };
    let body: serde_json::Value = reqwest::Client::new()
        .post(format!("{}/api/v1/command", d.http))
        .header("Authorization", basic(user))
        .json(&json!({"command": command, "request_id": "m", "payload": payload}))
        .send()
        .await
        .expect("envelope responds")
        .json()
        .await
        .expect("envelope json");
    match body["error"].as_str() {
        Some(error) if error.contains("NOPERM") => Outcome::Denied,
        _ => Outcome::Allowed,
    }
}

async fn over_resp3(d: &Deployment, user: &str, op: Op) -> Outcome {
    let mut client = Resp3::connect(d.resp3, Some(user)).await;
    let reply = match op {
        Op::Get(key) => client.call(&["GET", key]).await,
        Op::Set(key) => client.call(&["SET", key, "from-resp3"]).await,
        Op::AdminOnly => client.call(&["FLUSHALL"]).await,
    };
    if reply.starts_with("-NOPERM") {
        Outcome::Denied
    } else {
        Outcome::Allowed
    }
}

async fn over_rpc(d: &Deployment, user: &str, op: Op) -> Outcome {
    let client = rpc_client(d.rpc, user).await;
    let result = match op {
        Op::Get(key) => client.call("GET", vec![Value::Str(key.into())]).await,
        Op::Set(key) => {
            client
                .call(
                    "SET",
                    vec![Value::Str(key.into()), Value::bytes(b"from-rpc".to_vec())],
                )
                .await
        }
        Op::AdminOnly => client.call("FLUSHALL", vec![]).await,
    };
    match result {
        Err(e) if e.to_string().contains("NOPERM") => Outcome::Denied,
        _ => Outcome::Allowed,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_role_gets_the_same_decision_on_every_transport() {
    use Outcome::{Allowed, Denied};

    let d = deploy(true).await;
    d.state
        .kv_store
        .set("orders:1", b"seed".to_vec(), None)
        .await
        .expect("seed");

    let ops = [
        Op::Get("orders:1"),
        Op::Set("orders:2"),
        Op::Set("other"),
        Op::AdminOnly,
    ];
    let expected = [
        ("noroles", [Denied, Denied, Denied, Denied]),
        ("reader", [Allowed, Denied, Denied, Denied]),
        ("scoped", [Allowed, Allowed, Denied, Denied]),
    ];

    for (user, decisions) in expected {
        for (op, want) in ops.iter().zip(decisions) {
            assert_eq!(
                over_rest(&d, user, *op).await,
                want,
                "REST {op:?} as {user}"
            );
            assert_eq!(
                over_envelope(&d, user, *op).await,
                want,
                "envelope {op:?} as {user}"
            );
            assert_eq!(
                over_resp3(&d, user, *op).await,
                want,
                "RESP3 {op:?} as {user}"
            );
            assert_eq!(
                over_rpc(&d, user, *op).await,
                want,
                "SynapRPC {op:?} as {user}"
            );
        }
    }

    // Nothing a denied request attempted happened.
    assert_eq!(d.state.kv_store.get("other").await.unwrap(), None);
    assert_eq!(
        d.state.kv_store.get("orders:1").await.unwrap(),
        Some(b"seed".to_vec())
    );

    // The admin may do everything, the admin-only commands included.
    for op in ops {
        assert_eq!(over_rest(&d, "boss", op).await, Allowed, "REST {op:?}");
        assert_eq!(
            over_envelope(&d, "boss", op).await,
            Allowed,
            "envelope {op:?}"
        );
        assert_eq!(over_resp3(&d, "boss", op).await, Allowed, "RESP3 {op:?}");
        assert_eq!(over_rpc(&d, "boss", op).await, Allowed, "SynapRPC {op:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn envelope_flush_is_refused_for_a_read_only_user() {
    let d = deploy(true).await;
    d.state
        .kv_store
        .set("keep", b"me".to_vec(), None)
        .await
        .expect("seed");

    assert_eq!(
        over_envelope(&d, "reader", Op::AdminOnly).await,
        Outcome::Denied
    );
    assert_eq!(
        d.state.kv_store.get("keep").await.unwrap(),
        Some(b"me".to_vec())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_destination_outside_the_grant_is_refused() {
    let d = deploy(true).await;
    let body: serde_json::Value = reqwest::Client::new()
        .post(format!("{}/api/v1/command", d.http))
        .header("Authorization", basic("scoped"))
        .json(&json!({
            "command": "sortedset.zunionstore",
            "request_id": "z",
            "payload": {"destination": "b", "keys": ["a1", "a2"]}
        }))
        .send()
        .await
        .expect("envelope responds")
        .json()
        .await
        .expect("envelope json");

    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains("sortedset:b")),
        "{body}"
    );
    assert_eq!(d.state.sorted_set_store.zcard("b"), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_script_cannot_reach_a_forbidden_key() {
    let d = deploy(true).await;
    let mut client = Resp3::connect(d.resp3, Some("scoped")).await;

    let reply = client
        .call(&["EVAL", "return redis.call('SET', 'other', 'x')", "0"])
        .await;

    assert!(
        reply.starts_with('-') && reply.contains("kv:other"),
        "{reply}"
    );
    assert_eq!(d.state.kv_store.get("other").await.unwrap(), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_denied_queued_command_aborts_the_transaction() {
    let d = deploy(true).await;
    let client = rpc_client(d.rpc, "scoped").await;
    let s = |v: &str| Value::Str(v.into());

    client.call("MULTI", vec![s("tx1")]).await.expect("MULTI");
    client
        .call("TXQUEUE", vec![s("tx1"), s("SET"), s("orders:tx"), s("1")])
        .await
        .expect("allowed command is queued");
    let denied = client
        .call("TXQUEUE", vec![s("tx1"), s("SET"), s("other"), s("1")])
        .await
        .expect_err("forbidden command is refused");
    assert!(denied.to_string().contains("NOPERM"), "{denied}");

    let exec = client
        .call("EXEC", vec![s("tx1")])
        .await
        .expect_err("EXEC of an aborted transaction fails");
    assert!(exec.to_string().contains("EXECABORT"), "{exec}");
    assert_eq!(d.state.kv_store.get("orders:tx").await.unwrap(), None);
    assert_eq!(d.state.kv_store.get("other").await.unwrap(), None);
}

/// With `require_auth` off, a connection that never authenticates runs as
/// anonymous and holds no permissions — on the native ports as on REST.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unauthenticated_native_connection_has_no_permissions() {
    let d = deploy(false).await;
    d.state
        .kv_store
        .set("orders:1", b"seed".to_vec(), None)
        .await
        .expect("seed");

    let mut resp3 = Resp3::connect(d.resp3, None).await;
    assert_eq!(resp3.call(&["PING"]).await, "+PONG");
    assert!(
        resp3
            .call(&["GET", "orders:1"])
            .await
            .starts_with("-NOPERM")
    );

    let rpc = Client::connect_with(
        &format!("synap://{}", d.rpc),
        synap_config(),
        ClientConfig::new(),
    )
    .await
    .expect("rpc connects");
    let err = rpc
        .call("GET", vec![Value::Str("orders:1".into())])
        .await
        .expect_err("anonymous GET is refused");
    assert!(err.to_string().contains("NOPERM"), "{err}");

    let status = reqwest::Client::new()
        .get(format!("{}/kv/get/orders:1", d.http))
        .send()
        .await
        .expect("rest responds")
        .status();
    assert_eq!(status, StatusCode::FORBIDDEN);
}
