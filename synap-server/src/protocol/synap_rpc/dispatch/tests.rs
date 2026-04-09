use super::super::types::{Request, SynapValue};
use super::dispatch;
use crate::core::types::KVConfig;
use crate::core::{GeospatialStore, HashStore, HyperLogLogStore, KVStore, SortedSetStore};
use crate::monitoring::{ClientListManager, MonitoringManager};
use crate::scripting::ScriptManager;
use crate::server::handlers::AppState;
use std::sync::Arc;
use std::time::Duration;
fn make_state() -> AppState {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(crate::core::ListStore::new());
    let set_store = Arc::new(crate::core::SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());
    let hyperloglog_store = Arc::new(HyperLogLogStore::new());
    let bitmap_store = Arc::new(crate::core::BitmapStore::new());
    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));
    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let client_list_manager = Arc::new(ClientListManager::new());
    let transaction_manager = Arc::new(crate::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let script_manager = Arc::new(ScriptManager::new(Duration::from_secs(5)));

    AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store,
        bitmap_store,
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager,
        client_list_manager,
        cluster_topology: None,
        cluster_migration: None,
        hub_client: None,
    }
}

fn req(id: u32, cmd: &str, args: Vec<SynapValue>) -> Request {
    Request {
        id,
        command: cmd.to_owned(),
        args,
    }
}

fn str_arg(s: &str) -> SynapValue {
    SynapValue::Str(s.to_owned())
}

fn bytes_arg(b: &[u8]) -> SynapValue {
    SynapValue::Bytes(b.to_vec())
}

#[tokio::test]
async fn test_ping() {
    let state = make_state();
    let resp = dispatch(&state, req(1, "PING", vec![])).await;
    assert_eq!(resp.id, 1);
    assert_eq!(resp.result, Ok(SynapValue::Str("PONG".into())));
}

#[tokio::test]
async fn test_set_returns_ok() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(2, "SET", vec![str_arg("rpc_k1"), bytes_arg(b"val1")]),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Str("OK".into())));
}

#[tokio::test]
async fn test_get_after_set() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "SET", vec![str_arg("rpc_gk"), bytes_arg(b"gval")]),
    )
    .await;
    let resp = dispatch(&state, req(2, "GET", vec![str_arg("rpc_gk")])).await;
    assert_eq!(resp.result, Ok(SynapValue::Bytes(b"gval".to_vec())));
}

#[tokio::test]
async fn test_get_nonexistent_is_null() {
    let state = make_state();
    let resp = dispatch(&state, req(1, "GET", vec![str_arg("rpc_no_such")])).await;
    assert_eq!(resp.result, Ok(SynapValue::Null));
}

#[tokio::test]
async fn test_del_after_set() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "SET", vec![str_arg("rpc_dk"), bytes_arg(b"v")]),
    )
    .await;
    let resp = dispatch(&state, req(2, "DEL", vec![str_arg("rpc_dk")])).await;
    assert_eq!(resp.result, Ok(SynapValue::Int(1)));
}

#[tokio::test]
async fn test_incr_twice() {
    let state = make_state();
    let r1 = dispatch(&state, req(1, "INCR", vec![str_arg("rpc_ctr")])).await;
    assert_eq!(r1.result, Ok(SynapValue::Int(1)));
    let r2 = dispatch(&state, req(2, "INCR", vec![str_arg("rpc_ctr")])).await;
    assert_eq!(r2.result, Ok(SynapValue::Int(2)));
}

#[tokio::test]
async fn test_hset_returns_count() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(
            1,
            "HSET",
            vec![str_arg("rpc_h1"), str_arg("field"), bytes_arg(b"val")],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Int(1)));
}

#[tokio::test]
async fn test_hget_after_hset() {
    let state = make_state();
    dispatch(
        &state,
        req(
            1,
            "HSET",
            vec![str_arg("rpc_h2"), str_arg("f"), bytes_arg(b"v")],
        ),
    )
    .await;
    let resp = dispatch(
        &state,
        req(2, "HGET", vec![str_arg("rpc_h2"), str_arg("f")]),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Bytes(b"v".to_vec())));
}

#[tokio::test]
async fn test_lpush_returns_length() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(1, "LPUSH", vec![str_arg("rpc_lst"), bytes_arg(b"a")]),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Int(1)));
}

#[tokio::test]
async fn test_lpop_after_lpush() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "LPUSH", vec![str_arg("rpc_lst2"), bytes_arg(b"hello")]),
    )
    .await;
    let resp = dispatch(&state, req(2, "LPOP", vec![str_arg("rpc_lst2")])).await;
    assert_eq!(resp.result, Ok(SynapValue::Bytes(b"hello".to_vec())));
}

#[tokio::test]
async fn test_sadd_returns_count() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(
            1,
            "SADD",
            vec![str_arg("rpc_s1"), bytes_arg(b"a"), bytes_arg(b"b")],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Int(2)));
}

