//! One counter arithmetic on every transport (review finding E-04).
//!
//! The HTTP envelope applied a `kv.incr` / `kv.decr` `amount`, while the SDK's
//! native maps always sent one-argument INCR/DECR — so `INCR count 10` added 10
//! over HTTP and 1 over SynapRPC or RESP3. Each test starts HTTP, SynapRPC and
//! RESP3 listeners over one shared `AppState` and drives them through
//! `synap_sdk::SynapClient`, asserting the returned value and the stored one.

mod test_helper;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use synap_sdk::{SynapClient, SynapConfig};
use synap_server::AppState;
use synap_server::protocol::resp3::server::spawn_resp3_listener;
use synap_server::protocol::synap_rpc::server::spawn_synap_rpc_listener;
use thunder::server::ListenerHandle;

/// One shared state behind three listeners, plus an SDK client per transport.
struct Deployment {
    state: AppState,
    clients: Vec<(&'static str, SynapClient)>,
    _rpc_handle: Arc<ListenerHandle>,
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral port addr")
        .port()
}

async fn deploy() -> Deployment {
    let state = test_helper::create_test_app_state();

    let router = test_helper::create_test_router(state.clone());
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

    let client = |url: String| {
        SynapClient::new(SynapConfig::new(url).with_timeout(Duration::from_secs(5)))
            .expect("client builds")
    };
    let clients = vec![
        ("HTTP", client(http)),
        (
            "SynapRPC",
            client(format!("synap://{}", rpc_handle.local_addr())),
        ),
        ("RESP3", client(format!("resp3://{resp3}"))),
    ];

    tokio::time::sleep(Duration::from_millis(50)).await;
    Deployment {
        state,
        clients,
        _rpc_handle: rpc_handle,
    }
}

impl Deployment {
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

async fn counter(client: &SynapClient, cmd: &str, payload: Value) -> Result<i64, String> {
    client
        .send_command(cmd, payload)
        .await
        .map(|reply| reply["value"].as_i64().expect("integer reply"))
        .map_err(|e| e.to_string())
}

#[tokio::test]
async fn increment_by_ten_adds_ten_on_every_transport() {
    // Spec scenario: 0 + 10 over HTTP, then SynapRPC, then RESP3 = 30.
    let d = deploy().await;
    d.store("c", 0).await;

    for (i, (name, client)) in d.clients.iter().enumerate() {
        let got = counter(client, "kv.incr", json!({"key": "c", "amount": 10}))
            .await
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected = 10 * (i as i64 + 1);
        assert_eq!(got, expected, "{name} returned value");
        assert_eq!(d.stored("c").await, expected, "{name} stored value");
    }
    assert_eq!(d.stored("c").await, 30);
}

#[tokio::test]
async fn omitted_amount_changes_by_one_on_every_transport() {
    let d = deploy().await;
    d.store("up", 5).await;
    d.store("down", 5).await;

    for (i, (name, client)) in d.clients.iter().enumerate() {
        let step = i as i64 + 1;
        let up = counter(client, "kv.incr", json!({"key": "up"})).await;
        assert_eq!(up, Ok(5 + step), "{name} INCR");
        let down = counter(client, "kv.decr", json!({"key": "down"})).await;
        assert_eq!(down, Ok(5 - step), "{name} DECR");
        assert_eq!(d.stored("up").await, 5 + step, "{name} stored INCR");
        assert_eq!(d.stored("down").await, 5 - step, "{name} stored DECR");
    }
}

#[tokio::test]
async fn zero_and_negative_amounts_apply_as_given_on_every_transport() {
    let d = deploy().await;

    for (name, client) in &d.clients {
        d.store("z", 7).await;
        let zero = counter(client, "kv.incr", json!({"key": "z", "amount": 0})).await;
        assert_eq!(zero, Ok(7), "{name} INCR by 0");
        assert_eq!(d.stored("z").await, 7, "{name} stored after INCR by 0");

        let zero = counter(client, "kv.decr", json!({"key": "z", "amount": 0})).await;
        assert_eq!(zero, Ok(7), "{name} DECR by 0");

        let neg = counter(client, "kv.incr", json!({"key": "z", "amount": -3})).await;
        assert_eq!(neg, Ok(4), "{name} INCR by -3");
        assert_eq!(d.stored("z").await, 4, "{name} stored after INCR by -3");

        let neg = counter(client, "kv.decr", json!({"key": "z", "amount": -6})).await;
        assert_eq!(neg, Ok(10), "{name} DECR by -6");
        assert_eq!(d.stored("z").await, 10, "{name} stored after DECR by -6");
    }
}

#[tokio::test]
async fn overflow_fails_without_mutation_on_every_transport() {
    let d = deploy().await;

    for (name, client) in &d.clients {
        d.store("max", i64::MAX).await;
        let r = counter(client, "kv.incr", json!({"key": "max"})).await;
        assert!(r.is_err(), "{name}: INCR at i64::MAX must fail, got {r:?}");
        let r = counter(client, "kv.incr", json!({"key": "max", "amount": 1})).await;
        assert!(
            r.is_err(),
            "{name}: INCRBY 1 at i64::MAX must fail, got {r:?}"
        );
        assert_eq!(d.stored("max").await, i64::MAX, "{name}: max untouched");

        d.store("min", i64::MIN).await;
        let r = counter(client, "kv.decr", json!({"key": "min", "amount": 1})).await;
        assert!(
            r.is_err(),
            "{name}: DECRBY 1 at i64::MIN must fail, got {r:?}"
        );
        assert_eq!(d.stored("min").await, i64::MIN, "{name}: min untouched");

        // -i64::MIN does not fit in an i64: it used to wrap into an increment
        // by i64::MIN, storing i64::MIN instead of failing.
        d.store("zero", 0).await;
        let r = counter(
            client,
            "kv.decr",
            json!({"key": "zero", "amount": i64::MIN}),
        )
        .await;
        assert!(r.is_err(), "{name}: DECRBY i64::MIN must fail, got {r:?}");
        assert_eq!(d.stored("zero").await, 0, "{name}: zero untouched");
    }
}

#[tokio::test]
async fn malformed_amount_fails_without_mutation_on_every_transport() {
    // HTTP used to count a malformed amount as 1; the native protocols
    // refused it. Now every transport refuses it.
    let d = deploy().await;
    d.store("m", 5).await;

    for (name, client) in &d.clients {
        for amount in [json!("ten"), json!("10"), json!(1.5), json!(true)] {
            let r = counter(client, "kv.incr", json!({"key": "m", "amount": amount})).await;
            assert!(r.is_err(), "{name}: amount {amount} must fail, got {r:?}");
            let r = counter(client, "kv.decr", json!({"key": "m", "amount": amount})).await;
            assert!(r.is_err(), "{name}: amount {amount} must fail, got {r:?}");
        }
        assert_eq!(d.stored("m").await, 5, "{name}: counter untouched");
    }
}

#[tokio::test]
async fn ping_answers_pong_on_every_transport() {
    // The CLI's PING relies on the `ping` command answering the same on all
    // three transports.
    let d = deploy().await;
    for (name, client) in &d.clients {
        let reply = client
            .send_command("ping", json!({}))
            .await
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(reply, json!("PONG"), "{name}");
    }
}
