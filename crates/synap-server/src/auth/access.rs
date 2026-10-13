//! One authorization contract for every transport (review findings S-02 and
//! S-05).
//!
//! The REST handlers, the `/api/v1/command` envelope, RESP3 and SynapRPC all
//! answer the same question — may this caller run this command? — through
//! [`OperationContext::authorize`] over the `(resource, action)` pairs the
//! command touches, as listed by [`classify_native`] (RESP3, SynapRPC, script
//! `redis.call`) and [`classify_envelope`] (`/api/v1/command`). Resource names
//! follow the REST handlers — `kv:<key>`, `hash:<key>`, `list:<key>`,
//! `set:<key>`, `sortedset:<key>` (also geospatial keys, which live in sorted
//! sets), `hyperloglog:<key>`, `bitmap:<key>`, `queue:<name>`,
//! `stream:<room>`, `pubsub:<topic>`, `script:*` and `admin:*` — so a role
//! grants the same access on every transport.
//!
//! A command the classifiers do not know is denied while authentication is
//! enforced, and a test walks every dispatcher's command table to make sure
//! none is missing.

use std::borrow::Cow;

use super::{Action, AuthContext, Permission, User, UserManager, command_requires_admin};

/// One resource a command touches and what it does to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Access {
    pub resource: String,
    pub action: Action,
}

impl Access {
    pub fn new(resource: impl Into<String>, action: Action) -> Self {
        Self {
            resource: resource.into(),
            action,
        }
    }
}

/// Why a command was refused; the message names the missing grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denied(String);

impl Denied {
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Denied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Who is asking, and what they may do, for one request or connection.
///
/// Built once per REST request, envelope call, RESP3 connection or SynapRPC
/// session, and passed explicitly to everything that authorizes — never read
/// from ambient storage.
#[derive(Debug, Clone)]
pub struct OperationContext {
    principal: Option<String>,
    is_admin: bool,
    permissions: Vec<Permission>,
    /// False only when authentication is disabled by configuration.
    enforced: bool,
}

impl OperationContext {
    /// Authentication is disabled: every command is allowed.
    pub fn unrestricted() -> Self {
        Self {
            principal: None,
            is_admin: false,
            permissions: Vec::new(),
            enforced: false,
        }
    }

    /// From the context the REST middleware attached to a request. The router
    /// decides the mode by configuration: with authentication disabled it
    /// attaches an admin context, so the result is unrestricted.
    pub fn from_auth_context(ctx: &AuthContext) -> Self {
        Self {
            principal: ctx.user_id.clone(),
            is_admin: ctx.is_admin,
            permissions: ctx.permissions.clone(),
            enforced: true,
        }
    }

    /// For a native-protocol connection. `user_manager` is `None` exactly when
    /// authentication is disabled; `user` is the identity resolved at `AUTH`,
    /// or `None` for a connection that never authenticated (allowed to run
    /// commands only when `require_auth` is off, and then holding no
    /// permissions, like an anonymous REST request).
    pub fn for_user(user_manager: Option<&UserManager>, user: Option<&User>) -> Self {
        let Some(manager) = user_manager else {
            return Self::unrestricted();
        };
        match user {
            Some(user) => Self {
                principal: Some(user.username.clone()),
                is_admin: user.is_admin,
                permissions: manager.get_user_permissions(&user.username),
                enforced: true,
            },
            None => Self {
                principal: None,
                is_admin: false,
                permissions: Vec::new(),
                enforced: true,
            },
        }
    }

    /// The authenticated principal, if any.
    pub fn principal(&self) -> Option<&str> {
        self.principal.as_deref()
    }

    /// True when no check can fail: authentication disabled, or an admin.
    pub fn is_unrestricted(&self) -> bool {
        !self.enforced || self.is_admin
    }

    /// Whether this caller may perform `access`.
    pub fn allows(&self, access: &Access) -> bool {
        self.is_unrestricted()
            || self
                .permissions
                .iter()
                .any(|p| p.matches(&access.resource, access.action))
    }

    /// Allow only when every access is permitted; the error names the first
    /// one that is not.
    pub fn authorize(&self, accesses: &[Access]) -> Result<(), Denied> {
        match accesses.iter().find(|access| !self.allows(access)) {
            None => Ok(()),
            Some(access) => Err(Denied(format!(
                "NOPERM no '{}' permission on '{}'",
                access.action.as_str(),
                access.resource
            ))),
        }
    }

    /// Authorize a native (RESP3, SynapRPC or script) command. `cmd` is the
    /// upper-cased command name and `args` excludes it.
    pub fn authorize_native(&self, cmd: &str, args: &dyn CommandArgs) -> Result<(), Denied> {
        if self.is_unrestricted() {
            return Ok(());
        }
        match classify_native(cmd, args) {
            Some(accesses) => self.authorize(&accesses),
            None => Err(unclassified(cmd)),
        }
    }

