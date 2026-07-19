//! End-to-end tests for the pair users actually run: the Rust SDK against the
//! Synap server, both on Thunder, over a real socket.
//!
//! `synap_rpc_thunder_tests.rs` drives the listener with a bare Thunder client
//! to pin the transport contract. This file goes one layer up and drives it
//! through `synap_sdk::SynapClient`, so the SDK's command mapping, response
//! mapping, credentials and push path are all exercised against the real
//! server rather than a mock.

mod test_helper;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use synap_sdk::{SynapClient, SynapConfig};
use synap_server::AppState;
use synap_server::auth::UserManager;
use synap_server::protocol::synap_rpc::server::spawn_synap_rpc_listener;
use thunder::server::ListenerHandle;

/// Bind the listener on an ephemeral port and return an SDK config aimed at it.
async fn start(state: AppState) -> (Arc<ListenerHandle>, SynapConfig) {
    let addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback address");
    let handle = spawn_synap_rpc_listener(state, addr, Duration::ZERO, 1024)
        .await
        .expect("listener binds");
    let config = SynapConfig::new(format!("synap://{}", handle.local_addr()));
    (handle, config)
}

async fn start_open() -> (Arc<ListenerHandle>, SynapConfig) {
    start(test_helper::create_test_app_state()).await
}

#[tokio::test]
async fn sdk_kv_round_trip() {
    let (_handle, config) = start_open().await;
    let client = SynapClient::new(config).expect("client builds");
    let kv = client.kv();

    kv.set("sdk-key", "sdk-value", None)
        .await
        .expect("set succeeds");
    let got: Option<String> = kv.get("sdk-key").await.expect("get succeeds");

    assert_eq!(got.as_deref(), Some("sdk-value"));
}

#[tokio::test]
async fn sdk_pipelines_concurrent_commands_on_one_connection() {
    // The pre-Thunder transport held one `Mutex<Option<TcpStream>>`, so these
    // 32 commands serialized behind it and the request id it incremented was
    // decorative. Thunder demultiplexes by id, so they genuinely overlap — and
    // each response must still match its own request.
    let (_handle, config) = start_open().await;
    let client = SynapClient::new(config).expect("client builds");

    let mut tasks = Vec::new();
    for i in 0..32u32 {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            let key = format!("sdk-pipelined-{i}");
            let value = format!("value-{i}");
            client
                .kv()
                .set(&key, &value, None)
                .await
                .expect("set succeeds");
            let got: Option<String> = client.kv().get(&key).await.expect("get succeeds");
            (i, got)
        }));
    }

    for task in tasks {
        let (i, got) = task.await.expect("task completes");
        assert_eq!(
            got.as_deref(),
            Some(format!("value-{i}").as_str()),
            "response {i} did not match its own request"
        );
    }
}

#[tokio::test]
async fn sdk_binary_value_survives_the_round_trip() {
    let (_handle, config) = start_open().await;
    let client = SynapClient::new(config).expect("client builds");

    // Valid UTF-8, since the SDK's JSON-facing API returns strings — the raw
    // binary path is covered at the transport level in the Thunder tests.
    let payload = "ünïcødé ✓ payload";
    client
        .kv()
        .set("sdk-binary", payload, None)
        .await
        .expect("set succeeds");

    let got: Option<String> = client.kv().get("sdk-binary").await.expect("get succeeds");
    assert_eq!(got.as_deref(), Some(payload));
}

#[tokio::test]
async fn sdk_authenticates_on_the_rpc_port() {
    // Before the Thunder swap the RPC transport never sent AUTH, so the SDK
    // simply could not talk to a `require_auth` deployment on this port.
    let manager = UserManager::new();
    manager
        .create_user("alice", "s3cret-passphrase", false)
        .expect("user is created");

    let mut state = test_helper::create_test_app_state();
    state.user_manager = Some(Arc::new(manager));
    state.require_auth = true;

    let (_handle, mut config) = start(state).await;
    config.username = Some("alice".to_string());
    config.password = Some("s3cret-passphrase".to_string());

    let client = SynapClient::new(config).expect("client builds");
    client
        .kv()
        .set("authed-key", "v", None)
        .await
        .expect("an authenticated SDK client can write");
}

#[tokio::test]
async fn sdk_surfaces_unauthorized_without_credentials() {
    let manager = UserManager::new();
    manager
        .create_user("alice", "s3cret-passphrase", false)
        .expect("user is created");

    let mut state = test_helper::create_test_app_state();
    state.user_manager = Some(Arc::new(manager));
    state.require_auth = true;

    let (_handle, config) = start(state).await;
    let client = SynapClient::new(config).expect("client builds");

    let err = client
        .kv()
        .get::<_, String>("anything")
        .await
        .expect_err("an un-credentialed client is refused");

    assert!(
        matches!(err, synap_sdk::SynapError::Unauthorized(_)),
        "NOAUTH should surface as the typed auth error, got: {err:?}"
    );
}

/// The binary transport JSON-encodes a structured value into a string on the
/// way out (`to_wire` has no array or object arm) and the server stores those
/// bytes verbatim. Nothing re-parsed them on the way back, so a value the SDK
/// itself had written could not be read back into its own type.
///
/// Found by the cross-SDK interop matrix (`scripts/interop/`): the Rust cell
/// was the only one that could not complete a binary round-trip.
#[tokio::test]
async fn sdk_reads_back_a_structured_value_it_wrote() {
    let (_handle, config) = start_open().await;
    let client = SynapClient::new(config).expect("client builds");
    let kv = client.kv();

    let written = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
    kv.set("round-trip:bytes", written.clone(), None)
        .await
        .expect("set succeeds");

    let read: Option<Vec<u8>> = kv.get("round-trip:bytes").await.expect("get succeeds");

    assert_eq!(read.as_ref(), Some(&written));
}

/// The re-parse must not change what a plain string decodes to. A string that
/// happens to be valid JSON (`"123"`, `"null"`, `"[1,2]"`) still has to come
/// back as that string, because the direct decode succeeds first and the
/// fallback is never reached.
#[tokio::test]
async fn sdk_does_not_reinterpret_a_string_that_looks_like_json() {
    let (_handle, config) = start_open().await;
    let client = SynapClient::new(config).expect("client builds");
    let kv = client.kv();

    for value in ["123", "null", "[1,2]", "{\"a\":1}", "true"] {
        kv.set("round-trip:str", value, None)
            .await
            .expect("set succeeds");
        let read: Option<String> = kv.get("round-trip:str").await.expect("get succeeds");
        assert_eq!(
            read.as_deref(),
            Some(value),
            "a string value must survive as that string"
        );
    }
}
