//! Migrating data directories written by real Synap servers into Hub mode,
//! end to end.
//!
//! The fixtures under `tests/fixtures/` were written by a real 1.3.2 server
//! and by this release's server (see `fixtures/README.md`); `baseline.json` is
//! what each served for every resource right before it stopped. For each
//! fixture the test:
//!
//! 1. copies the data directory and runs the `synap-migrate` binary on the
//!    copy, into a new directory, for one tenant;
//! 2. checks that the copy is byte-for-byte unchanged and that the output is
//!    one snapshot in the server's current format and naming;
//! 3. boots Hub mode on the output in-process, the way `synap-server` starts
//!    with `hub.enabled: true`: a `ServerConfig` with Hub on and persistence
//!    pointed at the output is recovered by `persistence::recover_all` with
//!    that configuration (the call `main.rs` makes), and the recovered stores
//!    are served by the production router (`create_router`) with a
//!    `HubClient` in the state. The `synap-server` binary itself cannot serve
//!    the reads: in Hub mode it refuses every request, because the HiveHub
//!    SDK cannot validate access keys (hivellm/synap#230). So the `HubClient`
//!    gets an access-key validator that maps one key to the tenant, as
//!    synap-server's `hub_tenant_isolation_tests` does; everything after that
//!    check, tenant scoping included, is the server's own code;
//! 4. reads every resource of the baseline through `/api/v1/command` with the
//!    tenant's key and compares it with the baseline, allowing only for what
//!    the source release never persisted (see [`Release`] and the fixtures
//!    README) and for what any restart changes: expired keys are gone and
//!    unacknowledged deliveries are ready again;
//! 5. checks that the Hub-mode stores equal, name for name under the tenant's
//!    prefix, what this release recovers from the unmigrated directory in
//!    standalone mode: every key with its value and absolute expiry, every
//!    collection, HyperLogLog and bitmap, and every queue with its
//!    configuration and its messages' IDs and metadata;
//! 6. checks the migrated snapshot's stream section against the events the
//!    source served (the server does not restore streams at startup, so they
//!    cannot be read back through it).

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::future::BoxFuture;
use serde_json::{Value, json};
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::core::{GeospatialStore, SynapError, TransactionManager};
use synap_server::hub::{AccessKeyValidator, HubClient, HubConfig, MultiTenant, Plan};
use synap_server::monitoring::{ClientListManager, MonitoringManager};
use synap_server::persistence::types::{PersistenceConfig, SnapshotConfig, WALConfig};
use synap_server::persistence::{
    QueueMessageRecord, RecoveredStores, SnapshotImage, SnapshotManager, StoreRefs, recover_all,
};
use synap_server::{AppState, ScriptManager, ServerConfig, create_router};
use uuid::Uuid;

const TENANT_KEY: &str = "hub-key-migrated-tenant";
const OTHER_KEY: &str = "hub-key-other-tenant";
const MIGRATED_SNAPSHOT: &str = "snapshot-v5-00000000000000000001.bin";

fn tenant() -> Uuid {
    Uuid::parse_str("5a1e0c3b-8d7f-4e26-b9a4-3c2d1e0f9a8b").unwrap()
}

fn other_tenant() -> Uuid {
    Uuid::parse_str("6b2f1d4c-9e80-4f37-8ab5-4d3e2f1a0b9c").unwrap()
}

/// The release that wrote a fixture, and what its data directory can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Release {
    V1_3_2,
    V1_4_0,
}

impl Release {
    fn label(self) -> &'static str {
        match self {
            Release::V1_3_2 => "v1.3.2",
            Release::V1_4_0 => "v1.4.0",
        }
    }

    /// What the migration report says about the snapshot and the WAL.
    fn report_lines(self) -> [&'static str; 2] {
        match self {
            Release::V1_3_2 => ["(format v3)", "(format v1): 22 records, 10 replayed"],
            Release::V1_4_0 => ["(format v5)", "(format v2): 49 records, 19 replayed"],
        }
    }

    /// 1.3.2 persisted no HyperLogLog or bitmap, no hash, list, set or
    /// sorted-set write after a snapshot (it did not log them to the WAL), no
    /// expiry of the keys a snapshot holds, no queue configuration and no
    /// dead letter.
    fn persists_everything(self) -> bool {
        self == Release::V1_4_0
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn fixture_dir(release: Release) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(release.label())
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Every file under `dir` with its bytes.
fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                files.insert(path.strip_prefix(dir).unwrap().to_path_buf(), bytes);
            }
        }
    }
    files
}

