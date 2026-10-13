//! Configured request limits are enforced (review findings S-06 and S-07).
//!
//! Rate limiting: the router installs the limiter when `rate_limit.enabled`,
//! keyed by peer address before authentication and by principal after it,
//! answering 429 with `Retry-After`; `/health` and `/metrics` are exempt.
//!
//! UMICP: `POST /umicp` reads at most `mcp.umicp_max_body_bytes` and answers
//! 413 above it, without buffering past the limit.
//!
//! Most tests drive the production router in-process with the peer address
//! `axum::serve(.., into_make_service_with_connect_info)` would attach, which
//! lets them speak for several clients from one machine. The over-the-wire
//! tests serve it on a real socket the way `main` does.

mod test_helper;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use serde_json::json;
use synap_server::AppState;
use synap_server::auth::{Action, ApiKeyManager, Permission, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::server::router::create_router;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use umicp_core::{Envelope, OperationType};

const PASSWORD: &str = "limits-password-1";
const MIB: usize = 1024 * 1024;

fn rate_limit(requests_per_second: u64, burst_size: u64) -> RateLimitConfig {
    RateLimitConfig {
        enabled: true,
        requests_per_second,
        burst_size,
    }
}

/// The production router without authentication.
fn open_router(limits: RateLimitConfig, mcp: McpConfig) -> (Router, AppState) {
    let state = test_helper::create_test_app_state();
    let router = create_router(
        state.clone(),
        limits,
        mcp,
        Arc::new(UserManager::new()),
        Arc::new(ApiKeyManager::new()),
        false,
        false,
    );
    (router, state)
}

/// The production router requiring authentication, with user `alice` (password
/// [`PASSWORD`]) and two full-access API keys that belong to no user.
fn authenticated_router(limits: RateLimitConfig) -> (Router, [String; 2]) {
    let users = UserManager::new();
    users
        .create_user("alice", PASSWORD, true)
        .expect("alice is created");
    let users = Arc::new(users);
    let mut state = test_helper::create_test_app_state();
    state.user_manager = Some(Arc::clone(&users));
    state.require_auth = true;

    let api_keys = Arc::new(ApiKeyManager::new());
    let key = |name: &str| {
        api_keys
            .create(
                name,
                None,
                vec![Permission::new("*", Action::All)],
                vec![],
                None,
            )
            .expect("key is created")
            .key
    };
    let keys = [key("first"), key("second")];
    let router = create_router(
        state,
        limits,
        McpConfig::default(),
        users,
        Arc::clone(&api_keys),
        true,
        true,
    );
    (router, keys)
}

fn peer(last_octet: u8) -> SocketAddr {
    SocketAddr::from(([10, 0, 0, last_octet], 40_000))
}

/// Send `request` through `router` as if it arrived from `from`.
async fn send_from(router: &Router, from: SocketAddr, mut request: Request<Body>) -> Response {
    request.extensions_mut().insert(ConnectInfo(from));
    router
        .clone()
        .oneshot(request)
        .await
        .expect("the router is infallible")
}

fn get(path: &str) -> Request<Body> {
    Request::get(path)
        .body(Body::empty())
        .expect("request builds")
}

fn get_with(path: &str, authorization: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, authorization)
        .body(Body::empty())
        .expect("request builds")
}

fn basic(user: &str, password: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    format!(
        "Basic {}",
        general_purpose::STANDARD.encode(format!("{user}:{password}"))
    )
}

/// The `Retry-After` of a 429, which must be a whole number of seconds >= 1.
fn retry_after(response: &Response) -> u64 {
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let seconds: u64 = response
        .headers()
        .get(header::RETRY_AFTER)
        .expect("a 429 carries Retry-After")
        .to_str()
        .expect("Retry-After is ASCII")
        .parse()
        .expect("Retry-After is whole seconds");
    assert!(
        seconds >= 1,
        "Retry-After {seconds} invites an instant retry"
    );
    seconds
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_beyond_the_allowance_answers_429_with_retry_after() {
    // The spec scenario, over a real socket served the way `main` serves it.
    let (router, _) = open_router(rate_limit(10, 5), McpConfig::default());
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

    let client = reqwest::Client::new();
    let started = Instant::now();
    let responses = futures_util::future::join_all((0..20).map(|_| {
        let client = client.clone();
        let url = format!("{url}/kv/get/missing");
        async move { client.get(url).send().await.expect("request is sent") }
    }))
    .await;
    let elapsed = started.elapsed().as_secs_f64();

    let mut admitted = 0;
    for response in &responses {
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let seconds: u64 = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .expect("a 429 carries Retry-After")
                .to_str()
                .expect("Retry-After is ASCII")
                .parse()
                .expect("Retry-After is whole seconds");
            assert!(seconds >= 1);
        } else {
            admitted += 1;
        }
    }
    // The burst, plus whatever refilled at 10/s while the requests ran.
    let allowance = 5.0 + (elapsed * 10.0).floor();
    assert!(admitted >= 5, "the burst is admitted, got {admitted}");
    assert!(
        f64::from(admitted) <= allowance,
        "{admitted} admitted, allowance {allowance} after {elapsed:.3} s"
    );
    assert!(admitted < 20, "the rest are limited");
}

