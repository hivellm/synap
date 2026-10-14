//! Starting this release's `synap-server` binary on a data directory written
//! by a real Synap 1.3.2 server: the in-place upgrade, end to end.
//!
//! The data directory under `crates/synap-migrate/tests/fixtures/v1.3.2/` was
//! written by the 1.3.2 release binary, and its `baseline.json` is what that
//! server served for every resource right before it stopped (see the
//! fixtures' README and `generate_fixtures.py`). The test copies it and runs
//! the binary this build produced on the copy, the way an operator upgrades in
//! place, three times:
//!
//! 1. First start. The headerless 1.3.2 WAL (format 1) is upgraded in place to
//!    `SYNAPWAL` format 2: the same file now holds the 12-byte header followed
//!    by the 1.3.2 frames, byte for byte. Recovery loads the v3 snapshot,
//!    replays the 10 WAL records after its cutoff, and the server serves what
//!    the 1.3.2 server served, apart from what 1.3.2 never persisted (see
//!    [`upgraded`]): values, TTL deadlines, queue message IDs, payloads,
//!    priorities, retry counts and delivery order, and the stream room. Then
//!    it takes one write on every WAL-logged datatype (see
//!    [`write_after_upgrade`]), each appended to the upgraded WAL as one
//!    record.
//! 2. Second start. No upgrade runs again; recovery replays the 1.3.2 tail
//!    plus every write of the first start, and the server serves all of it.
//!    Then a snapshot is requested (`POST /snapshot`): it is written as format
//!    v5 under the first sequenced name, next to the 1.3.2 file, with its WAL
//!    cutoff after every record and the absolute deadline of every expiring
//!    key. One more write lands in the WAL after that cutoff.
//! 3. Third start. Recovery loads the v5 snapshot, replays the one write after
//!    it, and the server serves everything again.
//!
//! Every start ends with the process killed, so no shutdown hook runs. Nothing
//! acknowledged is lost by that: with `fsync_mode: always` a write is fsynced
//! before its reply, and a snapshot is fsynced and renamed into place before
//! `POST /snapshot` answers (`docs/operations/guarantees.md`, 1.3 and 1.6).
//!
//! Streams live in snapshots only. The binary runs its stream manager with the
//! default one-hour retention, whose first compaction pass runs at startup, so
//! it serves the fixture's events only while they are less than an hour old;
//! older ones are gone, the room and its offsets stay ([`check_feed`]). The
//! events themselves are compared with the baseline through the recovery the
//! binary runs: `recover_all` on another copy, then `restore_streams` into a
//! stream manager that keeps every event ([`snapshot_events_are_recovered`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use synap_server::persistence::types::{FsyncMode, PersistenceConfig, SnapshotConfig, WALConfig};
use synap_server::persistence::wal_frame::{self, ScanEnd, WalFormat};
use synap_server::persistence::{SnapshotManager, recover_all, restore_streams};
use synap_server::{ServerConfig, StreamConfig, StreamManager};

/// The sequenced name the first snapshot taken by this release gets.
const FIRST_V5_SNAPSHOT: &str = "snapshot-v5-00000000000000000001.bin";
/// A snapshot threshold no start reaches: no snapshot is taken.
const NO_SNAPSHOT: usize = 1_000_000;
/// Records in the 1.3.2 WAL (offsets 1 to 22); the v3 snapshot's cutoff is 13.
const LEGACY_RECORDS: u64 = 22;
const LEGACY_CUTOFF: u64 = 13;
const TEN_YEARS_SECS: u64 = 10 * 365 * 24 * 3600;
/// How far a TTL deadline may be from the expected one. TTL reads are whole
/// seconds remaining, on the source and here, and the 1.3.2 WAL records a
/// relative TTL that recovery anchors at the record's whole-second timestamp.
const TTL_TOLERANCE_MS: u64 = 3_000;
/// The priorities `generate_fixtures.py` published each message with.
const PRIORITIES: [(&str, u64); 7] = [
    ("job-1", 5),
    ("job-2", 9),
    ("job-3", 5),
    ("job-4", 1),
    ("job-5", 5),
    ("job-6", 7),
    ("late-1", 3),
];
/// Every message of the dataset is published with the queue's default.
const MAX_RETRIES: u64 = 3;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("synap-migrate")
        .join("tests")
        .join("fixtures")
        .join("v1.3.2")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// The configuration a server on the data directory `data` (`snapshots/`,
