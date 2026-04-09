//! RESP3 command → AppState dispatch.
//!
//! Maps the command name extracted from the parsed RESP3 array to the
//! appropriate store method and returns a `Resp3Value` response.
//! Reuses the same store handles as the HTTP handlers — no logic duplication.

use std::str;

use super::parser::Resp3Value;
use crate::server::handlers::AppState;

pub mod advanced;
pub mod collections;
pub mod kv;

/// Dispatch one parsed RESP3 command array and return the response value.
///
/// `args[0]` is the command name (case-insensitive).
/// `args[1..]` are the positional arguments.
#[tracing::instrument(
    name = "resp3.dispatch",
    skip(state, args),
    fields(cmd = args.first().and_then(|a| a.as_str()).unwrap_or("?"))
)]
pub async fn dispatch(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.is_empty() {
        return Resp3Value::Error("ERR empty command".into());
    }

    let cmd = match args[0].as_str() {
        Some(s) => s.to_ascii_uppercase(),
        None => return Resp3Value::Error("ERR command must be a string".into()),
    };

    match cmd.as_str() {
        "PING" => kv::cmd_ping(args),
        "QUIT" => Resp3Value::SimpleString("OK".into()),
        "SELECT" => Resp3Value::SimpleString("OK".into()), // single DB only

        "SET" => kv::cmd_set(state, args).await,
        "GET" => kv::cmd_get(state, args).await,
        "DEL" => kv::cmd_del(state, args).await,
        "EXISTS" => kv::cmd_exists(state, args).await,
        "EXPIRE" => kv::cmd_expire(state, args).await,
        "TTL" => kv::cmd_ttl(state, args).await,
        "PERSIST" => kv::cmd_persist(state, args).await,
        "INCR" => kv::cmd_incr(state, args).await,
        "INCRBY" => kv::cmd_incrby(state, args).await,
        "DECR" => kv::cmd_decr(state, args).await,
        "DECRBY" => kv::cmd_decrby(state, args).await,
        "MSET" => kv::cmd_mset(state, args).await,
        "MGET" => kv::cmd_mget(state, args).await,
        "KEYS" => kv::cmd_keys(state, args).await,
        "SCAN" => kv::cmd_scan(state, args).await,

        "HSET" => collections::cmd_hset(state, args).await,
        "HGET" => collections::cmd_hget(state, args).await,
        "HDEL" => collections::cmd_hdel(state, args).await,
        "HGETALL" => collections::cmd_hgetall(state, args).await,
        "HMSET" => collections::cmd_hmset(state, args).await,
        "HMGET" => collections::cmd_hmget(state, args).await,
        "HLEN" => collections::cmd_hlen(state, args).await,
        "HEXISTS" => collections::cmd_hexists(state, args).await,

        "LPUSH" => collections::cmd_lpush(state, args).await,
        "RPUSH" => collections::cmd_rpush(state, args).await,
        "LPOP" => collections::cmd_lpop(state, args).await,
        "RPOP" => collections::cmd_rpop(state, args).await,
        "LRANGE" => collections::cmd_lrange(state, args).await,
        "LLEN" => collections::cmd_llen(state, args).await,

        "SADD" => collections::cmd_sadd(state, args).await,
        "SMEMBERS" => collections::cmd_smembers(state, args).await,
        "SREM" => collections::cmd_srem(state, args).await,
        "SISMEMBER" => collections::cmd_sismember(state, args).await,
        "SCARD" => collections::cmd_scard(state, args).await,

        "ZADD" => collections::cmd_zadd(state, args).await,
        "ZRANGE" => collections::cmd_zrange(state, args).await,
        "ZSCORE" => collections::cmd_zscore(state, args).await,
        "ZCARD" => collections::cmd_zcard(state, args).await,
        "ZREM" => collections::cmd_zrem(state, args).await,

        "PFADD" => collections::cmd_pfadd(state, args).await,
        "PFCOUNT" => collections::cmd_pfcount(state, args).await,

        "BITCOUNT" => collections::cmd_bitcount(state, args).await,
        "SETBIT" => collections::cmd_setbit(state, args).await,
        "GETBIT" => collections::cmd_getbit(state, args).await,

        "FLUSHALL" | "FLUSHDB" => kv::cmd_flushall(state).await,

        // ── Queue (3.1) ───────────────────────────────────────────────────────────
        "QCREATE" => advanced::cmd_qcreate(state, args).await,
        "QDELETE" => advanced::cmd_qdelete(state, args).await,
        "QLIST" => advanced::cmd_qlist(state).await,
        "QPUBLISH" => advanced::cmd_qpublish(state, args).await,
        "QCONSUME" => advanced::cmd_qconsume(state, args).await,
        "QACK" => advanced::cmd_qack(state, args).await,
        "QNACK" => advanced::cmd_qnack(state, args).await,
        "QSTATS" => advanced::cmd_qstats(state, args).await,
        "QPURGE" => advanced::cmd_qpurge(state, args).await,

        // ── Stream — Redis X* names (3.2) ─────────────────────────────────────────
        "XADD" => advanced::cmd_xadd(state, args).await,
        "XREAD" => advanced::cmd_xread(state, args).await,
        "XREADGROUP" => advanced::cmd_xreadgroup(state, args).await,
        "XRANGE" => advanced::cmd_xrange(state, args).await,
        "XDEL" => advanced::cmd_xdel(state, args).await,
        "XINFO" => advanced::cmd_xinfo(state, args).await,
        "XACK" => advanced::cmd_xack(state, args).await,

        // ── Pub/Sub (3.3) ────────────────────────────────────────────────────────
        "PUBLISH" => advanced::cmd_publish(state, args).await,
        "SUBSCRIBE" => advanced::cmd_subscribe(state, args).await,
        "UNSUBSCRIBE" => advanced::cmd_unsubscribe(state, args).await,
        "PSUBSCRIBE" => advanced::cmd_psubscribe(state, args).await,
        "PUBSUB" => advanced::cmd_pubsub(state, args).await,

        // ── Transactions (3.4) ───────────────────────────────────────────────────
        "MULTI" => advanced::cmd_multi(state, args).await,
        "EXEC" => advanced::cmd_exec(state, args).await,
        "DISCARD" => advanced::cmd_discard(state, args).await,
        "WATCH" => advanced::cmd_watch(state, args).await,
        "UNWATCH" => advanced::cmd_unwatch(state, args).await,

        // ── Scripts (3.5) ────────────────────────────────────────────────────────
        "EVAL" => advanced::cmd_eval(state, args).await,
        "EVALSHA" => advanced::cmd_evalsha(state, args).await,
        "SCRIPT" => advanced::cmd_script(state, args).await,

        // ── HyperLogLog (3.6) ────────────────────────────────────────────────────
        "PFMERGE" => collections::cmd_pfmerge(state, args).await,

        // ── Geospatial (3.7) ─────────────────────────────────────────────────────
        "GEOADD" => advanced::cmd_geoadd(state, args).await,
        "GEOPOS" => advanced::cmd_geopos(state, args).await,
        "GEODIST" => advanced::cmd_geodist(state, args).await,
        "GEOHASH" => advanced::cmd_geohash(state, args).await,
        "GEORADIUS" => advanced::cmd_georadius(state, args).await,
        "GEORADIUSBYMEMBER" => advanced::cmd_georadiusbymember(state, args).await,
        "GEOSEARCH" => advanced::cmd_geosearch(state, args).await,

        // ── KV stats (3.8) ────────────────────────────────────────────────────────
        "SYNAP.KVSTATS" => kv::cmd_synap_kvstats(state).await,

        // ── Additional KV / hash parity ──────────────────────────────────────────
        "APPEND" => kv::cmd_append(state, args).await,
        "GETRANGE" => kv::cmd_getrange(state, args).await,
        "SETRANGE" => kv::cmd_setrange(state, args).await,
        "STRLEN" => kv::cmd_strlen(state, args).await,
        "GETSET" => kv::cmd_getset(state, args).await,
        "MSETNX" => kv::cmd_msetnx(state, args).await,
        "DBSIZE" => kv::cmd_dbsize(state).await,
        "HKEYS" => collections::cmd_hkeys(state, args).await,
        "HVALS" => collections::cmd_hvals(state, args).await,

        _ => Resp3Value::Error(format!("ERR unknown command '{cmd}'")),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn arg_str(args: &[Resp3Value], idx: usize) -> Option<String> {
    args.get(idx)?.as_str().map(|s| s.to_owned())
}

pub(crate) fn arg_bytes(args: &[Resp3Value], idx: usize) -> Option<Vec<u8>> {
    args.get(idx)?.as_bytes().map(|b| b.to_vec())
}

pub(crate) fn arg_i64(args: &[Resp3Value], idx: usize) -> Option<i64> {
    args.get(idx)?.as_str()?.parse().ok()
}

pub(crate) fn arg_u64(args: &[Resp3Value], idx: usize) -> Option<u64> {
    args.get(idx)?.as_str()?.parse().ok()
}

pub(crate) fn arg_f64(args: &[Resp3Value], idx: usize) -> Option<f64> {
    args.get(idx)?.as_str()?.parse().ok()
}

pub(crate) fn err_wrong_args(cmd: &str) -> Resp3Value {
    Resp3Value::Error(format!("ERR wrong number of arguments for '{cmd}' command"))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn args(strs: &[&str]) -> Vec<Resp3Value> {
        strs.iter()
            .map(|s| Resp3Value::BulkString(s.as_bytes().to_vec()))
            .collect()
    }

    #[tokio::test]
    async fn test_ping_no_arg() {
        let state = make_state();
        let result = dispatch(&state, &args(&["PING"])).await;
        assert_eq!(result, Resp3Value::SimpleString("PONG".into()));
    }

    #[tokio::test]
    async fn test_ping_with_message() {
        let state = make_state();
        let result = dispatch(&state, &args(&["PING", "hello"])).await;
        assert_eq!(result, Resp3Value::BulkString(b"hello".to_vec()));
    }

    #[tokio::test]
    async fn test_set_returns_ok() {
        let state = make_state();
        let result = dispatch(&state, &args(&["SET", "mykey", "myval"])).await;
        assert_eq!(result, Resp3Value::SimpleString("OK".into()));
    }

    #[tokio::test]
    async fn test_get_after_set() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "getkey", "getval"])).await;
        let result = dispatch(&state, &args(&["GET", "getkey"])).await;
        assert_eq!(result, Resp3Value::BulkString(b"getval".to_vec()));
    }

    #[tokio::test]
    async fn test_get_nonexistent_is_null() {
        let state = make_state();
        let result = dispatch(&state, &args(&["GET", "no_such_key_xyz"])).await;
        assert_eq!(result, Resp3Value::Null);
    }

    #[tokio::test]
    async fn test_del_existing_key() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "del_k", "v"])).await;
        let result = dispatch(&state, &args(&["DEL", "del_k"])).await;
        assert_eq!(result, Resp3Value::Integer(1));
    }

    #[tokio::test]
    async fn test_del_nonexistent_key() {
        let state = make_state();
        let result = dispatch(&state, &args(&["DEL", "ghost_key_del"])).await;
        assert_eq!(result, Resp3Value::Integer(0));
    }

    #[tokio::test]
    async fn test_exists_after_set() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "exists_k", "v"])).await;
        let result = dispatch(&state, &args(&["EXISTS", "exists_k"])).await;
        assert_eq!(result, Resp3Value::Integer(1));
    }

    #[tokio::test]
    async fn test_exists_nonexistent() {
        let state = make_state();
        let result = dispatch(&state, &args(&["EXISTS", "ghost_exists_xyz"])).await;
        assert_eq!(result, Resp3Value::Integer(0));
    }

    #[tokio::test]
    async fn test_incr_counter() {
        let state = make_state();
        let r1 = dispatch(&state, &args(&["INCR", "ctr1"])).await;
        assert_eq!(r1, Resp3Value::Integer(1));
        let r2 = dispatch(&state, &args(&["INCR", "ctr1"])).await;
        assert_eq!(r2, Resp3Value::Integer(2));
    }

    #[tokio::test]
    async fn test_mset_returns_ok() {
        let state = make_state();
        let result = dispatch(&state, &args(&["MSET", "mk1", "mv1", "mk2", "mv2"])).await;
        assert_eq!(result, Resp3Value::SimpleString("OK".into()));
    }

    #[tokio::test]
    async fn test_mget_after_mset() {
        let state = make_state();
        dispatch(&state, &args(&["MSET", "mg1", "va", "mg2", "vb"])).await;
        let result = dispatch(&state, &args(&["MGET", "mg1", "mg2", "mg_missing"])).await;
        assert_eq!(
            result,
            Resp3Value::Array(vec![
                Resp3Value::BulkString(b"va".to_vec()),
                Resp3Value::BulkString(b"vb".to_vec()),
                Resp3Value::Null,
            ])
        );
    }

    #[tokio::test]
    async fn test_hset_returns_field_count() {
        let state = make_state();
        let result = dispatch(&state, &args(&["HSET", "h1", "f1", "v1"])).await;
        assert_eq!(result, Resp3Value::Integer(1));
    }

    #[tokio::test]
    async fn test_hget_after_hset() {
        let state = make_state();
        dispatch(&state, &args(&["HSET", "h2", "field", "val"])).await;
        let result = dispatch(&state, &args(&["HGET", "h2", "field"])).await;
        assert_eq!(result, Resp3Value::BulkString(b"val".to_vec()));
    }

    #[tokio::test]
    async fn test_hgetall_returns_pairs() {
        let state = make_state();
        dispatch(&state, &args(&["HSET", "h3", "f1", "v1"])).await;
        let result = dispatch(&state, &args(&["HGETALL", "h3"])).await;
        if let Resp3Value::Array(items) = result {
            assert_eq!(items.len(), 2, "HGETALL should return field+value pairs");
        } else {
            panic!("Expected Array from HGETALL, got {result:?}");
        }
    }

    #[tokio::test]
    async fn test_lpush_returns_length() {
        let state = make_state();
        let result = dispatch(&state, &args(&["LPUSH", "list1", "a", "b", "c"])).await;
        assert_eq!(result, Resp3Value::Integer(3));
    }

    #[tokio::test]
    async fn test_lpop_returns_element() {
        let state = make_state();
        dispatch(&state, &args(&["LPUSH", "list2", "x"])).await;
        let result = dispatch(&state, &args(&["LPOP", "list2"])).await;
        assert!(
            matches!(result, Resp3Value::BulkString(_)),
            "Expected BulkString from LPOP, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_lrange_returns_array() {
        let state = make_state();
        dispatch(&state, &args(&["LPUSH", "list3", "a", "b", "c"])).await;
        let result = dispatch(&state, &args(&["LRANGE", "list3", "0", "-1"])).await;
        assert!(
            matches!(result, Resp3Value::Array(_)),
            "Expected Array from LRANGE, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_sadd_returns_added_count() {
        let state = make_state();
        let result = dispatch(&state, &args(&["SADD", "s1", "a", "b", "c"])).await;
        assert_eq!(result, Resp3Value::Integer(3));
    }

    #[tokio::test]
    async fn test_sismember_true() {
        let state = make_state();
        dispatch(&state, &args(&["SADD", "s2", "alpha"])).await;
        let result = dispatch(&state, &args(&["SISMEMBER", "s2", "alpha"])).await;
        assert_eq!(result, Resp3Value::Integer(1));
    }

    #[tokio::test]
    async fn test_sismember_false() {
        let state = make_state();
        dispatch(&state, &args(&["SADD", "s3", "beta"])).await;
        let result = dispatch(&state, &args(&["SISMEMBER", "s3", "zzz_absent"])).await;
        assert_eq!(result, Resp3Value::Integer(0));
    }

    #[tokio::test]
    async fn test_zadd_then_zscore() {
        let state = make_state();
        let zadd_result = dispatch(&state, &args(&["ZADD", "z1", "1.0", "alpha"])).await;
        assert_eq!(zadd_result, Resp3Value::Integer(1));

        let zscore_result = dispatch(&state, &args(&["ZSCORE", "z1", "alpha"])).await;
        // ZSCORE returns BulkString of the score as text
        if let Resp3Value::BulkString(bytes) = zscore_result {
            let score_str = std::str::from_utf8(&bytes).expect("valid utf8 score");
            let score: f64 = score_str.parse().expect("parseable float score");
            assert!(
                (score - 1.0).abs() < 1e-9,
                "Score should be 1.0, got {score}"
            );
        } else {
            panic!("Expected BulkString from ZSCORE, got {zscore_result:?}");
        }
    }

    #[tokio::test]
    async fn test_pfadd_returns_changed() {
        let state = make_state();
        let result = dispatch(&state, &args(&["PFADD", "hll1", "elem1", "elem2"])).await;
        assert_eq!(
            result,
            Resp3Value::Integer(1),
            "PFADD should return 1 when HLL was modified"
        );
    }

    #[tokio::test]
    async fn test_pfcount_after_pfadd() {
        let state = make_state();
        dispatch(&state, &args(&["PFADD", "hll2", "e1", "e2", "e3"])).await;
        let result = dispatch(&state, &args(&["PFCOUNT", "hll2"])).await;
        if let Resp3Value::Integer(n) = result {
            assert!(n > 0, "PFCOUNT should be positive after PFADD, got {n}");
        } else {
            panic!("Expected Integer from PFCOUNT, got {result:?}");
        }
    }

    #[tokio::test]
    async fn test_bitcount_empty_key() {
        let state = make_state();
        // BITCOUNT on a non-existent key may return 0 or an error depending on
        // whether the bitmap store treats a missing key as empty.  Accept either.
        let result = dispatch(&state, &args(&["BITCOUNT", "bm_empty"])).await;
        match result {
            Resp3Value::Integer(0) => {} // empty bitmap → 0
            Resp3Value::Error(_) => {}   // key not found → error acceptable
            other => panic!("Unexpected result from BITCOUNT on missing key: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_setbit_returns_previous_value() {
        let state = make_state();
        let result = dispatch(&state, &args(&["SETBIT", "bm1", "7", "1"])).await;
        assert_eq!(result, Resp3Value::Integer(0), "Previous bit should be 0");
    }

    #[tokio::test]
    async fn test_bitcount_after_setbit() {
        let state = make_state();
        dispatch(&state, &args(&["SETBIT", "bm2", "7", "1"])).await;
        let result = dispatch(&state, &args(&["BITCOUNT", "bm2"])).await;
        assert_eq!(result, Resp3Value::Integer(1));
    }

    #[tokio::test]
    async fn test_flushall_returns_ok() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "flush_k", "v"])).await;
        let result = dispatch(&state, &args(&["FLUSHALL"])).await;
        assert_eq!(result, Resp3Value::SimpleString("OK".into()));
    }

    #[tokio::test]
    async fn test_empty_args_returns_error() {
        let state = make_state();
        let result = dispatch(&state, &[]).await;
        assert!(
            matches!(result, Resp3Value::Error(_)),
            "Expected Error for empty args, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_unknown_command_returns_error() {
        let state = make_state();
        let result = dispatch(&state, &args(&["NOTAREALCMD"])).await;
        if let Resp3Value::Error(msg) = result {
            assert!(
                msg.starts_with("ERR unknown"),
                "Expected 'ERR unknown...' error, got: {msg}"
            );
        } else {
            panic!("Expected Error for unknown command, got {result:?}");
        }
    }

    // ── Phase 3 — new commands ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_append_returns_new_length() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "app_k", "hello"])).await;
        let result = dispatch(&state, &args(&["APPEND", "app_k", " world"])).await;
        assert_eq!(result, Resp3Value::Integer(11));
    }

    #[tokio::test]
    async fn test_strlen_existing_key() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "sl_k", "hello"])).await;
        let result = dispatch(&state, &args(&["STRLEN", "sl_k"])).await;
        assert_eq!(result, Resp3Value::Integer(5));
    }

    #[tokio::test]
    async fn test_strlen_missing_key_returns_zero() {
        let state = make_state();
        let result = dispatch(&state, &args(&["STRLEN", "no_such_key_strlen_xyz"])).await;
        assert_eq!(result, Resp3Value::Integer(0));
    }

    #[tokio::test]
    async fn test_dbsize_returns_integer() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "dbsize_k1", "v"])).await;
        dispatch(&state, &args(&["SET", "dbsize_k2", "v"])).await;
        let result = dispatch(&state, &args(&["DBSIZE"])).await;
        assert!(
            matches!(result, Resp3Value::Integer(n) if n >= 2),
            "Expected Integer >= 2 from DBSIZE, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_getset_returns_old_value() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "gs_k", "old"])).await;
        let result = dispatch(&state, &args(&["GETSET", "gs_k", "new"])).await;
        assert_eq!(result, Resp3Value::BulkString(b"old".to_vec()));
    }

    #[tokio::test]
    async fn test_getset_missing_key_is_null() {
        let state = make_state();
        let result = dispatch(&state, &args(&["GETSET", "gs_missing_xyz", "value"])).await;
        assert_eq!(result, Resp3Value::Null);
    }

    #[tokio::test]
    async fn test_msetnx_all_new_returns_one() {
        let state = make_state();
        let result = dispatch(&state, &args(&["MSETNX", "mnx1", "v1", "mnx2", "v2"])).await;
        assert_eq!(result, Resp3Value::Integer(1));
    }

    #[tokio::test]
    async fn test_msetnx_existing_key_returns_zero() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "mnx_exist", "v"])).await;
        let result = dispatch(
            &state,
            &args(&["MSETNX", "mnx_exist", "new", "mnx_fresh", "v"]),
        )
        .await;
        assert_eq!(result, Resp3Value::Integer(0));
    }

    #[tokio::test]
    async fn test_hkeys_returns_all_fields() {
        let state = make_state();
        dispatch(&state, &args(&["HSET", "hkeys_h", "f1", "v1", "f2", "v2"])).await;
        let result = dispatch(&state, &args(&["HKEYS", "hkeys_h"])).await;
        if let Resp3Value::Array(items) = result {
            assert_eq!(items.len(), 2, "HKEYS should return 2 fields");
        } else {
            panic!("Expected Array from HKEYS, got {result:?}");
        }
    }

    #[tokio::test]
    async fn test_hvals_returns_all_values() {
        let state = make_state();
        dispatch(&state, &args(&["HSET", "hvals_h", "f1", "v1", "f2", "v2"])).await;
        let result = dispatch(&state, &args(&["HVALS", "hvals_h"])).await;
        if let Resp3Value::Array(items) = result {
            assert_eq!(items.len(), 2, "HVALS should return 2 values");
        } else {
            panic!("Expected Array from HVALS, got {result:?}");
        }
    }

    #[tokio::test]
    async fn test_pfmerge_returns_ok() {
        let state = make_state();
        dispatch(&state, &args(&["PFADD", "pm_src1", "e1", "e2"])).await;
        dispatch(&state, &args(&["PFADD", "pm_src2", "e3", "e4"])).await;
        let result = dispatch(&state, &args(&["PFMERGE", "pm_dest", "pm_src1", "pm_src2"])).await;
        assert_eq!(result, Resp3Value::SimpleString("OK".into()));
    }

    #[tokio::test]
    async fn test_geoadd_returns_added_count() {
        let state = make_state();
        let result = dispatch(
            &state,
            &args(&["GEOADD", "geo1", "13.361389", "38.115556", "Palermo"]),
        )
        .await;
        assert_eq!(result, Resp3Value::Integer(1));
    }

    #[tokio::test]
    async fn test_geopos_returns_coordinates() {
        let state = make_state();
        dispatch(
            &state,
            &args(&["GEOADD", "geo2", "13.361389", "38.115556", "Palermo"]),
        )
        .await;
        let result = dispatch(&state, &args(&["GEOPOS", "geo2", "Palermo"])).await;
        if let Resp3Value::Array(positions) = result {
            assert_eq!(positions.len(), 1);
            assert!(
                matches!(positions[0], Resp3Value::Array(_)),
                "Expected Array position, got {:?}",
                positions[0]
            );
        } else {
            panic!("Expected Array from GEOPOS, got {result:?}");
        }
    }

    #[tokio::test]
    async fn test_geohash_returns_string() {
        let state = make_state();
        dispatch(
            &state,
            &args(&["GEOADD", "geo3", "13.361389", "38.115556", "Palermo"]),
        )
        .await;
        let result = dispatch(&state, &args(&["GEOHASH", "geo3", "Palermo"])).await;
        if let Resp3Value::Array(hashes) = result {
            assert_eq!(hashes.len(), 1);
            assert!(
                matches!(hashes[0], Resp3Value::BulkString(_)),
                "Expected BulkString hash, got {:?}",
                hashes[0]
            );
        } else {
            panic!("Expected Array from GEOHASH, got {result:?}");
        }
    }

    #[tokio::test]
    async fn test_geodist_palermo_catania_km() {
        let state = make_state();
        dispatch(
            &state,
            &args(&[
                "GEOADD",
                "geo4",
                "13.361389",
                "38.115556",
                "Palermo",
                "15.087269",
                "37.502669",
                "Catania",
            ]),
        )
        .await;
        let result = dispatch(
            &state,
            &args(&["GEODIST", "geo4", "Palermo", "Catania", "km"]),
        )
        .await;
        if let Resp3Value::BulkString(bytes) = result {
            let dist: f64 = str::from_utf8(&bytes).unwrap().parse().unwrap();
            assert!(
                dist > 150.0 && dist < 175.0,
                "Expected ~166 km distance Palermo-Catania, got {dist}"
            );
        } else {
            panic!("Expected BulkString from GEODIST, got {result:?}");
        }
    }

    #[tokio::test]
    async fn test_synap_kvstats_returns_stats_array() {
        let state = make_state();
        dispatch(&state, &args(&["SET", "stats_k", "v"])).await;
        let result = dispatch(&state, &args(&["SYNAP.KVSTATS"])).await;
        assert!(
            matches!(result, Resp3Value::Array(_)),
            "Expected Array from SYNAP.KVSTATS, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_multi_exec_empty_transaction() {
        let state = make_state();
        let multi = dispatch(&state, &args(&["MULTI", "client_test_1"])).await;
        assert_eq!(multi, Resp3Value::SimpleString("OK".into()));
        let exec = dispatch(&state, &args(&["EXEC", "client_test_1"])).await;
        assert!(
            matches!(exec, Resp3Value::Array(_) | Resp3Value::Null),
            "Expected Array or Null from empty EXEC, got {exec:?}"
        );
    }

    #[tokio::test]
    async fn test_discard_without_multi_returns_error() {
        let state = make_state();
        let result = dispatch(&state, &args(&["DISCARD", "no_tx_client_xyz"])).await;
        assert!(
            matches!(result, Resp3Value::Error(_)),
            "Expected Error from DISCARD without MULTI, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_script_load_then_exists() {
        let state = make_state();
        let load_result = dispatch(&state, &args(&["SCRIPT", "LOAD", "return 1"])).await;
        let sha = if let Resp3Value::BulkString(bytes) = load_result {
            String::from_utf8(bytes).expect("valid utf8 sha")
        } else {
            panic!("Expected BulkString from SCRIPT LOAD, got {load_result:?}");
        };
        let exists_result = dispatch(&state, &args(&["SCRIPT", "EXISTS", &sha])).await;
        assert_eq!(
            exists_result,
            Resp3Value::Array(vec![Resp3Value::Integer(1)])
        );
    }

    #[tokio::test]
    async fn test_eval_basic_script() {
        let state = make_state();
        let result = dispatch(&state, &args(&["EVAL", "return 42", "0"])).await;
        assert!(
            matches!(result, Resp3Value::BulkString(_)),
            "Expected BulkString from EVAL, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_qcreate_without_manager_returns_error() {
        let state = make_state(); // queue_manager is None
        let result = dispatch(&state, &args(&["QCREATE", "myqueue"])).await;
        assert!(
            matches!(result, Resp3Value::Error(_)),
            "Expected Error when queue subsystem disabled, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_xadd_without_stream_manager_returns_error() {
        let state = make_state(); // stream_manager is None
        let result = dispatch(&state, &args(&["XADD", "mystream", "*", "event", "data"])).await;
        assert!(
            matches!(result, Resp3Value::Error(_)),
            "Expected Error when stream subsystem disabled, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_publish_without_pubsub_returns_error() {
        let state = make_state(); // pubsub_router is None
        let result = dispatch(&state, &args(&["PUBLISH", "topic", "payload"])).await;
        assert!(
            matches!(result, Resp3Value::Error(_)),
            "Expected Error when pubsub subsystem disabled, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_xdel_returns_zero() {
        let state = make_state();
        let result = dispatch(&state, &args(&["XDEL", "mystream", "0-1", "0-2"])).await;
        assert_eq!(result, Resp3Value::Integer(0));
    }

    #[tokio::test]
    async fn test_xack_returns_id_count() {
        let state = make_state();
        let result = dispatch(
            &state,
            &args(&["XACK", "mystream", "mygroup", "0-1", "0-2"]),
        )
        .await;
        assert_eq!(result, Resp3Value::Integer(2));
    }

    // ── Phase 4.2: RESP3 dispatcher parity tests with real subsystems ──────────

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

    // ── Queue parity (4.2) ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_resp3_queue_create_publish_consume_ack_lifecycle() {
        let state = make_state_with_queues();

        // QCREATE
        let result = dispatch(&state, &args(&["QCREATE", "r3_q"])).await;
        assert_eq!(
            result,
            Resp3Value::SimpleString("OK".into()),
            "QCREATE failed"
        );

        // QLIST — queue should appear
        let result = dispatch(&state, &args(&["QLIST"])).await;
        match &result {
            Resp3Value::Array(names) => assert!(
                names.contains(&Resp3Value::BulkString(b"r3_q".to_vec())),
                "r3_q not in QLIST: {:?}",
                names
            ),
            other => panic!("unexpected QLIST result: {other:?}"),
        }

        // QPUBLISH
        let result = dispatch(&state, &args(&["QPUBLISH", "r3_q", "hello_resp3"])).await;
        let msg_id = match &result {
            Resp3Value::BulkString(b) => String::from_utf8(b.clone()).unwrap(),
            other => panic!("unexpected QPUBLISH result: {other:?}"),
        };
        assert!(!msg_id.is_empty());

        // QSTATS — returned as a flat Array of [key, val, key, val, ...]
        let result = dispatch(&state, &args(&["QSTATS", "r3_q"])).await;
        match &result {
            Resp3Value::Array(flat) => {
                // Find depth value: flat pairs are [BulkString("depth"), Integer(n), ...]
                let depth_pos = flat
                    .iter()
                    .position(|v| v == &Resp3Value::BulkString(b"depth".to_vec()));
                let depth_val = depth_pos.and_then(|i| flat.get(i + 1));
                assert_eq!(
                    depth_val,
                    Some(&Resp3Value::Integer(1)),
                    "expected depth=1 after publish, stats: {:?}",
                    flat
                );
            }
            other => panic!("unexpected QSTATS result: {other:?}"),
        }

        // QCONSUME
        let result = dispatch(&state, &args(&["QCONSUME", "r3_q", "consumer_r3"])).await;
        let consumed_id = match &result {
            Resp3Value::Array(fields) if !fields.is_empty() => match &fields[0] {
                Resp3Value::BulkString(b) => String::from_utf8(b.clone()).unwrap(),
                other => panic!("id field unexpected: {other:?}"),
            },
            other => panic!("unexpected QCONSUME result: {other:?}"),
        };
        assert!(!consumed_id.is_empty());

        // QACK
        let result = dispatch(&state, &args(&["QACK", "r3_q", &consumed_id])).await;
        assert_eq!(result, Resp3Value::SimpleString("OK".into()), "QACK failed");

        // QPURGE after publishing one more
        dispatch(&state, &args(&["QPUBLISH", "r3_q", "purge_me"])).await;
        let result = dispatch(&state, &args(&["QPURGE", "r3_q"])).await;
        match &result {
            Resp3Value::Integer(n) => assert!(*n >= 1, "QPURGE count should be ≥ 1"),
            other => panic!("unexpected QPURGE result: {other:?}"),
        }

        // QDELETE
        let result = dispatch(&state, &args(&["QDELETE", "r3_q"])).await;
        assert!(
            matches!(
                result,
                Resp3Value::Integer(_) | Resp3Value::SimpleString(_) | Resp3Value::Boolean(_)
            ),
            "QDELETE should succeed, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_resp3_queue_nack_requeues() {
        let state = make_state_with_queues();

        dispatch(&state, &args(&["QCREATE", "r3_nack"])).await;
        dispatch(&state, &args(&["QPUBLISH", "r3_nack", "retry_me"])).await;

        let result = dispatch(&state, &args(&["QCONSUME", "r3_nack", "c1"])).await;
        let consumed_id = match &result {
            Resp3Value::Array(fields) => match &fields[0] {
                Resp3Value::BulkString(b) => String::from_utf8(b.clone()).unwrap(),
                other => panic!("unexpected id: {other:?}"),
            },
            other => panic!("unexpected QCONSUME: {other:?}"),
        };

        let result = dispatch(&state, &args(&["QNACK", "r3_nack", &consumed_id, "true"])).await;
        assert_eq!(
            result,
            Resp3Value::SimpleString("OK".into()),
            "QNACK failed"
        );
    }

    // ── Stream parity (4.2) ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_resp3_xadd_xread_xinfo_lifecycle() {
        let state = make_state_with_streams();

        // Pre-create room (XADD requires it to exist — rooms are not auto-created)
        state
            .stream_manager
            .as_ref()
            .unwrap()
            .create_room("r3_room")
            .await
            .expect("create_room failed");

        // XADD — appends event
        let result = dispatch(
            &state,
            &args(&["XADD", "r3_room", "*", "click", r#"{"x":1}"#]),
        )
        .await;
        let offset = match &result {
            Resp3Value::BulkString(b) => String::from_utf8(b.clone()).unwrap(),
            other => panic!("unexpected XADD result: {other:?}"),
        };
        assert!(!offset.is_empty(), "XADD should return an offset string");

        // Publish one more event
        dispatch(
            &state,
            &args(&["XADD", "r3_room", "*", "scroll", r#"{"y":2}"#]),
        )
        .await;

        // XREAD — read from offset 0
        let result = dispatch(
            &state,
            &args(&["XREAD", "COUNT", "10", "STREAMS", "r3_room", "0"]),
        )
        .await;
        match &result {
            Resp3Value::Array(events) => {
                assert_eq!(events.len(), 2, "expected 2 events from XREAD");
            }
            other => panic!("unexpected XREAD result: {other:?}"),
        }

        // XINFO — room stats (flat Array of key-value pairs, same as Redis INFO response)
        let result = dispatch(&state, &args(&["XINFO", "STREAM", "r3_room"])).await;
        match &result {
            Resp3Value::Array(_) | Resp3Value::Map(_) => {}
            other => panic!("unexpected XINFO result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_resp3_xrange_reads_event_slice() {
        let state = make_state_with_streams();

        state
            .stream_manager
            .as_ref()
            .unwrap()
            .create_room("r3_range")
            .await
            .expect("create_room failed");

        for i in 0u8..3 {
            dispatch(
                &state,
                &args(&["XADD", "r3_range", "*", "tick", &i.to_string()]),
            )
            .await;
        }

        // XRANGE from offset 1
        let result = dispatch(&state, &args(&["XRANGE", "r3_range", "1", "10"])).await;
        match &result {
            Resp3Value::Array(events) => {
                assert_eq!(events.len(), 2, "expected events at offset ≥ 1");
            }
            other => panic!("unexpected XRANGE result: {other:?}"),
        }
    }

    // ── Pub/Sub parity (4.2) ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_resp3_subscribe_returns_subscription_count() {
        let state = make_state_with_pubsub();

        let result = dispatch(&state, &args(&["SUBSCRIBE", "r3.news", "r3.sports"])).await;
        match &result {
            Resp3Value::Integer(n) => assert_eq!(*n, 2, "expected subscription_count=2"),
            other => panic!("unexpected SUBSCRIBE result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_resp3_publish_matches_subscriber() {
        let state = make_state_with_pubsub();

        dispatch(&state, &args(&["SUBSCRIBE", "r3.alerts"])).await;

        let result = dispatch(
            &state,
            &args(&["PUBLISH", "r3.alerts", r#"{"level":"info"}"#]),
        )
        .await;
        match &result {
            Resp3Value::Integer(n) => assert_eq!(*n, 1, "expected 1 subscriber matched by PUBLISH"),
            other => panic!("unexpected PUBLISH result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_resp3_pubsub_channels_lists_topics() {
        let state = make_state_with_pubsub();

        dispatch(&state, &args(&["SUBSCRIBE", "r3.ta"])).await;
        dispatch(&state, &args(&["SUBSCRIBE", "r3.tb"])).await;

        let result = dispatch(&state, &args(&["PUBSUB", "CHANNELS"])).await;
        match &result {
            Resp3Value::Array(topics) => {
                let topic_strs: Vec<String> = topics
                    .iter()
                    .filter_map(|v| {
                        if let Resp3Value::BulkString(b) = v {
                            String::from_utf8(b.clone()).ok()
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    topic_strs.contains(&"r3.ta".to_owned()),
                    "r3.ta missing: {:?}",
                    topic_strs
                );
                assert!(
                    topic_strs.contains(&"r3.tb".to_owned()),
                    "r3.tb missing: {:?}",
                    topic_strs
                );
            }
            other => panic!("unexpected PUBSUB CHANNELS result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_resp3_pubsub_server_push_delivers_to_channel() {
        use crate::core::pubsub::Message;
        use tokio::sync::mpsc;

        let state = make_state_with_pubsub();

        // SUBSCRIBE — returns subscription_count; the router internally assigns a subscriber_id
        // We must subscribe directly through the router to get the id, then register the channel
        let sub_result = state
            .pubsub_router
            .as_ref()
            .unwrap()
            .subscribe(vec!["r3.push".to_owned()])
            .expect("subscribe failed");
        let subscriber_id = sub_result.subscriber_id;

        // Register mpsc channel to simulate the connection layer
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        state
            .pubsub_router
            .as_ref()
            .unwrap()
            .register_connection(subscriber_id, tx);

        // PUBLISH via RESP3 dispatcher
        let result = dispatch(
            &state,
            &args(&["PUBLISH", "r3.push", r#"{"kind":"push_test"}"#]),
        )
        .await;
        assert!(
            matches!(result, Resp3Value::Integer(_)),
            "PUBLISH should return integer, got {result:?}"
        );

        // Push frame should arrive on the channel
        let msg = rx
            .try_recv()
            .expect("expected push frame on channel after RESP3 PUBLISH");
        assert_eq!(msg.topic, "r3.push");
    }
}
