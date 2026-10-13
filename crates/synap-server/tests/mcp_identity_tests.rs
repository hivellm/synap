//! MCP and UMICP tool calls are authorized with the identity of their own
//! request (review finding S-04).
//!
//! The MCP side speaks the real Streamable HTTP protocol — `initialize`, the
//! `mcp-session-id` it returns, `tools/call` — against the production router,
//! so the identity has to survive rmcp's own session tasks.

mod test_helper;

use std::net::SocketAddr;
use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose};
use reqwest::StatusCode;
use serde_json::{Value, json};
use synap_server::AppState;
use synap_server::auth::{Action, ApiKeyManager, Permission, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::server::router::create_router;
use umicp_core::{Envelope, OperationType};

const PASSWORD: &str = "identity-password-1";
const PROTOCOL_VERSION: &str = "2025-06-18";

/// Bearer credentials for the high-volume test: API keys are checked without
/// the bcrypt work a password costs on every request.
struct Keys {
    full: String,
    read_only: String,
}

/// `boss` is an admin, `reader` holds the built-in read-only role.
async fn deploy() -> (String, AppState, Keys) {
    let users = UserManager::new();
    users
        .create_user("boss", PASSWORD, true)
        .expect("admin is created");
    users
        .create_user("reader", PASSWORD, false)
        .expect("reader is created");
    users
        .add_user_role("reader", "readonly")
        .expect("role is granted");
    let users = Arc::new(users);

    let mut state = test_helper::create_test_app_state();
    state.user_manager = Some(Arc::clone(&users));
    state.require_auth = true;

    let api_keys = Arc::new(ApiKeyManager::new());
    let keys = Keys {
        full: api_keys
            .create(
                "full",
                Some("boss".to_string()),
                vec![Permission::new("*", Action::All)],
                vec![],
                None,
            )
            .expect("key is created")
            .key,
        read_only: api_keys
            .create(
                "read-only",
                Some("reader".to_string()),
                vec![Permission::new("kv:*", Action::Read)],
                vec![],
                None,
            )
            .expect("key is created")
            .key,
    };

    let router = create_router(
        state.clone(),
        RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        McpConfig::default(),
        users,
        api_keys,
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
    (url, state, keys)
}

fn basic(user: &str) -> String {
    let encoded = general_purpose::STANDARD.encode(format!("{user}:{PASSWORD}"));
    format!("Basic {encoded}")
}

/// A minimal Streamable HTTP MCP client.
struct McpSession {
    http: reqwest::Client,
    url: String,
    authorization: String,
    session: String,
    next_id: i64,
}

impl McpSession {
    async fn open(base: &str, authorization: &str) -> Self {
        let http = reqwest::Client::new();
        let url = format!("{base}/mcp");
        let authorization = authorization.to_string();
        let init = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "identity-test", "version": "1"}
            }
        });
        let response = http
            .post(&url)
            .header("Authorization", &authorization)
            .header("Accept", "application/json, text/event-stream")
            .json(&init)
            .send()
            .await
            .expect("initialize is sent");
        assert_eq!(response.status(), StatusCode::OK, "initialize");
        let session = response
            .headers()
            .get("mcp-session-id")
            .expect("legacy session id")
            .to_str()
            .expect("ascii session id")
            .to_string();
        read_message(response, 0).await;

        let client = Self {
            http,
            url,
            authorization,
            session,
            next_id: 1,
        };
        let initialized = client
            .post(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        assert!(
            initialized.status().is_success(),
            "initialized notification"
        );
        client
    }

    async fn post(&self, body: Value) -> reqwest::Response {
        self.post_as(&self.authorization, body).await
    }

    async fn post_as(&self, authorization: &str, body: Value) -> reqwest::Response {
        self.http
            .post(&self.url)
            .header("Authorization", authorization)
            .header("Accept", "application/json, text/event-stream")
            .header("mcp-session-id", &self.session)
            .header("mcp-protocol-version", PROTOCOL_VERSION)
            .json(&body)
            .send()
            .await
            .expect("request is sent")
    }

    /// Call a tool; `Ok(result)` or `Err(error message)`.
    async fn call(&mut self, tool: &str, arguments: Value) -> Result<Value, String> {
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
            None => Ok(message["result"].clone()),
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

/// Admin and read-only sessions call tools concurrently on a multi-threaded
/// runtime: rmcp runs each session in its own task, so a thread-bound
/// identity would leak between them. Each call must be judged by its own
/// credentials.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn interleaved_sessions_keep_their_own_permissions() {
    let (url, state, keys) = deploy().await;
    let boss = McpSession::open(&url, &format!("Bearer {}", keys.full)).await;
    let reader = McpSession::open(&url, &format!("Bearer {}", keys.read_only)).await;

    let run = |mut session: McpSession, prefix: &'static str| {
        tokio::spawn(async move {
            let mut outcomes = Vec::new();
            for i in 0..25 {
                let key = format!("{prefix}:{i}");
                outcomes.push(
                    session
                        .call("synap_kv_set", json!({"key": key, "value": "v"}))
                        .await,
                );
                tokio::task::yield_now().await;
            }
            outcomes
        })
    };
    let (boss_outcomes, reader_outcomes) = tokio::join!(run(boss, "boss"), run(reader, "reader"));

    for outcome in boss_outcomes.expect("admin task") {
        assert!(outcome.is_ok(), "admin call failed: {outcome:?}");
    }
    for outcome in reader_outcomes.expect("reader task") {
        let message = outcome.expect_err("read-only call must be denied");
        assert!(message.contains("NOPERM"), "{message}");
    }
    assert_eq!(
        state.kv_store.get("boss:24").await.unwrap(),
        Some(b"v".to_vec())
    );
    assert_eq!(state.kv_store.get("reader:0").await.unwrap(), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_session_cannot_be_reused_under_other_credentials() {
    let (url, state, _) = deploy().await;
    let boss = McpSession::open(&url, &basic("boss")).await;

    let hijack = boss
        .post_as(
            &basic("reader"),
            json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "tools/call",
                "params": {"name": "synap_kv_set", "arguments": {"key": "stolen", "value": "x"}}
            }),
        )
        .await;

    assert_eq!(hijack.status(), StatusCode::FORBIDDEN);
    assert_eq!(state.kv_store.get("stolen").await.unwrap(), None);
}