    /// Authorize an `/api/v1/command` envelope.
    pub fn authorize_envelope(
        &self,
        command: &str,
        payload: &serde_json::Value,
    ) -> Result<(), Denied> {
        if self.is_unrestricted() {
            return Ok(());
        }
        match classify_envelope(command, payload) {
            Some(accesses) => self.authorize(&accesses),
            None => Err(unclassified(command)),
        }
    }
}

fn unclassified(command: &str) -> Denied {
    Denied(format!(
        "NOPERM command '{command}' has no access policy and is denied"
    ))
}

/// Positional arguments of a native command, the command name excluded.
///
/// Implementations must render every scalar argument the dispatcher could read
/// as a string (lossily when needed) and return `None` only past the end or for
/// a value no dispatcher reads as text: a classifier that skips an argument the
/// handler uses would authorize less than the handler touches.
pub trait CommandArgs {
    fn arg_count(&self) -> usize;
    fn arg(&self, index: usize) -> Option<Cow<'_, str>>;
}

impl<S: AsRef<str>> CommandArgs for &[S] {
    fn arg_count(&self) -> usize {
        self.len()
    }
    fn arg(&self, index: usize) -> Option<Cow<'_, str>> {
        self.get(index).map(|s| Cow::Borrowed(s.as_ref()))
    }
}

/// Arguments from `start` on, for classifying a wrapped command (`TXQUEUE`).
struct Shifted<'a> {
    inner: &'a dyn CommandArgs,
    start: usize,
}

impl CommandArgs for Shifted<'_> {
    fn arg_count(&self) -> usize {
        self.inner.arg_count().saturating_sub(self.start)
    }
    fn arg(&self, index: usize) -> Option<Cow<'_, str>> {
        self.inner.arg(self.start + index)
    }
}

const KV: &str = "kv:";
const HASH: &str = "hash:";
const LIST: &str = "list:";
const SET: &str = "set:";
const ZSET: &str = "sortedset:";
const HLL: &str = "hyperloglog:";
const BITMAP: &str = "bitmap:";
const QUEUE: &str = "queue:";
const STREAM: &str = "stream:";

fn admin() -> Vec<Access> {
    vec![Access::new("admin:*", Action::Admin)]
}

fn global(resource: &str, action: Action) -> Vec<Access> {
    vec![Access::new(resource, action)]
}

/// The access for argument `index`, if present.
fn at(args: &dyn CommandArgs, prefix: &str, index: usize, action: Action) -> Vec<Access> {
    args.arg(index)
        .map(|name| vec![Access::new(format!("{prefix}{name}"), action)])
        .unwrap_or_default()
}

/// Accesses for every argument in `range`, stepping by `step`.
fn each(
    args: &dyn CommandArgs,
    prefix: &str,
    range: std::ops::Range<usize>,
    step: usize,
    action: Action,
) -> Vec<Access> {
    range
        .step_by(step)
        .filter_map(|i| args.arg(i))
        .map(|name| Access::new(format!("{prefix}{name}"), action))
        .collect()
}

fn all_from(args: &dyn CommandArgs, prefix: &str, start: usize, action: Action) -> Vec<Access> {
    each(args, prefix, start..args.arg_count(), 1, action)
}

/// `CMD key [key ...] timeout`: every argument but the last is a key.
fn keys_then_timeout(args: &dyn CommandArgs, prefix: &str, action: Action) -> Vec<Access> {
    each(
        args,
        prefix,
        0..args.arg_count().saturating_sub(1),
        1,
        action,
    )
}

fn upper(args: &dyn CommandArgs, index: usize) -> Option<String> {
    args.arg(index).map(|s| s.to_ascii_uppercase())
}

/// The room an `XREAD`/`XREADGROUP` reads, parsed exactly as the RESP3
/// handlers do (`COUNT`/`BLOCK` take a value, `STREAMS` ends the options), so
/// a crafted option list cannot make the classifier and the handler disagree.
fn stream_after_options(
    args: &dyn CommandArgs,
    mut i: usize,
    block_takes_value: bool,
) -> Vec<Access> {
    let n = args.arg_count();
    while i < n {
        match upper(args, i).as_deref() {
            Some("COUNT") => i += 2,
            Some("BLOCK") if block_takes_value => i += 2,
            Some("STREAMS") => return at(args, STREAM, i + 1, Action::Read),
            _ => i += 1,
        }
    }
    Vec::new()
}

/// Access to a pub/sub topic. Internal channels carry key-value data
/// (`__watch@0__:<key>` delivers the value, `__keyspace@0__:<key>` key events,
/// `__keyevent@0__:<event>` key names), so they also need the matching `kv`
/// permission — reading to subscribe, writing to publish — and a wildcard
/// subscription that could match one needs `kv:*`.
pub fn pubsub_topic_access(topic: &str, action: Action) -> Vec<Access> {
    let mut out = vec![Access::new(format!("pubsub:{topic}"), action)];
    let is_subscribe = action == Action::Read;
    if let Some(key) = internal_key_channel(topic) {
        out.push(Access::new(format!("{KV}{key}"), action));
    } else if topic.starts_with("__keyevent@")
        || (is_subscribe && wildcard_may_match_internal(topic))
    {
        out.push(Access::new("kv:*", action));
    }
    out
}