#[tokio::test]
async fn test_sismember_true() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "SADD", vec![str_arg("rpc_s2"), bytes_arg(b"m1")]),
    )
    .await;
    let resp = dispatch(
        &state,
        req(2, "SISMEMBER", vec![str_arg("rpc_s2"), bytes_arg(b"m1")]),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Bool(true)));
}

#[tokio::test]
async fn test_zadd_returns_count() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(
            1,
            "ZADD",
            vec![str_arg("rpc_z1"), SynapValue::Float(1.5), bytes_arg(b"m1")],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Int(1)));
}

#[tokio::test]
async fn test_zscore_after_zadd() {
    let state = make_state();
    dispatch(
        &state,
        req(
            1,
            "ZADD",
            vec![str_arg("rpc_z2"), SynapValue::Float(1.5), bytes_arg(b"m1")],
        ),
    )
    .await;
    let resp = dispatch(
        &state,
        req(2, "ZSCORE", vec![str_arg("rpc_z2"), bytes_arg(b"m1")]),
    )
    .await;
    if let Ok(SynapValue::Float(f)) = resp.result {
        assert!((f - 1.5).abs() < 1e-9, "Expected score 1.5, got {f}");
    } else {
        panic!("Expected Float from ZSCORE, got {:?}", resp.result);
    }
}

#[tokio::test]
async fn test_pfadd_result_is_ok() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(
            1,
            "PFADD",
            vec![str_arg("rpc_hll"), bytes_arg(b"e1"), bytes_arg(b"e2")],
        ),
    )
    .await;
    assert!(
        resp.result.is_ok(),
        "PFADD should succeed, got {:?}",
        resp.result
    );
}

#[tokio::test]
async fn test_setbit_returns_previous() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(
            1,
            "SETBIT",
            vec![str_arg("rpc_bm"), SynapValue::Int(0), SynapValue::Int(1)],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Int(0)));
}

#[tokio::test]
async fn test_bitcount_after_setbit() {
    let state = make_state();
    dispatch(
        &state,
        req(
            1,
            "SETBIT",
            vec![str_arg("rpc_bm2"), SynapValue::Int(0), SynapValue::Int(1)],
        ),
    )
    .await;
    let resp = dispatch(&state, req(2, "BITCOUNT", vec![str_arg("rpc_bm2")])).await;
    assert_eq!(resp.result, Ok(SynapValue::Int(1)));
}

#[tokio::test]
async fn test_flushall_returns_ok() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "SET", vec![str_arg("rpc_flush_k"), bytes_arg(b"v")]),
    )
    .await;
    let resp = dispatch(&state, req(2, "FLUSHALL", vec![])).await;
    assert_eq!(resp.result, Ok(SynapValue::Str("OK".into())));
}

#[tokio::test]
async fn test_unknown_command_returns_err() {
    let state = make_state();
    let resp = dispatch(&state, req(99, "NOTAREALCMD", vec![])).await;
    match &resp.result {
        Err(msg) => assert!(
            msg.to_ascii_lowercase().contains("unknown"),
            "Expected 'unknown' in error message, got: {msg}"
        ),
        Ok(v) => panic!("Expected Err for unknown command, got Ok({v:?})"),
    }
}

// ── KV extension tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_append_and_strlen() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "SET", vec![str_arg("rpc_app"), bytes_arg(b"hello")]),
    )
    .await;
    let resp = dispatch(
        &state,
        req(2, "APPEND", vec![str_arg("rpc_app"), bytes_arg(b" world")]),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Int(11)));

    let resp = dispatch(&state, req(3, "STRLEN", vec![str_arg("rpc_app")])).await;
    assert_eq!(resp.result, Ok(SynapValue::Int(11)));
}

#[tokio::test]
async fn test_getset() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "SET", vec![str_arg("rpc_gs"), bytes_arg(b"old")]),
    )
    .await;
    let resp = dispatch(
        &state,
        req(2, "GETSET", vec![str_arg("rpc_gs"), bytes_arg(b"new")]),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Bytes(b"old".to_vec())));
    let resp = dispatch(&state, req(3, "GET", vec![str_arg("rpc_gs")])).await;
    assert_eq!(resp.result, Ok(SynapValue::Bytes(b"new".to_vec())));
}

#[tokio::test]
async fn test_getrange_setrange() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "SET", vec![str_arg("rpc_gr"), bytes_arg(b"abcdefgh")]),
    )
    .await;
    let resp = dispatch(
        &state,
        req(
            2,
            "GETRANGE",
            vec![str_arg("rpc_gr"), SynapValue::Int(2), SynapValue::Int(4)],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Bytes(b"cde".to_vec())));

    let resp = dispatch(
        &state,
        req(
            3,
            "SETRANGE",
            vec![str_arg("rpc_gr"), SynapValue::Int(2), bytes_arg(b"XY")],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Int(8)));
}

