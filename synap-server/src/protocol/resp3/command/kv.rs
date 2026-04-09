use super::{AppState, Resp3Value, arg_bytes, arg_i64, arg_str, arg_u64, err_wrong_args};

// ── KV commands ───────────────────────────────────────────────────────────────

pub(super) fn cmd_ping(args: &[Resp3Value]) -> Resp3Value {
    if args.len() > 1 {
        // PING <message> → bulk string echo
        Resp3Value::BulkString(args[1].as_bytes().unwrap_or(b"PONG").to_vec())
    } else {
        Resp3Value::SimpleString("PONG".into())
    }
}

pub(super) async fn cmd_set(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_get(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_del(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_exists(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_expire(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_ttl(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_persist(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_incr(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_incrby(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_decr(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_decrby(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_mset(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_mget(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_keys(state: &AppState, _args: &[Resp3Value]) -> Resp3Value {
    match state.kv_store.keys().await {
        Ok(keys) => Resp3Value::Array(
            keys.into_iter()
                .map(|k| Resp3Value::BulkString(k.into_bytes()))
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_scan(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

// ── Misc ──────────────────────────────────────────────────────────────────────

pub(super) async fn cmd_flushall(state: &AppState) -> Resp3Value {
    let _ = state.kv_store.flushall().await;
    Resp3Value::SimpleString("OK".into())
}

// ── KV stats (3.8) ────────────────────────────────────────────────────────────

pub(super) async fn cmd_synap_kvstats(state: &AppState) -> Resp3Value {
    let stats = state.kv_store.stats().await;
    Resp3Value::Array(vec![
        Resp3Value::BulkString(b"total_keys".to_vec()),
        Resp3Value::Integer(stats.total_keys),
        Resp3Value::BulkString(b"total_memory_bytes".to_vec()),
        Resp3Value::Integer(stats.total_memory_bytes),
        Resp3Value::BulkString(b"gets".to_vec()),
        Resp3Value::Integer(stats.gets as i64),
        Resp3Value::BulkString(b"sets".to_vec()),
        Resp3Value::Integer(stats.sets as i64),
        Resp3Value::BulkString(b"dels".to_vec()),
        Resp3Value::Integer(stats.dels as i64),
        Resp3Value::BulkString(b"hits".to_vec()),
        Resp3Value::Integer(stats.hits as i64),
        Resp3Value::BulkString(b"misses".to_vec()),
        Resp3Value::Integer(stats.misses as i64),
    ])
}

// ── Additional KV / hash parity ───────────────────────────────────────────────

pub(super) async fn cmd_append(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("APPEND");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let value = match arg_bytes(args, 2) {
        Some(v) => v,
        None => return Resp3Value::Error("ERR value required".into()),
    };
    match state.kv_store.append(&key, value).await {
        Ok(len) => Resp3Value::Integer(len as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_getrange(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("GETRANGE");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let start = match arg_i64(args, 2) {
        Some(s) => s as isize,
        None => return Resp3Value::Error("ERR start must be an integer".into()),
    };
    let end = match arg_i64(args, 3) {
        Some(e) => e as isize,
        None => return Resp3Value::Error("ERR end must be an integer".into()),
    };
    match state.kv_store.getrange(&key, start, end).await {
        Ok(bytes) => Resp3Value::BulkString(bytes),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_setrange(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("SETRANGE");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let offset = match arg_u64(args, 2) {
        Some(o) => o as usize,
        None => return Resp3Value::Error("ERR offset must be an integer".into()),
    };
    let value = match arg_bytes(args, 3) {
        Some(v) => v,
        None => return Resp3Value::Error("ERR value required".into()),
    };
    match state.kv_store.setrange(&key, offset, value).await {
        Ok(len) => Resp3Value::Integer(len as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_strlen(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("STRLEN");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    match state.kv_store.strlen(&key).await {
        Ok(len) => Resp3Value::Integer(len as i64),
        Err(_) => Resp3Value::Integer(0), // missing key → 0 (Redis behaviour)
    }
}

pub(super) async fn cmd_getset(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("GETSET");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let value = match arg_bytes(args, 2) {
        Some(v) => v,
        None => return Resp3Value::Error("ERR value required".into()),
    };
    match state.kv_store.getset(&key, value).await {
        Ok(Some(old)) => Resp3Value::BulkString(old),
        Ok(None) => Resp3Value::Null,
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_msetnx(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 || (args.len() - 1) % 2 != 0 {
        return err_wrong_args("MSETNX");
    }
    let pairs: Vec<(String, Vec<u8>)> = (0..(args.len() - 1) / 2)
        .filter_map(|i| {
            let key = arg_str(args, 1 + i * 2)?;
            let val = arg_bytes(args, 2 + i * 2)?;
            Some((key, val))
        })
        .collect();
    match state.kv_store.msetnx(pairs).await {
        Ok(true) => Resp3Value::Integer(1),
        Ok(false) => Resp3Value::Integer(0),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_dbsize(state: &AppState) -> Resp3Value {
    match state.kv_store.dbsize().await {
        Ok(size) => Resp3Value::Integer(size as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}
