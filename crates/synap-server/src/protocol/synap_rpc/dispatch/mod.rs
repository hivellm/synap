//! SynapRPC command dispatcher.
//!
//! Maps `Request.command` strings to AppState store operations and returns a
//! `SynapValue` result.  Mirrors the RESP3 command dispatcher but operates on
//! the binary `SynapValue` type instead of `Resp3Value`.

use super::types::{Request, Response, SynapValue};
use crate::server::handlers::AppState;

mod advanced;
mod collections;
mod kv;

#[cfg(test)]
mod tests;

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
    let cmd = command.to_ascii_uppercase();
    let args = &args;
    match cmd.as_str() {
        "PING" | "SET" | "GET" | "DEL" | "EXISTS" | "EXPIRE" | "TTL" | "PERSIST" | "INCR"
        | "INCRBY" | "DECR" | "DECRBY" | "MSET" | "MGET" | "KEYS" | "BITCOUNT" | "SETBIT"
        | "GETBIT" | "SCAN" | "APPEND" | "GETRANGE" | "SETRANGE" | "STRLEN" | "GETSET"
        | "MSETNX" | "DBSIZE" | "KVSTATS" | "FLUSHALL" | "FLUSHDB" => {
            kv::run(state, cmd.as_str(), args).await
        }

        "HSET" | "HGET" | "HDEL" | "HGETALL" | "HLEN" | "HEXISTS" | "LPUSH" | "RPUSH" | "LPOP"
        | "RPOP" | "LRANGE" | "LLEN" | "SADD" | "SMEMBERS" | "SREM" | "SISMEMBER" | "SCARD"
        | "ZADD" | "ZRANGE" | "ZSCORE" | "ZCARD" | "ZREM" | "PFADD" | "PFCOUNT" | "HMSET"
        | "HMGET" | "HKEYS" | "HVALS" | "PFMERGE" | "HLLSTATS" => {
            collections::run(state, cmd.as_str(), args).await
        }

        _ => advanced::run(state, cmd.as_str(), args).await,
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