/// The key (or key pattern) of a `__watch@<db>__:` / `__keyspace@<db>__:` topic.
fn internal_key_channel(topic: &str) -> Option<&str> {
    let rest = topic
        .strip_prefix("__watch@")
        .or_else(|| topic.strip_prefix("__keyspace@"))?;
    rest.split_once("__:").map(|(_, key)| key)
}

/// Whether a subscription pattern's first level could match a level starting
/// with `__`, where every internal channel lives.
fn wildcard_may_match_internal(topic: &str) -> bool {
    if !topic.contains('*') && !topic.contains('#') {
        return false;
    }
    let first = topic.split('.').next().unwrap_or("");
    match first {
        "*" | "#" => true,
        level if level.contains('*') => {
            let literal_prefix = level.split('*').next().unwrap_or("");
            "__".starts_with(literal_prefix) || literal_prefix.starts_with("__")
        }
        level => level.starts_with("__"),
    }
}

/// What a native command (RESP3, SynapRPC, or a script `redis.call`) touches.
/// `cmd` is upper-cased; `args` excludes the command name. `None` means the
/// command has no policy and must be denied while authentication is enforced.
pub fn classify_native(cmd: &str, args: &dyn CommandArgs) -> Option<Vec<Access>> {
    use Action::{Configure, Delete, Read, Write};

    if command_requires_admin(cmd) {
        return Some(admin());
    }

    let accesses = match cmd {
        // Connection and session commands touch no data.
        "PING" | "QUIT" | "SELECT" | "HELLO" | "AUTH" | "ECHO" => Vec::new(),

        // ── Strings ────────────────────────────────────────────────────────
        "SET" | "EXPIRE" | "PERSIST" | "INCR" | "INCRBY" | "DECR" | "DECRBY" | "APPEND"
        | "SETRANGE" => at(args, KV, 0, Write),
        "GET" | "TTL" | "STRLEN" | "GETRANGE" => at(args, KV, 0, Read),
        "GETSET" => [at(args, KV, 0, Read), at(args, KV, 0, Write)].concat(),
        "DEL" => all_from(args, KV, 0, Delete),
        "EXISTS" | "MGET" => all_from(args, KV, 0, Read),
        "MSET" | "MSETNX" => each(args, KV, 0..args.arg_count(), 2, Write),
        "KEYS" | "SCAN" | "DBSIZE" | "KVSTATS" | "SYNAP.KVSTATS" => global("kv:*", Read),

        // ── Bitmaps ────────────────────────────────────────────────────────
        "SETBIT" => at(args, BITMAP, 0, Write),
        "GETBIT" | "BITCOUNT" => at(args, BITMAP, 0, Read),

        // ── Hashes ─────────────────────────────────────────────────────────
        "HSET" | "HMSET" | "HSETNX" | "HINCRBY" | "HINCRBYFLOAT" => at(args, HASH, 0, Write),
        "HGET" | "HGETALL" | "HMGET" | "HLEN" | "HEXISTS" | "HKEYS" | "HVALS" | "HSCAN" => {
            at(args, HASH, 0, Read)
        }
        "HDEL" => at(args, HASH, 0, Delete),

        // ── Lists ──────────────────────────────────────────────────────────
        "LPUSH" | "RPUSH" | "LPUSHX" | "RPUSHX" | "LPOP" | "RPOP" | "LSET" | "LTRIM" | "LREM"
        | "LINSERT" => at(args, LIST, 0, Write),
        "LRANGE" | "LLEN" | "LINDEX" | "LPOS" => at(args, LIST, 0, Read),
        "BLPOP" | "BRPOP" => keys_then_timeout(args, LIST, Write),
        "BRPOPLPUSH" | "RPOPLPUSH" => [at(args, LIST, 0, Write), at(args, LIST, 1, Write)].concat(),

        // ── Sets ───────────────────────────────────────────────────────────
        "SADD" | "SPOP" => at(args, SET, 0, Write),
        "SREM" => at(args, SET, 0, Delete),
        "SMEMBERS" | "SISMEMBER" | "SCARD" | "SSCAN" | "SRANDMEMBER" => at(args, SET, 0, Read),

        // ── Sorted sets (and geospatial indexes, stored as sorted sets) ────
        "ZADD" | "ZINCRBY" | "ZPOPMIN" | "ZPOPMAX" | "GEOADD" => at(args, ZSET, 0, Write),
        "ZREM" | "ZREMRANGEBYRANK" | "ZREMRANGEBYSCORE" => at(args, ZSET, 0, Delete),
        "ZRANGE" | "ZREVRANGE" | "ZRANGEBYSCORE" | "ZSCORE" | "ZMSCORE" | "ZCARD" | "ZCOUNT"
        | "ZRANK" | "ZREVRANK" | "ZSCAN" | "GEOPOS" | "GEODIST" | "GEOHASH" | "GEORADIUS"
        | "GEORADIUSBYMEMBER" | "GEOSEARCH" => at(args, ZSET, 0, Read),
        "BZPOPMIN" | "BZPOPMAX" => keys_then_timeout(args, ZSET, Write),
        "GEOSTATS" => global("sortedset:*", Read),

        // ── HyperLogLog ────────────────────────────────────────────────────
        "PFADD" => at(args, HLL, 0, Write),
        "PFCOUNT" => all_from(args, HLL, 0, Read),
        "PFMERGE" => [at(args, HLL, 0, Write), all_from(args, HLL, 1, Read)].concat(),
        "HLLSTATS" => global("hyperloglog:*", Read),

        // ── Queues ─────────────────────────────────────────────────────────
        "QCREATE" => at(args, QUEUE, 0, Configure),
        "QDELETE" | "QPURGE" => at(args, QUEUE, 0, Delete),
        "QPUBLISH" | "QACK" | "QNACK" => at(args, QUEUE, 0, Write),
        "QCONSUME" | "QSTATS" => at(args, QUEUE, 0, Read),
        "QLIST" => global("queue:*", Read),

        // ── Streams ────────────────────────────────────────────────────────
        "SCREATE" | "SGETORCREATE" => at(args, STREAM, 0, Configure),
        "SPUBLISH" | "XADD" | "XACK" => at(args, STREAM, 0, Write),
        "SREAD" | "SSTATS" | "XRANGE" => at(args, STREAM, 0, Read),
        "SDELETE" | "XDEL" => at(args, STREAM, 0, Delete),
        "SLIST" => global("stream:*", Read),
        "XREAD" => stream_after_options(args, 0, true),
        // `XREADGROUP GROUP <group> <consumer> [COUNT n] STREAMS <room> <id>`
        "XREADGROUP" => stream_after_options(args, 3, false),
        "XINFO" => match upper(args, 0).as_deref() {
            Some("STREAM") => at(args, STREAM, 1, Read),
            _ => global("stream:*", Read),
        },

        // ── Pub/Sub ────────────────────────────────────────────────────────
        "PUBLISH" => args
            .arg(0)
            .map(|topic| pubsub_topic_access(&topic, Write))
            .unwrap_or_default(),
        "SUBSCRIBE" | "PSUBSCRIBE" => (0..args.arg_count())
            .filter_map(|i| args.arg(i))
            .flat_map(|topic| pubsub_topic_access(&topic, Read))
            .collect(),
        "UNSUBSCRIBE" | "PUNSUBSCRIBE" | "KV.UNWATCH" => Vec::new(),
        "PUBSUB" | "TOPICS" | "PSSTATS" => global("pubsub:*", Read),
        "KV.WATCH" => at(args, KV, 0, Read),

        // ── Transactions ───────────────────────────────────────────────────
        // Control commands touch no data; every queued command is authorized
        // on its own when it is queued.
        "MULTI" | "EXEC" | "DISCARD" | "WATCH" | "UNWATCH" => Vec::new(),
        "TXQUEUE" => match upper(args, 1) {
            None => Vec::new(),
            Some(inner) => classify_native(
                &inner,
                &Shifted {
                    inner: args,
                    start: 2,
                },
            )?,
        },

        // ── Scripting ──────────────────────────────────────────────────────
        // Running a script needs nothing by itself: every `redis.call` it
        // makes is authorized against the caller.
        "EVAL" | "EVALSHA" => Vec::new(),
        "SCRIPT.LOAD" => global("script:*", Write),
        "SCRIPT.EXISTS" => global("script:*", Read),
        "SCRIPT" => match upper(args, 0).as_deref() {
            None => Vec::new(),
            Some("LOAD") => global("script:*", Write),
            Some("EXISTS") => global("script:*", Read),
            Some("FLUSH" | "KILL") => admin(),
            Some(_) => return None,
        },

        _ => return None,
    };
    Some(accesses)
}

