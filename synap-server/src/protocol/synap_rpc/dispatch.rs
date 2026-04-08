//! SynapRPC command dispatcher.
//!
//! Maps `Request.command` strings to AppState store operations and returns a
//! `SynapValue` result.  Mirrors the RESP3 command dispatcher but operates on
//! the binary `SynapValue` type instead of `Resp3Value`.

use crate::core::sorted_set::ZAddOptions;
use crate::server::handlers::AppState;

use super::types::{Request, Response, SynapValue};

/// Dispatch one `Request` and return a `Response`.
#[tracing::instrument(name = "rpc.dispatch", skip(state, req), fields(cmd = %req.command, id = req.id))]
pub async fn dispatch(state: &AppState, req: Request) -> Response {
    let result = run(state, &req.command, req.args).await;
    match result {
        Ok(v) => Response::ok(req.id, v),
        Err(e) => Response::err(req.id, e),
    }
}

async fn run(state: &AppState, command: &str, args: Vec<SynapValue>) -> Result<SynapValue, String> {
    match command.to_ascii_uppercase().as_str() {
        "PING" => Ok(SynapValue::Str(if args.is_empty() {
            "PONG".into()
        } else {
            arg_str(&args, 0)?
        })),

        // ── KV ────────────────────────────────────────────────────────────────
        "SET" => {
            let key = arg_str(&args, 0)?;
            let value = arg_bytes(&args, 1)?;
            let ttl: Option<u64> = args.get(2).and_then(|v| v.as_int()).map(|n| n as u64);
            state
                .kv_store
                .set(key, value, ttl)
                .await
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "GET" => {
            let key = arg_str(&args, 0)?;
            state
                .kv_store
                .get(&key)
                .await
                .map(|opt| opt.map(SynapValue::Bytes).unwrap_or(SynapValue::Null))
                .map_err(|e| e.to_string())
        }
        "DEL" => {
            let mut deleted = 0i64;
            for v in &args {
                if let Some(k) = v.as_str() {
                    match state.kv_store.delete(k).await {
                        Ok(true) => deleted += 1,
                        Ok(false) => {}
                        Err(e) => return Err(e.to_string()),
                    }
                }
            }
            Ok(SynapValue::Int(deleted))
        }
        "EXISTS" => {
            let key = arg_str(&args, 0)?;
            state
                .kv_store
                .exists(&key)
                .await
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "EXPIRE" => {
            let key = arg_str(&args, 0)?;
            let secs = arg_int(&args, 1)? as u64;
            state
                .kv_store
                .expire(&key, secs)
                .await
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "TTL" => {
            let key = arg_str(&args, 0)?;
            state
                .kv_store
                .ttl(&key)
                .await
                .map(|opt| SynapValue::Int(opt.map(|s| s as i64).unwrap_or(-1)))
                .map_err(|_| "ERR key does not exist".into())
        }
        "PERSIST" => {
            let key = arg_str(&args, 0)?;
            state
                .kv_store
                .persist(&key)
                .await
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "INCR" => {
            let key = arg_str(&args, 0)?;
            state
                .kv_store
                .incr(&key, 1)
                .await
                .map(SynapValue::Int)
                .map_err(|e| e.to_string())
        }
        "INCRBY" => {
            let key = arg_str(&args, 0)?;
            let by = arg_int(&args, 1)?;
            state
                .kv_store
                .incr(&key, by)
                .await
                .map(SynapValue::Int)
                .map_err(|e| e.to_string())
        }
        "DECR" => {
            let key = arg_str(&args, 0)?;
            state
                .kv_store
                .decr(&key, 1)
                .await
                .map(SynapValue::Int)
                .map_err(|e| e.to_string())
        }
        "DECRBY" => {
            let key = arg_str(&args, 0)?;
            let by = arg_int(&args, 1)?;
            state
                .kv_store
                .decr(&key, by)
                .await
                .map(SynapValue::Int)
                .map_err(|e| e.to_string())
        }
        "MSET" => {
            if args.len() % 2 != 0 {
                return Err("ERR wrong number of arguments for 'MSET'".into());
            }
            let mut pairs = Vec::new();
            let mut i = 0;
            while i + 1 < args.len() {
                pairs.push((arg_str(&args, i)?, arg_bytes(&args, i + 1)?));
                i += 2;
            }
            state
                .kv_store
                .mset(pairs)
                .await
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "MGET" => {
            let keys: Vec<String> = args
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            state
                .kv_store
                .mget(&keys)
                .await
                .map(|values| {
                    SynapValue::Array(
                        values
                            .into_iter()
                            .map(|opt| opt.map(SynapValue::Bytes).unwrap_or(SynapValue::Null))
                            .collect(),
                    )
                })
                .map_err(|e| e.to_string())
        }
        "KEYS" => state
            .kv_store
            .keys()
            .await
            .map(|ks| SynapValue::Array(ks.into_iter().map(SynapValue::Str).collect()))
            .map_err(|e| e.to_string()),

        // ── Hash ──────────────────────────────────────────────────────────────
        "HSET" => {
            let key = arg_str(&args, 0)?;
            if (args.len() - 1) % 2 != 0 {
                return Err("ERR wrong number of arguments for 'HSET'".into());
            }
            let mut added = 0i64;
            let mut i = 1;
            while i + 1 < args.len() {
                let field = arg_str(&args, i)?;
                let value = arg_bytes(&args, i + 1)?;
                match state.hash_store.hset(&key, &field, value) {
                    Ok(is_new) => {
                        if is_new {
                            added += 1;
                        }
                    }
                    Err(e) => return Err(e.to_string()),
                }
                i += 2;
            }
            Ok(SynapValue::Int(added))
        }
        "HGET" => {
            let key = arg_str(&args, 0)?;
            let field = arg_str(&args, 1)?;
            state
                .hash_store
                .hget(&key, &field)
                .map(|opt| opt.map(SynapValue::Bytes).unwrap_or(SynapValue::Null))
                .map_err(|e| e.to_string())
        }
        "HDEL" => {
            let key = arg_str(&args, 0)?;
            let fields: Vec<String> = args[1..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            state
                .hash_store
                .hdel(&key, &fields)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "HGETALL" => {
            let key = arg_str(&args, 0)?;
            state
                .hash_store
                .hgetall(&key)
                .map(|map| {
                    let mut pairs = Vec::new();
                    for (f, v) in map {
                        pairs.push((SynapValue::Str(f), SynapValue::Bytes(v)));
                    }
                    SynapValue::Map(pairs)
                })
                .map_err(|e| e.to_string())
        }
        "HLEN" => {
            let key = arg_str(&args, 0)?;
            state
                .hash_store
                .hlen(&key)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "HEXISTS" => {
            let key = arg_str(&args, 0)?;
            let field = arg_str(&args, 1)?;
            state
                .hash_store
                .hexists(&key, &field)
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }

        // ── List ──────────────────────────────────────────────────────────────
        "LPUSH" => {
            let key = arg_str(&args, 0)?;
            let values: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            state
                .list_store
                .lpush(&key, values, false)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "RPUSH" => {
            let key = arg_str(&args, 0)?;
            let values: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            state
                .list_store
                .rpush(&key, values, false)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "LPOP" => {
            let key = arg_str(&args, 0)?;
            state
                .list_store
                .lpop(&key, Some(1))
                .map(|mut v| {
                    if v.is_empty() {
                        SynapValue::Null
                    } else {
                        SynapValue::Bytes(v.remove(0))
                    }
                })
                .map_err(|e| e.to_string())
        }
        "RPOP" => {
            let key = arg_str(&args, 0)?;
            state
                .list_store
                .rpop(&key, Some(1))
                .map(|mut v| {
                    if v.is_empty() {
                        SynapValue::Null
                    } else {
                        SynapValue::Bytes(v.remove(0))
                    }
                })
                .map_err(|e| e.to_string())
        }
        "LRANGE" => {
            let key = arg_str(&args, 0)?;
            let start = arg_int(&args, 1)?;
            let stop = arg_int(&args, 2)?;
            state
                .list_store
                .lrange(&key, start, stop)
                .map(|items| SynapValue::Array(items.into_iter().map(SynapValue::Bytes).collect()))
                .map_err(|e| e.to_string())
        }
        "LLEN" => {
            let key = arg_str(&args, 0)?;
            state
                .list_store
                .llen(&key)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }

        // ── Set ───────────────────────────────────────────────────────────────
        "SADD" => {
            let key = arg_str(&args, 0)?;
            let members: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            state
                .set_store
                .sadd(&key, members)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "SMEMBERS" => {
            let key = arg_str(&args, 0)?;
            state
                .set_store
                .smembers(&key)
                .map(|ms| SynapValue::Array(ms.into_iter().map(SynapValue::Bytes).collect()))
                .map_err(|e| e.to_string())
        }
        "SREM" => {
            let key = arg_str(&args, 0)?;
            let members: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            state
                .set_store
                .srem(&key, members)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "SISMEMBER" => {
            let key = arg_str(&args, 0)?;
            let member = arg_bytes(&args, 1)?;
            state
                .set_store
                .sismember(&key, member)
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "SCARD" => {
            let key = arg_str(&args, 0)?;
            state
                .set_store
                .scard(&key)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }

        // ── Sorted set ────────────────────────────────────────────────────────
        "ZADD" => {
            let key = arg_str(&args, 0)?;
            let score = arg_float(&args, 1)?;
            let member = arg_bytes(&args, 2)?;
            let opts = ZAddOptions::default();
            let (added, _) = state.sorted_set_store.zadd(&key, member, score, &opts);
            Ok(SynapValue::Int(added as i64))
        }
        "ZRANGE" => {
            let key = arg_str(&args, 0)?;
            let start = arg_int(&args, 1)?;
            let stop = arg_int(&args, 2)?;
            let with_scores = args
                .get(3)
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case("withscores"))
                .unwrap_or(false);
            let members = state
                .sorted_set_store
                .zrange(&key, start, stop, with_scores);
            if with_scores {
                let pairs = members
                    .into_iter()
                    .map(|sm| (SynapValue::Bytes(sm.member), SynapValue::Float(sm.score)))
                    .collect();
                Ok(SynapValue::Map(pairs))
            } else {
                Ok(SynapValue::Array(
                    members
                        .into_iter()
                        .map(|sm| SynapValue::Bytes(sm.member))
                        .collect(),
                ))
            }
        }
        "ZSCORE" => {
            let key = arg_str(&args, 0)?;
            let member = arg_bytes(&args, 1)?;
            Ok(state
                .sorted_set_store
                .zscore(&key, &member)
                .map(SynapValue::Float)
                .unwrap_or(SynapValue::Null))
        }
        "ZCARD" => {
            let key = arg_str(&args, 0)?;
            Ok(SynapValue::Int(state.sorted_set_store.zcard(&key) as i64))
        }
        "ZREM" => {
            let key = arg_str(&args, 0)?;
            let members: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            Ok(SynapValue::Int(
                state.sorted_set_store.zrem(&key, &members) as i64
            ))
        }

        // ── HyperLogLog ───────────────────────────────────────────────────────
        "PFADD" => {
            let key = arg_str(&args, 0)?;
            let elements: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            state
                .hyperloglog_store
                .pfadd(&key, elements, None)
                .map(|n| SynapValue::Bool(n > 0))
                .map_err(|e| e.to_string())
        }
        "PFCOUNT" => {
            let key = arg_str(&args, 0)?;
            state
                .hyperloglog_store
                .pfcount(&key)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }

        // ── Bitmap ────────────────────────────────────────────────────────────
        "BITCOUNT" => {
            let key = arg_str(&args, 0)?;
            let (start, end) = if args.len() >= 3 {
                let s = arg_int(&args, 1)?.max(0) as usize;
                let e = arg_int(&args, 2)?;
                (Some(s), if e < 0 { None } else { Some(e as usize) })
            } else {
                (None, None)
            };
            state
                .bitmap_store
                .bitcount(&key, start, end)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "SETBIT" => {
            let key = arg_str(&args, 0)?;
            let offset = arg_int(&args, 1)? as usize;
            let bit = arg_int(&args, 2)? as u8 & 1;
            state
                .bitmap_store
                .setbit(&key, offset, bit)
                .map(|prev| SynapValue::Int(prev as i64))
                .map_err(|e| e.to_string())
        }
        "GETBIT" => {
            let key = arg_str(&args, 0)?;
            let offset = arg_int(&args, 1)? as usize;
            state
                .bitmap_store
                .getbit(&key, offset)
                .map(|b| SynapValue::Int(b as i64))
                .map_err(|e| e.to_string())
        }

        // ── Misc ──────────────────────────────────────────────────────────────
        "FLUSHALL" | "FLUSHDB" => {
            let _ = state.kv_store.flushall().await;
            Ok(SynapValue::Str("OK".into()))
        }

        _ => Err(format!("ERR unknown command '{command}'")),
    }
}

// ── Argument helpers ──────────────────────────────────────────────────────────

fn arg_str(args: &[SynapValue], idx: usize) -> Result<String, String> {
    match args.get(idx) {
        Some(SynapValue::Str(s)) => Ok(s.clone()),
        Some(SynapValue::Bytes(b)) => std::str::from_utf8(b)
            .map(|s| s.to_owned())
            .map_err(|_| format!("ERR argument {idx} is not valid UTF-8")),
        Some(_) => Err(format!("ERR argument {idx} must be a string")),
        None => Err(format!("ERR missing argument {idx}")),
    }
}

fn arg_bytes(args: &[SynapValue], idx: usize) -> Result<Vec<u8>, String> {
    match args.get(idx) {
        Some(SynapValue::Bytes(b)) => Ok(b.clone()),
        Some(SynapValue::Str(s)) => Ok(s.as_bytes().to_vec()),
        Some(_) => Err(format!("ERR argument {idx} must be bytes")),
        None => Err(format!("ERR missing argument {idx}")),
    }
}

fn arg_int(args: &[SynapValue], idx: usize) -> Result<i64, String> {
    match args.get(idx) {
        Some(SynapValue::Int(n)) => Ok(*n),
        Some(SynapValue::Str(s)) => s
            .parse::<i64>()
            .map_err(|_| format!("ERR argument {idx} is not an integer")),
        Some(_) => Err(format!("ERR argument {idx} must be an integer")),
        None => Err(format!("ERR missing argument {idx}")),
    }
}

fn arg_float(args: &[SynapValue], idx: usize) -> Result<f64, String> {
    match args.get(idx) {
        Some(SynapValue::Float(f)) => Ok(*f),
        Some(SynapValue::Int(n)) => Ok(*n as f64),
        Some(SynapValue::Str(s)) => s
            .parse::<f64>()
            .map_err(|_| format!("ERR argument {idx} is not a float")),
        Some(_) => Err(format!("ERR argument {idx} must be a float")),
        None => Err(format!("ERR missing argument {idx}")),
    }
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
}