#[tokio::test]
async fn test_msetnx_all_new() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(
            1,
            "MSETNX",
            vec![
                str_arg("rpc_nx1"),
                bytes_arg(b"v1"),
                str_arg("rpc_nx2"),
                bytes_arg(b"v2"),
            ],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Bool(true)));
}

#[tokio::test]
async fn test_dbsize_and_kvstats() {
    let state = make_state();
    dispatch(
        &state,
        req(1, "SET", vec![str_arg("rpc_sz_k"), bytes_arg(b"v")]),
    )
    .await;
    let resp = dispatch(&state, req(2, "DBSIZE", vec![])).await;
    match &resp.result {
        Ok(SynapValue::Int(n)) => assert!(*n >= 1),
        other => panic!("unexpected DBSIZE result: {other:?}"),
    }

    let resp = dispatch(&state, req(3, "KVSTATS", vec![])).await;
    assert!(matches!(resp.result, Ok(SynapValue::Map(_))));
}

#[tokio::test]
async fn test_scan_prefix() {
    let state = make_state();
    for i in 0..5 {
        dispatch(
            &state,
            req(
                i,
                "SET",
                vec![str_arg(&format!("scan:{i}")), bytes_arg(b"val")],
            ),
        )
        .await;
    }
    let resp = dispatch(
        &state,
        req(10, "SCAN", vec![str_arg("scan:"), SynapValue::Int(10)]),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Array(keys)) => assert_eq!(keys.len(), 5),
        other => panic!("unexpected SCAN result: {other:?}"),
    }
}

// ── Hash extension tests ──────────────────────────────────────────────────

#[tokio::test]
async fn test_hmset_hmget() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(
            1,
            "HMSET",
            vec![
                str_arg("rpc_hm"),
                str_arg("f1"),
                bytes_arg(b"v1"),
                str_arg("f2"),
                bytes_arg(b"v2"),
            ],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Str("OK".into())));

    let resp = dispatch(
        &state,
        req(
            2,
            "HMGET",
            vec![
                str_arg("rpc_hm"),
                str_arg("f1"),
                str_arg("f2"),
                str_arg("missing"),
            ],
        ),
    )
    .await;
    assert_eq!(
        resp.result,
        Ok(SynapValue::Array(vec![
            SynapValue::Bytes(b"v1".to_vec()),
            SynapValue::Bytes(b"v2".to_vec()),
            SynapValue::Null,
        ]))
    );
}

#[tokio::test]
async fn test_hkeys_hvals() {
    let state = make_state();
    dispatch(
        &state,
        req(
            1,
            "HSET",
            vec![str_arg("rpc_hkv"), str_arg("fa"), bytes_arg(b"va")],
        ),
    )
    .await;
    let resp = dispatch(&state, req(2, "HKEYS", vec![str_arg("rpc_hkv")])).await;
    assert_eq!(
        resp.result,
        Ok(SynapValue::Array(vec![SynapValue::Str("fa".into())]))
    );
    let resp = dispatch(&state, req(3, "HVALS", vec![str_arg("rpc_hkv")])).await;
    assert_eq!(
        resp.result,
        Ok(SynapValue::Array(vec![SynapValue::Bytes(b"va".to_vec())]))
    );
}

// ── HyperLogLog extension tests ───────────────────────────────────────────

#[tokio::test]
async fn test_pfmerge() {
    let state = make_state();
    dispatch(
        &state,
        req(
            1,
            "PFADD",
            vec![str_arg("hll_a"), bytes_arg(b"x"), bytes_arg(b"y")],
        ),
    )
    .await;
    dispatch(
        &state,
        req(
            2,
            "PFADD",
            vec![str_arg("hll_b"), bytes_arg(b"y"), bytes_arg(b"z")],
        ),
    )
    .await;
    let resp = dispatch(
        &state,
        req(
            3,
            "PFMERGE",
            vec![str_arg("hll_dest"), str_arg("hll_a"), str_arg("hll_b")],
        ),
    )
    .await;
    assert!(matches!(resp.result, Ok(SynapValue::Int(_))));

    // Merged count should be ≥ individual counts
    let resp = dispatch(&state, req(4, "PFCOUNT", vec![str_arg("hll_dest")])).await;
    match resp.result {
        Ok(SynapValue::Int(n)) => assert!(n >= 2),
        other => panic!("unexpected PFCOUNT result: {other:?}"),
    }
}

#[tokio::test]
async fn test_hllstats() {
    let state = make_state();
    let resp = dispatch(&state, req(1, "HLLSTATS", vec![])).await;
    assert!(matches!(resp.result, Ok(SynapValue::Map(_))));
}