/// Run the `synap-migrate` binary.
fn synap_migrate(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_synap-migrate"))
        .args(args)
        .output()
        .unwrap()
}

fn migrate(source: &Path, output: &Path) -> Output {
    let tenant = tenant().to_string();
    synap_migrate(&[
        "migrate".as_ref(),
        "--data-dir".as_ref(),
        source.as_os_str(),
        "--output".as_ref(),
        output.as_os_str(),
        "--user-id".as_ref(),
        tenant.as_ref(),
    ])
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The configuration of a server whose data directory is `data_dir`
/// (`snapshots/`, `wal/synap.wal`), in Hub mode or standalone.
fn server_config(data_dir: &Path, hub: bool) -> ServerConfig {
    let defaults = ServerConfig::default();
    ServerConfig {
        persistence: PersistenceConfig {
            enabled: true,
            wal: WALConfig {
                path: data_dir.join("wal").join("synap.wal"),
                ..WALConfig::default()
            },
            snapshot: SnapshotConfig {
                directory: data_dir.join("snapshots"),
                ..SnapshotConfig::default()
            },
        },
        hub: if hub {
            HubConfig {
                enabled: true,
                api_url: "http://127.0.0.1:9".to_string(),
                service_api_key: "service-key".to_string(),
                usage_report_interval: 300,
                cache_ttl: 60,
                timeout: 30,
            }
        } else {
            defaults.hub.clone()
        },
        ..defaults
    }
}

/// Recover `config`'s data directory the way `synap-server` starts.
async fn recover(config: &ServerConfig) -> RecoveredStores {
    recover_all(
        &config.persistence,
        config.to_kv_config(),
        config.to_queue_config(),
    )
    .await
    .unwrap()
}

/// The HiveHub API as the server sees it: two known access keys.
struct Keys;

impl AccessKeyValidator for Keys {
    fn validate<'a>(
        &'a self,
        access_key: &'a str,
    ) -> BoxFuture<'a, Result<(Uuid, Plan), SynapError>> {
        let owner = match access_key {
            TENANT_KEY => Some(tenant()),
            OTHER_KEY => Some(other_tenant()),
            _ => None,
        };
        Box::pin(async move {
            owner
                .map(|tenant| (tenant, Plan::Pro))
                .ok_or_else(|| SynapError::Unauthorized("unknown access key".to_string()))
        })
    }
}

/// A Hub-mode server recovered from a migrated data directory.
struct HubServer {
    url: String,
    http: reqwest::Client,
    state: AppState,
}

impl HubServer {
    /// Recover `data_dir` with a Hub-mode configuration and serve the
    /// production router over it.
    async fn boot(data_dir: &Path) -> Self {
        let config = server_config(data_dir, true);
        let RecoveredStores {
            kv_store,
            hash_store,
            list_store,
            set_store,
            sorted_set_store,
            queue_manager,
            hyperloglog_store,
            bitmap_store,
            ..
        } = recover(&config).await;
        let kv_store = Arc::new(kv_store);
        let hash_store = Arc::new(hash_store.unwrap());
        let list_store = Arc::new(list_store.unwrap());
        let set_store = Arc::new(set_store.unwrap());
        let sorted_set_store = Arc::new(sorted_set_store.unwrap());
        let hub = HubClient::with_validator(config.hub.clone(), Arc::new(Keys)).unwrap();
        let state = AppState {
            monitoring: Arc::new(MonitoringManager::new(
                kv_store.clone(),
                hash_store.clone(),
                list_store.clone(),
                set_store.clone(),
                sorted_set_store.clone(),
            )),
            transaction_manager: Arc::new(TransactionManager::new(
                kv_store.clone(),
                hash_store.clone(),
                list_store.clone(),
                set_store.clone(),
                sorted_set_store.clone(),
            )),
            geospatial_store: Arc::new(GeospatialStore::new(sorted_set_store.clone())),
            kv_store,
            hash_store,
            list_store,
            set_store,
            sorted_set_store,
            hyperloglog_store: Arc::new(hyperloglog_store),
            bitmap_store: Arc::new(bitmap_store),
            queue_manager: queue_manager.map(Arc::new),
            stream_manager: None,
            partition_manager: None,
            consumer_group_manager: None,
            pubsub_router: None,
            persistence: None,
            script_manager: Arc::new(ScriptManager::default()),
            client_list_manager: Arc::new(ClientListManager::new()),
            cluster_topology: None,
            cluster_migration: None,
            hub_client: Some(Arc::new(hub)),
            user_manager: None,
            require_auth: false,
            replication: None,
            server_config: None,
        };
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
            false,
            false,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        Self {
            url,
            http: reqwest::Client::new(),
            state,
        }
    }