/// String form of a payload field — any scalar, so a classifier never skips a
/// field a handler would read through a lenient conversion.
fn field<'a>(payload: &'a serde_json::Value, name: &str) -> Option<Cow<'a, str>> {
    match payload.get(name)? {
        serde_json::Value::String(s) => Some(Cow::Borrowed(s.as_str())),
        serde_json::Value::Number(n) => Some(Cow::Owned(n.to_string())),
        serde_json::Value::Bool(b) => Some(Cow::Owned(b.to_string())),
        _ => None,
    }
}

fn field_access(
    payload: &serde_json::Value,
    name: &str,
    prefix: &str,
    action: Action,
) -> Vec<Access> {
    field(payload, name)
        .map(|value| vec![Access::new(format!("{prefix}{value}"), action)])
        .unwrap_or_default()
}

/// Accesses for every scalar in the array field `name`.
fn list_access(
    payload: &serde_json::Value,
    name: &str,
    prefix: &str,
    action: Action,
) -> Vec<Access> {
    payload
        .get(name)
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| match item {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .map(|name| Access::new(format!("{prefix}{name}"), action))
                .collect()
        })
        .unwrap_or_default()
}

/// What an `/api/v1/command` envelope touches, from its payload fields. `None`
/// means the command has no policy and must be denied while authentication is
/// enforced.
pub fn classify_envelope(command: &str, payload: &serde_json::Value) -> Option<Vec<Access>> {
    use Action::{Configure, Delete, Read, Write};

    let key = |prefix: &str, action: Action| field_access(payload, "key", prefix, action);

    let accesses = match command {
        // ── Strings and keys ───────────────────────────────────────────────
        "kv.set" | "kv.incr" | "kv.decr" | "kv.append" | "kv.setrange" | "kv.expire"
        | "kv.persist" => key(KV, Write),
        "kv.get" | "kv.exists" | "kv.ttl" | "kv.strlen" | "kv.getrange" | "key.type"
        | "key.exists" | "memory.usage" => key(KV, Read),
        "kv.getset" => [key(KV, Read), key(KV, Write)].concat(),
        "kv.del" => key(KV, Delete),
        "kv.mdel" => list_access(payload, "keys", KV, Delete),
        "kv.mget" => list_access(payload, "keys", KV, Read),
        "kv.mset" | "kv.msetnx" => {
            let mut out = key(KV, Write);
            if let Some(pairs) = payload.get("pairs").and_then(serde_json::Value::as_array) {
                for pair in pairs {
                    out.extend(field_access(pair, "key", KV, Write));
                }
            }
            out
        }
        "kv.scan" | "kv.keys" | "kv.dbsize" | "kv.stats" | "key.randomkey" => global("kv:*", Read),
        "kv.flushdb" | "kv.flushall" | "slowlog.reset" => admin(),
        "key.rename" | "key.renamenx" => [
            field_access(payload, "source", KV, Delete),
            field_access(payload, "destination", KV, Write),
        ]
        .concat(),
        "key.copy" => [
            field_access(payload, "source", KV, Read),
            field_access(payload, "destination", KV, Write),
        ]
        .concat(),

        // ── Server introspection ───────────────────────────────────────────
        // Admin-only, as `GET /info` is: the slow log records command
        // arguments (values included) and the client list exposes addresses.
        "info" | "slowlog.get" | "client.list" => admin(),

        // ── Transactions ───────────────────────────────────────────────────
        "transaction.multi"
        | "transaction.exec"
        | "transaction.discard"
        | "transaction.watch"
        | "transaction.unwatch" => Vec::new(),

        // ── Hashes ─────────────────────────────────────────────────────────
        "hash.set" | "hash.mset" | "hash.incrby" | "hash.incrbyfloat" | "hash.setnx" => {
            key(HASH, Write)
        }
        "hash.get" | "hash.getall" | "hash.exists" | "hash.len" | "hash.keys" | "hash.vals"
        | "hash.mget" => key(HASH, Read),
        "hash.del" => key(HASH, Delete),
        "hash.stats" => global("hash:*", Read),

        // ── Lists ──────────────────────────────────────────────────────────
        "list.lpush" | "list.lpushx" | "list.rpush" | "list.rpushx" | "list.lpop" | "list.rpop"
        | "list.lset" | "list.set" | "list.ltrim" | "list.trim" | "list.lrem" | "list.linsert" => {
            key(LIST, Write)
        }
        "list.lrange" | "list.range" | "list.llen" | "list.len" | "list.lindex" | "list.index"
        | "list.lpos" => key(LIST, Read),
        "list.rpoplpush" => [
            field_access(payload, "source", LIST, Write),
            field_access(payload, "destination", LIST, Write),
        ]
        .concat(),
        "list.stats" => global("list:*", Read),

        // ── HyperLogLog ────────────────────────────────────────────────────
        "hyperloglog.pfadd" => key(HLL, Write),
        "hyperloglog.pfcount" => [key(HLL, Read), list_access(payload, "keys", HLL, Read)].concat(),
        "hyperloglog.pfmerge" => [
            field_access(payload, "destination", HLL, Write),
            list_access(payload, "sources", HLL, Read),
        ]
        .concat(),
        "hyperloglog.stats" => global("hyperloglog:*", Read),

        // ── Bitmaps ────────────────────────────────────────────────────────
        "bitmap.setbit" | "bitmap.bitfield" => key(BITMAP, Write),
        "bitmap.getbit" | "bitmap.bitcount" | "bitmap.bitpos" => key(BITMAP, Read),
        "bitmap.bitop" => [
            field_access(payload, "destination", BITMAP, Write),
            list_access(payload, "source_keys", BITMAP, Read),
        ]
        .concat(),
        "bitmap.stats" => global("bitmap:*", Read),

        // ── Geospatial (sorted sets) ───────────────────────────────────────
        "geospatial.geoadd" => key(ZSET, Write),
        "geospatial.geodist"
        | "geospatial.geopos"
        | "geospatial.geohash"
        | "geospatial.georadius"
        | "geospatial.georadiusbymember"
        | "geospatial.geosearch" => key(ZSET, Read),
        "geospatial.stats" => global("sortedset:*", Read),

        // ── Queues ─────────────────────────────────────────────────────────
        "queue.create" => [
            field_access(payload, "name", QUEUE, Configure),
            field_access(payload, "queue", QUEUE, Configure),
        ]
        .concat(),
        "queue.delete" => [
            field_access(payload, "queue", QUEUE, Delete),
            field_access(payload, "name", QUEUE, Delete),
        ]
        .concat(),
        "queue.purge" => field_access(payload, "queue", QUEUE, Delete),
        "queue.publish" | "queue.ack" | "queue.nack" => {
            field_access(payload, "queue", QUEUE, Write)
        }
        "queue.consume" | "queue.stats" => field_access(payload, "queue", QUEUE, Read),
        "queue.list" => global("queue:*", Read),

        // ── Sets ───────────────────────────────────────────────────────────
        "set.add" | "set.pop" => key(SET, Write),
        "set.rem" => key(SET, Delete),
        "set.ismember" | "set.members" | "set.size" | "set.card" | "set.randmember" => {
            key(SET, Read)
        }
        "set.move" => [
            field_access(payload, "source", SET, Write),
            field_access(payload, "destination", SET, Write),
        ]
        .concat(),
        "set.inter" | "set.interstore" | "set.union" | "set.diff" => [
            list_access(payload, "keys", SET, Read),
            field_access(payload, "destination", SET, Write),
        ]
        .concat(),
        "set.stats" => global("set:*", Read),

        // ── Sorted sets ────────────────────────────────────────────────────
        "sortedset.zadd" | "sortedset.zincrby" | "sortedset.zpopmin" | "sortedset.zpopmax" => {
            key(ZSET, Write)
        }
        "sortedset.zrem" | "sortedset.zremrangebyrank" | "sortedset.zremrangebyscore" => {
            key(ZSET, Delete)
        }
        "sortedset.zscore"
        | "sortedset.zcard"
        | "sortedset.zrange"
        | "sortedset.zrevrange"
        | "sortedset.zrank"
        | "sortedset.zrevrank"
        | "sortedset.zcount"
        | "sortedset.zrangebyscore"
        | "sortedset.zmscore" => key(ZSET, Read),
        "sortedset.zinterstore" | "sortedset.zunionstore" | "sortedset.zdiffstore" => [
            field_access(payload, "destination", ZSET, Write),
            list_access(payload, "keys", ZSET, Read),
        ]
        .concat(),
        "sortedset.stats" => global("sortedset:*", Read),

        // ── Scripting ──────────────────────────────────────────────────────
        "script.eval" | "script.evalsha" => Vec::new(),
        "script.load" => global("script:*", Write),
        "script.exists" => global("script:*", Read),
        "script.flush" | "script.kill" => admin(),

        // ── Pub/Sub ────────────────────────────────────────────────────────
        "pubsub.subscribe" => payload
            .get("topics")
            .and_then(serde_json::Value::as_array)
            .map(|topics| {
                topics
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .flat_map(|topic| pubsub_topic_access(topic, Read))
                    .collect()
            })
            .unwrap_or_default(),
        "pubsub.publish" => field(payload, "topic")
            .map(|topic| pubsub_topic_access(&topic, Write))
            .unwrap_or_default(),
        "pubsub.unsubscribe" => Vec::new(),
        "pubsub.stats" | "pubsub.topics" => global("pubsub:*", Read),
        "pubsub.info" => field(payload, "topic")
            .map(|topic| pubsub_topic_access(&topic, Read))
            .unwrap_or_default(),

        // ── Streams ────────────────────────────────────────────────────────
        "stream.create" | "stream.get_or_create" => {
            field_access(payload, "room", STREAM, Configure)
        }
        "stream.publish" => field_access(payload, "room", STREAM, Write),
        "stream.consume" | "stream.stats" => field_access(payload, "room", STREAM, Read),
        "stream.delete" => field_access(payload, "room", STREAM, Delete),
        "stream.list" => global("stream:*", Read),

        _ => return None,
    };
    Some(accesses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn native(cmd: &str, args: &[&str]) -> Vec<Access> {
        classify_native(cmd, &args).unwrap_or_else(|| panic!("{cmd} has no policy"))
    }

    fn access(resource: &str, action: Action) -> Access {
        Access::new(resource, action)
    }

    fn user_with(permissions: Vec<Permission>) -> OperationContext {
        OperationContext {
            principal: Some("u".into()),
            is_admin: false,
            permissions,
            enforced: true,
        }
    }

    /// Names of the match arms of the command table that starts at `marker`:
    /// string literals on lines indented exactly eight spaces, up to the
    /// table's `_ =>` catch-all. Reading the dispatcher sources keeps this
    /// test from drifting when a command is added.
    fn command_table(source: &str, marker: &str) -> Vec<String> {
        let start = source
            .find(marker)
            .unwrap_or_else(|| panic!("marker `{marker}` not found"));
        let mut names = Vec::new();
        for line in source[start..].lines().skip(1) {
            let Some(body) = line.strip_prefix("        ") else {
                continue;
            };
            if body.starts_with(' ') {
                continue;
            }
            if body.starts_with("_ =>") {
                break;
            }
            if !(body.starts_with('"') || body.starts_with("| \"")) {
                continue;
            }
            let patterns = body.split("=>").next().unwrap_or("");
            for piece in patterns.split('"').skip(1).step_by(2) {
                names.push(piece.to_string());
            }
        }
        names
    }

    #[test]
    fn every_dispatcher_command_has_a_policy() {
        let empty: &[&str] = &[];
        let native_tables = [
            (
                "RESP3",
                command_table(
                    include_str!("../protocol/resp3/command/mod.rs"),
                    "match cmd {",
                ),
                100,
            ),
            (
                "SynapRPC",
                command_table(
                    include_str!("../protocol/synap_rpc/dispatch/mod.rs"),
                    "match cmd {",
                ),
                60,
            ),
            (
                "SynapRPC advanced",
                command_table(
                    include_str!("../protocol/synap_rpc/dispatch/advanced.rs"),
                    "match command {",
                ),
                40,
            ),
            (
                "redis.call",
                command_table(
                    include_str!("../scripting/mod.rs"),
                    "match normalized.as_str() {",
                )
                .into_iter()
                .map(|name| name.to_ascii_uppercase())
                .collect(),
                35,
            ),
        ];
        for (table, names, minimum) in native_tables {
            assert!(
                names.len() >= minimum,
                "{table}: read only {} commands; has the table moved?",
                names.len()
            );
            for name in names {
                assert!(
                    classify_native(&name, &empty).is_some(),
                    "{table} command `{name}` has no access policy"
                );
            }
        }

        let envelope = command_table(
            include_str!("../server/handlers/mod.rs"),
            "async fn handle_command",
        );
        assert!(
            envelope.len() >= 150,
            "read only {} envelope commands",
            envelope.len()
        );
        for name in envelope {
            assert!(
                classify_envelope(&name, &json!({})).is_some(),
                "envelope command `{name}` has no access policy"
            );
        }
    }

    #[test]
    fn unknown_commands_have_no_policy() {
        assert!(classify_native("NOPE", &(&["k"] as &[&str])).is_none());
        assert!(classify_native("TXQUEUE", &(&["c1", "NOPE", "k"] as &[&str])).is_none());
        assert!(classify_native("SCRIPT", &(&["DEBUG"] as &[&str])).is_none());
        assert!(classify_envelope("kv.nope", &json!({"key": "a"})).is_none());

        let denied = user_with(vec![Permission::new("*", Action::All)])
            .authorize_native("NOPE", &(&[] as &[&str]))
            .unwrap_err();
        assert!(denied.message().contains("NOPE"), "{denied}");
    }

    #[test]
    fn native_commands_touch_every_key_they_use() {
        assert_eq!(
            native("SET", &["a", "1"]),
            vec![access("kv:a", Action::Write)]
        );
        assert_eq!(
            native("MSET", &["a", "1", "b", "2"]),
            vec![access("kv:a", Action::Write), access("kv:b", Action::Write)]
        );
        assert_eq!(
            native("DEL", &["a", "b"]),
            vec![
                access("kv:a", Action::Delete),
                access("kv:b", Action::Delete)
            ]
        );
        assert_eq!(
            native("BLPOP", &["l1", "l2", "5"]),
            vec![
                access("list:l1", Action::Write),
                access("list:l2", Action::Write)
            ]
        );
        assert_eq!(
            native("PFMERGE", &["dst", "s1", "s2"]),
            vec![
                access("hyperloglog:dst", Action::Write),
                access("hyperloglog:s1", Action::Read),
                access("hyperloglog:s2", Action::Read),
            ]
        );
        assert_eq!(
            native("GEOADD", &["g", "1", "2", "m"]),
            vec![access("sortedset:g", Action::Write)]
        );
        assert_eq!(
            native("FLUSHALL", &[]),
            vec![access("admin:*", Action::Admin)]
        );
        assert_eq!(
            native("SCRIPT", &["flush"]),
            vec![access("admin:*", Action::Admin)]
        );
        assert_eq!(native("KEYS", &["*"]), vec![access("kv:*", Action::Read)]);
        assert_eq!(
            native("TXQUEUE", &["c1", "set", "a", "1"]),
            vec![access("kv:a", Action::Write)]
        );
    }

    /// `XREAD COUNT STREAMS STREAMS room 0`: the handler takes the first
    /// STREAMS as the value of COUNT and reads `room`; the classifier agrees.
    #[test]
    fn xread_room_is_parsed_like_the_handler() {
        assert_eq!(
            native("XREAD", &["COUNT", "STREAMS", "STREAMS", "room", "0"]),
            vec![access("stream:room", Action::Read)]
        );
        assert_eq!(
            native("XREAD", &["BLOCK", "0", "STREAMS", "orders", "0"]),
            vec![access("stream:orders", Action::Read)]
        );
        assert_eq!(
            native(
                "XREADGROUP",
                &["GROUP", "STREAMS", "c", "STREAMS", "r", "0"]
            ),
            vec![access("stream:r", Action::Read)]
        );
        assert!(native("XREAD", &["COUNT", "1"]).is_empty());
    }

    #[test]
    fn internal_channels_need_key_access() {
        assert_eq!(
            native("SUBSCRIBE", &["__watch@0__:secret"]),
            vec![
                access("pubsub:__watch@0__:secret", Action::Read),
                access("kv:secret", Action::Read),
            ]
        );
        assert_eq!(
            native("PUBLISH", &["__keyspace@0__:a", "set"]),
            vec![
                access("pubsub:__keyspace@0__:a", Action::Write),
                access("kv:a", Action::Write),
            ]
        );
        assert!(
            native("SUBSCRIBE", &["__keyevent@0__:set"]).contains(&access("kv:*", Action::Read))
        );
        for pattern in ["*", "#", "_*", "__w*.x", "*.orders"] {
            assert!(
                native("SUBSCRIBE", &[pattern]).contains(&access("kv:*", Action::Read)),
                "`{pattern}` can match an internal channel"
            );
        }
        for topic in ["orders", "orders.*", "user*", "a.#"] {
            assert_eq!(
                native("SUBSCRIBE", &[topic]).len(),
                1,
                "`{topic}` is a plain topic"
            );
        }

        let pubsub_only = user_with(vec![Permission::new("pubsub:*", Action::All)]);
        assert!(
            pubsub_only
                .authorize_native("SUBSCRIBE", &(&["orders"] as &[&str]))
                .is_ok()
        );
        assert!(
            pubsub_only
                .authorize_native("SUBSCRIBE", &(&["#"] as &[&str]))
                .is_err()
        );
        assert!(
            pubsub_only
                .authorize_native("SUBSCRIBE", &(&["__watch@0__:secret"] as &[&str]))
                .is_err()
        );
    }

    #[test]
    fn envelope_commands_touch_their_payload_resources() {
        assert_eq!(
            classify_envelope("kv.flushall", &json!({})).unwrap(),
            vec![access("admin:*", Action::Admin)]
        );
        assert_eq!(
            classify_envelope(
                "kv.mset",
                &json!({"pairs": [{"key": "a", "value": 1}, {"key": "b", "value": 2}]})
            )
            .unwrap(),
            vec![access("kv:a", Action::Write), access("kv:b", Action::Write)]
        );
        assert_eq!(
            classify_envelope("key.rename", &json!({"source": "a", "destination": "b"})).unwrap(),
            vec![
                access("kv:a", Action::Delete),
                access("kv:b", Action::Write)
            ]
        );
        assert_eq!(
            classify_envelope(
                "sortedset.zunionstore",
                &json!({"destination": "d", "keys": ["x"]})
            )
            .unwrap(),
            vec![
                access("sortedset:d", Action::Write),
                access("sortedset:x", Action::Read)
            ]
        );
        // A numeric key is still a key.
        assert_eq!(
            classify_envelope("kv.get", &json!({"key": 42})).unwrap(),
            vec![access("kv:42", Action::Read)]
        );
    }

    #[test]
    fn context_modes() {
        let unrestricted = OperationContext::unrestricted();
        assert!(
            unrestricted
                .authorize_native("FLUSHALL", &(&[] as &[&str]))
                .is_ok()
        );

        let admin = OperationContext {
            principal: Some("root".into()),
            is_admin: true,
            permissions: Vec::new(),
            enforced: true,
        };
        assert!(
            admin
                .authorize_native("FLUSHALL", &(&[] as &[&str]))
                .is_ok()
        );

        let reader = user_with(vec![Permission::new("kv:*", Action::Read)]);
        assert!(reader.authorize_native("GET", &(&["a"] as &[&str])).is_ok());
        let denied = reader
            .authorize_native("SET", &(&["a", "1"] as &[&str]))
            .unwrap_err();
        assert_eq!(denied.message(), "NOPERM no 'write' permission on 'kv:a'");
        assert!(
            reader
                .authorize_native("FLUSHALL", &(&[] as &[&str]))
                .is_err()
        );

        let anonymous = OperationContext::for_user(Some(&UserManager::new()), None);
        assert!(
            anonymous
                .authorize_native("PING", &(&[] as &[&str]))
                .is_ok()
        );
        assert!(
            anonymous
                .authorize_native("GET", &(&["a"] as &[&str]))
                .is_err()
        );
        assert!(OperationContext::for_user(None, None).is_unrestricted());
    }
}