// ── Geospatial tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_geoadd_geopos_geodist() {
    let state = make_state();
    // GEOADD key lat lon member
    let resp = dispatch(
        &state,
        req(
            1,
            "GEOADD",
            vec![
                str_arg("geo_k"),
                SynapValue::Float(48.8566), // Paris lat
                SynapValue::Float(2.3522),  // Paris lon
                bytes_arg(b"paris"),
                SynapValue::Float(51.5074), // London lat
                SynapValue::Float(-0.1278), // London lon
                bytes_arg(b"london"),
            ],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Int(2)));

    // GEOPOS
    let resp = dispatch(
        &state,
        req(
            2,
            "GEOPOS",
            vec![
                str_arg("geo_k"),
                bytes_arg(b"paris"),
                bytes_arg(b"nonexist"),
            ],
        ),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Array(positions)) => {
            assert_eq!(positions.len(), 2);
            assert!(matches!(positions[0], SynapValue::Array(_)));
            assert_eq!(positions[1], SynapValue::Null);
        }
        other => panic!("unexpected GEOPOS result: {other:?}"),
    }

    // GEODIST (km)
    let resp = dispatch(
        &state,
        req(
            3,
            "GEODIST",
            vec![
                str_arg("geo_k"),
                bytes_arg(b"paris"),
                bytes_arg(b"london"),
                str_arg("km"),
            ],
        ),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Float(dist)) => {
            // Paris–London ≈ 341 km; geospatial encoding loses some precision
            assert!(
                *dist > 200.0 && *dist < 450.0,
                "unexpected distance: {dist}"
            );
        }
        other => panic!("unexpected GEODIST result: {other:?}"),
    }
}

#[tokio::test]
async fn test_geohash() {
    let state = make_state();
    dispatch(
        &state,
        req(
            1,
            "GEOADD",
            vec![
                str_arg("geo_h"),
                SynapValue::Float(48.8566),
                SynapValue::Float(2.3522),
                bytes_arg(b"paris"),
            ],
        ),
    )
    .await;
    let resp = dispatch(
        &state,
        req(
            2,
            "GEOHASH",
            vec![str_arg("geo_h"), bytes_arg(b"paris"), bytes_arg(b"missing")],
        ),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Array(hashes)) => {
            assert_eq!(hashes.len(), 2);
            assert!(matches!(hashes[0], SynapValue::Str(_)));
            assert_eq!(hashes[1], SynapValue::Null);
        }
        other => panic!("unexpected GEOHASH result: {other:?}"),
    }
}

#[tokio::test]
async fn test_georadius() {
    let state = make_state();
    dispatch(
        &state,
        req(
            1,
            "GEOADD",
            vec![
                str_arg("geo_r"),
                SynapValue::Float(48.8566),
                SynapValue::Float(2.3522),
                bytes_arg(b"paris"),
            ],
        ),
    )
    .await;
    let resp = dispatch(
        &state,
        req(
            2,
            "GEORADIUS",
            vec![
                str_arg("geo_r"),
                SynapValue::Float(48.8566), // center lat
                SynapValue::Float(2.3522),  // center lon
                SynapValue::Float(100.0),   // radius
                str_arg("km"),
            ],
        ),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Array(members)) => assert!(!members.is_empty()),
        other => panic!("unexpected GEORADIUS result: {other:?}"),
    }
}

#[tokio::test]
async fn test_geostats() {
    let state = make_state();
    let resp = dispatch(&state, req(1, "GEOSTATS", vec![])).await;
    assert!(matches!(resp.result, Ok(SynapValue::Map(_))));
}

// ── Transaction tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_multi_exec_discard() {
    let state = make_state();
    let client = "tx_client_1".to_owned();

    let resp = dispatch(&state, req(1, "MULTI", vec![str_arg(&client)])).await;
    assert_eq!(resp.result, Ok(SynapValue::Str("OK".into())));

    let resp = dispatch(&state, req(2, "DISCARD", vec![str_arg(&client)])).await;
    assert_eq!(resp.result, Ok(SynapValue::Str("OK".into())));
}

#[tokio::test]
async fn test_watch_unwatch() {
    let state = make_state();
    let client = "tx_client_2";

    let resp = dispatch(
        &state,
        req(1, "WATCH", vec![str_arg(client), str_arg("watched_key")]),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Str("OK".into())));

    let resp = dispatch(&state, req(2, "UNWATCH", vec![str_arg(client)])).await;
    assert_eq!(resp.result, Ok(SynapValue::Str("OK".into())));
}