    fn refs(&self) -> StoreRefs<'_> {
        StoreRefs {
            kv_store: &self.state.kv_store,
            hash_store: Some(&self.state.hash_store),
            list_store: Some(&self.state.list_store),
            set_store: Some(&self.state.set_store),
            sorted_set_store: Some(&self.state.sorted_set_store),
            queue_manager: self.state.queue_manager.as_deref(),
            stream_manager: None,
            hyperloglog_store: Some(&self.state.hyperloglog_store),
            bitmap_store: Some(&self.state.bitmap_store),
        }
    }

    /// Run one `/api/v1/command` envelope with the access key `key`.
    async fn envelope(&self, key: &str, command: &str, payload: Value) -> Value {
        let response = self
            .http
            .post(format!("{}/api/v1/command", self.url))
            .header("x-hub-access-key", key)
            .json(&json!({"command": command, "request_id": "r", "payload": payload}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK, "{command}");
        response.json().await.unwrap()
    }

    /// `GET path` with the access key `key`.
    async fn get(&self, key: &str, path: &str) -> Value {
        let response = self
            .http
            .get(format!("{}{path}", self.url))
            .header("x-hub-access-key", key)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK, "{path}");
        response.json().await.unwrap()
    }

    /// The payload of a command the tenant runs successfully.
    async fn read(&self, command: &str, payload: Value) -> Value {
        let response = self.envelope(TENANT_KEY, command, payload.clone()).await;
        assert_eq!(response["success"], true, "{command} {payload}: {response}");
        response["payload"].clone()
    }
}

fn standalone_refs(stores: &RecoveredStores) -> StoreRefs<'_> {
    StoreRefs {
        kv_store: &stores.kv_store,
        hash_store: stores.hash_store.as_ref(),
        list_store: stores.list_store.as_ref(),
        set_store: stores.set_store.as_ref(),
        sorted_set_store: stores.sorted_set_store.as_ref(),
        queue_manager: stores.queue_manager.as_ref(),
        stream_manager: None,
        hyperloglog_store: Some(&stores.hyperloglog_store),
        bitmap_store: Some(&stores.bitmap_store),
    }
}

/// One queue as compared: configuration and every message with its metadata.
#[derive(Debug, PartialEq)]
struct QueueDump {
    config: Value,
    ready: Vec<QueueMessageRecord>,
    in_flight: Vec<QueueMessageRecord>,
    dead_letter: Vec<QueueMessageRecord>,
}

/// Every store's contents in a comparable form. HyperLogLogs and bitmaps are
/// compared by their contents and absolute expiry, not by the bookkeeping
/// timestamps a replayed write stamps with the time of the replay.
#[derive(Debug, PartialEq)]
struct Dump {
    kv: BTreeMap<String, (Vec<u8>, Option<u64>)>,
    hashes: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
    lists: BTreeMap<String, Vec<Vec<u8>>>,
    sets: BTreeMap<String, BTreeSet<Vec<u8>>>,
    sorted_sets: BTreeMap<String, Vec<(Vec<u8>, f64)>>,
    hyperloglogs: BTreeMap<String, (Vec<u8>, Option<u64>)>,
    bitmaps: BTreeMap<String, (Vec<u8>, Option<u64>)>,
    queues: BTreeMap<String, QueueDump>,
}

