//! Artifact smoke test for the Rust SDK.
//!
//! Run by scripts/release/artifact-smoke/run.py as the `main.rs` of a scratch
//! crate outside the workspace whose `synap-sdk` dependency is the unpacked
//! `.crate` that `cargo package` produced, so the code under test is exactly
//! what crates.io will receive.
//!
//!   argv:   <http-url> <synap-url> <resp3-url>
//!   stdout: one `STEP <transport>.<name> PASS|FAIL <detail>` line per step
//!   exit:   0 if every step passed

use std::process::ExitCode;
use std::time::Duration;

use serde_json::json;
use synap_sdk::{Result, SynapClient, SynapConfig, SynapError};

struct Steps {
    failures: usize,
}

impl Steps {
    fn report(&mut self, step: &str, outcome: Result<(bool, String)>) {
        let (ok, detail) = match outcome {
            Ok(result) => result,
            Err(e) => (false, format!("error: {e}")),
        };
        println!("STEP {step} {} {detail}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            self.failures += 1;
        }
    }
}

fn connect(url: &str) -> Result<SynapClient> {
    SynapClient::new(SynapConfig::new(url).with_timeout(Duration::from_secs(5)))
}

/// `kv.incr` / `kv.decr` with an explicit amount. The typed `KVStore` methods
/// take no amount, so this goes through the client's public command API, the
/// same path the typed methods use.
async fn count(client: &SynapClient, command: &str, key: &str, amount: i64) -> Result<i64> {
    let reply = client
        .send_command(command, json!({ "key": key, "amount": amount }))
        .await?;
    Ok(reply["value"].as_i64().unwrap_or(i64::MIN))
}

async fn transport_suite(steps: &mut Steps, name: &str, url: &str) {
    let client = match connect(url) {
        Ok(client) => client,
        Err(e) => {
            steps.report(&format!("{name}.connect"), Err(e));
            return;
        }
    };
    let key = format!("smoke:rs:{name}:key");
    let counter = format!("smoke:rs:{name}:counter");
    let expected = format!("value-{name}");

    let outcome = async {
        client.kv().set(&key, &expected, None).await?;
        let got: Option<String> = client.kv().get(&key).await?;
        Ok::<_, SynapError>((
            got.as_deref() == Some(expected.as_str()),
            format!("got {got:?}"),
        ))
    }
    .await;
    steps.report(&format!("{name}.set_get"), outcome);

    let outcome = async {
        client.kv().set(&counter, "0", None).await?;
        let seen = [
            count(&client, "kv.incr", &counter, 10).await?,
            count(&client, "kv.decr", &counter, 4).await?,
            client.kv().incr(&counter).await?,
        ];
        Ok::<_, SynapError>((
            seen == [10, 6, 7],
            format!("incr 10, decr 4, incr -> {seen:?}"),
        ))
    }
    .await;
    steps.report(&format!("{name}.counter_amounts"), outcome);

    let outcome = async {
        client.kv().delete(&key).await?;
        client.kv().delete(&counter).await?;
        let got: Option<String> = client.kv().get(&key).await?;
        Ok::<_, SynapError>((got.is_none(), format!("get after delete -> {got:?}")))
    }
    .await;
    steps.report(&format!("{name}.delete"), outcome);
}

async fn cross_transport(steps: &mut Steps, http: &str, rpc: &str, resp3: &str) {
    let outcome = async {
        let (http, rpc, resp3) = (connect(http)?, connect(rpc)?, connect(resp3)?);
        let key = "smoke:rs:cross";
        http.kv().set(key, "from-http", None).await?;
        let seen: [Option<String>; 2] = [rpc.kv().get(key).await?, resp3.kv().get(key).await?];
        http.kv().delete(key).await?;
        Ok::<_, SynapError>((
            seen.iter().all(|v| v.as_deref() == Some("from-http")),
            format!("synaprpc, resp3 -> {seen:?}"),
        ))
    }
    .await;
    steps.report("cross.http_to_native", outcome);
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [http, rpc, resp3] = args.as_slice() else {
        eprintln!("usage: smoke <http-url> <synap-url> <resp3-url>");
        return ExitCode::from(2);
    };
    let mut steps = Steps { failures: 0 };
    transport_suite(&mut steps, "http", http).await;
    transport_suite(&mut steps, "synaprpc", rpc).await;
    transport_suite(&mut steps, "resp3", resp3).await;
    cross_transport(&mut steps, http, rpc, resp3).await;
    if steps.failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
