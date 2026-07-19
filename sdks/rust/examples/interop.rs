//! Interop cell: Rust SDK (thunder-rpc) against a Thunder-based server.
//!
//! Driven by `scripts/interop/run-matrix.py`. Prints one
//! `STEP <name> PASS|FAIL <detail>` line per step and exits non-zero if any
//! step failed.
//!
//! Usage: `cargo run --release -p synap-sdk --example interop -- <host> <port> <user> <pass>`

use std::time::Duration;

use futures::StreamExt;
use serde_json::json;
use synap_sdk::{SynapClient, SynapConfig};

/// Not valid UTF-8, so a transport that quietly round-trips through a string
/// cannot pass the binary step.
const BINARY: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];
const TOPIC: &str = "interop.rust";

fn report(step: &str, ok: bool, detail: impl AsRef<str>) -> u32 {
    println!(
        "STEP {step} {} {}",
        if ok { "PASS" } else { "FAIL" },
        detail.as_ref()
    );
    u32::from(!ok)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (host, port, user, pass) = (
        args[1].clone(),
        args[2].parse::<u16>()?,
        args[3].clone(),
        args[4].clone(),
    );

    let config = SynapConfig::new(format!("synap://{host}:{port}"))
        .with_basic_auth(user, pass)
        .with_timeout(Duration::from_secs(15));
    let client = SynapClient::new(config)?;
    let mut failures = 0u32;

    // 1. Authenticate. The handshake rides the first command; before the
    //    Thunder swap this transport never sent AUTH at all, so a
    //    `require_auth` server was unreachable.
    //
    //    `kv.exists` rather than PING: the SDK maps dot-notation commands to
    //    wire commands and has no `ping` entry, so a PING here would fail in
    //    the client's own command map without ever reaching the server.
    match client
        .send_command("kv.exists", json!({"key": "interop:rust:probe"}))
        .await
    {
        Ok(v) => failures += report("auth", true, format!("kv.exists -> {v}")),
        Err(e) => {
            report("auth", false, e.to_string());
            std::process::exit(1);
        }
    }

    // 2. SET/GET a binary value.
    //
    //    The Rust SDK's public KV surface is generic over `Serialize`, so a
    //    `Vec<u8>` reaches the wire as a MessagePack array of integers rather
    //    than as a `bin`. That is the SDK's own encoding choice and predates
    //    Thunder; what this step proves is that the bytes survive the round
    //    trip intact. The canonical `bin` path is pinned server-side by
    //    `crates/synap-server/tests/synap_rpc_thunder_tests.rs`.
    let key = "interop:rust:bin";
    match client.kv().set(key, BINARY.to_vec(), None).await {
        Ok(()) => match client.kv().get::<_, Vec<u8>>(key).await {
            Ok(Some(got)) => {
                let ok = got == BINARY;
                failures += report(
                    "kv_binary",
                    ok,
                    format!("{} -> {}", hex(&BINARY), hex(&got)),
                );
            }
            Ok(None) => failures += report("kv_binary", false, "GET returned nothing"),
            Err(e) => failures += report("kv_binary", false, format!("GET: {e}")),
        },
        Err(e) => failures += report("kv_binary", false, format!("SET: {e}")),
    }

    // 3. SUBSCRIBE then PUBLISH. `observe` takes the SynapRPC push path.
    let (mut stream, handle) = client.pubsub().observe("interop-rust", vec![TOPIC.into()]);
    tokio::time::sleep(Duration::from_millis(500)).await;
    client
        .pubsub()
        .publish(TOPIC, json!("interop-payload"), None, None)
        .await?;

    match tokio::time::timeout(Duration::from_secs(10), stream.next()).await {
        Ok(Some(msg)) => {
            let ok = msg.topic == TOPIC;
            failures += report(
                "pubsub",
                ok,
                format!("topic={} data={}", msg.topic, msg.data),
            );
        }
        Ok(None) => failures += report("pubsub", false, "stream ended before a message arrived"),
        Err(_) => failures += report("pubsub", false, "no push frame within 10s"),
    }
    handle.unsubscribe();

    // 4. Error round-trip. It has to be an error the *server* raises, so
    //    INCR on a key holding a non-numeric string: an unmapped command name
    //    would be rejected by the SDK's command map and never reach the wire.
    //    The connection must survive it -- one failed call on a multiplexed
    //    connection must not take the others down.
    client
        .kv()
        .set("interop:rust:str", "not-a-number", None)
        .await?;
    match client
        .send_command("kv.incr", json!({"key": "interop:rust:str"}))
        .await
    {
        Ok(v) => failures += report("error", false, format!("expected an error, got {v}")),
        Err(e) => {
            let alive = client
                .send_command("kv.exists", json!({"key": "interop:rust:str"}))
                .await
                .is_ok();
            failures += report("error", alive, format!("{e}; connection alive={alive}"));
        }
    }

    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