/// Dump `stores`. With `drop_creation_time`, queue messages compare without
/// their creation time: 1.3.2 stored none, so every read stamps its own.
async fn dump(stores: StoreRefs<'_>, drop_creation_time: bool) -> Dump {
    let image = SnapshotImage::capture(stores).await.unwrap();
    let records = |messages: &[synap_server::core::QueueMessage]| -> Vec<QueueMessageRecord> {
        messages
            .iter()
            .map(|m| {
                let mut record = QueueMessageRecord::from(m);
                if drop_creation_time {
                    record.created_at = 0;
                }
                record
            })
            .collect()
    };
    Dump {
        kv: image
            .kv
            .into_iter()
            .map(|(key, value, expires)| (key, (value.to_vec(), expires)))
            .collect(),
        hashes: image
            .hashes
            .into_iter()
            .map(|(key, fields)| (key, fields.into_iter().collect()))
            .collect(),
        lists: image
            .lists
            .into_iter()
            .map(|(key, list)| (key, list.into_elements()))
            .collect(),
        sets: image
            .sets
            .into_iter()
            .map(|(key, set)| (key, set.into_members().into_iter().collect()))
            .collect(),
        sorted_sets: image
            .sorted_sets
            .into_iter()
            .map(|(key, mut members)| {
                members.sort_by(|a, b| a.0.cmp(&b.0));
                (key, members)
            })
            .collect(),
        hyperloglogs: image
            .hyperloglogs
            .into_iter()
            .map(|(key, hll)| {
                let expires = hll.expires_at_ms();
                (key, (hll.registers, expires))
            })
            .collect(),
        bitmaps: image
            .bitmaps
            .into_iter()
            .map(|(key, bitmap)| {
                let expires = bitmap.ttl_secs.map(|ttl| bitmap.created_at + ttl);
                (key, (bitmap.data, expires))
            })
            .collect(),
        queues: image
            .queues
            .into_iter()
            .map(|(name, state)| {
                let queue = QueueDump {
                    config: serde_json::to_value(&state.config).unwrap(),
                    ready: records(&state.ready),
                    in_flight: records(&state.in_flight),
                    dead_letter: records(&state.dead_letter),
                };
                (name, queue)
            })
            .collect(),
    }
}

/// `dump` with every name in the tenant's Hub namespace.
fn scoped(dump: Dump) -> Dump {
    let t = tenant();
    let key = |name: String| MultiTenant::scope_kv_key(Some(&t), &name).into_owned();
    fn rename<V>(map: BTreeMap<String, V>, f: impl Fn(String) -> String) -> BTreeMap<String, V> {
        map.into_iter().map(|(k, v)| (f(k), v)).collect()
    }
    Dump {
        kv: rename(dump.kv, key),
        hashes: rename(dump.hashes, key),
        lists: rename(dump.lists, key),
        sets: rename(dump.sets, key),
        sorted_sets: rename(dump.sorted_sets, key),
        hyperloglogs: rename(dump.hyperloglogs, key),
        bitmaps: rename(dump.bitmaps, key),
        queues: rename(dump.queues, |n| MultiTenant::scope_queue_name(Some(&t), &n)),
    }
}

fn scoped_key(name: &str) -> String {
    MultiTenant::scope_kv_key(Some(&tenant()), name).into_owned()
}

fn scoped_queue(name: &str) -> String {
    MultiTenant::scope_queue_name(Some(&tenant()), name)
}

/// `value` (a JSON array or object) without the entries `drop` selects.
fn without(value: &Value, drop: impl Fn(&Value) -> bool) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().filter(|v| !drop(v)).cloned().collect()),
        other => other.clone(),
    }
}