// ── Scripting tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_script_load_exists_flush() {
    let state = make_state();
    let script = "return 42";

    let resp = dispatch(&state, req(1, "SCRIPT.LOAD", vec![str_arg(script)])).await;
    let sha = match &resp.result {
        Ok(SynapValue::Str(s)) => s.clone(),
        other => panic!("unexpected SCRIPT.LOAD result: {other:?}"),
    };
    assert!(!sha.is_empty());

    let resp = dispatch(
        &state,
        req(
            2,
            "SCRIPT.EXISTS",
            vec![str_arg(&sha), str_arg("nonexistent_sha")],
        ),
    )
    .await;
    assert_eq!(
        resp.result,
        Ok(SynapValue::Array(vec![
            SynapValue::Bool(true),
            SynapValue::Bool(false),
        ]))
    );

    let resp = dispatch(&state, req(3, "SCRIPT.FLUSH", vec![])).await;
    assert_eq!(resp.result, Ok(SynapValue::Int(1)));
}

#[tokio::test]
async fn test_eval_script() {
    let state = make_state();
    let resp = dispatch(
        &state,
        req(
            1,
            "EVAL",
            vec![
                str_arg("return 'hello'"),
                SynapValue::Int(0), // numkeys
            ],
        ),
    )
    .await;
    assert!(matches!(resp.result, Ok(SynapValue::Str(_))));
}

#[tokio::test]
async fn test_script_kill_no_running() {
    let state = make_state();
    let resp = dispatch(&state, req(1, "SCRIPT.KILL", vec![])).await;
    // No script running → returns false
    assert_eq!(resp.result, Ok(SynapValue::Bool(false)));
}

// ── Optional-subsystem error tests ────────────────────────────────────────

#[tokio::test]
async fn test_queue_disabled_returns_err() {
    let state = make_state(); // queue_manager = None
    let resp = dispatch(&state, req(1, "QCREATE", vec![str_arg("q1")])).await;
    match &resp.result {
        Err(msg) => assert!(
            msg.contains("not enabled"),
            "Expected 'not enabled', got: {msg}"
        ),
        Ok(v) => panic!("Expected Err for disabled queue, got Ok({v:?})"),
    }
}

#[tokio::test]
async fn test_stream_disabled_returns_err() {
    let state = make_state(); // stream_manager = None
    let resp = dispatch(&state, req(1, "SCREATE", vec![str_arg("room1")])).await;
    match &resp.result {
        Err(msg) => assert!(
            msg.contains("not enabled"),
            "Expected 'not enabled', got: {msg}"
        ),
        Ok(v) => panic!("Expected Err for disabled stream, got Ok({v:?})"),
    }
}

#[tokio::test]
async fn test_pubsub_disabled_returns_err() {
    let state = make_state(); // pubsub_router = None
    let resp = dispatch(&state, req(1, "SUBSCRIBE", vec![str_arg("topic")])).await;
    match &resp.result {
        Err(msg) => assert!(
            msg.contains("not enabled"),
            "Expected 'not enabled', got: {msg}"
        ),
        Ok(v) => panic!("Expected Err for disabled pubsub, got Ok({v:?})"),
    }
}

// ── Phase 4.1: SynapRPC dispatcher integration tests with real subsystems ──

fn make_state_with_queues() -> AppState {
    use crate::core::{QueueConfig, QueueManager};
    let mut s = make_state();
    s.queue_manager = Some(Arc::new(QueueManager::new(QueueConfig::default())));
    s
}

fn make_state_with_streams() -> AppState {
    use crate::core::{StreamConfig, StreamManager};
    let mut s = make_state();
    s.stream_manager = Some(Arc::new(StreamManager::new(StreamConfig::default())));
    s
}

fn make_state_with_pubsub() -> AppState {
    use crate::core::PubSubRouter;
    let mut s = make_state();
    s.pubsub_router = Some(Arc::new(PubSubRouter::new()));
    s
}

// ── Queue lifecycle (4.1) ─────────────────────────────────────────────────

