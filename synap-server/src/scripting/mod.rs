use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use mlua::{Lua, Value as LuaValue, Variadic};
use parking_lot::RwLock;
use sha1::{Digest, Sha1};
use tokio::time;

use crate::core::{
    HashStore, KVStore, ListStore, ScoredMember, SetStore, SortedSetStore, SynapError, ZAddOptions,
};

/// Context passed into script executions for Redis-style bridge calls
#[derive(Clone)]
pub struct ScriptExecContext {
    pub kv_store: Arc<KVStore>,
    pub hash_store: Arc<HashStore>,
    pub list_store: Arc<ListStore>,
    pub set_store: Arc<SetStore>,
    pub sorted_set_store: Arc<SortedSetStore>,
}

#[derive(Default)]
struct ScriptCacheEntry {
    source: Arc<String>,
}

const DISABLED_GLOBALS: &[&str] = &[
    "dofile",
    "load",
    "loadfile",
    "require",
    "collectgarbage",
    "package",
    "os",
    "io",
    "debug",
];

/// Central manager for Lua scripting support
pub struct ScriptManager {
    cache: RwLock<HashMap<String, ScriptCacheEntry>>,
    default_timeout: Duration,
    running: AtomicBool,
}

impl ScriptManager {
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            default_timeout,
            running: AtomicBool::new(false),
        }
    }

    pub fn load_script(&self, source: &str) -> String {
        let sha = compute_sha1(source);
        let mut cache = self.cache.write();
        cache
            .entry(sha.clone())
            .or_insert_with(|| ScriptCacheEntry {
                source: Arc::new(source.to_string()),
            });
        sha
    }

    pub fn script_exists(&self, hashes: &[String]) -> Vec<bool> {
        let cache = self.cache.read();
        hashes.iter().map(|h| cache.contains_key(h)).collect()
    }

    pub fn flush(&self) -> usize {
        let mut cache = self.cache.write();
        let len = cache.len();
        cache.clear();
        len
    }

    pub fn kill_running(&self) -> bool {
        // Cooperative cancellation is not implemented yet; this flag allows callers to
        // observe whether a script was running and reset the state.
        self.running.swap(false, Ordering::SeqCst)
    }

    pub async fn eval(
        &self,
        context: ScriptExecContext,
        script: &str,
        keys: Vec<String>,
        args: Vec<String>,
        timeout: Option<Duration>,
    ) -> Result<(serde_json::Value, String), SynapError> {
        let sha = self.load_script(script);
        self.evalsha(context, &sha, keys, args, timeout)
            .await
            .map(|value| (value, sha))
    }

    pub async fn evalsha(
        &self,
        context: ScriptExecContext,
        sha: &str,
        keys: Vec<String>,
        args: Vec<String>,
        timeout: Option<Duration>,
    ) -> Result<serde_json::Value, SynapError> {
        let source = {
            let cache = self.cache.read();
            cache.get(sha).map(|entry| entry.source.clone())
        }
        .ok_or_else(|| SynapError::InvalidRequest(format!("NOSCRIPT {}", sha)))?;

        let lua = Lua::new();
        apply_sandbox(&lua)?;
        let globals = lua.globals();

        // Populate KEYS table
        let keys_table = lua.create_table().map_err(map_lua_error)?;
        for (idx, key) in keys.iter().enumerate() {
            keys_table
                .set((idx + 1) as i64, key.clone())
                .map_err(map_lua_error)?;
        }
        globals.set("KEYS", keys_table).map_err(map_lua_error)?;

        // Populate ARGV table
        let argv_table = lua.create_table().map_err(map_lua_error)?;
        for (idx, arg) in args.iter().enumerate() {
            argv_table
                .set((idx + 1) as i64, arg.clone())
                .map_err(map_lua_error)?;
        }
        globals.set("ARGV", argv_table).map_err(map_lua_error)?;

        // Inject redis.call bridge
        let redis_table = lua.create_table().map_err(map_lua_error)?;
        let ctx = context.clone();
        let call_fn = lua
            .create_async_function(move |lua, (command, args): (String, Variadic<LuaValue>)| {
                let ctx = ctx.clone();
                async move { handle_redis_call(&lua, command, args, ctx).await }
            })
            .map_err(map_lua_error)?;
        redis_table.set("call", call_fn).map_err(map_lua_error)?;
        globals.set("redis", redis_table).map_err(map_lua_error)?;

        let chunk = lua.load(source.as_str());
        let function = chunk.into_function().map_err(map_lua_error)?;

        let duration = timeout.unwrap_or(self.default_timeout);
        self.running.store(true, Ordering::SeqCst);
        let future = function.call_async::<LuaValue>(());
        let result = time::timeout(duration, future).await;
        self.running.store(false, Ordering::SeqCst);

        let value = match result {
            Ok(inner) => inner.map_err(map_lua_error)?,
            Err(_) => return Err(SynapError::Timeout),
        };

        lua_value_to_json(&lua, value)
    }
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self::new(Duration::from_secs(5))
    }
}