async fn migrates_into_hub_mode(release: Release) {
    let fixture = fixture_dir(release);
    let baseline: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.join("baseline.json")).unwrap())
            .unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("source");
    copy_dir(&fixture.join("data"), &source);
    let before = tree(&source);
    let output = scratch.path().join("migrated");

    // 1. The binary migrates the copy, reporting what it read.
    let run = migrate(&source, &output);
    let report = text(&run);
    assert!(run.status.success(), "{report}");
    let source_snapshot = baseline["source"]["snapshot"].as_str().unwrap();
    assert!(report.contains(source_snapshot), "{report}");
    for line in release.report_lines() {
        assert!(report.contains(line), "{line:?} missing from\n{report}");
    }
    for structure in [
        "KV keys:",
        "hashes:",
        "lists:",
        "sets:",
        "sorted sets:",
        "HyperLogLogs:",
        "bitmaps:",
        "queues:",
        "streams:",
    ] {
        assert!(
            report.contains(structure),
            "{structure} missing from\n{report}"
        );
    }
    if !release.persists_everything() {
        assert!(report.contains("stores no key expiry"), "{report}");
        assert!(report.contains("stores no queue configuration"), "{report}");
    }

    // 2. The source is unchanged; the output is one current-format snapshot.
    assert_eq!(tree(&source), before, "the source directory changed");
    let migrated = Path::new("snapshots").join(MIGRATED_SNAPSHOT);
    assert_eq!(
        tree(&output).into_keys().collect::<Vec<_>>(),
        vec![migrated.clone()]
    );

    // 6. The stream section carries the events the source snapshot held, with
    //    their IDs, offsets and timestamps, under the tenant's name.
    check_streams(&output.join(&migrated), &baseline).await;

    // 3. Boot Hub mode on the output.
    let hub = HubServer::boot(&output).await;

    // 5. Hub mode holds exactly what this release recovers from the source in
    //    standalone mode, every name under the tenant's prefix. Recovery
    //    repairs and upgrades the WAL in place, so it runs on another copy.
    let standalone_dir = scratch.path().join("standalone");
    copy_dir(&source, &standalone_dir);
    let standalone = recover(&server_config(&standalone_dir, false)).await;
    let drop_creation_time = release == Release::V1_3_2;
    let expected = scoped(dump(standalone_refs(&standalone), drop_creation_time).await);
    let found = dump(hub.refs(), drop_creation_time).await;
    assert_eq!(found, expected);

    // 4. The tenant reads what the source served.
    check_kv(&hub, &found, &baseline, release).await;
    check_collections(&hub, &found, &baseline, release).await;
    check_queues(&hub, &found, &baseline, release).await;

    // Another tenant sees none of it.
    let other = hub
        .envelope(OTHER_KEY, "kv.get", json!({"key": "plain"}))
        .await;
    assert_eq!(other["payload"], Value::Null, "{other}");
    let other_queues = hub.get(OTHER_KEY, "/queue/list").await;
    assert_eq!(other_queues["queues"], json!([]), "{other_queues}");
}

async fn check_streams(snapshot_path: &Path, baseline: &Value) {
    let manager = SnapshotManager::new(SnapshotConfig {
        directory: snapshot_path.parent().unwrap().to_path_buf(),
        ..SnapshotConfig::default()
    });
    let snapshot = manager.load(snapshot_path).await.unwrap();
    assert_eq!(snapshot.version, 5);
    assert_eq!(snapshot.stream_data.len(), 1);
    let room = &snapshot.stream_data[&MultiTenant::scope_stream_name(Some(&tenant()), "feed")];
    // The event published after the snapshot was never persisted.
    let served = without(&baseline["streams"]["feed"]["events"], |e| {
        e["event"] == "after-snapshot"
    });
    let served = served.as_array().unwrap();
    assert_eq!(room.len(), served.len());
    assert_eq!(room.len(), 3);
    for (event, served) in room.iter().zip(served) {
        assert_eq!(event.id, served["id"].as_str().unwrap());
        assert_eq!(event.offset, served["offset"].as_u64().unwrap());
        assert_eq!(event.event_type, served["event"].as_str().unwrap());
        assert_eq!(event.timestamp, served["timestamp"].as_u64().unwrap());
        let data: Vec<u8> = serde_json::from_value(served["data"].clone()).unwrap();
        assert_eq!(event.data, data);
    }
}

