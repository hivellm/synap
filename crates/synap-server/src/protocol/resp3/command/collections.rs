use super::{AppState, Resp3Value, arg_bytes, arg_f64, arg_i64, arg_str, arg_u64, err_wrong_args};
use crate::core::sorted_set::ZAddOptions;

// ── Hash commands ─────────────────────────────────────────────────────────────

pub(super) async fn cmd_hset(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_hget(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_hdel(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_hgetall(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_hmset(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    // HMSET is an alias for HSET in Redis 4+
    cmd_hset(state, args).await;
    Resp3Value::SimpleString("OK".into())
}

pub(super) async fn cmd_hmget(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_hlen(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_hexists(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_lpush(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_rpush(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_lpop(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_rpop(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_lrange(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_llen(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_sadd(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_smembers(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_srem(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_sismember(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_scard(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_zadd(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_zrange(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_zscore(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_zcard(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("ZCARD");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("ZCARD"),
    };
    Resp3Value::Integer(state.sorted_set_store.zcard(&key) as i64)
}

pub(super) async fn cmd_zrem(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_pfadd(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_pfcount(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_bitcount(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_setbit(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

pub(super) async fn cmd_getbit(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
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

// ── HyperLogLog (3.6) ─────────────────────────────────────────────────────────

/// `PFMERGE <dest> <src1> [src2 ...]`
pub(super) async fn cmd_pfmerge(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("PFMERGE");
    }
    let dest = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR dest key must be a string".into()),
    };
    let sources: Vec<String> = (2..args.len()).filter_map(|i| arg_str(args, i)).collect();
    match state.hyperloglog_store.pfmerge(&dest, sources) {
        Ok(_) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_hkeys(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("HKEYS");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    match state.hash_store.hkeys(&key) {
        Ok(fields) => Resp3Value::Array(
            fields
                .into_iter()
                .map(|f| Resp3Value::BulkString(f.into_bytes()))
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_hvals(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("HVALS");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    match state.hash_store.hvals(&key) {
        Ok(vals) => Resp3Value::Array(vals.into_iter().map(Resp3Value::BulkString).collect()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── SCAN cursors (HSCAN / SSCAN / ZSCAN) ──────────────────────────────────────

/// Parse the optional `[MATCH pattern] [COUNT count]` tail of a `*SCAN` command,
/// starting at `args[start]`. Returns `(pattern, count)` with `count` defaulting
/// to 10 (Redis default), or `None` on a malformed option.
fn parse_scan_opts(args: &[Resp3Value], start: usize) -> Option<(Option<String>, usize)> {
    let mut pattern = None;
    let mut count = 10usize;
    let mut i = start;
    while i < args.len() {
        let opt = arg_str(args, i)?;
        match opt.to_ascii_uppercase().as_str() {
            "MATCH" => {
                pattern = Some(arg_str(args, i + 1)?);
                i += 2;
            }
            "COUNT" => {
                count = arg_i64(args, i + 1)?.max(1) as usize;
                i += 2;
            }
            _ => return None,
        }
    }
    Some((pattern, count))
}

/// `HSCAN key cursor [MATCH pattern] [COUNT count]`
pub(super) async fn cmd_hscan(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("HSCAN");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("HSCAN"),
    };
    let cursor = match arg_u64(args, 2) {
        Some(c) => c,
        None => return Resp3Value::Error("ERR invalid cursor".into()),
    };
    let (pattern, count) = match parse_scan_opts(args, 3) {
        Some(o) => o,
        None => return Resp3Value::Error("ERR syntax error".into()),
    };
    match state
        .hash_store
        .hscan(&key, cursor, pattern.as_deref(), count)
    {
        Ok((next, items)) => {
            let mut pairs = Vec::with_capacity(items.len() * 2);
            for (f, v) in items {
                pairs.push(Resp3Value::BulkString(f.into_bytes()));
                pairs.push(Resp3Value::BulkString(v));
            }
            Resp3Value::Array(vec![
                Resp3Value::BulkString(next.to_string().into_bytes()),
                Resp3Value::Array(pairs),
            ])
        }
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `SSCAN key cursor [MATCH pattern] [COUNT count]`
pub(super) async fn cmd_sscan(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("SSCAN");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("SSCAN"),
    };
    let cursor = match arg_u64(args, 2) {
        Some(c) => c,
        None => return Resp3Value::Error("ERR invalid cursor".into()),
    };
    let (pattern, count) = match parse_scan_opts(args, 3) {
        Some(o) => o,
        None => return Resp3Value::Error("ERR syntax error".into()),
    };
    match state
        .set_store
        .sscan(&key, cursor, pattern.as_deref(), count)
    {
        Ok((next, items)) => Resp3Value::Array(vec![
            Resp3Value::BulkString(next.to_string().into_bytes()),
            Resp3Value::Array(items.into_iter().map(Resp3Value::BulkString).collect()),
        ]),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `ZSCAN key cursor [MATCH pattern] [COUNT count]`
pub(super) async fn cmd_zscan(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("ZSCAN");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return err_wrong_args("ZSCAN"),
    };
    let cursor = match arg_u64(args, 2) {
        Some(c) => c,
        None => return Resp3Value::Error("ERR invalid cursor".into()),
    };
    let (pattern, count) = match parse_scan_opts(args, 3) {
        Some(o) => o,
        None => return Resp3Value::Error("ERR syntax error".into()),
    };
    let (next, items) = state
        .sorted_set_store
        .zscan(&key, cursor, pattern.as_deref(), count);
    let mut pairs = Vec::with_capacity(items.len() * 2);
    for (m, score) in items {
        pairs.push(Resp3Value::BulkString(m));
        pairs.push(Resp3Value::BulkString(score.to_string().into_bytes()));
    }
    Resp3Value::Array(vec![
        Resp3Value::BulkString(next.to_string().into_bytes()),
        Resp3Value::Array(pairs),
    ])
}

// ── Blocking pops (BLPOP / BRPOP / BZPOPMIN / BZPOPMAX / BRPOPLPUSH) ───────────

/// Split a blocking `CMD key [key ...] timeout` into `(keys, timeout)`. The last
/// argument is the timeout in seconds; Redis treats `0` as "block forever"
/// (mapped to `None`). Returns `None` on a malformed command.
fn parse_block_keys_timeout(args: &[Resp3Value]) -> Option<(Vec<String>, Option<u64>)> {
    if args.len() < 3 {
        return None;
    }
    let timeout_secs = arg_f64(args, args.len() - 1)?;
    if timeout_secs < 0.0 {
        return None;
    }
    let timeout = if timeout_secs == 0.0 {
        None
    } else {
        Some(timeout_secs as u64)
    };
    let keys: Vec<String> = (1..args.len() - 1)
        .filter_map(|i| arg_str(args, i))
        .collect();
    if keys.is_empty() {
        return None;
    }
    Some((keys, timeout))
}

/// `BLPOP key [key ...] timeout` — blocking left pop. Reply: `[key, value]` on a
/// hit, Null on timeout.
pub(super) async fn cmd_blpop(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    let (keys, timeout) = match parse_block_keys_timeout(args) {
        Some(x) => x,
        None => return err_wrong_args("BLPOP"),
    };
    match state.list_store.blpop(keys, timeout).await {
        Ok((key, value)) => Resp3Value::Array(vec![
            Resp3Value::BulkString(key.into_bytes()),
            Resp3Value::BulkString(value),
        ]),
        Err(_) => Resp3Value::Null,
    }
}

/// `BRPOP key [key ...] timeout` — blocking right pop.
pub(super) async fn cmd_brpop(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    let (keys, timeout) = match parse_block_keys_timeout(args) {
        Some(x) => x,
        None => return err_wrong_args("BRPOP"),
    };
    match state.list_store.brpop(keys, timeout).await {
        Ok((key, value)) => Resp3Value::Array(vec![
            Resp3Value::BulkString(key.into_bytes()),
            Resp3Value::BulkString(value),
        ]),
        Err(_) => Resp3Value::Null,
    }
}

/// `BZPOPMIN key [key ...] timeout` — blocking pop of the lowest-scored member.
/// Reply: `[key, member, score]` on a hit, Null on timeout.
pub(super) async fn cmd_bzpopmin(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    let (keys, timeout) = match parse_block_keys_timeout(args) {
        Some(x) => x,
        None => return err_wrong_args("BZPOPMIN"),
    };
    match state.sorted_set_store.bzpopmin(keys, timeout).await {
        Ok((key, member, score)) => Resp3Value::Array(vec![
            Resp3Value::BulkString(key.into_bytes()),
            Resp3Value::BulkString(member),
            Resp3Value::BulkString(score.to_string().into_bytes()),
        ]),
        Err(_) => Resp3Value::Null,
    }
}

/// `BZPOPMAX key [key ...] timeout` — blocking pop of the highest-scored member.
pub(super) async fn cmd_bzpopmax(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    let (keys, timeout) = match parse_block_keys_timeout(args) {
        Some(x) => x,
        None => return err_wrong_args("BZPOPMAX"),
    };
    match state.sorted_set_store.bzpopmax(keys, timeout).await {
        Ok((key, member, score)) => Resp3Value::Array(vec![
            Resp3Value::BulkString(key.into_bytes()),
            Resp3Value::BulkString(member),
            Resp3Value::BulkString(score.to_string().into_bytes()),
        ]),
        Err(_) => Resp3Value::Null,
    }
}

/// `BRPOPLPUSH source destination timeout` — blocking variant of RPOPLPUSH.
/// Reply: the popped/pushed value, or Null on timeout.
pub(super) async fn cmd_brpoplpush(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("BRPOPLPUSH");
    }
    let source = match arg_str(args, 1) {
        Some(s) => s,
        None => return err_wrong_args("BRPOPLPUSH"),
    };
    let dest = match arg_str(args, 2) {
        Some(d) => d,
        None => return err_wrong_args("BRPOPLPUSH"),
    };
    let timeout_secs = match arg_f64(args, 3) {
        Some(t) if t >= 0.0 => t,
        _ => return Resp3Value::Error("ERR timeout is not a float or out of range".into()),
    };
    let timeout = if timeout_secs == 0.0 {
        None
    } else {
        Some(timeout_secs as u64)
    };
    match state.list_store.brpoplpush(&source, &dest, timeout).await {
        Ok(value) => Resp3Value::BulkString(value),
        Err(_) => Resp3Value::Null,
    }
}
