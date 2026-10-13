use super::{AppState, SynapValue, arg_bytes, arg_float, arg_int, arg_str};
use crate::core::sorted_set::ZAddOptions;
use crate::persistence::durable;

pub(super) async fn run(
    state: &AppState,
    command: &str,
    args: &[SynapValue],
) -> Result<SynapValue, String> {
    match command {
        // ── Hash ──────────────────────────────────────────────────────────────
        "HSET" => {
            let key = arg_str(args, 0)?;
            if !(args.len() - 1).is_multiple_of(2) {
                return Err("ERR wrong number of arguments for 'HSET'".into());
            }
            let mut added = 0i64;
            let mut i = 1;
            while i + 1 < args.len() {
                let field = arg_str(args, i)?;
                let value = arg_bytes(args, i + 1)?;
                match durable::hset(
                    state.persistence.as_deref(),
                    &state.hash_store,
                    &key,
                    &field,
                    value,
                )
                .await
                {
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
            let key = arg_str(args, 0)?;
            let field = arg_str(args, 1)?;
            state
                .hash_store
                .hget(&key, &field)
                .map(|opt| opt.map(SynapValue::from).unwrap_or(SynapValue::Null))
                .map_err(|e| e.to_string())
        }
        "HDEL" => {
            let key = arg_str(args, 0)?;
            let fields: Vec<String> = args[1..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            durable::hdel(
                state.persistence.as_deref(),
                &state.hash_store,
                &key,
                &fields,
            )
            .await
            .map(|n| SynapValue::Int(n as i64))
            .map_err(|e| e.to_string())
        }
        "HINCRBY" => {
            let key = arg_str(args, 0)?;
            let field = arg_str(args, 1)?;
            let increment = arg_int(args, 2)?;
            durable::hincrby(
                state.persistence.as_deref(),
                &state.hash_store,
                &key,
                &field,
                increment,
            )
            .await
            .map(SynapValue::Int)
            .map_err(|e| e.to_string())
        }
        "HINCRBYFLOAT" => {
            let key = arg_str(args, 0)?;
            let field = arg_str(args, 1)?;
            let increment = arg_float(args, 2)?;
            durable::hincrbyfloat(
                state.persistence.as_deref(),
                &state.hash_store,
                &key,
                &field,
                increment,
            )
            .await
            .map(SynapValue::Float)
            .map_err(|e| e.to_string())
        }
        "HGETALL" => {
            let key = arg_str(args, 0)?;
            state
                .hash_store
                .hgetall(&key)
                .map(|map| {
                    let mut pairs = Vec::new();
                    for (f, v) in map {
                        pairs.push((SynapValue::Str(f), SynapValue::from(v)));
                    }
                    SynapValue::Map(pairs)
                })
                .map_err(|e| e.to_string())
        }
        "HLEN" => {
            let key = arg_str(args, 0)?;
            state
                .hash_store
                .hlen(&key)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "HEXISTS" => {
            let key = arg_str(args, 0)?;
            let field = arg_str(args, 1)?;
            state
                .hash_store
                .hexists(&key, &field)
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }

        // ── List ──────────────────────────────────────────────────────────────
        "LPUSH" => {
            let key = arg_str(args, 0)?;
            let values: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            durable::lpush(
                state.persistence.as_deref(),
                &state.list_store,
                &key,
                values,
                false,
            )
            .await
            .map(|n| SynapValue::Int(n as i64))
            .map_err(|e| e.to_string())
        }
        "RPUSH" => {
            let key = arg_str(args, 0)?;
            let values: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            durable::rpush(
                state.persistence.as_deref(),
                &state.list_store,
                &key,
                values,
                false,
            )
            .await
            .map(|n| SynapValue::Int(n as i64))
            .map_err(|e| e.to_string())
        }
        "LPOP" => {
            let key = arg_str(args, 0)?;
            durable::lpop(
                state.persistence.as_deref(),
                &state.list_store,
                &key,
                Some(1),
            )
            .await
            .map(|mut v| {
                if v.is_empty() {
                    SynapValue::Null
                } else {
                    SynapValue::from(v.remove(0))
                }
            })
            .map_err(|e| e.to_string())
        }
        "RPOP" => {
            let key = arg_str(args, 0)?;
            durable::rpop(
                state.persistence.as_deref(),
                &state.list_store,
                &key,
                Some(1),
            )
            .await
            .map(|mut v| {
                if v.is_empty() {
                    SynapValue::Null
                } else {
                    SynapValue::from(v.remove(0))
                }
            })
            .map_err(|e| e.to_string())
        }
        "LRANGE" => {
            let key = arg_str(args, 0)?;
            let start = arg_int(args, 1)?;
            let stop = arg_int(args, 2)?;
            state
                .list_store
                .lrange(&key, start, stop)
                .map(|items| SynapValue::Array(items.into_iter().map(SynapValue::from).collect()))
                .map_err(|e| e.to_string())
        }
        "LLEN" => {
            let key = arg_str(args, 0)?;
            state
                .list_store
                .llen(&key)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }

        // ── Set ───────────────────────────────────────────────────────────────
        "SADD" => {
            let key = arg_str(args, 0)?;
            let members: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            durable::sadd(
                state.persistence.as_deref(),
                &state.set_store,
                &key,
                members,
            )
            .await
            .map(|n| SynapValue::Int(n as i64))
            .map_err(|e| e.to_string())
        }
        "SMEMBERS" => {
            let key = arg_str(args, 0)?;
            state
                .set_store
                .smembers(&key)
                .map(|ms| SynapValue::Array(ms.into_iter().map(SynapValue::from).collect()))
                .map_err(|e| e.to_string())
        }
        "SREM" => {
            let key = arg_str(args, 0)?;
            let members: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            durable::srem(
                state.persistence.as_deref(),
                &state.set_store,
                &key,
                members,
            )
            .await
            .map(|n| SynapValue::Int(n as i64))
            .map_err(|e| e.to_string())
        }
        "SISMEMBER" => {
            let key = arg_str(args, 0)?;
            let member = arg_bytes(args, 1)?;
            state
                .set_store
                .sismember(&key, member)
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "SCARD" => {
            let key = arg_str(args, 0)?;
            state
                .set_store
                .scard(&key)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }

        // ── Sorted set ────────────────────────────────────────────────────────
        "ZADD" => {
            let key = arg_str(args, 0)?;
            let score = arg_float(args, 1)?;
            let member = arg_bytes(args, 2)?;
            let opts = ZAddOptions::default();
            let (added, _) = durable::zadd(
                state.persistence.as_deref(),
                &state.sorted_set_store,
                &key,
                member,
                score,
                &opts,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(SynapValue::Int(added as i64))
        }
        "ZRANGE" => {
            let key = arg_str(args, 0)?;
            let start = arg_int(args, 1)?;
            let stop = arg_int(args, 2)?;
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
                    .map(|sm| (SynapValue::from(sm.member), SynapValue::Float(sm.score)))
                    .collect();
                Ok(SynapValue::Map(pairs))
            } else {
                Ok(SynapValue::Array(
                    members
                        .into_iter()
                        .map(|sm| SynapValue::from(sm.member))
                        .collect(),
                ))
            }
        }
        "ZSCORE" => {
            let key = arg_str(args, 0)?;
            let member = arg_bytes(args, 1)?;
            Ok(state
                .sorted_set_store
                .zscore(&key, &member)
                .map(SynapValue::Float)
                .unwrap_or(SynapValue::Null))
        }
        "ZCARD" => {
            let key = arg_str(args, 0)?;
            Ok(SynapValue::Int(state.sorted_set_store.zcard(&key) as i64))
        }
        "ZREM" => {
            let key = arg_str(args, 0)?;
            let members: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            let removed = durable::zrem(
                state.persistence.as_deref(),
                &state.sorted_set_store,
                &key,
                &members,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(SynapValue::Int(removed as i64))
        }

        // ── HyperLogLog ───────────────────────────────────────────────────────
        "PFADD" => {
            let key = arg_str(args, 0)?;
            let elements: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            durable::pfadd(
                state.persistence.as_deref(),
                &state.hyperloglog_store,
                &key,
                elements,
                None,
            )
            .await
            .map(|n| SynapValue::Bool(n > 0))
            .map_err(|e| e.to_string())
        }
        "PFCOUNT" => {
            let key = arg_str(args, 0)?;
            state
                .hyperloglog_store
                .pfcount(&key)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }

        // ── Hash extensions ────────────────────────────────────────────────────
        "HMSET" => {
            let key = arg_str(args, 0)?;
            if !(args.len() - 1).is_multiple_of(2) {
                return Err("ERR wrong number of arguments for 'HMSET'".into());
            }
            let mut i = 1;
            while i + 1 < args.len() {
                let field = arg_str(args, i)?;
                let value = arg_bytes(args, i + 1)?;
                durable::hset(
                    state.persistence.as_deref(),
                    &state.hash_store,
                    &key,
                    &field,
                    value,
                )
                .await
                .map_err(|e| e.to_string())?;
                i += 2;
            }
            Ok(SynapValue::Str("OK".into()))
        }
        "HMGET" => {
            let key = arg_str(args, 0)?;
            let fields: Vec<String> = args[1..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            let values: Vec<SynapValue> = fields
                .iter()
                .map(|f| {
                    state
                        .hash_store
                        .hget(&key, f)
                        .map(|opt| opt.map(SynapValue::from).unwrap_or(SynapValue::Null))
                        .unwrap_or(SynapValue::Null)
                })
                .collect();
            Ok(SynapValue::Array(values))
        }
        "HKEYS" => {
            let key = arg_str(args, 0)?;
            state
                .hash_store
                .hgetall(&key)
                .map(|map| SynapValue::Array(map.into_keys().map(SynapValue::Str).collect()))
                .map_err(|e| e.to_string())
        }
        "HVALS" => {
            let key = arg_str(args, 0)?;
            state
                .hash_store
                .hgetall(&key)
                .map(|map| SynapValue::Array(map.into_values().map(SynapValue::from).collect()))
                .map_err(|e| e.to_string())
        }

        // ── HyperLogLog extensions ─────────────────────────────────────────────
        "PFMERGE" => {
            let dest = arg_str(args, 0)?;
            let sources: Vec<String> = args[1..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            durable::pfmerge(
                state.persistence.as_deref(),
                &state.hyperloglog_store,
                &dest,
                sources,
            )
            .await
            .map(|n| SynapValue::Int(n as i64))
            .map_err(|e| e.to_string())
        }
        "HLLSTATS" => {
            let s = state.hyperloglog_store.stats();
            Ok(SynapValue::Map(vec![
                (
                    SynapValue::Str("total_hlls".into()),
                    SynapValue::Int(s.total_hlls as i64),
                ),
                (
                    SynapValue::Str("pfadd_count".into()),
                    SynapValue::Int(s.pfadd_count as i64),
                ),
                (
                    SynapValue::Str("pfcount_count".into()),
                    SynapValue::Int(s.pfcount_count as i64),
                ),
                (
                    SynapValue::Str("pfmerge_count".into()),
                    SynapValue::Int(s.pfmerge_count as i64),
                ),
                (
                    SynapValue::Str("total_cardinality".into()),
                    SynapValue::Int(s.total_cardinality as i64),
                ),
            ]))
        }

        // ── SCAN cursors ───────────────────────────────────────────────────────
        "HSCAN" => {
            let key = arg_str(args, 0)?;
            let cursor = arg_int(args, 1)?.max(0) as u64;
            let (pattern, count) = parse_scan_opts(args, 2)?;
            state
                .hash_store
                .hscan(&key, cursor, pattern.as_deref(), count)
                .map(|(next, items)| {
                    let pairs = items
                        .into_iter()
                        .map(|(f, v)| (SynapValue::Str(f), SynapValue::from(v)))
                        .collect();
                    SynapValue::Array(vec![SynapValue::Int(next as i64), SynapValue::Map(pairs)])
                })
                .map_err(|e| e.to_string())
        }
        "SSCAN" => {
            let key = arg_str(args, 0)?;
            let cursor = arg_int(args, 1)?.max(0) as u64;
            let (pattern, count) = parse_scan_opts(args, 2)?;
            state
                .set_store
                .sscan(&key, cursor, pattern.as_deref(), count)
                .map(|(next, items)| {
                    SynapValue::Array(vec![
                        SynapValue::Int(next as i64),
                        SynapValue::Array(items.into_iter().map(SynapValue::from).collect()),
                    ])
                })
                .map_err(|e| e.to_string())
        }
        "ZSCAN" => {
            let key = arg_str(args, 0)?;
            let cursor = arg_int(args, 1)?.max(0) as u64;
            let (pattern, count) = parse_scan_opts(args, 2)?;
            let (next, items) =
                state
                    .sorted_set_store
                    .zscan(&key, cursor, pattern.as_deref(), count);
            let pairs = items
                .into_iter()
                .map(|(m, score)| (SynapValue::from(m), SynapValue::Float(score)))
                .collect();
            Ok(SynapValue::Array(vec![
                SynapValue::Int(next as i64),
                SynapValue::Map(pairs),
            ]))
        }

        // ── Blocking pops ──────────────────────────────────────────────────────
        "BLPOP" => {
            let (keys, timeout) = parse_block_keys_timeout(args)?;
            match durable::blpop(
                state.persistence.as_deref(),
                &state.list_store,
                keys,
                timeout,
            )
            .await
            {
                Ok((key, value)) => Ok(SynapValue::Array(vec![
                    SynapValue::Str(key),
                    SynapValue::from(value),
                ])),
                Err(_) => Ok(SynapValue::Null),
            }
        }
        "BRPOP" => {
            let (keys, timeout) = parse_block_keys_timeout(args)?;
            match durable::brpop(
                state.persistence.as_deref(),
                &state.list_store,
                keys,
                timeout,
            )
            .await
            {
                Ok((key, value)) => Ok(SynapValue::Array(vec![
                    SynapValue::Str(key),
                    SynapValue::from(value),
                ])),
                Err(_) => Ok(SynapValue::Null),
            }
        }
        "BRPOPLPUSH" => {
            let source = arg_str(args, 0)?;
            let dest = arg_str(args, 1)?;
            let timeout = block_timeout(arg_int(args, 2)?)?;
            match durable::brpoplpush(
                state.persistence.as_deref(),
                &state.list_store,
                &source,
                &dest,
                timeout,
            )
            .await
            {
                Ok(value) => Ok(SynapValue::from(value)),
                Err(_) => Ok(SynapValue::Null),
            }
        }
        "BZPOPMIN" => {
            let (keys, timeout) = parse_block_keys_timeout(args)?;
            match durable::bzpopmin(
                state.persistence.as_deref(),
                &state.sorted_set_store,
                keys,
                timeout,
            )
            .await
            {
                Ok((key, member, score)) => Ok(SynapValue::Array(vec![
                    SynapValue::Str(key),
                    SynapValue::from(member),
                    SynapValue::Float(score),
                ])),
                Err(_) => Ok(SynapValue::Null),
            }
        }
        "BZPOPMAX" => {
            let (keys, timeout) = parse_block_keys_timeout(args)?;
            match durable::bzpopmax(
                state.persistence.as_deref(),
                &state.sorted_set_store,
                keys,
                timeout,
            )
            .await
            {
                Ok((key, member, score)) => Ok(SynapValue::Array(vec![
                    SynapValue::Str(key),
                    SynapValue::from(member),
                    SynapValue::Float(score),
                ])),
                Err(_) => Ok(SynapValue::Null),
            }
        }

        _ => Err(format!("ERR unknown command '{command}'")),
    }
}

/// Map a blocking-op timeout in seconds to `Option<u64>`. Redis treats `0` as
/// "block forever" (`None`); a negative timeout is an error.
fn block_timeout(secs: i64) -> Result<Option<u64>, String> {
    if secs < 0 {
        return Err("ERR timeout is negative".into());
    }
    Ok(if secs == 0 { None } else { Some(secs as u64) })
}

/// Split a blocking `CMD key [key ...] timeout` into `(keys, timeout)`. The last
/// argument is the timeout in seconds (`0` = block forever).
fn parse_block_keys_timeout(args: &[SynapValue]) -> Result<(Vec<String>, Option<u64>), String> {
    if args.len() < 2 {
        return Err("ERR wrong number of arguments for blocking pop".into());
    }
    let timeout = block_timeout(arg_int(args, args.len() - 1)?)?;
    let keys: Vec<String> = (0..args.len() - 1)
        .map(|i| arg_str(args, i))
        .collect::<Result<_, _>>()?;
    Ok((keys, timeout))
}

/// Parse the optional `[MATCH pattern] [COUNT count]` tail of a `*SCAN` command,
/// starting at `args[start]`. `count` defaults to 10 (Redis default).
fn parse_scan_opts(args: &[SynapValue], start: usize) -> Result<(Option<String>, usize), String> {
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
                count = arg_int(args, i + 1)?.max(1) as usize;
                i += 2;
            }
            _ => return Err("ERR syntax error".into()),
        }
    }
    Ok((pattern, count))
}