async fn check_kv(hub: &HubServer, found: &Dump, baseline: &Value, release: Release) {
    let now = now_ms();
    for (key, entry) in baseline["kv"].as_object().unwrap() {
        let read_at = entry["ttl_read_at_ms"].as_u64().unwrap();
        let ttl = entry["ttl"]["ttl"].as_u64();
        // `ttl-short` had a 60 s TTL: it expired long before this test.
        let expired = ttl.is_some_and(|ttl| read_at + (ttl + 1) * 1000 <= now);
        let expected = if expired {
            Value::Null
        } else {
            entry["get"].clone()
        };
        let got = hub.read("kv.get", json!({ "key": key })).await;
        assert_eq!(got, expected, "kv.get {key}");
        if expired || entry["get"].is_null() {
            assert!(!found.kv.contains_key(&scoped_key(key)), "{key}");
            continue;
        }

        // No key outlives its deadline in the source: the TTL read was whole
        // seconds remaining, so that deadline is before read_at + (ttl+1) s.
        let deadline = found.kv[&scoped_key(key)].1;
        match (ttl, deadline) {
            (None, None) => {}
            (Some(ttl), Some(deadline)) => {
                assert!(
                    deadline <= read_at + (ttl + 1) * 1000,
                    "{key}: deadline {deadline} is later than in the source"
                );
                let remaining = hub.read("kv.ttl", json!({ "key": key })).await;
                assert!(remaining["ttl"].as_u64().unwrap() > 0, "{key}: {remaining}");
            }
            // A v3 snapshot stores no expiry: the keys it holds load without
            // one, as the server loads them (the report says so).
            (Some(_), None) if !release.persists_everything() && key == "ttl-long" => {}
            (ttl, deadline) => panic!("{key}: source TTL {ttl:?}, migrated deadline {deadline:?}"),
        }
    }
}

async fn check_collections(hub: &HubServer, found: &Dump, baseline: &Value, release: Release) {
    let all = release.persists_everything();

    // Writes after the snapshot that 1.3.2 never logged.
    let mut fields = baseline["hashes"]["user:1"]["fields"].clone();
    if !all {
        fields.as_object_mut().unwrap().remove("city");
    }
    let got = hub.read("hash.getall", json!({"key": "user:1"})).await;
    assert_eq!(got["fields"], fields);

    let mut values = baseline["lists"]["events"]["values"].clone();
    if !all {
        values = without(&values, |v| v == "d");
    }
    let got = hub
        .read(
            "list.lrange",
            json!({"key": "events", "start": 0, "stop": -1}),
        )
        .await;
    assert_eq!(got["values"], values);

    let members = |v: &Value| -> BTreeSet<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|m| m.as_str().unwrap().to_string())
            .filter(|m| all || m != "w")
            .collect()
    };
    let got = hub.read("set.members", json!({"key": "tags"})).await;
    assert_eq!(
        members(&got["members"]),
        members(&baseline["sets"]["tags"]["members"])
    );

    for key in ["scores", "places"] {
        let mut expected = baseline["sorted_sets"][key]["members"].clone();
        if !all {
            expected = without(&expected, |m| m["member"] == "dave");
        }
        let got = hub
            .read(
                "sortedset.zrange",
                json!({"key": key, "start": 0, "stop": -1, "withscores": true}),
            )
            .await;
        assert_eq!(got["members"], expected, "{key}");
    }
    let got = hub
        .read(
            "geospatial.geopos",
            json!({"key": "places", "members": ["sao-paulo", "paris"]}),
        )
        .await;
    assert_eq!(got["coordinates"], baseline["geo"]["places"]["coordinates"]);

    if all {
        let got = hub
            .read("hyperloglog.pfcount", json!({"key": "visitors"}))
            .await;
        assert_eq!(got["count"], baseline["hyperloglogs"]["visitors"]["count"]);
        let got = hub.read("bitmap.bitcount", json!({"key": "flags"})).await;
        assert_eq!(
            got["count"],
            baseline["bitmaps"]["flags"]["bitcount"]["count"]
        );
        for (offset, bit) in baseline["bitmaps"]["flags"]["bits"].as_object().unwrap() {
            let offset: u64 = offset.parse().unwrap();
            let got = hub
                .read("bitmap.getbit", json!({"key": "flags", "offset": offset}))
                .await;
            assert_eq!(got["value"], bit["value"], "bit {offset}");
        }
    } else {
        // 1.3.2 persisted neither.
        assert!(found.hyperloglogs.is_empty());
        assert!(found.bitmaps.is_empty());
    }
}