#[tokio::test]
async fn test_queue_create_publish_consume_ack_lifecycle() {
    let state = make_state_with_queues();

    // QCREATE
    let resp = dispatch(&state, req(1, "QCREATE", vec![str_arg("q_lifecycle")])).await;
    assert_eq!(
        resp.result,
        Ok(SynapValue::Str("OK".into())),
        "QCREATE failed"
    );

    // QLIST — queue should appear
    let resp = dispatch(&state, req(2, "QLIST", vec![])).await;
    match &resp.result {
        Ok(SynapValue::Array(names)) => {
            assert!(
                names.contains(&SynapValue::Str("q_lifecycle".into())),
                "queue not in QLIST: {:?}",
                names
            );
        }
        other => panic!("unexpected QLIST result: {other:?}"),
    }

    // QPUBLISH
    let resp = dispatch(
        &state,
        req(
            3,
            "QPUBLISH",
            vec![str_arg("q_lifecycle"), bytes_arg(b"hello_queue")],
        ),
    )
    .await;
    let msg_id = match &resp.result {
        Ok(SynapValue::Str(id)) => id.clone(),
        other => panic!("unexpected QPUBLISH result: {other:?}"),
    };
    assert!(!msg_id.is_empty());

    // QSTATS — depth should be 1
    let resp = dispatch(&state, req(4, "QSTATS", vec![str_arg("q_lifecycle")])).await;
    match &resp.result {
        Ok(SynapValue::Map(pairs)) => {
            let depth = pairs
                .iter()
                .find(|(k, _)| *k == SynapValue::Str("depth".into()))
                .map(|(_, v)| v.clone());
            assert_eq!(
                depth,
                Some(SynapValue::Int(1)),
                "expected depth=1 after publish"
            );
        }
        other => panic!("unexpected QSTATS result: {other:?}"),
    }

    // QCONSUME
    let resp = dispatch(
        &state,
        req(
            5,
            "QCONSUME",
            vec![str_arg("q_lifecycle"), str_arg("consumer_1")],
        ),
    )
    .await;
    let consumed_id = match &resp.result {
        Ok(SynapValue::Map(pairs)) => pairs
            .iter()
            .find(|(k, _)| *k == SynapValue::Str("id".into()))
            .map(|(_, v)| {
                if let SynapValue::Str(s) = v {
                    s.clone()
                } else {
                    panic!()
                }
            })
            .expect("id field missing from QCONSUME result"),
        other => panic!("unexpected QCONSUME result: {other:?}"),
    };
    assert!(!consumed_id.is_empty());

    // QACK
    let resp = dispatch(
        &state,
        req(
            6,
            "QACK",
            vec![str_arg("q_lifecycle"), str_arg(&consumed_id)],
        ),
    )
    .await;
    assert_eq!(resp.result, Ok(SynapValue::Str("OK".into())), "QACK failed");

    // QPURGE — publish another then purge
    dispatch(
        &state,
        req(
            7,
            "QPUBLISH",
            vec![str_arg("q_lifecycle"), bytes_arg(b"purge_me")],
        ),
    )
    .await;
    let resp = dispatch(&state, req(8, "QPURGE", vec![str_arg("q_lifecycle")])).await;
    match &resp.result {
        Ok(SynapValue::Int(n)) => assert!(*n >= 1, "QPURGE should report purged count"),
        other => panic!("unexpected QPURGE result: {other:?}"),
    }

    // QDELETE
    let resp = dispatch(&state, req(9, "QDELETE", vec![str_arg("q_lifecycle")])).await;
    assert!(resp.result.is_ok(), "QDELETE failed: {:?}", resp.result);
}

#[tokio::test]
async fn test_queue_nack_requeues_message() {
    let state = make_state_with_queues();

    dispatch(&state, req(1, "QCREATE", vec![str_arg("q_nack")])).await;
    dispatch(
        &state,
        req(
            2,
            "QPUBLISH",
            vec![str_arg("q_nack"), bytes_arg(b"retry_me")],
        ),
    )
    .await;

    // Consume and NACK (requeue=true)
    let resp = dispatch(
        &state,
        req(3, "QCONSUME", vec![str_arg("q_nack"), str_arg("c1")]),
    )
    .await;
    let consumed_id = match &resp.result {
        Ok(SynapValue::Map(pairs)) => pairs
            .iter()
            .find(|(k, _)| *k == SynapValue::Str("id".into()))
            .map(|(_, v)| {
                if let SynapValue::Str(s) = v {
                    s.clone()
                } else {
                    panic!()
                }
            })
            .expect("id missing"),
        other => panic!("QCONSUME failed: {other:?}"),
    };

    let resp = dispatch(
        &state,
        req(
            4,
            "QNACK",
            vec![str_arg("q_nack"), str_arg(&consumed_id), str_arg("true")],
        ),
    )
    .await;
    assert_eq!(
        resp.result,
        Ok(SynapValue::Str("OK".into())),
        "QNACK with requeue=true failed"
    );
}

// ── Stream lifecycle (4.1) ────────────────────────────────────────────────

