use super::{AppState, SynapValue, arg_bytes, arg_float, arg_int, arg_str};
use crate::core::sorted_set::ZAddOptions;

pub(super) async fn run(
    state: &AppState,
    command: &str,
    args: &[SynapValue],
) -> Result<SynapValue, String> {
    match command {
        // ── Hash ──────────────────────────────────────────────────────────────
        "HSET" => {
            let key = arg_str(args, 0)?;
            if (args.len() - 1) % 2 != 0 {
                return Err("ERR wrong number of arguments for 'HSET'".into());
            }
            let mut added = 0i64;
            let mut i = 1;
            while i + 1 < args.len() {
                let field = arg_str(args, i)?;
                let value = arg_bytes(args, i + 1)?;
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
            let key = arg_str(args, 0)?;
            let field = arg_str(args, 1)?;
            state
                .hash_store
                .hget(&key, &field)
                .map(|opt| opt.map(SynapValue::Bytes).unwrap_or(SynapValue::Null))
                .map_err(|e| e.to_string())
        }
        "HDEL" => {
            let key = arg_str(args, 0)?;
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
            let key = arg_str(args, 0)?;
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
            state
                .list_store
                .lpush(&key, values, false)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "RPUSH" => {
            let key = arg_str(args, 0)?;
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
            let key = arg_str(args, 0)?;
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
            let key = arg_str(args, 0)?;
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
            let key = arg_str(args, 0)?;
            let start = arg_int(args, 1)?;
            let stop = arg_int(args, 2)?;
            state
                .list_store
                .lrange(&key, start, stop)
                .map(|items| SynapValue::Array(items.into_iter().map(SynapValue::Bytes).collect()))
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
            state
                .set_store
                .sadd(&key, members)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "SMEMBERS" => {
            let key = arg_str(args, 0)?;
            state
                .set_store
                .smembers(&key)
                .map(|ms| SynapValue::Array(ms.into_iter().map(SynapValue::Bytes).collect()))
                .map_err(|e| e.to_string())
        }
        "SREM" => {
            let key = arg_str(args, 0)?;
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
            let (added, _) = state.sorted_set_store.zadd(&key, member, score, &opts);
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
            Ok(SynapValue::Int(
                state.sorted_set_store.zrem(&key, &members) as i64
            ))
        }

        // ── HyperLogLog ───────────────────────────────────────────────────────
        "PFADD" => {
            let key = arg_str(args, 0)?;
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
            if (args.len() - 1) % 2 != 0 {
                return Err("ERR wrong number of arguments for 'HMSET'".into());
            }
            let mut i = 1;
            while i + 1 < args.len() {
                let field = arg_str(args, i)?;
                let value = arg_bytes(args, i + 1)?;
                state
                    .hash_store
                    .hset(&key, &field, value)
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
                        .map(|opt| opt.map(SynapValue::Bytes).unwrap_or(SynapValue::Null))
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
                .map(|map| SynapValue::Array(map.into_values().map(SynapValue::Bytes).collect()))
                .map_err(|e| e.to_string())
        }

        // ── HyperLogLog extensions ─────────────────────────────────────────────
        "PFMERGE" => {
            let dest = arg_str(args, 0)?;
            let sources: Vec<String> = args[1..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            state
                .hyperloglog_store
                .pfmerge(&dest, sources)
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

        _ => Err(format!("ERR unknown command '{command}'")),
    }
}