fn compute_sha1(source: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(source.as_bytes());
    hex::encode(hasher.finalize())
}

fn map_lua_error(err: mlua::Error) -> SynapError {
    match err {
        mlua::Error::RuntimeError(msg) | mlua::Error::SyntaxError { message: msg, .. } => {
            SynapError::InvalidRequest(msg)
        }
        other => SynapError::InternalError(other.to_string()),
    }
}

async fn handle_redis_call(
    lua: &Lua,
    command: String,
    args: Variadic<LuaValue>,
    context: ScriptExecContext,
) -> Result<LuaValue, mlua::Error> {
    let normalized = command.to_ascii_lowercase();
    let command_name = command.to_ascii_uppercase();

    match normalized.as_str() {
        "get" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let store = context.kv_store.clone();
            let value = store.get(&key).await.map_err(synap_err_to_lua)?;
            Ok(match value {
                Some(bytes) => LuaValue::String(lua.create_string(bytes)?),
                None => LuaValue::Nil,
            })
        }
        "set" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let value = lua_value_to_bytes(&args[1])?;
            let ttl = if args.len() > 2 {
                Some(lua_value_to_u64(&args[2], &command_name)?)
            } else {
                None
            };
            context
                .kv_store
                .set(&key, value, ttl)
                .await
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::String(lua.create_string("OK")?))
        }
        "del" => {
            ensure_min_args(&args, 1, &command_name)?;
            let store = context.kv_store.clone();
            let mut deleted = 0i64;
            for value in args.iter() {
                let key = lua_value_to_string(value, &command_name)?;
                let removed = store.delete(&key).await.map_err(synap_err_to_lua)?;
                if removed {
                    deleted += 1;
                }
            }
            Ok(LuaValue::Integer(deleted))
        }
        "exists" => {
            ensure_min_args(&args, 1, &command_name)?;
            let store = context.kv_store.clone();
            let mut count = 0i64;
            for value in args.iter() {
                let key = lua_value_to_string(value, &command_name)?;
                let exists = store.exists(&key).await.map_err(synap_err_to_lua)?;
                if exists {
                    count += 1;
                }
            }
            Ok(LuaValue::Integer(count))
        }
        "incr" | "incrby" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let amount = if args.len() > 1 {
                lua_value_to_i64(&args[1], &command_name)?
            } else {
                1
            };
            let result = context
                .kv_store
                .incr(&key, amount)
                .await
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(result))
        }
        "decr" | "decrby" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let amount = if args.len() > 1 {
                lua_value_to_i64(&args[1], &command_name)?
            } else {
                1
            };
            let result = context
                .kv_store
                .incr(&key, -amount)
                .await
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(result))
        }
        "expire" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let ttl = lua_value_to_u64(&args[1], &command_name)?;
            let success = context
                .kv_store
                .expire(&key, ttl)
                .await
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(if success { 1 } else { 0 }))
        }
        "persist" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let success = context
                .kv_store
                .persist(&key)
                .await
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(if success { 1 } else { 0 }))
        }
        "ttl" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            match context.kv_store.ttl(&key).await {
                Ok(Some(ttl)) => Ok(LuaValue::Integer(ttl as i64)),
                Ok(None) => Ok(LuaValue::Integer(-1)),
                Err(SynapError::KeyNotFound(_)) => Ok(LuaValue::Integer(-2)),
                Err(err) => Err(synap_err_to_lua(err)),
            }
        }
        "hset" => {
            ensure_min_args(&args, 3, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let field = lua_value_to_string(&args[1], &command_name)?;
            let value = lua_value_to_bytes(&args[2])?;
            let created = context
                .hash_store
                .hset(&key, &field, value)
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(if created { 1 } else { 0 }))
        }
        "hget" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let field = lua_value_to_string(&args[1], &command_name)?;
            let value = context
                .hash_store
                .hget(&key, &field)
                .map_err(synap_err_to_lua)?;
            Ok(match value {
                Some(bytes) => LuaValue::String(lua.create_string(bytes)?),
                None => LuaValue::Nil,
            })
        }
        "hdel" => {
            let fields = collect_string_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let deleted = context
                .hash_store
                .hdel(&key, &fields)
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(deleted as i64))
        }
        "hexists" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let field = lua_value_to_string(&args[1], &command_name)?;
            let exists = context
                .hash_store
                .hexists(&key, &field)
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(if exists { 1 } else { 0 }))
        }
        "lpush" => {
            let values = collect_bytes_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let len = context
                .list_store
                .lpush(&key, values, false)
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(len as i64))
        }
        "rpush" => {
            let values = collect_bytes_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let len = context
                .list_store
                .rpush(&key, values, false)
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(len as i64))
        }
        "lpop" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let count = if args.len() > 1 {
                Some(lua_value_to_usize(&args[1], &command_name)?)
            } else {
                None
            };
            match context.list_store.lpop(&key, count) {
                Ok(values) => vec_bytes_to_lua(lua, values),
                Err(SynapError::NotFound) | Err(SynapError::KeyExpired) => Ok(LuaValue::Nil),
                Err(err) => Err(synap_err_to_lua(err)),
            }
        }
        "rpop" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let count = if args.len() > 1 {
                Some(lua_value_to_usize(&args[1], &command_name)?)
            } else {
                None
            };
            match context.list_store.rpop(&key, count) {
                Ok(values) => vec_bytes_to_lua(lua, values),
                Err(SynapError::NotFound) | Err(SynapError::KeyExpired) => Ok(LuaValue::Nil),
                Err(err) => Err(synap_err_to_lua(err)),
            }
        }
        "llen" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            match context.list_store.llen(&key) {
                Ok(len) => Ok(LuaValue::Integer(len as i64)),
                Err(SynapError::NotFound) | Err(SynapError::KeyExpired) => Ok(LuaValue::Integer(0)),
                Err(err) => Err(synap_err_to_lua(err)),
            }
        }
        "sadd" => {
            let members = collect_bytes_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let added = context
                .set_store
                .sadd(&key, members)
                .map_err(synap_err_to_lua)?;
            Ok(LuaValue::Integer(added as i64))
        }
        "srem" => {
            let members = collect_bytes_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            match context.set_store.srem(&key, members) {
                Ok(removed) => Ok(LuaValue::Integer(removed as i64)),
                Err(SynapError::NotFound) | Err(SynapError::KeyExpired) => Ok(LuaValue::Integer(0)),
                Err(err) => Err(synap_err_to_lua(err)),
            }
        }
        "sismember" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let member = lua_value_to_bytes(&args[1])?;
            match context.set_store.sismember(&key, member) {
                Ok(result) => Ok(LuaValue::Integer(if result { 1 } else { 0 })),
                Err(SynapError::NotFound) | Err(SynapError::KeyExpired) => Ok(LuaValue::Integer(0)),
                Err(err) => Err(synap_err_to_lua(err)),
            }
        }
        "scard" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            match context.set_store.scard(&key) {
                Ok(len) => Ok(LuaValue::Integer(len as i64)),
                Err(SynapError::NotFound) | Err(SynapError::KeyExpired) => Ok(LuaValue::Integer(0)),
                Err(err) => Err(synap_err_to_lua(err)),
            }
        }
        "zadd" => {
            ensure_min_args(&args, 3, &command_name)?;
            let pair_count = args.len().saturating_sub(1);
            if pair_count % 2 != 0 {
                return Err(mlua::Error::RuntimeError(
                    "ZADD requires score/member pairs".to_string(),
                ));
            }
            let key = lua_value_to_string(&args[0], &command_name)?;
            let mut added = 0i64;
            let opts = ZAddOptions::default();
            let mut idx = 1;
            while idx < args.len() {
                let score = lua_value_to_f64(&args[idx], &command_name)?;
                let member = lua_value_to_bytes(&args[idx + 1])?;
                let (a, _) = context.sorted_set_store.zadd(&key, member, score, &opts);
                added += a as i64;
                idx += 2;
            }
            Ok(LuaValue::Integer(added))
        }
        "zrem" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let members = collect_bytes_args(&args, 1, &command_name)?;
            let removed = context.sorted_set_store.zrem(&key, &members);
            Ok(LuaValue::Integer(removed as i64))
        }
        "zscore" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let member = lua_value_to_bytes(&args[1])?;
            match context.sorted_set_store.zscore(&key, &member) {
                Some(score) => Ok(LuaValue::String(lua.create_string(score.to_string())?)),
                None => Ok(LuaValue::Nil),
            }
        }
        "zcard" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            Ok(LuaValue::Integer(
                context.sorted_set_store.zcard(&key) as i64
            ))
        }
        "zincrby" => {
            ensure_min_args(&args, 3, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let increment = lua_value_to_f64(&args[1], &command_name)?;
            let member = lua_value_to_bytes(&args[2])?;
            let new_score = context.sorted_set_store.zincrby(&key, member, increment);
            Ok(LuaValue::String(lua.create_string(new_score.to_string())?))
        }
        "zcount" => {
            ensure_min_args(&args, 3, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let min = lua_value_to_f64(&args[1], &command_name)?;
            let max = lua_value_to_f64(&args[2], &command_name)?;
            Ok(LuaValue::Integer(
                context.sorted_set_store.zcount(&key, min, max) as i64,
            ))
        }
        "zrange" | "zrevrange" => {
            ensure_min_args(&args, 3, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let start = lua_value_to_i64(&args[1], &command_name)?;
            let stop = lua_value_to_i64(&args[2], &command_name)?;
            let with_scores = args
                .get(3)
                .map(|arg| lua_value_to_string(arg, &command_name))
                .transpose()? // convert Option<Result> -> Result<Option>
                .map(|val| val.eq_ignore_ascii_case("withscores"))
                .unwrap_or(false);

            let members = if normalized == "zrange" {
                context
                    .sorted_set_store
                    .zrange(&key, start, stop, with_scores)
            } else {
                context
                    .sorted_set_store
                    .zrevrange(&key, start, stop, with_scores)
            };

            if with_scores {
                scored_members_to_lua(lua, members)
            } else {
                let only_members: Vec<Vec<u8>> = members.into_iter().map(|m| m.member).collect();
                vec_bytes_to_lua(lua, only_members)
            }
        }
        "zrangebyscore" => {
            ensure_min_args(&args, 3, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let min = lua_value_to_f64(&args[1], &command_name)?;
            let max = lua_value_to_f64(&args[2], &command_name)?;
            let with_scores = args
                .get(3)
                .map(|arg| lua_value_to_string(arg, &command_name))
                .transpose()? // Option<Result> -> Result<Option>
                .map(|val| val.eq_ignore_ascii_case("withscores"))
                .unwrap_or(false);

            let members = context
                .sorted_set_store
                .zrangebyscore(&key, min, max, with_scores);
            if with_scores {
                scored_members_to_lua(lua, members)
            } else {
                let only_members: Vec<Vec<u8>> = members.into_iter().map(|m| m.member).collect();
                vec_bytes_to_lua(lua, only_members)
            }
        }
        "zrank" | "zrevrank" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let member = lua_value_to_bytes(&args[1])?;
            let rank = if normalized == "zrank" {
                context.sorted_set_store.zrank(&key, &member)
            } else {
                context.sorted_set_store.zrevrank(&key, &member)
            };
            Ok(match rank {
                Some(value) => LuaValue::Integer(value as i64),
                None => LuaValue::Nil,
            })
        }
        "zpopmin" | "zpopmax" => {
            ensure_min_args(&args, 1, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let count = if args.len() > 1 {
                lua_value_to_usize(&args[1], &command_name)?
            } else {
                1
            };
            let members = if normalized == "zpopmin" {
                context.sorted_set_store.zpopmin(&key, count)
            } else {
                context.sorted_set_store.zpopmax(&key, count)
            };
            scored_members_to_lua(lua, members)
        }
        "zremrangebyrank" => {
            ensure_min_args(&args, 3, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let start = lua_value_to_i64(&args[1], &command_name)?;
            let stop = lua_value_to_i64(&args[2], &command_name)?;
            let removed = context.sorted_set_store.zremrangebyrank(&key, start, stop);
            Ok(LuaValue::Integer(removed as i64))
        }
        "zremrangebyscore" => {
            ensure_min_args(&args, 3, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let min = lua_value_to_f64(&args[1], &command_name)?;
            let max = lua_value_to_f64(&args[2], &command_name)?;
            let removed = context.sorted_set_store.zremrangebyscore(&key, min, max);
            Ok(LuaValue::Integer(removed as i64))
        }
        "zmscore" => {
            ensure_min_args(&args, 2, &command_name)?;
            let key = lua_value_to_string(&args[0], &command_name)?;
            let members = collect_bytes_args(&args, 1, &command_name)?;
            let scores = context.sorted_set_store.zmscore(&key, &members);
            optional_scores_to_lua(lua, scores)
        }
        _ => Err(mlua::Error::RuntimeError(format!(
            "redis.call does not support command {}",
            command_name
        ))),
    }
}

fn ensure_min_args(
    args: &Variadic<LuaValue>,
    required: usize,
    command: &str,
) -> Result<(), mlua::Error> {
    if args.len() < required {
        Err(mlua::Error::RuntimeError(format!(
            "{} requires at least {} argument(s)",
            command, required
        )))
    } else {
        Ok(())
    }
}

fn synap_err_to_lua(err: SynapError) -> mlua::Error {
    mlua::Error::RuntimeError(err.to_string())
}

fn lua_value_to_string(value: &LuaValue, command: &str) -> Result<String, mlua::Error> {
    match value {
        LuaValue::String(s) => {
            let bytes = s.as_bytes();
            Ok(String::from_utf8_lossy(bytes.as_ref()).into_owned())
        }
        LuaValue::Number(n) => Ok(n.to_string()),
        LuaValue::Integer(i) => Ok(i.to_string()),
        LuaValue::Boolean(b) => Ok(if *b { "1" } else { "0" }.to_string()),
        LuaValue::Nil => Ok(String::new()),
        other => Err(mlua::Error::RuntimeError(format!(
            "{} expects string-like arguments (got {:?})",
            command, other
        ))),
    }
}

fn lua_value_to_i64(value: &LuaValue, command: &str) -> Result<i64, mlua::Error> {
    let s = lua_value_to_string(value, command)?;
    s.parse::<i64>().map_err(|_| {
        mlua::Error::RuntimeError(format!("{} received non-integer argument: {}", command, s))
    })
}

fn lua_value_to_u64(value: &LuaValue, command: &str) -> Result<u64, mlua::Error> {
    let s = lua_value_to_string(value, command)?;
    s.parse::<u64>().map_err(|_| {
        mlua::Error::RuntimeError(format!(
            "{} received non-unsigned-integer argument: {}",
            command, s
        ))
    })
}

fn lua_value_to_usize(value: &LuaValue, command: &str) -> Result<usize, mlua::Error> {
    let amount = lua_value_to_i64(value, command)?;
    if amount < 0 {
        return Err(mlua::Error::RuntimeError(format!(
            "{} expects non-negative integer, got {}",
            command, amount
        )));
    }
    Ok(amount as usize)
}

fn collect_string_args(
    args: &Variadic<LuaValue>,
    start: usize,
    command: &str,
) -> Result<Vec<String>, mlua::Error> {
    if args.len() <= start {
        return Err(mlua::Error::RuntimeError(format!(
            "{} requires at least {} argument(s)",
            command,
            start + 1
        )));
    }

    args[start..]
        .iter()
        .map(|value| lua_value_to_string(value, command))
        .collect()
}

fn collect_bytes_args(
    args: &Variadic<LuaValue>,
    start: usize,
    command: &str,
) -> Result<Vec<Vec<u8>>, mlua::Error> {
    if args.len() <= start {
        return Err(mlua::Error::RuntimeError(format!(
            "{} requires at least {} argument(s)",
            command,
            start + 1
        )));
    }

    args[start..].iter().map(lua_value_to_bytes).collect()
}

fn vec_bytes_to_lua(lua: &Lua, values: Vec<Vec<u8>>) -> Result<LuaValue, mlua::Error> {
    match values.len() {
        0 => Ok(LuaValue::Nil),
        1 => {
            let value = values.into_iter().next().unwrap();
            Ok(LuaValue::String(lua.create_string(value)?))
        }
        _ => {
            let table = lua.create_table()?;
            for (idx, value) in values.into_iter().enumerate() {
                table.set((idx + 1) as i64, lua.create_string(value)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

fn lua_value_to_f64(value: &LuaValue, command: &str) -> Result<f64, mlua::Error> {
    let content = lua_value_to_string(value, command)?;
    content.parse::<f64>().map_err(|_| {
        mlua::Error::RuntimeError(format!(
            "{} expects floating point arguments (got {})",
            command, content
        ))
    })
}

fn scored_members_to_lua(lua: &Lua, members: Vec<ScoredMember>) -> Result<LuaValue, mlua::Error> {
    let table = lua.create_table()?;
    let mut index: i64 = 1;
    for member in members {
        table.set(index, lua.create_string(member.member)?)?;
        index += 1;
        table.set(index, lua.create_string(member.score.to_string())?)?;
        index += 1;
    }
    Ok(LuaValue::Table(table))
}

fn optional_scores_to_lua(lua: &Lua, scores: Vec<Option<f64>>) -> Result<LuaValue, mlua::Error> {
    let table = lua.create_table()?;
    for (idx, score) in scores.into_iter().enumerate() {
        let key = (idx + 1) as i64;
        match score {
            Some(value) => table.set(key, lua.create_string(value.to_string())?)?,
            None => table.set(key, LuaValue::Nil)?,
        }
    }
    Ok(LuaValue::Table(table))
}

fn lua_value_to_bytes(value: &LuaValue) -> Result<Vec<u8>, mlua::Error> {
    match value {
        LuaValue::String(s) => Ok(s.as_bytes().to_vec()),
        LuaValue::Number(n) => Ok(n.to_string().into_bytes()),
        LuaValue::Integer(i) => Ok(i.to_string().into_bytes()),
        LuaValue::Boolean(b) => Ok(if *b { b"1".to_vec() } else { b"0".to_vec() }),
        LuaValue::Nil => Ok(Vec::new()),
        other => Err(mlua::Error::RuntimeError(format!(
            "Unsupported argument type: {:?}",
            other
        ))),
    }
}

fn lua_value_to_json(lua: &Lua, value: LuaValue) -> Result<serde_json::Value, SynapError> {
    match value {
        LuaValue::Nil => Ok(serde_json::Value::Null),
        LuaValue::Boolean(b) => Ok(serde_json::Value::Bool(b)),
        LuaValue::Integer(i) => Ok(serde_json::Value::Number(i.into())),
        LuaValue::Number(n) => serde_json::Number::from_f64(n)
            .map(serde_json::Value::Number)
            .ok_or_else(|| SynapError::InvalidValue("Invalid floating point result".into())),
        LuaValue::String(s) => {
            let bytes = s.as_bytes();
            Ok(serde_json::Value::String(
                String::from_utf8_lossy(bytes.as_ref()).into_owned(),
            ))
        }
        LuaValue::Table(t) => table_to_json(lua, t),
        LuaValue::LightUserData(_) | LuaValue::Thread(_) | LuaValue::Function(_) => Err(
            SynapError::InvalidValue("Unsupported Lua value returned from script".into()),
        ),
        LuaValue::UserData(_) => Err(SynapError::InvalidValue(
            "User data not supported in script results".into(),
        )),
        LuaValue::Error(err) => Err(SynapError::InvalidRequest(err.to_string())),
        LuaValue::Other(_) => Err(SynapError::InvalidValue(
            "Unsupported Lua value returned from script".into(),
        )),
    }
}

fn table_to_json(lua: &Lua, table: mlua::Table) -> Result<serde_json::Value, SynapError> {
    let mut is_array = true;
    let mut max_index = 0usize;
    let mut array_entries: HashMap<usize, serde_json::Value> = HashMap::new();
    let mut map_entries: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

    for pair in table.clone().pairs::<LuaValue, LuaValue>() {
        let (key, value) = pair.map_err(map_lua_error)?;
        let json_value = lua_value_to_json(lua, value)?;
        match key {
            LuaValue::Integer(i) if i >= 1 => {
                let index = (i - 1) as usize;
                max_index = max_index.max(index);
                array_entries.insert(index, json_value);
            }
            LuaValue::String(s) => {
                is_array = false;
                map_entries.insert(s.to_str().map_err(map_lua_error)?.to_string(), json_value);
            }
            other => {
                is_array = false;
                map_entries.insert(format!("{:?}", other), json_value);
            }
        }
    }

    if is_array && map_entries.is_empty() {
        let mut array = vec![serde_json::Value::Null; max_index + 1];
        for (idx, value) in array_entries {
            array[idx] = value;
        }
        Ok(serde_json::Value::Array(array))
    } else {
        if map_entries.is_empty() {
            for (idx, value) in array_entries {
                map_entries.insert(idx.to_string(), value);
            }
        }
        Ok(serde_json::Value::Object(map_entries))
    }
}

fn apply_sandbox(lua: &Lua) -> Result<(), SynapError> {
    let globals = lua.globals();

    for symbol in DISABLED_GLOBALS {
        globals.set(*symbol, LuaValue::Nil).map_err(map_lua_error)?;
    }

    if let Ok(string_table) = globals.get::<mlua::Table>("string") {
        string_table
            .set("dump", LuaValue::Nil)
            .map_err(map_lua_error)?;
    }

    Ok(())
}