async fn umicp_call(url: &str, user: &str, operation: &str, arguments: Value) -> String {
    let mut builder = Envelope::builder()
        .from("identity-test")
        .to("synap")
        .operation(OperationType::Request)
        .capability("operation", json!(operation));
    if let Value::Object(map) = arguments {
        for (key, value) in map {
            builder = builder.capability(&key, value);
        }
    }
    let envelope = builder.build().expect("envelope builds");
    reqwest::Client::new()
        .post(format!("{url}/umicp"))
        .header("Authorization", basic(user))
        .body(envelope.serialize().expect("envelope serializes"))
        .send()
        .await
        .expect("umicp responds")
        .text()
        .await
        .expect("umicp body")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_read_only_umicp_caller_cannot_write_or_delete() {
    let (url, state, _) = deploy().await;
    state
        .kv_store
        .set("kept", b"original".to_vec(), None)
        .await
        .expect("seed");

    let read = umicp_call(&url, "reader", "synap_kv_get", json!({"key": "kept"})).await;
    assert!(read.contains("original"), "{read}");

    let write = umicp_call(
        &url,
        "reader",
        "synap_kv_set",
        json!({"key": "kept", "value": "overwritten"}),
    )
    .await;
    assert!(write.contains("NOPERM"), "{write}");

    let delete = umicp_call(&url, "reader", "synap_kv_delete", json!({"key": "kept"})).await;
    assert!(delete.contains("NOPERM"), "{delete}");

    assert_eq!(
        state.kv_store.get("kept").await.unwrap(),
        Some(b"original".to_vec())
    );

    let admin = umicp_call(
        &url,
        "boss",
        "synap_kv_set",
        json!({"key": "kept", "value": "by-admin"}),
    )
    .await;
    assert!(!admin.contains("NOPERM"), "{admin}");
    assert_eq!(
        state.kv_store.get("kept").await.unwrap(),
        Some(b"by-admin".to_vec())
    );
}
