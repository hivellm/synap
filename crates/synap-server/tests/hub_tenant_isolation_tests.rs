//! Hub mode isolates tenants on the production router (review finding S-03).
//!
//! Every test serves `create_router` with a `HubClient` in the state — the
//! startup path `main.rs` takes when `hub.enabled` is true — and a mock
//! access-key validator standing in for the HiveHub API. Two tenants write the
//! same logical names through REST, the `/api/v1/command` envelope, MCP (the
//! real Streamable HTTP handshake), UMICP and WebSockets, and each must see
//! only its own data.

mod test_helper;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use reqwest::StatusCode;
use serde_json::{Value, json};
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::core::{KeyWatchNotifier, SynapError};
use synap_server::hub::restrictions::Plan;
use synap_server::hub::{AccessKeyValidator, HubClient, HubConfig, ResourceNaming};
use synap_server::{
    AppState, KVStore, PubSubRouter, QueueConfig, QueueManager, ServerConfig, StreamConfig,
    StreamManager, create_router,
};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest, http::HeaderValue};
use umicp_core::{Envelope, OperationType};
use uuid::Uuid;

const KEY_A: &str = "hub-key-tenant-a";
const KEY_B: &str = "hub-key-tenant-b";
const PROTOCOL_VERSION: &str = "2025-06-18";

fn tenant_a() -> Uuid {
    Uuid::parse_str("aaaaaaaa-0000-4000-8000-00000000000a").unwrap()
}

fn tenant_b() -> Uuid {
    Uuid::parse_str("bbbbbbbb-0000-4000-8000-00000000000b").unwrap()
}

/// The HiveHub API as the server sees it: two known keys, everything else
/// unknown.
struct MockHub {
    tenants: HashMap<&'static str, Uuid>,
}

impl AccessKeyValidator for MockHub {
    fn validate<'a>(
        &'a self,
        access_key: &'a str,
    ) -> BoxFuture<'a, Result<(Uuid, Plan), SynapError>> {
        let owner = self.tenants.get(access_key).copied();
        Box::pin(async move {
            owner
                .map(|tenant| (tenant, Plan::Pro))
                .ok_or_else(|| SynapError::Unauthorized("unknown access key".to_string()))
        })
    }
}

fn hub_config() -> HubConfig {
    HubConfig {
        enabled: true,
        api_url: "http://127.0.0.1:9".to_string(),
        service_api_key: "service-key".to_string(),
        usage_report_interval: 300,
        cache_ttl: 60,
        timeout: 30,
    }
}

fn mock_hub() -> HubClient {
    let validator = MockHub {
        tenants: HashMap::from([(KEY_A, tenant_a()), (KEY_B, tenant_b())]),
    };
    HubClient::with_validator(hub_config(), Arc::new(validator)).expect("hub client")
}

/// An AppState wired the way `main.rs` wires it for these surfaces: a KV
/// store that publishes watch events, pub/sub, queues and streams.
fn hub_state(hub: HubClient) -> AppState {
    let mut state = test_helper::create_test_app_state();
    let pubsub = Arc::new(PubSubRouter::new());
    let notifier = Arc::new(KeyWatchNotifier::new(pubsub.clone(), 0));
    let kv_store = Arc::new(
        KVStore::new(ServerConfig::default().to_kv_config()).with_watch_notifier(Some(notifier)),
    );
    state.transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        state.hash_store.clone(),
        state.list_store.clone(),
        state.set_store.clone(),
        state.sorted_set_store.clone(),
    ));
    state.kv_store = kv_store;
    state.pubsub_router = Some(pubsub);
    state.queue_manager = Some(Arc::new(QueueManager::new(QueueConfig::default())));
    state.stream_manager = Some(Arc::new(StreamManager::new(StreamConfig::default())));
    state.hub_client = Some(Arc::new(hub));
    state
}

struct Server {
    url: String,
    state: AppState,
    http: reqwest::Client,
}

