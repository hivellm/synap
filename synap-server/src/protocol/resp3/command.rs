//! RESP3 command → AppState dispatch.
//!
//! Maps the command name extracted from the parsed RESP3 array to the
//! appropriate store method and returns a `Resp3Value` response.
//! Reuses the same store handles as the HTTP handlers — no logic duplication.

use std::str;

use super::parser::Resp3Value;
use crate::core::sorted_set::ZAddOptions;
use crate::server::handlers::AppState;

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
        "PING" => cmd_ping(args),
        "QUIT" => Resp3Value::SimpleString("OK".into()),
        "SELECT" => Resp3Value::SimpleString("OK".into()), // single DB only

        "SET" => cmd_set(state, args).await,
        "GET" => cmd_get(state, args).await,
        "DEL" => cmd_del(state, args).await,
        "EXISTS" => cmd_exists(state, args).await,
        "EXPIRE" => cmd_expire(state, args).await,
        "TTL" => cmd_ttl(state, args).await,
        "PERSIST" => cmd_persist(state, args).await,
        "INCR" => cmd_incr(state, args).await,
        "INCRBY" => cmd_incrby(state, args).await,
        "DECR" => cmd_decr(state, args).await,
        "DECRBY" => cmd_decrby(state, args).await,
        "MSET" => cmd_mset(state, args).await,
        "MGET" => cmd_mget(state, args).await,
        "KEYS" => cmd_keys(state, args).await,
        "SCAN" => cmd_scan(state, args).await,

        "HSET" => cmd_hset(state, args).await,
        "HGET" => cmd_hget(state, args).await,
        "HDEL" => cmd_hdel(state, args).await,
        "HGETALL" => cmd_hgetall(state, args).await,
        "HMSET" => cmd_hmset(state, args).await,
        "HMGET" => cmd_hmget(state, args).await,
        "HLEN" => cmd_hlen(state, args).await,
        "HEXISTS" => cmd_hexists(state, args).await,

        "LPUSH" => cmd_lpush(state, args).await,
        "RPUSH" => cmd_rpush(state, args).await,
        "LPOP" => cmd_lpop(state, args).await,
        "RPOP" => cmd_rpop(state, args).await,
        "LRANGE" => cmd_lrange(state, args).await,
        "LLEN" => cmd_llen(state, args).await,

        "SADD" => cmd_sadd(state, args).await,
        "SMEMBERS" => cmd_smembers(state, args).await,
        "SREM" => cmd_srem(state, args).await,
        "SISMEMBER" => cmd_sismember(state, args).await,
        "SCARD" => cmd_scard(state, args).await,

        "ZADD" => cmd_zadd(state, args).await,
        "ZRANGE" => cmd_zrange(state, args).await,
        "ZSCORE" => cmd_zscore(state, args).await,
        "ZCARD" => cmd_zcard(state, args).await,
        "ZREM" => cmd_zrem(state, args).await,

        "PFADD" => cmd_pfadd(state, args).await,
        "PFCOUNT" => cmd_pfcount(state, args).await,

        "BITCOUNT" => cmd_bitcount(state, args).await,
        "SETBIT" => cmd_setbit(state, args).await,
        "GETBIT" => cmd_getbit(state, args).await,

        "FLUSHALL" | "FLUSHDB" => cmd_flushall(state).await,

        _ => Resp3Value::Error(format!("ERR unknown command '{cmd}'")),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn arg_str(args: &[Resp3Value], idx: usize) -> Option<String> {
    args.get(idx)?.as_str().map(|s| s.to_owned())
}

fn arg_bytes(args: &[Resp3Value], idx: usize) -> Option<Vec<u8>> {
    args.get(idx)?.as_bytes().map(|b| b.to_vec())
}

fn arg_i64(args: &[Resp3Value], idx: usize) -> Option<i64> {
    args.get(idx)?.as_str()?.parse().ok()
}

fn arg_u64(args: &[Resp3Value], idx: usize) -> Option<u64> {
    args.get(idx)?.as_str()?.parse().ok()
}

fn arg_f64(args: &[Resp3Value], idx: usize) -> Option<f64> {
    args.get(idx)?.as_str()?.parse().ok()
}