async fn check_queues(hub: &HubServer, found: &Dump, baseline: &Value, release: Release) {
    let all = release.persists_everything();
    let queues = &baseline["queues"];
    // The command envelope has no tenant-confined queue listing; REST does.
    let mut names: Vec<String> =
        serde_json::from_value(hub.get(TENANT_KEY, "/queue/list").await["queues"].clone()).unwrap();
    names.sort();
    assert_eq!(names, ["dead", "jobs", "late"], "`temp` was deleted");

    // Configuration: 1.4.0 keeps each queue's own; 1.3.2 stored none.
    let config =
        |queue: &str, field: &str| found.queues[&scoped_queue(queue)].config[field].clone();
    if all {
        assert_eq!(config("jobs", "ack_deadline_secs"), 3600);
        assert_eq!(config("late", "max_depth"), 500);
    } else {
        assert_eq!(config("jobs", "ack_deadline_secs"), 30);
        assert_eq!(config("late", "max_depth"), 100_000);
    }

    // Dead letters: kept by 1.4.0, never stored by 1.3.2.
    let dead: Vec<&str> = found.queues[&scoped_queue("dead")]
        .dead_letter
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    let dead_lettered: Vec<&str> = queues["dead"]["dead_lettered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect();
    assert_eq!(dead, if all { dead_lettered } else { vec![] });

    // Every message published and not acknowledged is delivered again with
    // its ID, payload, priority and retry count; deliveries never acknowledged
    // before the stop are ready again.
    let jobs = &queues["jobs"];
    let acked: BTreeSet<&str> = jobs["acked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect();
    let requeued = jobs["requeued_by_nack"][0].as_str().unwrap();
    let expected: BTreeMap<String, String> = jobs["published"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, id)| !acked.contains(id.as_str().unwrap()))
        .map(|(body, id)| (id.as_str().unwrap().to_string(), body.clone()))
        .collect();
    assert_eq!(expected.len(), 4);
    let mut delivered = BTreeMap::new();
    loop {
        let got = hub
            .read(
                "queue.consume",
                json!({"queue": "jobs", "consumer_id": "c"}),
            )
            .await;
        let Some(message) = got["message"].as_object() else {
            break;
        };
        let id = message["id"].as_str().unwrap().to_string();
        let body: Vec<u8> = serde_json::from_value(message["payload"].clone()).unwrap();
        let retries = message["retry_count"].as_u64().unwrap();
        assert_eq!(retries, u64::from(id == requeued), "{id} retry count");
        delivered.insert(id, String::from_utf8(body).unwrap());
    }
    assert_eq!(delivered, expected);

    let late = hub
        .read(
            "queue.consume",
            json!({"queue": "late", "consumer_id": "c"}),
        )
        .await;
    assert_eq!(
        late["message"]["id"], queues["late"]["published"]["late-1"],
        "{late}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_1_3_2_data_directory_migrates_into_hub_mode() {
    migrates_into_hub_mode(Release::V1_3_2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_1_4_0_data_directory_migrates_into_hub_mode() {
    migrates_into_hub_mode(Release::V1_4_0).await;
}

/// A real data directory whose snapshot checksum does not match is rejected:
/// the error names the file, no output is written and the source is
/// byte-for-byte unchanged.
#[test]
fn a_corrupt_snapshot_is_rejected_before_anything_is_written() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("source");
    copy_dir(&fixture_dir(Release::V1_4_0).join("data"), &source);
    let snapshot = source.join("snapshots").join(MIGRATED_SNAPSHOT);
    let mut bytes = std::fs::read(&snapshot).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&snapshot, &bytes).unwrap();
    let before = tree(&source);
    let output = scratch.path().join("migrated");

    let run = migrate(&source, &output);
    let report = text(&run);
    assert!(!run.status.success(), "{report}");
    assert!(report.contains(MIGRATED_SNAPSHOT), "{report}");
    assert!(report.contains("checksum mismatch"), "{report}");
    assert!(!output.exists(), "nothing may be written");
    assert_eq!(tree(&source), before, "the source directory changed");
}