impl Server {
    /// Serve the production router in Hub mode. Local authentication is
    /// enabled too, to show Hub mode replaces it rather than adding to it.
    async fn start(hub: HubClient) -> Self {
        let state = hub_state(hub);
        let router = create_router(
            state.clone(),
            RateLimitConfig {
                enabled: false,
                requests_per_second: 100,
                burst_size: 10,
            },
            McpConfig::default(),
            Arc::new(UserManager::new()),
            Arc::new(ApiKeyManager::new()),
            true,
            true,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("http binds");
        let url = format!("http://{}", listener.local_addr().expect("http addr"));
        tokio::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .expect("http serves");
        });
        Self {
            url,
            state,
            http: reqwest::Client::new(),
        }
    }

    async fn post(&self, key: Option<&str>, path: &str, body: Value) -> reqwest::Response {
        let mut request = self.http.post(format!("{}{path}", self.url)).json(&body);
        if let Some(key) = key {
            request = request.header("x-hub-access-key", key);
        }
        request.send().await.expect("request is sent")
    }

    async fn get(&self, key: &str, path: &str) -> reqwest::Response {
        self.http
            .get(format!("{}{path}", self.url))
            .header("x-hub-access-key", key)
            .send()
            .await
            .expect("request is sent")
    }

    async fn json(&self, response: reqwest::Response) -> Value {
        assert_eq!(response.status(), StatusCode::OK, "unexpected status");
        response.json().await.expect("json body")
    }

    /// Run one `/api/v1/command` envelope for the tenant holding `key`.
    async fn command(&self, key: &str, command: &str, payload: Value) -> Value {
        let response = self
            .post(
                Some(key),
                "/api/v1/command",
                json!({"command": command, "request_id": "r1", "payload": payload}),
            )
            .await;
        self.json(response).await
    }

    fn ws_url(&self, path: &str) -> String {
        format!("{}{path}", self.url.replacen("http://", "ws://", 1))
    }
}

