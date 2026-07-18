//! SynapRPC listener integration tests — a real Thunder client over a real
//! socket against the real listener.
//!
//! The dispatch-tree unit tests cover command semantics; what they cannot cover
//! is the transport swap itself: framing, the handshake, the auth gate, the
//! per-command ACL, push delivery and the three wire-behavior deltas the
//! Thunder swap introduced (canonical `bin` `Bytes`, pre-auth `PING`, `HELLO`).

mod test_helper;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use synap_server::AppState;
use synap_server::auth::UserManager;
use synap_server::protocol::synap_rpc::server::spawn_synap_rpc_listener;
use synap_server::protocol::synap_rpc::synap_config;
use thunder::client::{Client, ClientConfig};
use thunder::server::ListenerHandle;
use thunder::{Request, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Bind the listener on an ephemeral port and return it with its address.
async fn start(state: AppState) -> (Arc<ListenerHandle>, SocketAddr) {
    let addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback address");
    let handle = spawn_synap_rpc_listener(state, addr, Duration::ZERO, 1024)
        .await
        .expect("listener binds");
    let local = handle.local_addr();
    (handle, local)
}

/// An open (no-auth) deployment.
async fn start_open() -> (Arc<ListenerHandle>, SocketAddr) {
    start(test_helper::create_test_app_state()).await
}

/// A deployment that enforces credentials, with one admin user.
async fn start_authenticated() -> (Arc<ListenerHandle>, SocketAddr, String) {
    let manager = UserManager::new();
    let password = "s3cret-passphrase";
    manager
        .create_user("alice", password, false)
        .expect("user is created");

    let mut state = test_helper::create_test_app_state();
    state.user_manager = Some(Arc::new(manager));
    state.require_auth = true;

    let (handle, addr) = start(state).await;
    (handle, addr, "alice".to_string())
}

async fn connect(addr: SocketAddr, cfg: ClientConfig) -> Client {
    Client::connect_with(&format!("synap://{addr}"), synap_config(), cfg)
        .await
        .expect("client connects")
}

// ── Baseline: the listener serves the catalog ────────────────────────────────

#[tokio::test]
async fn ping_round_trips() {
    let (_handle, addr) = start_open().await;
    let client = connect(addr, ClientConfig::new()).await;

    let pong = client.call("PING", vec![]).await.expect("PING succeeds");
    assert_eq!(pong, Value::Str("PONG".into()));
}

#[tokio::test]
async fn set_then_get_round_trips_a_binary_value() {
    let (_handle, addr) = start_open().await;
    let client = connect(addr, ClientConfig::new()).await;

    // Deliberately not valid UTF-8: this is the path that would silently
    // corrupt if the `Bytes` encoding were wrong in either direction.
    let payload = vec![0x00u8, 0xff, 0x10, 0x80, 0x7f];

    let set = client
        .call(
            "SET",
            vec![Value::Str("bin-key".into()), Value::bytes(payload.clone())],
        )
        .await
        .expect("SET succeeds");
    assert_eq!(set, Value::Str("OK".into()));

    let got = client
        .call("GET", vec![Value::Str("bin-key".into())])
        .await
        .expect("GET succeeds");
    assert_eq!(got, Value::bytes(payload));
}

#[tokio::test]
async fn unknown_command_errors_without_closing_the_connection() {
    let (_handle, addr) = start_open().await;
    let client = connect(addr, ClientConfig::new()).await;

    let err = client
        .call("NOT_A_COMMAND", vec![])
        .await
        .expect_err("unknown command is an error");
    assert!(
        err.to_string().contains("NOT_A_COMMAND") || err.to_string().to_lowercase().contains("err"),
        "error should name the failure, got: {err}"
    );

    // The connection survives a dispatch error (SRV-005).
    let pong = client
        .call("PING", vec![])
        .await
        .expect("connection still usable");
    assert_eq!(pong, Value::Str("PONG".into()));
}

#[tokio::test]
async fn requests_pipeline_on_one_connection() {
    let (_handle, addr) = start_open().await;
    let client = Arc::new(connect(addr, ClientConfig::new()).await);

    let mut tasks = Vec::new();
    for i in 0..32i64 {
        let client = Arc::clone(&client);
        tasks.push(tokio::spawn(async move {
            let key = format!("pipelined-{i}");
            client
                .call(
                    "SET",
                    vec![
                        Value::Str(key.clone()),
                        Value::bytes(i.to_le_bytes().to_vec()),
                    ],
                )
                .await
                .expect("SET succeeds");
            client
                .call("GET", vec![Value::Str(key)])
                .await
                .expect("GET succeeds")
        }));
    }

    for (i, task) in tasks.into_iter().enumerate() {
        let value = task.await.expect("task completes");
        // Each response must match its own request — the demultiplexer's job.
        assert_eq!(value, Value::bytes((i as i64).to_le_bytes().to_vec()));
    }
}

// ── Authentication and ACL ───────────────────────────────────────────────────

#[tokio::test]
async fn unauthenticated_command_is_refused_with_noauth() {
    let (_handle, addr, _user) = start_authenticated().await;
    let client = connect(addr, ClientConfig::new()).await;

    let err = client
        .call("GET", vec![Value::Str("k".into())])
        .await
        .expect_err("command before AUTH is refused");
    assert!(
        err.to_string().contains("NOAUTH"),
        "expected NOAUTH, got: {err}"
    );
}

#[tokio::test]
async fn wrong_password_is_refused() {
    let (_handle, addr, user) = start_authenticated().await;

    let result = Client::connect_with(
        &format!("synap://{addr}"),
        synap_config(),
        ClientConfig::new().user_pass(user, "not-the-password"),
    )
    .await;

    let err = result.expect_err("bad credentials are rejected");
    assert!(
        err.to_string().contains("WRONGPASS") || err.to_string().to_lowercase().contains("auth"),
        "expected an auth failure, got: {err}"
    );
}

#[tokio::test]
async fn authenticated_commands_execute() {
    let (_handle, addr, user) = start_authenticated().await;
    let client = connect(
        addr,
        ClientConfig::new().user_pass(user, "s3cret-passphrase"),
    )
    .await;

    let set = client
        .call(
            "SET",
            vec![Value::Str("authed".into()), Value::bytes(b"yes".to_vec())],
        )
        .await
        .expect("SET succeeds after AUTH");
    assert_eq!(set, Value::Str("OK".into()));
}

#[tokio::test]
async fn admin_only_command_is_refused_for_a_non_admin() {
    let (_handle, addr, user) = start_authenticated().await;
    let client = connect(
        addr,
        ClientConfig::new().user_pass(user, "s3cret-passphrase"),
    )
    .await;

    let err = client
        .call("FLUSHALL", vec![])
        .await
        .expect_err("non-admin cannot FLUSHALL");
    assert!(
        err.to_string().contains("NOPERM"),
        "expected NOPERM, got: {err}"
    );
}

#[tokio::test]
async fn open_deployment_serves_without_credentials() {
    let (_handle, addr) = start_open().await;
    let client = connect(addr, ClientConfig::new()).await;

    let set = client
        .call(
            "SET",
            vec![Value::Str("open".into()), Value::bytes(b"v".to_vec())],
        )
        .await
        .expect("open deployment needs no AUTH");
    assert_eq!(set, Value::Str("OK".into()));
}

// ── Server push (SUBSCRIBE) ──────────────────────────────────────────────────

#[tokio::test]
async fn published_message_reaches_a_subscriber_as_a_push_frame() {
    let mut state = test_helper::create_test_app_state();
    state.pubsub_router = Some(Arc::new(synap_server::core::pubsub::PubSubRouter::new()));
    let (_handle, addr) = start(state).await;

    let client = connect(addr, ClientConfig::new()).await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    client.on_push(move |value| {
        let _ = tx.send(value);
    });

    let subscribed = client
        .call("SUBSCRIBE", vec![Value::Str("room".into())])
        .await
        .expect("SUBSCRIBE succeeds");
    assert!(
        subscribed.map_get("subscriber_id").is_some(),
        "SUBSCRIBE should return a subscriber_id, got {subscribed:?}"
    );

    let publisher = connect(addr, ClientConfig::new()).await;
    publisher
        .call(
            "PUBLISH",
            vec![Value::Str("room".into()), Value::Str("hello".into())],
        )
        .await
        .expect("PUBLISH succeeds");

    let pushed = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("a push frame arrives within the timeout")
        .expect("the push channel stays open");

    assert_eq!(
        pushed.map_get("topic").and_then(Value::as_str),
        Some("room"),
        "push frame should name the topic, got {pushed:?}"
    );
    assert!(
        pushed
            .map_get("payload")
            .and_then(Value::as_str)
            .is_some_and(|p| p.contains("hello")),
        "push frame should carry the payload, got {pushed:?}"
    );
}

// ── Wire-behavior deltas introduced by the Thunder swap ──────────────────────

/// Write one length-prefixed frame and read one back, bypassing the client so
/// the raw bytes can be inspected.
async fn raw_round_trip(addr: SocketAddr, body: Vec<u8>) -> Vec<u8> {
    let mut sock = TcpStream::connect(addr).await.expect("socket connects");
    let mut frame = (body.len() as u32).to_le_bytes().to_vec();
    frame.extend_from_slice(&body);
    sock.write_all(&frame).await.expect("frame is written");

    let mut len_buf = [0u8; 4];
    sock.read_exact(&mut len_buf).await.expect("length prefix");
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut response = vec![0u8; len];
    sock.read_exact(&mut response).await.expect("frame body");
    response
}

#[tokio::test]
async fn server_emits_bytes_as_messagepack_bin() {
    let (_handle, addr) = start_open().await;
    let client = connect(addr, ClientConfig::new()).await;
    client
        .call(
            "SET",
            vec![
                Value::Str("bin-shape".into()),
                Value::bytes(vec![0xde, 0xad, 0xbe, 0xef]),
            ],
        )
        .await
        .expect("SET succeeds");

    let request = Request {
        id: 7,
        command: "GET".into(),
        args: vec![Value::Str("bin-shape".into())],
    };
    let body = rmp_serde::to_vec(&request).expect("request encodes");
    let response = raw_round_trip(addr, body).await;

    // MessagePack bin8 for a 4-byte payload is `0xc4 0x04 …`; the legacy
    // int-array form would start with the 4-element array marker `0x94`.
    let bin_marker = response
        .windows(6)
        .any(|w| w == [0xc4, 0x04, 0xde, 0xad, 0xbe, 0xef]);
    assert!(
        bin_marker,
        "expected canonical bin-encoded Bytes in {response:02x?}"
    );
}

#[tokio::test]
async fn server_still_decodes_legacy_int_array_bytes() {
    let (_handle, addr) = start_open().await;

    // A pre-Thunder SDK encodes `Bytes` as a MessagePack array of integers.
    // rmp-serde emits exactly that for a plain `Vec<u8>` inside the newtype
    // variant, so this frame is byte-identical to what an old client sends.
    #[derive(serde::Serialize)]
    enum LegacyValue {
        #[allow(dead_code)]
        Null,
        #[allow(dead_code)]
        Bool(bool),
        #[allow(dead_code)]
        Int(i64),
        #[allow(dead_code)]
        Float(f64),
        Bytes(Vec<u8>),
        Str(String),
        #[allow(dead_code)]
        Array(Vec<LegacyValue>),
        #[allow(dead_code)]
        Map(Vec<(LegacyValue, LegacyValue)>),
    }
    #[derive(serde::Serialize)]
    struct LegacyRequest {
        id: u32,
        command: String,
        args: Vec<LegacyValue>,
    }

    let legacy_set = LegacyRequest {
        id: 1,
        command: "SET".into(),
        args: vec![
            LegacyValue::Str("legacy-key".into()),
            LegacyValue::Bytes(vec![1, 2, 3]),
        ],
    };
    let body = rmp_serde::to_vec(&legacy_set).expect("legacy request encodes");
    // Sanity: this really is the int-array form, not bin.
    assert!(
        !body.windows(2).any(|w| w == [0xc4, 0x03]),
        "the legacy fixture must not encode Bytes as bin"
    );
    let response = raw_round_trip(addr, body).await;
    let decoded: thunder::Response = rmp_serde::from_slice(&response).expect("response decodes");
    assert_eq!(decoded.result, Ok(Value::Str("OK".into())));

    // And the value really landed in the store.
    let client = connect(addr, ClientConfig::new()).await;
    let got = client
        .call("GET", vec![Value::Str("legacy-key".into())])
        .await
        .expect("GET succeeds");
    assert_eq!(got, Value::bytes(vec![1, 2, 3]));
}

#[tokio::test]
async fn ping_is_allowed_before_authentication() {
    // Thunder's `AuthCommand` handshake allows PING/HELLO/AUTH/QUIT pre-auth;
    // the pre-Thunder server answered NOAUTH to a pre-auth PING.
    let (_handle, addr, _user) = start_authenticated().await;
    let client = connect(addr, ClientConfig::new()).await;

    let pong = client
        .call("PING", vec![])
        .await
        .expect("PING is on the pre-auth allowlist");
    assert_eq!(pong, Value::Str("PONG".into()));
}

// ── Resource bounds and lifecycle (restored by thunder-rpc 0.2.0) ────────────

#[tokio::test]
async fn connections_beyond_the_ceiling_are_refused() {
    // `network.max_connections` bounds the RPC port again (thunder#2): the
    // listener refuses the accept rather than queueing it, so a client fails
    // fast instead of hanging on a socket that will never be read.
    let addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback address");
    let handle = spawn_synap_rpc_listener(
        test_helper::create_test_app_state(),
        addr,
        Duration::ZERO,
        1, // ceiling of exactly one connection
    )
    .await
    .expect("listener binds");
    let addr = handle.local_addr();

    // The one permitted connection works.
    let first = connect(addr, ClientConfig::new()).await;
    assert_eq!(
        first
            .call("PING", vec![])
            .await
            .expect("first client works"),
        Value::Str("PONG".into())
    );

    // The second is refused: the socket may connect (the OS backlog accepts it)
    // but the listener drops it without ever serving a frame.
    let refused = tokio::time::timeout(Duration::from_secs(5), async {
        let mut sock = TcpStream::connect(addr).await.expect("socket connects");
        let request = Request {
            id: 1,
            command: "PING".into(),
            args: vec![],
        };
        let body = rmp_serde::to_vec(&request).expect("request encodes");
        let mut frame = (body.len() as u32).to_le_bytes().to_vec();
        frame.extend_from_slice(&body);
        // Either the write or the read fails once the listener drops it.
        if sock.write_all(&frame).await.is_err() {
            return true;
        }
        let mut buf = [0u8; 1];
        matches!(sock.read(&mut buf).await, Ok(0) | Err(_))
    })
    .await
    .expect("the refusal is prompt, not a hang");

    assert!(refused, "the connection past the ceiling must be refused");

    // And the first connection is unaffected by the refusal.
    assert_eq!(
        first.call("PING", vec![]).await.expect("first still works"),
        Value::Str("PONG".into())
    );
}

#[tokio::test]
async fn listener_drains_gracefully_on_stop() {
    // `stop()` takes `&self` again (thunder#5), so shutdown waits for in-flight
    // work instead of downgrading to the fire-and-forget `Drop` path.
    let (handle, addr) = start_open().await;
    let client = connect(addr, ClientConfig::new()).await;

    client
        .call(
            "SET",
            vec![Value::Str("drained".into()), Value::bytes(b"v".to_vec())],
        )
        .await
        .expect("SET succeeds");

    tokio::time::timeout(Duration::from_secs(5), handle.stop())
        .await
        .expect("graceful stop resolves");

    // Once drained, the port no longer serves.
    let after = tokio::time::timeout(
        Duration::from_secs(5),
        Client::connect_with(
            &format!("synap://{addr}"),
            synap_config(),
            ClientConfig::new(),
        ),
    )
    .await
    .expect("the connect attempt resolves");
    assert!(
        after.is_err(),
        "the listener should be closed after a graceful stop"
    );
}

// ── Frame cap ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn over_cap_length_prefix_is_refused() {
    let (_handle, addr) = start_open().await;
    let mut sock = TcpStream::connect(addr).await.expect("socket connects");

    // One byte past Synap's 512 MiB cap — the server must reject on the prefix
    // alone, without waiting for (or allocating) the claimed body.
    let claimed = (synap_config().max_frame_bytes as u32) + 1;
    sock.write_all(&claimed.to_le_bytes())
        .await
        .expect("prefix is written");

    let mut buf = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(5), sock.read(&mut buf)).await;
    match read {
        Ok(Ok(0)) | Ok(Err(_)) => {} // connection closed, as required
        Ok(Ok(n)) => panic!("expected the connection to close, read {n} bytes"),
        Err(_) => panic!("server neither closed the connection nor answered"),
    }
}
