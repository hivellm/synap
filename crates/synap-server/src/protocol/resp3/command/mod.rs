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

    // Uppercase the command name into a stack buffer instead of allocating a
    // `String` on every command. Redis command names are short; anything longer
    // than the buffer is an unknown command anyway, so it falls through to the
    // error arm without an allocation.
    let raw = match args[0].as_bytes() {
        Some(b) => b,
        None => return Resp3Value::Error("ERR command must be a string".into()),
    };
    let mut buf = [0u8; 32];
    let cmd: &str = if raw.len() <= buf.len() {
        for (i, &c) in raw.iter().enumerate() {
            buf[i] = c.to_ascii_uppercase();
        }
        // Bytes came from a valid command token; ASCII-uppercasing keeps it UTF-8.
        std::str::from_utf8(&buf[..raw.len()]).unwrap_or("")
    } else {
        ""
    };

    match cmd {
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
        "HINCRBY" => collections::cmd_hincrby(state, args).await,
        "HINCRBYFLOAT" => collections::cmd_hincrbyfloat(state, args).await,
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
        "BLPOP" => collections::cmd_blpop(state, args).await,
        "BRPOP" => collections::cmd_brpop(state, args).await,
        "BRPOPLPUSH" => collections::cmd_brpoplpush(state, args).await,

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
        "BZPOPMIN" => collections::cmd_bzpopmin(state, args).await,
        "BZPOPMAX" => collections::cmd_bzpopmax(state, args).await,

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

        // Event streams (mirrors the SynapRPC stream family)
        "SCREATE" => advanced::cmd_screate(state, args).await,
        "SGETORCREATE" => advanced::cmd_sgetorcreate(state, args).await,
        "SPUBLISH" => advanced::cmd_spublish(state, args).await,
        "SREAD" => advanced::cmd_sread(state, args).await,
        "SDELETE" => advanced::cmd_sdelete(state, args).await,
        "SLIST" => advanced::cmd_slist(state, args).await,
        "SSTATS" => advanced::cmd_sstats(state, args).await,
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
        "TXQUEUE" => advanced::cmd_txqueue(state, args).await,
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
        "HSCAN" => collections::cmd_hscan(state, args).await,
        "SSCAN" => collections::cmd_sscan(state, args).await,
        "ZSCAN" => collections::cmd_zscan(state, args).await,

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
mod tests;