#[tokio::test]
async fn capacity_recovers_after_the_refill_interval() {
    let (router, _) = open_router(rate_limit(10, 1), McpConfig::default());
    let client = peer(1);

    assert_ne!(
        send_from(&router, client, get("/kv/get/missing"))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    let limited = send_from(&router, client, get("/kv/get/missing")).await;
    assert_eq!(retry_after(&limited), 1, "a token is 100 ms away at 10/s");

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_ne!(
        send_from(&router, client, get("/kv/get/missing"))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS,
        "one refill interval later the client is served again"
    );
}

#[tokio::test]
async fn clients_are_limited_independently() {
    let (router, _) = open_router(rate_limit(1, 2), McpConfig::default());
    let (a, b) = (peer(1), peer(2));

    for _ in 0..2 {
        assert_ne!(
            send_from(&router, a, get("/kv/get/missing")).await.status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    retry_after(&send_from(&router, a, get("/kv/get/missing")).await);

    assert_ne!(
        send_from(&router, b, get("/kv/get/missing")).await.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "client B is served while A is limited"
    );
}

#[tokio::test]
async fn health_and_metrics_are_exempt() {
    let (router, _) = open_router(rate_limit(1, 1), McpConfig::default());
    let client = peer(1);
    send_from(&router, client, get("/kv/get/missing")).await;
    retry_after(&send_from(&router, client, get("/kv/get/missing")).await);

    for _ in 0..5 {
        assert_eq!(
            send_from(&router, client, get("/health")).await.status(),
            StatusCode::OK,
            "a limited client is still answered on /health"
        );
        assert_ne!(
            send_from(&router, client, get("/metrics")).await.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "/metrics is not rate limited"
        );
    }
}

#[tokio::test]
async fn authentication_attempts_are_limited_before_credentials_are_checked() {
    let (router, _) = authenticated_router(rate_limit(1, 2));
    let attacker = peer(1);

    // Two failed attempts spend the budget of the address...
    for _ in 0..2 {
        let refused = send_from(
            &router,
            attacker,
            get_with("/kv/get/missing", "Bearer sk_not_a_real_key"),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
    }
    // ...so further attempts are refused before the password is verified:
    // a wrong password is not answered 401, nor the right one served.
    for password in ["wrong-password", PASSWORD] {
        let limited = send_from(
            &router,
            attacker,
            get_with("/kv/get/missing", &basic("alice", password)),
        )
        .await;
        retry_after(&limited);
    }

    // Another address still authenticates.
    let served = send_from(
        &router,
        peer(2),
        get_with("/kv/get/missing", &basic("alice", PASSWORD)),
    )
    .await;
    assert_ne!(served.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_ne!(served.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_principal_is_limited_across_addresses() {
    let (router, [first, second]) = authenticated_router(rate_limit(1, 3));
    let first = format!("Bearer {first}");

    // Three addresses, one token each from their own buckets, but all three
    // from the principal's.
    for client in 1..=3 {
        let served = send_from(&router, peer(client), get_with("/kv/get/missing", &first)).await;
        assert_ne!(served.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_ne!(served.status(), StatusCode::UNAUTHORIZED);
    }
    let limited = send_from(&router, peer(4), get_with("/kv/get/missing", &first)).await;
    retry_after(&limited);

    // Another principal at that address is served.
    let other = send_from(
        &router,
        peer(4),
        get_with("/kv/get/missing", &format!("Bearer {second}")),
    )
    .await;
    assert_ne!(other.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_ne!(other.status(), StatusCode::UNAUTHORIZED);
}

/// Log output captured from a scoped subscriber.
#[derive(Clone, Default)]
struct CapturedLogs(Arc<parking_lot::Mutex<Vec<u8>>>);

impl std::io::Write for CapturedLogs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLogs {
    type Writer = CapturedLogs;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// The rate limiting lines logged while the router is built.
fn rate_limit_log_lines(limits: RateLimitConfig) -> Vec<String> {
    let logs = CapturedLogs::default();
    let capture = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_writer(logs.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    );
    // tracing-core decides whether a log statement is wanted when a thread
    // first reaches it. With a single dispatcher registered it asks only that
    // thread's default, which on the other test threads is no subscriber at
    // all, and the statement would then be skipped here too. A second live
    // dispatcher makes it ask every registered one, this capture included.
    let _second = tracing::Dispatch::new(tracing_subscriber::registry());
    tracing::dispatcher::with_default(&capture, || {
        let (_router, _state) = open_router(limits, McpConfig::default());
    });
    let output = logs.0.lock().clone();
    String::from_utf8(output)
        .expect("logs are UTF-8")
        .lines()
        .filter(|line| line.to_lowercase().contains("rate limit"))
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn startup_logs_say_whether_rate_limiting_is_enforced() {
    let disabled = rate_limit_log_lines(RateLimitConfig {
        enabled: false,
        requests_per_second: 1000,
        burst_size: 100,
    });
    assert!(
        disabled.iter().any(|line| line.contains("disabled")),
        "{disabled:?}"
    );
    for line in &disabled {
        let line = line.to_lowercase();
        assert!(
            !line.contains("enabled") && !line.contains("enforced"),
            "a disabled limiter must not be reported as active: {line}"
        );
    }

    let enabled = rate_limit_log_lines(rate_limit(1000, 100));
    assert!(
        enabled
            .iter()
            .any(|line| line.contains("Rate limiting enforced: 1000 req/s, burst 100")),
        "{enabled:?}"
    );
}

/// UMICP config with a 1 MiB body limit.
fn umicp_limited() -> McpConfig {
    McpConfig {
        umicp_max_body_bytes: MIB,
        ..McpConfig::default()
    }
}

/// A `size`-byte body streamed in 64 KiB chunks, counting what is pulled.
fn counted_stream(size: usize, pulled: Arc<AtomicUsize>) -> Body {
    const CHUNK: usize = 64 * 1024;
    let chunks = size / CHUNK;
    let stream = futures_util::stream::iter((0..chunks).map(move |_| {
        pulled.fetch_add(CHUNK, Ordering::SeqCst);
        Ok::<_, Infallible>(Bytes::from(vec![b'x'; CHUNK]))
    }));
    Body::from_stream(stream)
}

fn umicp_post(body: Body) -> Request<Body> {
    Request::post("/umicp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .expect("request builds")
}

#[tokio::test]
async fn an_oversized_chunked_umicp_body_is_refused_without_reading_it_all() {
    let (router, _) = open_router(
        RateLimitConfig {
            enabled: false,
            ..rate_limit(1, 1)
        },
        umicp_limited(),
    );
    let pulled = Arc::new(AtomicUsize::new(0));
    let response = send_from(
        &router,
        peer(1),
        umicp_post(counted_stream(50 * MIB, Arc::clone(&pulled))),
    )
    .await;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let pulled = pulled.load(Ordering::SeqCst);
    assert!(
        pulled <= MIB + 64 * 1024,
        "{pulled} bytes read: the limit plus the chunk that crossed it at most"
    );
}

#[tokio::test]
async fn an_oversized_declared_umicp_body_is_refused_before_it_is_read() {
    let (router, _) = open_router(
        RateLimitConfig {
            enabled: false,
            ..rate_limit(1, 1)
        },
        umicp_limited(),
    );
    let pulled = Arc::new(AtomicUsize::new(0));
    let mut request = umicp_post(counted_stream(50 * MIB, Arc::clone(&pulled)));
    request
        .headers_mut()
        .insert(header::CONTENT_LENGTH, (50 * MIB).into());
    let response = send_from(&router, peer(1), request).await;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(pulled.load(Ordering::SeqCst), 0, "nothing of it is read");
}

#[tokio::test]
async fn an_oversized_declared_umicp_body_is_refused_over_the_wire() {
    // The client declares 50 MiB and sends none of it: the server answers on
    // the headers alone, so it cannot have been waiting for the body.
    let (router, _) = open_router(
        RateLimitConfig {
            enabled: false,
            ..rate_limit(1, 1)
        },
        umicp_limited(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("http binds");
    let addr = listener.local_addr().expect("http addr");
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("http serves");
    });

    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connects");
    let head = format!(
        "POST /umicp HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n",
        50 * MIB
    );
    stream
        .write_all(head.as_bytes())
        .await
        .expect("headers are sent");
    let mut reply = [0u8; 64];
    let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut reply))
        .await
        .expect("the server answers without the body")
        .expect("the reply is read");
    let status_line = String::from_utf8_lossy(&reply[..read]);
    assert!(
        status_line.starts_with("HTTP/1.1 413"),
        "unexpected reply: {status_line}"
    );
}

#[tokio::test]
async fn a_umicp_envelope_of_exactly_the_limit_is_processed() {
    let (router, state) = open_router(
        RateLimitConfig {
            enabled: false,
            ..rate_limit(1, 1)
        },
        umicp_limited(),
    );
    let envelope = Envelope::builder()
        .from("limits-test")
        .to("synap")
        .operation(OperationType::Request)
        .capability("operation", json!("synap_kv_set"))
        .capability("key", json!("big"))
        .capability("value", json!(""))
        .build()
        .expect("envelope builds")
        .serialize()
        .expect("envelope serializes");
    // Pad the value until the envelope is exactly the limit.
    let empty_value = r#""value":"""#;
    assert_eq!(envelope.matches(empty_value).count(), 1, "{envelope}");
    let padding = "x".repeat(MIB - envelope.len());
    let body = envelope.replace(empty_value, &format!(r#""value":"{padding}""#));
    assert_eq!(body.len(), MIB);

    let mut request = umicp_post(Body::from(body));
    request
        .headers_mut()
        .insert(header::CONTENT_LENGTH, MIB.into());
    let response = send_from(&router, peer(1), request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let stored = state
        .kv_store
        .get("big")
        .await
        .expect("kv get")
        .expect("the envelope was processed");
    assert_eq!(stored, padding.into_bytes());
}
