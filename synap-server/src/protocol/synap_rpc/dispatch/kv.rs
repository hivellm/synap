use super::{AppState, SynapValue, arg_bytes, arg_int, arg_str};

pub(super) async fn run(
    state: &AppState,
    command: &str,
    args: &[SynapValue],
) -> Result<SynapValue, String> {
    match command {
        // ── KV ────────────────────────────────────────────────────────────────
        "PING" => Ok(SynapValue::Str(if args.is_empty() {
            "PONG".into()
        } else {
            arg_str(args, 0)?
        })),

        "SET" => {
            let key = arg_str(args, 0)?;
            let value = arg_bytes(args, 1)?;
            let ttl: Option<u64> = args.get(2).and_then(|v| v.as_int()).map(|n| n as u64);
            state
                .kv_store
                .set(key, value, ttl)
                .await
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "GET" => {
            let key = arg_str(args, 0)?;
            state
                .kv_store
                .get(&key)
                .await
                .map(|opt| opt.map(SynapValue::Bytes).unwrap_or(SynapValue::Null))
                .map_err(|e| e.to_string())
        }
        "DEL" => {
            let mut deleted = 0i64;
            for v in args {
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
            let key = arg_str(args, 0)?;
            state
                .kv_store
                .exists(&key)
                .await
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "EXPIRE" => {
            let key = arg_str(args, 0)?;
            let secs = arg_int(args, 1)? as u64;
            state
                .kv_store
                .expire(&key, secs)
                .await
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "TTL" => {
            let key = arg_str(args, 0)?;
            state
                .kv_store
                .ttl(&key)
                .await
                .map(|opt| SynapValue::Int(opt.map(|s| s as i64).unwrap_or(-1)))
                .map_err(|_| "ERR key does not exist".into())
        }
        "PERSIST" => {
            let key = arg_str(args, 0)?;
            state
                .kv_store
                .persist(&key)
                .await
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "INCR" => {
            let key = arg_str(args, 0)?;
            state
                .kv_store
                .incr(&key, 1)
                .await
                .map(SynapValue::Int)
                .map_err(|e| e.to_string())
        }
        "INCRBY" => {
            let key = arg_str(args, 0)?;
            let by = arg_int(args, 1)?;
            state
                .kv_store
                .incr(&key, by)
                .await
                .map(SynapValue::Int)
                .map_err(|e| e.to_string())
        }
        "DECR" => {
            let key = arg_str(args, 0)?;
            state
                .kv_store
                .decr(&key, 1)
                .await
                .map(SynapValue::Int)
                .map_err(|e| e.to_string())
        }
        "DECRBY" => {
            let key = arg_str(args, 0)?;
            let by = arg_int(args, 1)?;
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
                pairs.push((arg_str(args, i)?, arg_bytes(args, i + 1)?));
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

        // ── Bitmap ────────────────────────────────────────────────────────────
        "BITCOUNT" => {
            let key = arg_str(args, 0)?;
            let (start, end) = if args.len() >= 3 {
                let s = arg_int(args, 1)?.max(0) as usize;
                let e = arg_int(args, 2)?;
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
            let key = arg_str(args, 0)?;
            let offset = arg_int(args, 1)? as usize;
            let bit = arg_int(args, 2)? as u8 & 1;
            state
                .bitmap_store
                .setbit(&key, offset, bit)
                .map(|prev| SynapValue::Int(prev as i64))
                .map_err(|e| e.to_string())
        }
        "GETBIT" => {
            let key = arg_str(args, 0)?;
            let offset = arg_int(args, 1)? as usize;
            state
                .bitmap_store
                .getbit(&key, offset)
                .map(|b| SynapValue::Int(b as i64))
                .map_err(|e| e.to_string())
        }

        // ── KV extensions ─────────────────────────────────────────────────────
        "SCAN" => {
            let prefix = args.first().and_then(|v| v.as_str()).map(|s| s.to_owned());
            let limit = args
                .get(1)
                .and_then(|v| v.as_int())
                .map(|n| n as usize)
                .unwrap_or(100);
            state
                .kv_store
                .scan(prefix.as_deref(), limit)
                .await
                .map(|ks| SynapValue::Array(ks.into_iter().map(SynapValue::Str).collect()))
                .map_err(|e| e.to_string())
        }
        "APPEND" => {
            let key = arg_str(args, 0)?;
            let value = arg_bytes(args, 1)?;
            state
                .kv_store
                .append(&key, value)
                .await
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "GETRANGE" => {
            let key = arg_str(args, 0)?;
            let start = arg_int(args, 1)? as isize;
            let end = arg_int(args, 2)? as isize;
            state
                .kv_store
                .getrange(&key, start, end)
                .await
                .map(SynapValue::Bytes)
                .map_err(|e| e.to_string())
        }
        "SETRANGE" => {
            let key = arg_str(args, 0)?;
            let offset = arg_int(args, 1)? as usize;
            let value = arg_bytes(args, 2)?;
            state
                .kv_store
                .setrange(&key, offset, value)
                .await
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "STRLEN" => {
            let key = arg_str(args, 0)?;
            state
                .kv_store
                .strlen(&key)
                .await
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "GETSET" => {
            let key = arg_str(args, 0)?;
            let value = arg_bytes(args, 1)?;
            state
                .kv_store
                .getset(&key, value)
                .await
                .map(|opt| opt.map(SynapValue::Bytes).unwrap_or(SynapValue::Null))
                .map_err(|e| e.to_string())
        }
        "MSETNX" => {
            if args.len() % 2 != 0 {
                return Err("ERR wrong number of arguments for 'MSETNX'".into());
            }
            let mut pairs = Vec::new();
            let mut i = 0;
            while i + 1 < args.len() {
                pairs.push((arg_str(args, i)?, arg_bytes(args, i + 1)?));
                i += 2;
            }
            state
                .kv_store
                .msetnx(pairs)
                .await
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "DBSIZE" => state
            .kv_store
            .dbsize()
            .await
            .map(|n| SynapValue::Int(n as i64))
            .map_err(|e| e.to_string()),
        "KVSTATS" => {
            let s = state.kv_store.stats().await;
            Ok(SynapValue::Map(vec![
                (
                    SynapValue::Str("total_keys".into()),
                    SynapValue::Int(s.total_keys),
                ),
                (
                    SynapValue::Str("total_memory_bytes".into()),
                    SynapValue::Int(s.total_memory_bytes),
                ),
                (
                    SynapValue::Str("gets".into()),
                    SynapValue::Int(s.gets as i64),
                ),
                (
                    SynapValue::Str("sets".into()),
                    SynapValue::Int(s.sets as i64),
                ),
                (
                    SynapValue::Str("dels".into()),
                    SynapValue::Int(s.dels as i64),
                ),
                (
                    SynapValue::Str("hits".into()),
                    SynapValue::Int(s.hits as i64),
                ),
                (
                    SynapValue::Str("misses".into()),
                    SynapValue::Int(s.misses as i64),
                ),
            ]))
        }

        // ── Misc ──────────────────────────────────────────────────────────────
        "FLUSHALL" | "FLUSHDB" => {
            let _ = state.kv_store.flushall().await;
            Ok(SynapValue::Str("OK".into()))
        }

        _ => Err(format!("ERR unknown command '{command}'")),
    }
}