#[tokio::test]
async fn test_stream_create_publish_read_stats_delete_lifecycle() {
    let state = make_state_with_streams();

    // SCREATE
    let resp = dispatch(&state, req(1, "SCREATE", vec![str_arg("room_lifecycle")])).await;
    assert_eq!(
        resp.result,
        Ok(SynapValue::Str("OK".into())),
        "SCREATE failed"
    );

    // SLIST — room should appear
    let resp = dispatch(&state, req(2, "SLIST", vec![])).await;
    match &resp.result {
        Ok(SynapValue::Array(rooms)) => {
            assert!(
                rooms.contains(&SynapValue::Str("room_lifecycle".into())),
                "room not in SLIST: {:?}",
                rooms
            );
        }
        other => panic!("unexpected SLIST result: {other:?}"),
    }

    // SPUBLISH
    let resp = dispatch(
        &state,
        req(
            3,
            "SPUBLISH",
            vec![
                str_arg("room_lifecycle"),
                str_arg("click"),
                bytes_arg(b"{\"x\":42}"),
            ],
        ),
    )
    .await;
    let offset = match &resp.result {
        Ok(SynapValue::Int(o)) => *o,
        other => panic!("unexpected SPUBLISH result: {other:?}"),
    };
    assert_eq!(offset, 0, "first publish should get offset 0");

    // SREAD — should return 1 event
    let resp = dispatch(
        &state,
        req(
            4,
            "SREAD",
            vec![
                str_arg("room_lifecycle"),
                str_arg("sub_1"),
                SynapValue::Int(0),  // from_offset
                SynapValue::Int(10), // limit
            ],
        ),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Array(events)) => {
            assert_eq!(events.len(), 1, "expected 1 event from SREAD");
        }
        other => panic!("unexpected SREAD result: {other:?}"),
    }

    // SSTATS
    let resp = dispatch(&state, req(5, "SSTATS", vec![str_arg("room_lifecycle")])).await;
    match &resp.result {
        Ok(SynapValue::Map(pairs)) => {
            let msg_count = pairs
                .iter()
                .find(|(k, _)| *k == SynapValue::Str("message_count".into()))
                .map(|(_, v)| v.clone());
            assert_eq!(
                msg_count,
                Some(SynapValue::Int(1)),
                "expected message_count=1 in SSTATS"
            );
        }
        other => panic!("unexpected SSTATS result: {other:?}"),
    }

    // SDELETE
    let resp = dispatch(&state, req(6, "SDELETE", vec![str_arg("room_lifecycle")])).await;
    assert_eq!(
        resp.result,
        Ok(SynapValue::Str("OK".into())),
        "SDELETE failed"
    );
}

#[tokio::test]
async fn test_stream_replay_from_offset() {
    let state = make_state_with_streams();

    dispatch(&state, req(1, "SCREATE", vec![str_arg("room_replay")])).await;

    // Publish 3 events
    for i in 0u8..3 {
        dispatch(
            &state,
            req(
                i as u32 + 2,
                "SPUBLISH",
                vec![str_arg("room_replay"), str_arg("tick"), bytes_arg(&[i])],
            ),
        )
        .await;
    }

    // SREAD from offset 1 — should return events 1 and 2 only
    let resp = dispatch(
        &state,
        req(
            10,
            "SREAD",
            vec![
                str_arg("room_replay"),
                str_arg("sub_2"),
                SynapValue::Int(1),  // from_offset
                SynapValue::Int(10), // limit
            ],
        ),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Array(events)) => {
            assert_eq!(events.len(), 2, "expected 2 events from offset 1");
        }
        other => panic!("unexpected SREAD replay result: {other:?}"),
    }
}

// ── Pub/Sub dispatch tests (4.1) ──────────────────────────────────────────

#[tokio::test]
async fn test_pubsub_subscribe_returns_subscriber_id() {
    let state = make_state_with_pubsub();

    let resp = dispatch(
        &state,
        req(1, "SUBSCRIBE", vec![str_arg("news"), str_arg("sports")]),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Map(pairs)) => {
            let sub_id = pairs
                .iter()
                .find(|(k, _)| *k == SynapValue::Str("subscriber_id".into()))
                .map(|(_, v)| v.clone());
            assert!(
                matches!(sub_id, Some(SynapValue::Str(ref s)) if !s.is_empty()),
                "subscriber_id should be a non-empty string"
            );

            let count = pairs
                .iter()
                .find(|(k, _)| *k == SynapValue::Str("subscription_count".into()))
                .map(|(_, v)| v.clone());
            assert_eq!(
                count,
                Some(SynapValue::Int(2)),
                "subscription_count should be 2"
            );
        }
        other => panic!("unexpected SUBSCRIBE result: {other:?}"),
    }
}

#[tokio::test]
async fn test_pubsub_publish_matches_subscribers() {
    let state = make_state_with_pubsub();

    // Subscribe to topic
    dispatch(&state, req(1, "SUBSCRIBE", vec![str_arg("alerts")])).await;

    // Publish to same topic
    let resp = dispatch(
        &state,
        req(
            2,
            "PUBLISH",
            vec![str_arg("alerts"), str_arg("{\"level\":\"warn\"}")],
        ),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Map(pairs)) => {
            let matched = pairs
                .iter()
                .find(|(k, _)| *k == SynapValue::Str("subscribers_matched".into()))
                .map(|(_, v)| v.clone());
            assert_eq!(
                matched,
                Some(SynapValue::Int(1)),
                "expected 1 subscriber matched"
            );
        }
        other => panic!("unexpected PUBLISH result: {other:?}"),
    }
}