fn scoped(tenant: &Uuid, name: &str) -> String {
    ResourceNaming::format(tenant, name)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_and_invalid_hub_keys_are_refused() {
    let server = Server::start(mock_hub()).await;
    let body = json!({"key": "config", "value": "x"});

    let missing = server.post(None, "/kv/set", body.clone()).await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let invalid = server.post(Some("bad"), "/kv/set", body.clone()).await;
    assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);

    let invalid_bearer = server
        .http
        .post(format!("{}/kv/set", server.url))
        .header("Authorization", "Bearer bad")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(invalid_bearer.status(), StatusCode::UNAUTHORIZED);

    // Local credentials alone are not a Hub key.
    let local = server
        .http
        .post(format!("{}/kv/set", server.url))
        .basic_auth("root", Some("root"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(local.status(), StatusCode::UNAUTHORIZED);

    // Every surface is behind the same gate.
    for path in ["/api/v1/command", "/mcp", "/umicp"] {
        let response = server.post(None, path, json!({})).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
    let socket = tokio_tungstenite::connect_async(server.ws_url("/kv/ws?keys=config")).await;
    assert!(socket.is_err(), "a WebSocket upgrade needs a Hub key too");

    // Liveness and metrics stay open.
    for path in ["/health", "/metrics"] {
        let response = server
            .http
            .get(format!("{}{path}", server.url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }

    assert_eq!(server.state.kv_store.get("config").await.unwrap(), None);
    for tenant in [tenant_a(), tenant_b()] {
        assert_eq!(
            server
                .state
                .kv_store
                .get(&scoped(&tenant, "config"))
                .await
                .unwrap(),
            None
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rest_keeps_each_tenants_data_apart() {
    let server = Server::start(mock_hub()).await;
    for (key, value) in [(KEY_A, "a"), (KEY_B, "b")] {
        let response = server
            .post(
                Some(key),
                "/kv/set",
                json!({"key": "config", "value": value}),
            )
            .await;
        server.json(response).await;
    }
    for (key, value) in [(KEY_A, "a"), (KEY_B, "b")] {
        let response = server.get(key, "/kv/get/config").await;
        assert_eq!(server.json(response).await, json!(value));
    }
    let kv = &server.state.kv_store;
    assert_eq!(kv.get("config").await.unwrap(), None, "no raw key");
    assert_eq!(
        kv.get(&scoped(&tenant_a(), "config")).await.unwrap(),
        Some(b"a".to_vec())
    );

    // Sorted sets, including the store commands' destination and sources.
    let response = server
        .post(
            Some(KEY_A),
            "/sortedset/board/zadd",
            json!({"member": "alice", "score": 1.0}),
        )
        .await;
    server.json(response).await;
    let zcard = |key| server.get(key, "/sortedset/board/zcard");
    assert_eq!(server.json(zcard(KEY_A).await).await["count"], 1);
    assert_eq!(server.json(zcard(KEY_B).await).await["count"], 0);
    let union = server
        .post(
            Some(KEY_B),
            "/sortedset/zunionstore",
            json!({"destination": "copy", "keys": ["board"]}),
        )
        .await;
    assert_eq!(
        server.json(union).await["count"],
        0,
        "B unions its own board"
    );
    let union = server
        .post(
            Some(KEY_A),
            "/sortedset/zunionstore",
            json!({"destination": "copy", "keys": ["board"]}),
        )
        .await;
    assert_eq!(server.json(union).await["count"], 1);
    assert_eq!(
        server
            .state
            .sorted_set_store
            .zcard(&scoped(&tenant_a(), "copy")),
        1
    );
    assert_eq!(server.state.sorted_set_store.zcard("copy"), 0);

    // Geospatial indexes.
    let response = server
        .post(
            Some(KEY_A),
            "/geospatial/places/geoadd",
            json!({"locations": [{"lat": 10.0, "lon": 20.0, "member": "home"}]}),
        )
        .await;
    server.json(response).await;
    let geopos = |key| {
        server.post(
            Some(key),
            "/geospatial/places/geopos",
            json!({"members": ["home"]}),
        )
    };
    assert!(!server.json(geopos(KEY_A).await).await["coordinates"][0].is_null());
    assert!(server.json(geopos(KEY_B).await).await["coordinates"][0].is_null());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_envelope_keeps_each_tenants_data_apart() {
    let server = Server::start(mock_hub()).await;
    for (key, value) in [(KEY_A, "env-a"), (KEY_B, "env-b")] {
        let reply = server
            .command(key, "kv.set", json!({"key": "config", "value": value}))
            .await;
        assert_eq!(reply["success"], true, "{reply}");
    }
    for (key, value) in [(KEY_A, "env-a"), (KEY_B, "env-b")] {
        let reply = server
            .command(key, "kv.get", json!({"key": "config"}))
            .await;
        assert_eq!(reply["payload"], json!(value), "{reply}");
    }
    assert_eq!(server.state.kv_store.get("config").await.unwrap(), None);

    // Keyspace-wide commands and scripts cannot be confined to a tenant;
    // admin commands are not even granted to one.
    for (command, payload, reason) in [
        ("kv.keys", json!({"pattern": "*"}), "Hub mode"),
        (
            "script.eval",
            json!({"script": "return 1", "keys": ["config"]}),
            "Hub mode",
        ),
        ("kv.flushall", json!({}), "admin"),
        ("info", json!({}), "admin"),
    ] {
        let reply = server.command(KEY_A, command, payload).await;
        assert_eq!(reply["success"], false, "{command}: {reply}");
        let error = reply["error"].as_str().unwrap();
        assert!(
            error.contains("NOPERM") && error.contains(reason),
            "{command}: {reply}"
        );
    }

    // A transaction belongs to its tenant: B cannot run A's, even by id.
    let multi = server
        .command(KEY_A, "transaction.multi", json!({"client_id": "tx"}))
        .await;
    assert_eq!(multi["success"], true, "{multi}");
    let queued = server
        .command(
            KEY_A,
            "kv.set",
            json!({"key": "pending", "value": "1", "client_id": "tx"}),
        )
        .await;
    assert_eq!(queued["payload"]["queued"], true, "{queued}");
    let hijack = server
        .command(KEY_B, "transaction.exec", json!({"client_id": "tx"}))
        .await;
    assert_eq!(hijack["success"], false, "{hijack}");
    let exec = server
        .command(KEY_A, "transaction.exec", json!({"client_id": "tx"}))
        .await;
    assert_eq!(exec["success"], true, "{exec}");
    let mine = server
        .command(KEY_A, "kv.get", json!({"key": "pending"}))
        .await;
    assert_eq!(mine["payload"], json!("1"));
    let theirs = server
        .command(KEY_B, "kv.get", json!({"key": "pending"}))
        .await;
    assert_eq!(theirs["payload"], Value::Null);
}

/// A minimal Streamable HTTP MCP client (as in `mcp_identity_tests.rs`).
struct McpSession {
    http: reqwest::Client,
    url: String,
    key: String,
    session: String,
    next_id: i64,
}

impl McpSession {
    async fn open(base: &str, key: &str) -> Self {
        let http = reqwest::Client::new();
        let url = format!("{base}/mcp");
        let response = http
            .post(&url)
            .header("Authorization", format!("Bearer {key}"))
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "initialize",
                "params": {
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "hub-test", "version": "1"}
                }
            }))
            .send()
            .await
            .expect("initialize is sent");
        assert_eq!(response.status(), StatusCode::OK, "initialize");
        let session = response
            .headers()
            .get("mcp-session-id")
            .expect("session id")
            .to_str()
            .expect("ascii session id")
            .to_string();
        read_message(response, 0).await;
        let client = Self {
            http,
            url,
            key: key.to_string(),
            session,
            next_id: 1,
        };
        let initialized = client
            .post(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        assert!(initialized.status().is_success(), "initialized");
        client
    }

    async fn post(&self, body: Value) -> reqwest::Response {
        self.http
            .post(&self.url)
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Accept", "application/json, text/event-stream")
            .header("mcp-session-id", &self.session)
            .header("mcp-protocol-version", PROTOCOL_VERSION)
            .json(&body)
            .send()
            .await
            .expect("request is sent")
    }

    /// The text of a tool's first content block, or the error message.
    async fn call(&mut self, tool: &str, arguments: Value) -> Result<String, String> {
        let id = self.next_id;
        self.next_id += 1;
        let response = self
            .post(json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {"name": tool, "arguments": arguments}
            }))
            .await;
        assert_eq!(response.status(), StatusCode::OK, "tools/call {tool}");
        let message = read_message(response, id).await;
        match message.get("error") {
            Some(error) => Err(error["message"].as_str().unwrap_or_default().to_string()),
            None => Ok(message["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string()),
        }
    }
}

/// The JSON-RPC message answering `id`, from a JSON or an SSE response.
async fn read_message(mut response: reqwest::Response, id: i64) -> Value {
    let is_sse = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/event-stream"));
    if !is_sse {
        return response.json().await.expect("json response");
    }
    let mut pending = String::new();
    while let Some(chunk) = response.chunk().await.expect("sse chunk") {
        pending.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = pending.find('\n') {
            let line = pending[..end].trim_end_matches('\r').to_string();
            pending.drain(..=end);
            if let Some(data) = line.strip_prefix("data:")
                && let Ok(message) = serde_json::from_str::<Value>(data.trim())
                && message.get("id") == Some(&json!(id))
            {
                return message;
            }
        }
    }
    panic!("stream ended without a reply to {id}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_keeps_each_tenants_data_apart() {
    let server = Server::start(mock_hub()).await;
    let mut a = McpSession::open(&server.url, KEY_A).await;
    let mut b = McpSession::open(&server.url, KEY_B).await;

    a.call("synap_kv_set", json!({"key": "config", "value": "mcp-a"}))
        .await
        .expect("A writes");
    b.call("synap_kv_set", json!({"key": "config", "value": "mcp-b"}))
        .await
        .expect("B writes");

    assert_eq!(
        a.call("synap_kv_get", json!({"key": "config"}))
            .await
            .unwrap(),
        "mcp-a"
    );
    assert_eq!(
        b.call("synap_kv_get", json!({"key": "config"}))
            .await
            .unwrap(),
        "mcp-b"
    );
    assert_eq!(server.state.kv_store.get("config").await.unwrap(), None);
    assert_eq!(
        server
            .state
            .kv_store
            .get(&scoped(&tenant_b(), "config"))
            .await
            .unwrap(),
        Some(b"mcp-b".to_vec())
    );

    // A transaction tool without an id could not be addressed again.
    let refused = a.call("synap_transaction_multi", json!({})).await;
    assert!(refused.unwrap_err().contains("Hub mode"));
}

async fn umicp_call(server: &Server, key: &str, operation: &str, arguments: Value) -> String {
    let mut builder = Envelope::builder()
        .from("hub-test")
        .to("synap")
        .operation(OperationType::Request)
        .capability("operation", json!(operation));
    if let Value::Object(map) = arguments {
        for (name, value) in map {
            builder = builder.capability(&name, value);
        }
    }
    let envelope = builder.build().expect("envelope builds");
    let response = server
        .http
        .post(format!("{}/umicp", server.url))
        .header("Authorization", format!("Bearer {key}"))
        .body(envelope.serialize().expect("envelope serializes"))
        .send()
        .await
        .expect("umicp responds");
    assert_eq!(response.status(), StatusCode::OK);
    response.text().await.expect("umicp body")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn umicp_keeps_each_tenants_data_apart() {
    let server = Server::start(mock_hub()).await;
    for (key, value) in [(KEY_A, "umicp-a"), (KEY_B, "umicp-b")] {
        let reply = umicp_call(
            &server,
            key,
            "synap_kv_set",
            json!({"key": "config", "value": value}),
        )
        .await;
        assert!(reply.contains("success"), "{reply}");
    }
    let a = umicp_call(&server, KEY_A, "synap_kv_get", json!({"key": "config"})).await;
    let b = umicp_call(&server, KEY_B, "synap_kv_get", json!({"key": "config"})).await;
    assert!(a.contains("umicp-a") && !a.contains("umicp-b"), "{a}");
    assert!(b.contains("umicp-b") && !b.contains("umicp-a"), "{b}");
    assert_eq!(server.state.kv_store.get("config").await.unwrap(), None);
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Open a WebSocket as the tenant holding `key` and consume its welcome frame.
async fn open_socket(server: &Server, key: &str, path: &str) -> (Socket, Value) {
    let mut request = server.ws_url(path).into_client_request().unwrap();
    request
        .headers_mut()
        .insert("x-hub-access-key", HeaderValue::from_str(key).unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("the socket connects");
    let welcome = next_frame(&mut socket, "connected").await;
    (socket, welcome)
}

/// The next frame of `kind` (`type` field), within two seconds.
async fn next_frame(socket: &mut Socket, kind: &str) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(Ok(frame)) = socket.next().await {
            if let Message::Text(text) = frame {
                let frame: Value = serde_json::from_str(&text).unwrap();
                if frame["type"] == kind {
                    return frame;
                }
            }
        }
        panic!("the socket closed before a `{kind}` frame");
    })
    .await
    .unwrap_or_else(|_| panic!("no `{kind}` frame within the timeout"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_kv_watch_sees_only_the_tenants_own_key() {
    let server = Server::start(mock_hub()).await;
    let (mut a, welcome) = open_socket(&server, KEY_A, "/kv/ws?keys=config").await;
    assert_eq!(
        welcome["topics"][0],
        format!("__watch@0__:{}", scoped(&tenant_a(), "config"))
    );
    let (mut b, _) = open_socket(&server, KEY_B, "/kv/ws?keys=config").await;

    // B writes first: if A's socket saw B's key, its first event would be B's.
    for (key, value) in [(KEY_B, "ws-b"), (KEY_A, "ws-a")] {
        let response = server
            .post(
                Some(key),
                "/kv/set",
                json!({"key": "config", "value": value}),
            )
            .await;
        server.json(response).await;
    }
    assert_eq!(
        next_frame(&mut a, "message").await["payload"]["value"],
        "ws-a"
    );
    assert_eq!(
        next_frame(&mut b, "message").await["payload"]["value"],
        "ws-b"
    );
    a.close(None).await.ok();
    b.close(None).await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_pubsub_sees_only_the_tenants_own_topic() {
    let server = Server::start(mock_hub()).await;
    let (mut a, _) = open_socket(&server, KEY_A, "/pubsub/ws?topics=news").await;
    let (mut b, _) = open_socket(&server, KEY_B, "/pubsub/ws?topics=news").await;

    for (key, payload) in [(KEY_B, "from-b"), (KEY_A, "from-a")] {
        let response = server
            .post(
                Some(key),
                "/pubsub/news/publish",
                json!({"payload": payload}),
            )
            .await;
        server.json(response).await;
    }
    assert_eq!(next_frame(&mut a, "message").await["payload"], "from-a");
    assert_eq!(next_frame(&mut b, "message").await["payload"], "from-b");
    a.close(None).await.ok();
    b.close(None).await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_queue_and_stream_see_only_the_tenants_own_data() {
    let server = Server::start(mock_hub()).await;
    for (key, byte, event) in [(KEY_A, 1, "event-a"), (KEY_B, 2, "event-b")] {
        server
            .json(server.post(Some(key), "/queue/jobs", json!({})).await)
            .await;
        let published = server
            .post(Some(key), "/queue/jobs/publish", json!({"payload": [byte]}))
            .await;
        server.json(published).await;
        server
            .json(server.post(Some(key), "/stream/room", json!({})).await)
            .await;
        let published = server
            .post(
                Some(key),
                "/stream/room/publish",
                json!({"event": event, "data": {}}),
            )
            .await;
        server.json(published).await;
    }

    for (key, byte, event) in [(KEY_A, 1, "event-a"), (KEY_B, 2, "event-b")] {
        let (mut queue, _) = open_socket(&server, key, "/queue/jobs/ws/consumer").await;
        assert_eq!(
            next_frame(&mut queue, "message").await["payload"],
            json!([byte])
        );
        queue.close(None).await.ok();

        let (mut stream, _) = open_socket(&server, key, "/stream/room/ws/subscriber").await;
        assert_eq!(next_frame(&mut stream, "event").await["event"], event);
        stream.close(None).await.ok();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn routes_without_tenant_scoping_answer_403() {
    let server = Server::start(mock_hub()).await;
    for path in [
        "/info",
        "/kv/stats",
        "/slowlog",
        "/clients",
        "/memory/config/usage",
        "/topics",
        "/cluster/info",
        "/auth/me",
    ] {
        let response = server.get(KEY_A, path).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "GET {path}");
    }
    for path in [
        "/script/eval",
        "/transaction/multi",
        "/snapshot",
        "/topics/t/publish",
    ] {
        let response = server
            .post(Some(KEY_A), path, json!({"script": "return 1"}))
            .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "POST {path}");
    }
    // Without a key the answer is still 401: credentials are checked first.
    let response = server.post(None, "/info", json!({})).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// The production validator cannot validate any key yet (hivellm/synap#230),
/// so a Hub deployment refuses every data request instead of guessing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_a_validation_endpoint_hub_mode_fails_closed() {
    let server = Server::start(HubClient::new(hub_config()).expect("hub client")).await;
    let response = server
        .post(
            Some("sk_live_anything"),
            "/kv/set",
            json!({"key": "config", "value": "x"}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = response.text().await.unwrap();
    assert!(body.contains("hivellm/synap#230"), "{body}");
    assert_eq!(server.state.kv_store.get("config").await.unwrap(), None);

    let health = server
        .http
        .get(format!("{}/health", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
}