/// `wal/synap.wal`) runs with. Only HTTP listens. A snapshot is taken only
/// once `operation_threshold` WAL records were written since the last one, by
/// `POST /snapshot` or by the background task's check every minute, never by
/// time (the interval is a year). Messages the checks deliver stay in flight
/// until the process is killed, never requeued by an ACK deadline: queues that
/// a 1.3.2 snapshot or WAL created carry no configuration and get this
/// default.
fn server_config(data: &Path, port: u16, operation_threshold: usize) -> ServerConfig {
    let mut config = ServerConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = port;
    config.resp3.enabled = false;
    config.synap_rpc.enabled = false;
    config.queue.ack_deadline_secs = 3600;
    config.persistence = PersistenceConfig {
        enabled: true,
        wal: WALConfig {
            path: data.join("wal").join("synap.wal"),
            fsync_mode: FsyncMode::Always,
            ..WALConfig::default()
        },
        snapshot: SnapshotConfig {
            directory: data.join("snapshots"),
            interval_secs: 365 * 24 * 3600,
            operation_threshold,
            ..SnapshotConfig::default()
        },
    };
    config
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A running `synap-server` process.
struct Server {
    child: Child,
    url: String,
    log: PathBuf,
    http: reqwest::Client,
}

impl Server {
    /// Start the binary on `data` and wait until it answers `/health`. The
    /// configuration and the log (JSON lines) go to `<run>.yml` and
    /// `<run>.log` in `dir`.
    async fn start(dir: &Path, data: &Path, run: &str, operation_threshold: usize) -> Self {
        let port = free_port();
        let config_path = dir.join(format!("{run}.yml"));
        let config = server_config(data, port, operation_threshold);
        fs::write(&config_path, serde_saphyr::to_string(&config).unwrap()).unwrap();
        let log = dir.join(format!("{run}.log"));
        let out = File::create(&log).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_synap-server"));
        command
            .arg("--config")
            .arg(&config_path)
            .current_dir(dir)
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(out.try_clone().unwrap())
            .stderr(out);
        // Environment overrides (SYNAP_AUTH_ENABLED and the like) would change
        // the configuration under test.
        for (name, _) in std::env::vars() {
            if name.starts_with("SYNAP_") {
                command.env_remove(name);
            }
        }
        let mut server = Self {
            child: command.spawn().unwrap(),
            url: format!("http://127.0.0.1:{port}"),
            log,
            http: reqwest::Client::new(),
        };
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(status) = server.child.try_wait().unwrap() {
                panic!("{run}: the server exited with {status}:\n{}", server.log());
            }
            if let Ok(response) = server
                .http
                .get(format!("{}/health", server.url))
                .send()
                .await
                && response.status().is_success()
            {
                let health: Value = response.json().await.unwrap();
                assert_eq!(health["version"], env!("CARGO_PKG_VERSION"), "{health}");
                return server;
            }
            assert!(Instant::now() < deadline, "{run}: not healthy after 60 s");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn log(&self) -> String {
        fs::read_to_string(&self.log).unwrap()
    }

    /// Kill the process and return its log.
    fn kill(mut self) -> String {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
        self.log()
    }

    /// Run one `/api/v1/command` envelope.
    async fn envelope(&self, command: &str, payload: Value) -> Value {
        let response = self
            .http
            .post(format!("{}/api/v1/command", self.url))
            .json(&json!({"command": command, "request_id": "upgrade", "payload": payload}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK, "{command}");
        response.json().await.unwrap()
    }

    /// The payload of a command that succeeds.
    async fn read(&self, command: &str, payload: Value) -> Value {
        let response = self.envelope(command, payload.clone()).await;
        assert_eq!(response["success"], true, "{command} {payload}: {response}");
        response["payload"].clone()
    }

    /// `POST /snapshot`.
    async fn take_snapshot(&self) -> Value {
        let response = self
            .http
            .post(format!("{}/snapshot", self.url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    /// A command that must fail because its key does not exist.
    async fn missing(&self, command: &str, payload: Value) {
        let response = self.envelope(command, payload.clone()).await;
        assert_eq!(
            response["success"], false,
            "{command} {payload}: {response}"
        );
        let error = response["error"].as_str().unwrap();
        assert_eq!(error, "Not found", "{command}");
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The one log line containing `marker`.
fn log_line<'a>(log: &'a str, marker: &str) -> &'a str {
    let lines: Vec<&str> = log.lines().filter(|l| l.contains(marker)).collect();
    assert_eq!(lines.len(), 1, "{marker:?} in\n{log}");
    lines[0]
}

/// Recovery loaded `snapshot` with WAL cutoff `cutoff`, then replayed
/// `replayed` records.
fn assert_recovered_from(log: &str, snapshot: &str, cutoff: u64, replayed: u64) {
    let loaded = log_line(log, "Loaded snapshot from");
    assert!(loaded.contains(snapshot), "{loaded}");
    assert!(loaded.contains(&format!("at offset {cutoff}")), "{loaded}");
    log_line(
        log,
        &format!("Recovery complete. Replayed {replayed} operations"),
    );
}

/// A KV key the server must serve: its value and, for an expiring key, the
/// deadline (Unix ms) it must have, within [`TTL_TOLERANCE_MS`].
#[derive(Debug, Clone)]
struct Kv {
    value: String,
    deadline_ms: Option<u64>,
}

fn kv(value: &str, deadline_ms: Option<u64>) -> Option<Kv> {
    Some(Kv {
        value: value.to_string(),
        deadline_ms,
    })
}

#[derive(Debug, Clone, PartialEq)]
struct Message {
    body: String,
    priority: u64,
    retry_count: u64,
}

/// What the server must serve.
#[derive(Debug, Clone)]
struct Expected {
    /// Every key the checks read; `None` for a key that must not exist.
    kv: BTreeMap<String, Option<Kv>>,
    /// Fields of the hash `user:1`.
    hash: BTreeMap<String, String>,
    /// The list `events`.
    list: Vec<String>,
    /// The set `tags`.
    set: BTreeSet<String>,
    /// The sorted set `scores`, by ascending score.
    scores: Vec<(String, f64)>,
    /// The HyperLogLog `visitors`' count; `None`: it does not exist.
    visitors: Option<u64>,
    /// Bits set in the bitmap `flags`; empty: it does not exist.
    flags: BTreeSet<u64>,
    /// Every queue, with its messages by ID. After a restart every message is
    /// ready: deliveries are not persisted.
    queues: BTreeMap<String, BTreeMap<String, Message>>,
    /// Every event room `feed` was restored or published with, in offset
    /// order from offset 0.
    feed: Vec<Value>,
}

fn priority(body: &str) -> u64 {
    PRIORITIES.iter().find(|(b, _)| *b == body).unwrap().1
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

/// What 1.4.0 serves right after upgrading the 1.3.2 directory: everything the
/// 1.3.2 server served (`baseline.json`), except what any restart changes and
/// what 1.3.2 never persisted (see the fixtures' README):
///
/// - `ttl-short` expired long before the test runs;
/// - the v3 snapshot stores no expiry, so `ttl-long`, which it holds, loads
///   without one; `ttl-tail`, a WAL record, keeps its deadline;
/// - 1.3.2 logged no hash, list, set or sorted-set write and no HyperLogLog or
///   bitmap to the WAL, and its snapshot has no HyperLogLog or bitmap
///   section: `user:1.city`, `d`, `w`, `dave`, `visitors` and `flags` are
///   gone;
/// - 1.3.2 stored no dead letter (the poison message in `dead`) and no
///   delivery: every message published and not acknowledged is ready, with
///   its ID, payload, priority and retry count;
/// - the stream event published after the snapshot was never persisted.
fn upgraded(baseline: &Value) -> Expected {
    let now = now_ms();
    let mut keys = BTreeMap::new();
    for (key, entry) in baseline["kv"].as_object().unwrap() {
        let read_at = entry["ttl_read_at_ms"].as_u64().unwrap();
        let ttl = entry["ttl"]["ttl"].as_u64();
        let expired = ttl.is_some_and(|ttl| read_at + (ttl + 1) * 1000 <= now);
        let expected = match entry["get"].as_str() {
            Some(value) if !expired => {
                let deadline_ms = ttl
                    .filter(|_| key != "ttl-long")
                    .map(|ttl| read_at + ttl * 1000);
                kv(value, deadline_ms)
            }
            _ => None,
        };
        keys.insert(key.clone(), expected);
    }
    assert!(
        keys["ttl-short"].is_none() && keys["ttl-tail"].as_ref().unwrap().deadline_ms.is_some()
    );

    let mut hash: BTreeMap<String, String> =
        serde_json::from_value(baseline["hashes"]["user:1"]["fields"].clone()).unwrap();
    assert!(hash.remove("city").is_some());
    let list: Vec<String> = strings(&baseline["lists"]["events"]["values"])
        .into_iter()
        .filter(|v| v != "d")
        .collect();
    let set: BTreeSet<String> = strings(&baseline["sets"]["tags"]["members"])
        .into_iter()
        .filter(|m| m != "w")
        .collect();
    let scores: Vec<(String, f64)> = baseline["sorted_sets"]["scores"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            let member = m["member"].as_str().unwrap().to_string();
            (member, m["score"].as_f64().unwrap())
        })
        .filter(|(member, _)| member != "dave")
        .collect();

    let queues = &baseline["queues"];
    let jobs = &queues["jobs"];
    let acked: BTreeSet<&str> = jobs["acked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect();
    let requeued = jobs["requeued_by_nack"][0].as_str().unwrap();
    let ready = |published: &Value| -> BTreeMap<String, Message> {
        published
            .as_object()
            .unwrap()
            .iter()
            .map(|(body, id)| (body, id.as_str().unwrap()))
            .filter(|(_, id)| !acked.contains(id))
            .map(|(body, id)| {
                let message = Message {
                    body: body.clone(),
                    priority: priority(body),
                    retry_count: u64::from(id == requeued),
                };
                (id.to_string(), message)
            })
            .collect()
    };
    let mut expected_queues = BTreeMap::new();
    expected_queues.insert("jobs".to_string(), ready(&jobs["published"]));
    expected_queues.insert("late".to_string(), ready(&queues["late"]["published"]));
    expected_queues.insert("dead".to_string(), BTreeMap::new());
    assert_eq!(expected_queues["jobs"].len(), 4);

    let feed: Vec<Value> = baseline["streams"]["feed"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["event"] != "after-snapshot")
        .cloned()
        .collect();
    assert_eq!(feed.len(), 3);

    Expected {
        kv: keys,
        hash,
        list,
        set,
        scores,
        visitors: None,
        flags: BTreeSet::new(),
        queues: expected_queues,
        feed,
    }
}

/// Check that `server` serves `expected` and the parts of the baseline no
/// write of this test touches. Every message is delivered, and stays in
/// flight; returns the bodies of each queue's messages in delivery order.
async fn check(
    server: &Server,
    expected: &Expected,
    baseline: &Value,
) -> BTreeMap<String, Vec<String>> {
    for (key, kv) in &expected.kv {
        let got = server.read("kv.get", json!({ "key": key })).await;
        let Some(kv) = kv else {
            assert_eq!(got, Value::Null, "kv.get {key}");
            continue;
        };
        assert_eq!(got, kv.value.as_str(), "kv.get {key}");
        let read_at = now_ms();
        let ttl = server.read("kv.ttl", json!({ "key": key })).await;
        match kv.deadline_ms {
            None => assert_eq!(ttl["ttl"], Value::Null, "kv.ttl {key}"),
            Some(deadline) => {
                let remaining = ttl["ttl"]
                    .as_u64()
                    .unwrap_or_else(|| panic!("{key}: {ttl} served, deadline {deadline} expected"));
                let served = read_at + remaining * 1000;
                assert!(
                    served.abs_diff(deadline) <= TTL_TOLERANCE_MS,
                    "{key}: deadline {served} served, {deadline} expected"
                );
            }
        }
    }

    let got = server.read("hash.getall", json!({"key": "user:1"})).await;
    assert_eq!(got["fields"], json!(expected.hash));
    let got = server
        .read(
            "list.lrange",
            json!({"key": "events", "start": 0, "stop": -1}),
        )
        .await;
    assert_eq!(got["values"], json!(expected.list));
    let got = server.read("set.members", json!({"key": "tags"})).await;
    let members: BTreeSet<String> = strings(&got["members"]).into_iter().collect();
    assert_eq!(members, expected.set);

    let mut scores = expected.scores.clone();
    scores.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let scores: Vec<Value> = scores
        .iter()
        .map(|(member, score)| json!({"member": member, "score": score}))
        .collect();
    let range = json!({"start": 0, "stop": -1, "withscores": true});
    let mut payload = range.clone();
    payload["key"] = json!("scores");
    let got = server.read("sortedset.zrange", payload).await;
    assert_eq!(got["members"], json!(scores));
    // The geospatial set, written before the 1.3.2 snapshot, is never touched.
    let mut payload = range;
    payload["key"] = json!("places");
    let got = server.read("sortedset.zrange", payload).await;
    assert_eq!(got["members"], baseline["sorted_sets"]["places"]["members"]);
    let got = server
        .read(
            "geospatial.geopos",
            json!({"key": "places", "members": ["sao-paulo", "paris"]}),
        )
        .await;
    assert_eq!(got["coordinates"], baseline["geo"]["places"]["coordinates"]);

    match expected.visitors {
        None => {
            server
                .missing("hyperloglog.pfcount", json!({"key": "visitors"}))
                .await
        }
        Some(count) => {
            let got = server
                .read("hyperloglog.pfcount", json!({"key": "visitors"}))
                .await;
            assert_eq!(got["count"], count);
        }
    }
    if expected.flags.is_empty() {
        server
            .missing("bitmap.bitcount", json!({"key": "flags"}))
            .await;
    } else {
        let got = server
            .read("bitmap.bitcount", json!({"key": "flags"}))
            .await;
        assert_eq!(got["count"], expected.flags.len());
        let probes: BTreeSet<u64> = baseline["bitmaps"]["flags"]["bits"]
            .as_object()
            .unwrap()
            .keys()
            .map(|offset| offset.parse().unwrap())
            .chain(expected.flags.iter().copied())
            .collect();
        for offset in probes {
            let got = server
                .read("bitmap.getbit", json!({"key": "flags", "offset": offset}))
                .await;
            let bit = u64::from(expected.flags.contains(&offset));
            assert_eq!(got["value"], bit, "bit {offset}");
        }
    }

    let order = check_queues(server, expected).await;
    check_feed(server, &expected.feed).await;
    order
}

async fn check_queues(server: &Server, expected: &Expected) -> BTreeMap<String, Vec<String>> {
    let listed = server.read("queue.list", json!({})).await;
    let names: BTreeSet<String> = strings(&listed["queues"]).into_iter().collect();
    assert_eq!(
        names,
        expected.queues.keys().cloned().collect::<BTreeSet<_>>()
    );
    let mut order = BTreeMap::new();
    for (queue, messages) in &expected.queues {
        let stats = server.read("queue.stats", json!({ "queue": queue })).await;
        assert_eq!(stats["depth"], messages.len(), "{queue}: {stats}");
        let mut delivered = BTreeMap::new();
        let mut bodies = Vec::new();
        loop {
            let got = server
                .read(
                    "queue.consume",
                    json!({"queue": queue, "consumer_id": "check"}),
                )
                .await;
            let Some(message) = got["message"].as_object() else {
                break;
            };
            let payload: Vec<u8> = serde_json::from_value(message["payload"].clone()).unwrap();
            let body = String::from_utf8(payload).unwrap();
            assert_eq!(message["max_retries"], MAX_RETRIES, "{body}");
            bodies.push(body.clone());
            let message = Message {
                body,
                priority: message["priority"].as_u64().unwrap(),
                retry_count: message["retry_count"].as_u64().unwrap(),
            };
            let id = got["message"]["id"].as_str().unwrap().to_string();
            assert!(
                delivered.insert(id, message).is_none(),
                "{queue}: redelivered"
            );
        }
        assert_eq!(&delivered, messages, "{queue}");
        order.insert(queue.clone(), bodies);
    }
    order
}

/// Fields a served stream event must match.
fn assert_same_events(got: &[Value], expected: &[Value]) {
    assert_eq!(got.len(), expected.len(), "{got:?}");
    for (event, expected) in got.iter().zip(expected) {
        for field in ["id", "offset", "event", "data", "timestamp"] {
            assert_eq!(event[field], expected[field], "{field}: {event}");
        }
    }
}

/// Room `feed` holds `events` (offsets 0 to n-1), less the oldest ones the
/// binary's default retention removed: every event gone is older than the
/// retention, every one kept is served as it was published.
async fn check_feed(server: &Server, events: &[Value]) {
    let rooms = server.read("stream.list", json!({})).await;
    assert_eq!(rooms["rooms"], json!(["feed"]), "{rooms}");
    let stats = server.read("stream.stats", json!({"room": "feed"})).await;
    let published = events.len() as u64;
    assert_eq!(stats["max_offset"], published - 1, "{stats}");
    assert_eq!(stats["total_published"], published, "{stats}");
    let first_kept = stats["min_offset"].as_u64().unwrap();
    let (gone, kept) = events.split_at(first_kept as usize);
    assert_eq!(stats["message_count"], kept.len(), "{stats}");
    let retention = StreamConfig::default().retention_secs;
    let cutoff = now_ms() / 1000 - retention;
    for event in gone {
        assert!(event["timestamp"].as_u64().unwrap() <= cutoff, "{event}");
    }
    let got = server
        .read(
            "stream.consume",
            json!({"room": "feed", "subscriber_id": "check", "from_offset": first_kept, "limit": 100}),
        )
        .await;
    assert_same_events(got["events"].as_array().unwrap(), kept);
    assert_eq!(got["next_offset"], published, "{got}");
}

/// The 1.3.2 snapshot's stream events, as the recovery the binary runs
/// rebuilds them: `recover_all` on a copy of the directory, then
/// `restore_streams` into a stream manager that keeps every event, however
/// old.
async fn snapshot_events_are_recovered(dir: &Path, expected: &[Value]) {
    let data = dir.join("library");
    copy_dir(&fixture_dir().join("data"), &data);
    let config = server_config(&data, 0, NO_SNAPSHOT);
    let recovered = recover_all(
        &config.persistence,
        config.to_kv_config(),
        config.to_queue_config(),
    )
    .await
    .unwrap();
    let streams = StreamManager::new(StreamConfig {
        retention_secs: 0,
        ..StreamConfig::default()
    });
    assert_eq!(restore_streams(&streams, recovered.streams).await, 1);
    let events: Vec<Value> = streams
        .consume("feed", "check", 0, 100)
        .await
        .unwrap()
        .iter()
        .map(|event| serde_json::to_value(event).unwrap())
        .collect();
    assert_same_events(&events, expected);
}

/// Scan the WAL: format 2, nothing invalid, records 1 to `records` in order.
fn assert_wal_records(wal: &Path, records: u64) {
    let mut offsets = Vec::new();
    let report = wal_frame::scan(wal, |entry| offsets.push(entry.offset)).unwrap();
    assert_eq!(
        report.format,
        WalFormat::Versioned(wal_frame::WAL_FORMAT_VERSION)
    );
    assert_eq!(report.end, ScanEnd::Clean);
    assert_eq!(offsets, (1..=records).collect::<Vec<_>>());
}

fn id_of(expected: &Expected, queue: &str, body: &str) -> String {
    expected.queues[queue]
        .iter()
        .find(|(_, m)| m.body == body)
        .map(|(id, _)| id.clone())
        .unwrap()
}

/// One write on every WAL-logged datatype, recorded in `expected`. Returns the
/// number of WAL records they add: one each. `job-4` and `job-5` must be in
/// flight.
async fn write_after_upgrade(server: &Server, expected: &mut Expected) -> u64 {
    server
        .read("kv.set", json!({"key": "after-upgrade", "value": "kept"}))
        .await;
    expected.kv.insert("after-upgrade".into(), kv("kept", None));
    let written_at = now_ms();
    server
        .read(
            "kv.set",
            json!({"key": "after-upgrade-ttl", "value": "kept until", "ttl": TEN_YEARS_SECS}),
        )
        .await;
    let deadline = written_at + TEN_YEARS_SECS * 1000;
    expected
        .kv
        .insert("after-upgrade-ttl".into(), kv("kept until", Some(deadline)));
    server.read("kv.del", json!({"key": "plain"})).await;
    expected.kv.insert("plain".into(), None);

    server
        .read(
            "hash.set",
            json!({"key": "user:1", "field": "city", "value": "Paris"}),
        )
        .await;
    expected.hash.insert("city".into(), "Paris".into());
    server
        .read("list.rpush", json!({"key": "events", "values": ["e"]}))
        .await;
    expected.list.push("e".into());
    server
        .read("set.add", json!({"key": "tags", "members": ["v"]}))
        .await;
    expected.set.insert("v".into());
    server
        .read(
            "sortedset.zadd",
            json!({"key": "scores", "member": "eve", "score": 3}),
        )
        .await;
    expected.scores.push(("eve".into(), 3.0));
    server
        .read(
            "hyperloglog.pfadd",
            json!({"key": "visitors", "elements": ["u7", "u8"]}),
        )
        .await;
    expected.visitors = Some(2);
    server
        .read(
            "bitmap.setbit",
            json!({"key": "flags", "offset": 300, "value": 1}),
        )
        .await;
    expected.flags.insert(300);

    let published = server
        .read(
            "queue.publish",
            json!({"queue": "jobs", "payload": b"job-7".to_vec(), "priority": 2}),
        )
        .await;
    let job_7 = Message {
        body: "job-7".into(),
        priority: 2,
        retry_count: 0,
    };
    let jobs = expected.queues.get_mut("jobs").unwrap();
    jobs.insert(published["message_id"].as_str().unwrap().into(), job_7);
    let job_4 = id_of(expected, "jobs", "job-4");
    server
        .read("queue.ack", json!({"queue": "jobs", "message_id": job_4}))
        .await;
    let job_5 = id_of(expected, "jobs", "job-5");
    server
        .read(
            "queue.nack",
            json!({"queue": "jobs", "message_id": job_5, "requeue": true}),
        )
        .await;
    let jobs = expected.queues.get_mut("jobs").unwrap();
    jobs.remove(&job_4);
    jobs.get_mut(&job_5).unwrap().retry_count += 1;
    server
        .read(
            "queue.create",
            json!({"name": "fresh", "config": {"max_depth": 7, "ack_deadline_secs": 3600}}),
        )
        .await;
    expected.queues.insert("fresh".into(), BTreeMap::new());
    server.read("queue.delete", json!({"queue": "dead"})).await;
    expected.queues.remove("dead");
    14
}

/// The first snapshot this release writes on the upgraded directory: format
/// v5 (`SYNAP005` and version byte 5), WAL cutoff `cutoff`, and the state
/// `expected` describes, every message in flight.
async fn check_v5_snapshot(path: &Path, expected: &Expected, cutoff: u64) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[..9], b"SYNAP005\x05");
    let snapshot = SnapshotManager::new(SnapshotConfig {
        directory: path.parent().unwrap().to_path_buf(),
        ..SnapshotConfig::default()
    })
    .load(path)
    .await
    .unwrap();
    assert_eq!(snapshot.version, 5);
    assert_eq!(snapshot.wal_offset, cutoff);

    let keys: BTreeSet<&String> = snapshot.kv_data.keys().collect();
    let present: BTreeSet<&String> = expected
        .kv
        .iter()
        .filter(|(_, kv)| kv.is_some())
        .map(|(key, _)| key)
        .collect();
    assert_eq!(keys, present);
    let deadlines: BTreeMap<&String, u64> = expected
        .kv
        .iter()
        .filter_map(|(key, kv)| Some((key, kv.as_ref()?.deadline_ms?)))
        .collect();
    assert_eq!(
        snapshot.kv_expires_at_ms.keys().collect::<BTreeSet<_>>(),
        deadlines.keys().copied().collect::<BTreeSet<_>>()
    );
    for (key, deadline) in deadlines {
        let stored = snapshot.kv_expires_at_ms[key];
        assert!(
            stored.abs_diff(deadline) <= TTL_TOLERANCE_MS,
            "{key}: deadline {stored} stored, {deadline} expected"
        );
    }

    assert_eq!(
        snapshot.queue_data.keys().collect::<BTreeSet<_>>(),
        expected.queues.keys().collect::<BTreeSet<_>>()
    );
    for (queue, messages) in &expected.queues {
        let image = &snapshot.queue_data[queue];
        let config = image.config.as_ref().unwrap();
        assert_eq!(config.ack_deadline_secs, 3600, "{queue}");
        assert!(
            image.ready.is_empty() && image.dead_letter.is_empty(),
            "{queue}"
        );
        let in_flight: BTreeMap<String, Message> = image
            .in_flight
            .iter()
            .map(|m| {
                let message = Message {
                    body: String::from_utf8(m.payload.clone()).unwrap(),
                    priority: u64::from(m.priority),
                    retry_count: u64::from(m.retry_count),
                };
                (m.id.clone(), message)
            })
            .collect();
        assert_eq!(&in_flight, messages, "{queue}");
    }
    assert_eq!(
        snapshot.queue_data["fresh"]
            .config
            .as_ref()
            .unwrap()
            .max_depth,
        7
    );

    assert!(snapshot.hyperloglog_data.contains_key("visitors"));
    assert!(snapshot.bitmap_data.contains_key("flags"));
    let stored = &snapshot.stream_data["feed"];
    let newest = stored.last().unwrap();
    let published = expected.feed.last().unwrap();
    assert_eq!(newest.id, published["id"].as_str().unwrap());
    assert_eq!(newest.offset, published["offset"].as_u64().unwrap());
    assert_eq!(newest.event_type, published["event"].as_str().unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_1_3_2_data_directory_upgrades_in_place_and_recovers_fully() {
    let fixture = fixture_dir();
    let baseline: Value =
        serde_json::from_str(&fs::read_to_string(fixture.join("baseline.json")).unwrap()).unwrap();
    let legacy_snapshot = baseline["source"]["snapshot"].as_str().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let data = dir.join("data");
    copy_dir(&fixture.join("data"), &data);
    let wal = data.join("wal").join("synap.wal");
    let snapshots = data.join("snapshots");
    let legacy = fs::read(&wal).unwrap();
    assert_ne!(
        legacy[..8],
        wal_frame::WAL_MAGIC,
        "the fixture WAL has no header"
    );
    let mut expected = upgraded(&baseline);

    snapshot_events_are_recovered(dir, &expected.feed).await;

    // 1. First start: the WAL is upgraded in place, nothing else is written.
    let server = Server::start(dir, &data, "start1", NO_SNAPSHOT).await;
    let upgraded_wal = [wal_frame::encode_file_header().as_slice(), &legacy].concat();
    assert_eq!(fs::read(&wal).unwrap(), upgraded_wal);
    assert_eq!(file_names(&data.join("wal")), ["synap.wal"]);
    assert_eq!(file_names(&snapshots), [legacy_snapshot]);
    let log = server.log();
    log_line(
        &log,
        &format!(
            "upgraded from format version 1 (Synap 1.3.x, no header) to version 2 ({LEGACY_RECORDS} frames, {} bytes)",
            legacy.len()
        ),
    );
    assert_recovered_from(
        &log,
        legacy_snapshot,
        LEGACY_CUTOFF,
        LEGACY_RECORDS - LEGACY_CUTOFF + 1,
    );

    let order = check(&server, &expected, &baseline).await;
    // The order the 1.3.2 queue held its messages in: a publish went ahead of
    // the lower priorities, but a NACK requeued job-3 at the back.
    assert_eq!(order["jobs"], ["job-6", "job-5", "job-4", "job-3"]);
    assert_eq!(order["late"], ["late-1"]);

    let after_upgrade = write_after_upgrade(&server, &mut expected).await;
    let log = server.kill();
    let records = LEGACY_RECORDS + after_upgrade;
    assert_wal_records(&wal, records);
    assert!(fs::read(&wal).unwrap().starts_with(&upgraded_wal));
    assert_eq!(file_names(&snapshots), [legacy_snapshot], "{log}");

    // 2. Second start: the 1.3.2 tail plus every write after the upgrade.
    let before_snapshot = 2;
    let server = Server::start(dir, &data, "start2", before_snapshot).await;
    let log = server.log();
    assert!(!log.contains("upgraded from format version"), "{log}");
    assert_recovered_from(
        &log,
        legacy_snapshot,
        LEGACY_CUTOFF,
        records - LEGACY_CUTOFF + 1,
    );
    check(&server, &expected, &baseline).await;

    // A stream event (snapshots only), then two WAL records, which reach the
    // snapshot threshold; the one record after the snapshot stays below it,
    // so no other snapshot is taken. Every message is in flight.
    let published = server
        .read(
            "stream.publish",
            json!({"room": "feed", "event": "after-upgrade", "data": {"n": 4}}),
        )
        .await;
    assert_eq!(published["offset"], expected.feed.len(), "{published}");
    let got = server
        .read(
            "stream.consume",
            json!({"room": "feed", "subscriber_id": "writer", "from_offset": published["offset"], "limit": 1}),
        )
        .await;
    expected.feed.push(got["events"][0].clone());
    server
        .read(
            "kv.set",
            json!({"key": "before-snapshot", "value": "in the snapshot"}),
        )
        .await;
    expected
        .kv
        .insert("before-snapshot".into(), kv("in the snapshot", None));
    let job_6 = id_of(&expected, "jobs", "job-6");
    server
        .read("queue.ack", json!({"queue": "jobs", "message_id": job_6}))
        .await;
    expected.queues.get_mut("jobs").unwrap().remove(&job_6);
    let records = records + before_snapshot as u64;

    let taken = server.take_snapshot().await;
    assert_eq!(taken["success"], true, "{taken}");
    assert_eq!(file_names(&snapshots), [legacy_snapshot, FIRST_V5_SNAPSHOT]);
    let cutoff = records + 1;
    check_v5_snapshot(&snapshots.join(FIRST_V5_SNAPSHOT), &expected, cutoff).await;

    server
        .read(
            "kv.set",
            json!({"key": "after-snapshot", "value": "replayed"}),
        )
        .await;
    expected
        .kv
        .insert("after-snapshot".into(), kv("replayed", None));
    let log = server.kill();
    log_line(&log, &format!("at WAL offset {cutoff}"));
    assert_wal_records(&wal, records + 1);
    assert_eq!(file_names(&snapshots), [legacy_snapshot, FIRST_V5_SNAPSHOT]);

    // 3. Third start: the v5 snapshot plus the record after its cutoff.
    let server = Server::start(dir, &data, "start3", NO_SNAPSHOT).await;
    let log = server.log();
    assert!(!log.contains("upgraded from format version"), "{log}");
    assert_recovered_from(&log, FIRST_V5_SNAPSHOT, cutoff, 1);
    check(&server, &expected, &baseline).await;
    server.kill();
    assert_wal_records(&wal, records + 1);
}