fn err_wrong_args(cmd: &str) -> Resp3Value {
    Resp3Value::Error(format!("ERR wrong number of arguments for '{cmd}' command"))
}

// ── KV commands ───────────────────────────────────────────────────────────────

fn cmd_ping(args: &[Resp3Value]) -> Resp3Value {
    if args.len() > 1 {
        // PING <message> → bulk string echo
        Resp3Value::BulkString(args[1].as_bytes().unwrap_or(b"PONG").to_vec())
    } else {
        Resp3Value::SimpleString("PONG".into())
    }
}

async fn cmd_set(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("SET");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let value = match arg_bytes(args, 2) {
        Some(v) => v,
        None => return Resp3Value::Error("ERR value required".into()),
    };

    // Optional: EX <seconds>
    let mut ttl: Option<u64> = None;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("EX") => {
                ttl = arg_u64(args, i + 1);
                i += 2;
            }
            Some("PX") => {
                // Convert milliseconds to seconds (rounded up)
                ttl = arg_u64(args, i + 1).map(|ms| ms.div_ceil(1000));
                i += 2;
            }
            _ => i += 1,
        }
    }

    match state.kv_store.set(key, value, ttl).await {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_get(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("GET");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    match state.kv_store.get(&key).await {
        Ok(Some(v)) => Resp3Value::BulkString(v),
        Ok(None) => Resp3Value::Null,
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_del(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("DEL");
    }
    let mut deleted = 0i64;
    for i in 1..args.len() {
        if let Some(key) = arg_str(args, i) {
            match state.kv_store.delete(&key).await {
                Ok(true) => deleted += 1,
                Ok(false) => {}
                Err(e) => return Resp3Value::Error(format!("ERR {e}")),
            }
        }
    }
    Resp3Value::Integer(deleted)
}

async fn cmd_exists(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("EXISTS");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    match state.kv_store.exists(&key).await {
        Ok(true) => Resp3Value::Integer(1),
        Ok(false) => Resp3Value::Integer(0),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_expire(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("EXPIRE");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("EXPIRE"),
    };
    let secs = match arg_u64(args, 2) {
        Some(s) => s,
        None => return Resp3Value::Error("ERR value is not an integer".into()),
    };
    match state.kv_store.expire(&key, secs).await {
        Ok(true) => Resp3Value::Integer(1),
        Ok(false) => Resp3Value::Integer(0),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_ttl(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("TTL");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("TTL"),
    };
    match state.kv_store.ttl(&key).await {
        Ok(Some(secs)) => Resp3Value::Integer(secs as i64),
        Ok(None) => Resp3Value::Integer(-1),
        Err(_) => Resp3Value::Integer(-2), // key doesn't exist
    }
}

async fn cmd_persist(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("PERSIST");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("PERSIST"),
    };
    match state.kv_store.persist(&key).await {
        Ok(true) => Resp3Value::Integer(1),
        Ok(false) => Resp3Value::Integer(0),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_incr(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("INCR");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("INCR"),
    };
    match state.kv_store.incr(&key, 1).await {
        Ok(v) => Resp3Value::Integer(v),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_incrby(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("INCRBY");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("INCRBY"),
    };
    let by = match arg_i64(args, 2) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR value is not an integer".into()),
    };
    match state.kv_store.incr(&key, by).await {
        Ok(v) => Resp3Value::Integer(v),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_decr(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("DECR");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("DECR"),
    };
    match state.kv_store.decr(&key, 1).await {
        Ok(v) => Resp3Value::Integer(v),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_decrby(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("DECRBY");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("DECRBY"),
    };
    let by = match arg_i64(args, 2) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR value is not an integer".into()),
    };
    match state.kv_store.decr(&key, by).await {
        Ok(v) => Resp3Value::Integer(v),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_mset(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 || (args.len() - 1) % 2 != 0 {
        return Resp3Value::Error("ERR syntax error — MSET key value [key value ...]".into());
    }
    let mut pairs = Vec::new();
    let mut i = 1;
    while i + 1 < args.len() {
        let k = match arg_str(args, i) {
            Some(k) => k,
            None => return err_wrong_args("MSET"),
        };
        let v = match arg_bytes(args, i + 1) {
            Some(v) => v,
            None => return err_wrong_args("MSET"),
        };
        pairs.push((k, v));
        i += 2;
    }
    match state.kv_store.mset(pairs).await {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_mget(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("MGET");
    }
    let keys: Vec<String> = (1..args.len()).filter_map(|i| arg_str(args, i)).collect();
    match state.kv_store.mget(&keys).await {
        Ok(values) => Resp3Value::Array(
            values
                .into_iter()
                .map(|v| v.map(Resp3Value::BulkString).unwrap_or(Resp3Value::Null))
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_keys(state: &AppState, _args: &[Resp3Value]) -> Resp3Value {
    match state.kv_store.keys().await {
        Ok(keys) => Resp3Value::Array(
            keys.into_iter()
                .map(|k| Resp3Value::BulkString(k.into_bytes()))
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_scan(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    // SCAN cursor [MATCH pattern] [COUNT count]
    // We ignore cursor (always 0) and use prefix from MATCH
    let mut prefix: Option<String> = None;
    let mut limit = 100usize;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("MATCH") => {
                prefix = arg_str(args, i + 1).filter(|p| p != "*");
                i += 2;
            }
            Some("COUNT") => {
                limit = arg_u64(args, i + 1).unwrap_or(100) as usize;
                i += 2;
            }
            _ => i += 1,
        }
    }
    match state.kv_store.scan(prefix.as_deref(), limit).await {
        Ok(keys) => {
            let items: Vec<Resp3Value> = keys
                .into_iter()
                .map(|k| Resp3Value::BulkString(k.into_bytes()))
                .collect();
            // SCAN response: [cursor, [keys]]
            Resp3Value::Array(vec![
                Resp3Value::BulkString(b"0".to_vec()),
                Resp3Value::Array(items),
            ])
        }
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── Hash commands ─────────────────────────────────────────────────────────────

async fn cmd_hset(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 || (args.len() - 2) % 2 != 0 {
        return Resp3Value::Error("ERR syntax — HSET key field value [field value ...]".into());
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("HSET"),
    };
    let mut added = 0i64;
    let mut i = 2;
    while i + 1 < args.len() {
        let field = match arg_str(args, i) {
            Some(f) => f,
            None => return err_wrong_args("HSET"),
        };
        let value = match arg_bytes(args, i + 1) {
            Some(v) => v,
            None => return err_wrong_args("HSET"),
        };
        match state.hash_store.hset(&key, &field, value) {
            Ok(is_new) => {
                if is_new {
                    added += 1;
                }
            }
            Err(e) => return Resp3Value::Error(format!("ERR {e}")),
        }
        i += 2;
    }
    Resp3Value::Integer(added)
}

async fn cmd_hget(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("HGET");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("HGET"),
    };
    let field = match arg_str(args, 2) {
        Some(f) => f,
        None => return err_wrong_args("HGET"),
    };
    match state.hash_store.hget(&key, &field) {
        Ok(Some(v)) => Resp3Value::BulkString(v),
        Ok(None) => Resp3Value::Null,
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_hdel(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("HDEL");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("HDEL"),
    };
    let fields: Vec<String> = (2..args.len()).filter_map(|i| arg_str(args, i)).collect();
    match state.hash_store.hdel(&key, &fields) {
        Ok(n) => Resp3Value::Integer(n as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_hgetall(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("HGETALL");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("HGETALL"),
    };
    match state.hash_store.hgetall(&key) {
        Ok(map) => {
            let mut items = Vec::with_capacity(map.len() * 2);
            for (f, v) in map {
                items.push(Resp3Value::BulkString(f.into_bytes()));
                items.push(Resp3Value::BulkString(v));
            }
            Resp3Value::Array(items)
        }
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_hmset(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    // HMSET is an alias for HSET in Redis 4+
    cmd_hset(state, args).await;
    Resp3Value::SimpleString("OK".into())
}

async fn cmd_hmget(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("HMGET");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("HMGET"),
    };
    let mut results = Vec::new();
    for i in 2..args.len() {
        if let Some(field) = arg_str(args, i) {
            match state.hash_store.hget(&key, &field) {
                Ok(Some(v)) => results.push(Resp3Value::BulkString(v)),
                Ok(None) => results.push(Resp3Value::Null),
                Err(e) => return Resp3Value::Error(format!("ERR {e}")),
            }
        }
    }
    Resp3Value::Array(results)
}

async fn cmd_hlen(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("HLEN");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("HLEN"),
    };
    match state.hash_store.hlen(&key) {
        Ok(n) => Resp3Value::Integer(n as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_hexists(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("HEXISTS");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("HEXISTS"),
    };
    let field = match arg_str(args, 2) {
        Some(f) => f,
        None => return err_wrong_args("HEXISTS"),
    };
    match state.hash_store.hexists(&key, &field) {
        Ok(b) => Resp3Value::Integer(if b { 1 } else { 0 }),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── List commands ─────────────────────────────────────────────────────────────

async fn cmd_lpush(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("LPUSH");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("LPUSH"),
    };
    let values: Vec<Vec<u8>> = (2..args.len()).filter_map(|i| arg_bytes(args, i)).collect();
    match state.list_store.lpush(&key, values, false) {
        Ok(len) => Resp3Value::Integer(len as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_rpush(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("RPUSH");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("RPUSH"),
    };
    let values: Vec<Vec<u8>> = (2..args.len()).filter_map(|i| arg_bytes(args, i)).collect();
    match state.list_store.rpush(&key, values, false) {
        Ok(len) => Resp3Value::Integer(len as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_lpop(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("LPOP");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("LPOP"),
    };
    match state.list_store.lpop(&key, Some(1)) {
        Ok(mut v) if !v.is_empty() => Resp3Value::BulkString(v.remove(0)),
        Ok(_) => Resp3Value::Null,
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_rpop(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("RPOP");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("RPOP"),
    };
    match state.list_store.rpop(&key, Some(1)) {
        Ok(mut v) if !v.is_empty() => Resp3Value::BulkString(v.remove(0)),
        Ok(_) => Resp3Value::Null,
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_lrange(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("LRANGE");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("LRANGE"),
    };
    let start = match arg_i64(args, 2) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR value is not an integer".into()),
    };
    let stop = match arg_i64(args, 3) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR value is not an integer".into()),
    };
    match state.list_store.lrange(&key, start, stop) {
        Ok(items) => Resp3Value::Array(items.into_iter().map(Resp3Value::BulkString).collect()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_llen(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("LLEN");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("LLEN"),
    };
    match state.list_store.llen(&key) {
        Ok(n) => Resp3Value::Integer(n as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── Set commands ──────────────────────────────────────────────────────────────

async fn cmd_sadd(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("SADD");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("SADD"),
    };
    let members: Vec<Vec<u8>> = (2..args.len()).filter_map(|i| arg_bytes(args, i)).collect();
    match state.set_store.sadd(&key, members) {
        Ok(added) => Resp3Value::Integer(added as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_smembers(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("SMEMBERS");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("SMEMBERS"),
    };
    match state.set_store.smembers(&key) {
        Ok(members) => Resp3Value::Array(members.into_iter().map(Resp3Value::BulkString).collect()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_srem(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("SREM");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("SREM"),
    };
    let members: Vec<Vec<u8>> = (2..args.len()).filter_map(|i| arg_bytes(args, i)).collect();
    match state.set_store.srem(&key, members) {
        Ok(removed) => Resp3Value::Integer(removed as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_sismember(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("SISMEMBER");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("SISMEMBER"),
    };
    let member = match arg_bytes(args, 2) {
        Some(m) => m,
        None => return err_wrong_args("SISMEMBER"),
    };
    match state.set_store.sismember(&key, member) {
        Ok(b) => Resp3Value::Integer(if b { 1 } else { 0 }),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_scard(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("SCARD");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("SCARD"),
    };
    match state.set_store.scard(&key) {
        Ok(n) => Resp3Value::Integer(n as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── Sorted set commands ───────────────────────────────────────────────────────

async fn cmd_zadd(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("ZADD");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("ZADD"),
    };
    let score = match arg_f64(args, 2) {
        Some(s) => s,
        None => return Resp3Value::Error("ERR score is not a float".into()),
    };
    let member = match arg_bytes(args, 3) {
        Some(m) => m,
        None => return err_wrong_args("ZADD"),
    };
    let opts = ZAddOptions::default();
    let (added, _) = state.sorted_set_store.zadd(&key, member, score, &opts);
    Resp3Value::Integer(added as i64)
}

async fn cmd_zrange(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("ZRANGE");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("ZRANGE"),
    };
    let start = match arg_i64(args, 2) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR value is not an integer".into()),
    };
    let stop = match arg_i64(args, 3) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR value is not an integer".into()),
    };
    let with_scores = args
        .get(4)
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case("WITHSCORES"))
        .unwrap_or(false);
    let members = state
        .sorted_set_store
        .zrange(&key, start, stop, with_scores);
    if with_scores {
        let mut items = Vec::new();
        for sm in &members {
            items.push(Resp3Value::BulkString(sm.member.clone()));
            items.push(Resp3Value::BulkString(sm.score.to_string().into_bytes()));
        }
        Resp3Value::Array(items)
    } else {
        Resp3Value::Array(
            members
                .into_iter()
                .map(|sm| Resp3Value::BulkString(sm.member))
                .collect(),
        )
    }
}

async fn cmd_zscore(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("ZSCORE");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("ZSCORE"),
    };
    let member = match arg_bytes(args, 2) {
        Some(m) => m,
        None => return err_wrong_args("ZSCORE"),
    };
    match state.sorted_set_store.zscore(&key, &member) {
        Some(s) => Resp3Value::BulkString(s.to_string().into_bytes()),
        None => Resp3Value::Null,
    }
}

async fn cmd_zcard(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("ZCARD");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("ZCARD"),
    };
    Resp3Value::Integer(state.sorted_set_store.zcard(&key) as i64)
}

async fn cmd_zrem(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("ZREM");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("ZREM"),
    };
    let members: Vec<Vec<u8>> = (2..args.len()).filter_map(|i| arg_bytes(args, i)).collect();
    let removed = state.sorted_set_store.zrem(&key, &members);
    Resp3Value::Integer(removed as i64)
}

// ── HyperLogLog ───────────────────────────────────────────────────────────────

async fn cmd_pfadd(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("PFADD");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("PFADD"),
    };
    let elements: Vec<Vec<u8>> = (2..args.len()).filter_map(|i| arg_bytes(args, i)).collect();
    match state.hyperloglog_store.pfadd(&key, elements, None) {
        Ok(n) => Resp3Value::Integer(if n > 0 { 1 } else { 0 }),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_pfcount(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("PFCOUNT");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("PFCOUNT"),
    };
    match state.hyperloglog_store.pfcount(&key) {
        Ok(n) => Resp3Value::Integer(n as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── Bitmap ────────────────────────────────────────────────────────────────────

async fn cmd_bitcount(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("BITCOUNT");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("BITCOUNT"),
    };
    let (start, end) = if args.len() >= 4 {
        let s = arg_i64(args, 2).unwrap_or(0).max(0) as usize;
        let e = arg_i64(args, 3).unwrap_or(-1);
        let e = if e < 0 { None } else { Some(e as usize) };
        (Some(s), e)
    } else {
        (None, None)
    };
    match state.bitmap_store.bitcount(&key, start, end) {
        Ok(n) => Resp3Value::Integer(n as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_setbit(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("SETBIT");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("SETBIT"),
    };
    let offset = match arg_u64(args, 2) {
        Some(o) => o as usize,
        None => {
            return Resp3Value::Error("ERR bit offset is not an integer or out of range".into());
        }
    };
    let bit = match arg_i64(args, 3) {
        Some(v) => v as u8 & 1,
        None => return Resp3Value::Error("ERR bit is not an integer or out of range".into()),
    };
    match state.bitmap_store.setbit(&key, offset, bit) {
        Ok(prev) => Resp3Value::Integer(prev as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

async fn cmd_getbit(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("GETBIT");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("GETBIT"),
    };
    let offset = match arg_u64(args, 2) {
        Some(o) => o as usize,
        None => {
            return Resp3Value::Error("ERR bit offset is not an integer or out of range".into());
        }
    };
    match state.bitmap_store.getbit(&key, offset) {
        Ok(b) => Resp3Value::Integer(b as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── Misc ──────────────────────────────────────────────────────────────────────

async fn cmd_flushall(state: &AppState) -> Resp3Value {
    let _ = state.kv_store.flushall().await;
    Resp3Value::SimpleString("OK".into())
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
}