#[tokio::test]
async fn test_pubsub_topics_lists_active_topics() {
    let state = make_state_with_pubsub();

    dispatch(&state, req(1, "SUBSCRIBE", vec![str_arg("topic_a")])).await;
    dispatch(&state, req(2, "SUBSCRIBE", vec![str_arg("topic_b")])).await;

    let resp = dispatch(&state, req(3, "TOPICS", vec![])).await;
    match &resp.result {
        Ok(SynapValue::Array(topics)) => {
            let topic_strs: Vec<String> = topics
                .iter()
                .filter_map(|v| {
                    if let SynapValue::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect();
            assert!(
                topic_strs.contains(&"topic_a".to_owned()),
                "topic_a missing: {:?}",
                topic_strs
            );
            assert!(
                topic_strs.contains(&"topic_b".to_owned()),
                "topic_b missing: {:?}",
                topic_strs
            );
        }
        other => panic!("unexpected TOPICS result: {other:?}"),
    }
}

#[tokio::test]
async fn test_pubsub_unsubscribe_removes_subscription() {
    let state = make_state_with_pubsub();

    let sub_resp = dispatch(&state, req(1, "SUBSCRIBE", vec![str_arg("ch_unsub")])).await;
    let sub_id = match &sub_resp.result {
        Ok(SynapValue::Map(pairs)) => pairs
            .iter()
            .find(|(k, _)| *k == SynapValue::Str("subscriber_id".into()))
            .map(|(_, v)| {
                if let SynapValue::Str(s) = v {
                    s.clone()
                } else {
                    panic!()
                }
            })
            .expect("subscriber_id missing"),
        other => panic!("SUBSCRIBE failed: {other:?}"),
    };

    let resp = dispatch(&state, req(2, "UNSUBSCRIBE", vec![str_arg(&sub_id)])).await;
    match &resp.result {
        Ok(SynapValue::Int(n)) => assert!(*n >= 1, "expected unsubscribed ≥ 1, got {n}"),
        other => panic!("unexpected UNSUBSCRIBE result: {other:?}"),
    }
}

// ── Phase 4.3: Server-push integration test ───────────────────────────────

#[tokio::test]
async fn test_pubsub_server_push_delivers_to_registered_channel() {
    use crate::core::pubsub::Message;
    use tokio::sync::mpsc;

    let state = make_state_with_pubsub();

    // Subscribe via dispatch — get back the subscriber_id
    let sub_resp = dispatch(&state, req(1, "SUBSCRIBE", vec![str_arg("push.events")])).await;
    let subscriber_id = match &sub_resp.result {
        Ok(SynapValue::Map(pairs)) => pairs
            .iter()
            .find(|(k, _)| *k == SynapValue::Str("subscriber_id".into()))
            .map(|(_, v)| {
                if let SynapValue::Str(s) = v {
                    s.clone()
                } else {
                    panic!()
                }
            })
            .expect("subscriber_id missing from SUBSCRIBE result"),
        other => panic!("SUBSCRIBE failed: {other:?}"),
    };

    // Register an mpsc channel for this subscriber (simulates the connection layer)
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    state
        .pubsub_router
        .as_ref()
        .unwrap()
        .register_connection(subscriber_id, tx);

    // Publish a message via dispatch
    let pub_resp = dispatch(
        &state,
        req(
            2,
            "PUBLISH",
            vec![str_arg("push.events"), str_arg("{\"kind\":\"test\"}")],
        ),
    )
    .await;
    assert!(
        pub_resp.result.is_ok(),
        "PUBLISH failed: {:?}",
        pub_resp.result
    );

    // Verify the push frame arrived on the channel (no await — unbounded channel is sync)
    let msg = rx
        .try_recv()
        .expect("expected push frame in channel after PUBLISH");
    assert_eq!(msg.topic, "push.events");
}

#[tokio::test]
async fn test_stream_reactive_read_after_publish() {
    let state = make_state_with_streams();

    // Create room and publish two events
    dispatch(&state, req(1, "SCREATE", vec![str_arg("room_reactive")])).await;
    dispatch(
        &state,
        req(
            2,
            "SPUBLISH",
            vec![str_arg("room_reactive"), str_arg("a"), bytes_arg(b"1")],
        ),
    )
    .await;
    dispatch(
        &state,
        req(
            3,
            "SPUBLISH",
            vec![str_arg("room_reactive"), str_arg("b"), bytes_arg(b"2")],
        ),
    )
    .await;

    // Consumer reads from offset 0 — should get both events
    let resp = dispatch(
        &state,
        req(
            4,
            "SREAD",
            vec![
                str_arg("room_reactive"),
                str_arg("reactive_sub"),
                SynapValue::Int(0),
                SynapValue::Int(100),
            ],
        ),
    )
    .await;
    match &resp.result {
        Ok(SynapValue::Array(events)) => {
            assert_eq!(
                events.len(),
                2,
                "reactive consumer expected 2 events, got {}",
                events.len()
            );
            // Verify each event is a Map with expected fields
            for ev in events {
                assert!(matches!(ev, SynapValue::Map(_)), "event should be a Map");
            }
        }
        other => panic!("unexpected SREAD result: {other:?}"),
    }
}
