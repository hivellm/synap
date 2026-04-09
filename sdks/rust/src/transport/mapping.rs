use super::WireValue;
use serde_json::{Value, json};

// ── Command mapper ────────────────────────────────────────────────────────────

/// Translate a dotted SDK command + JSON payload into a raw Redis-style command
/// plus an ordered arg list for the native protocols.
///
/// Returns `None` for commands that have no native mapping (e.g. pub/sub,
/// queues, streams); the caller should fall back to HTTP in that case.
pub(crate) fn map_command(cmd: &str, payload: &Value) -> Option<(&'static str, Vec<WireValue>)> {
    // Helper: extract a string field → WireValue::Str
    let field_str = |key: &str| -> WireValue {
        match &payload[key] {
            Value::String(s) => WireValue::Str(s.clone()),
            Value::Number(n) => WireValue::Str(n.to_string()),
            Value::Bool(b) => WireValue::Str(b.to_string()),
            _ => WireValue::Str(String::new()),
        }
    };

    // Helper: convert any JSON value → WireValue
    let to_wire = |v: &Value| -> WireValue {
        match v {
            Value::Null => WireValue::Null,
            Value::Bool(b) => WireValue::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    WireValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    WireValue::Float(f)
                } else {
                    WireValue::Str(n.to_string())
                }
            }
            Value::String(s) => WireValue::Str(s.clone()),
            _ => WireValue::Str(v.to_string()),
        }
    };

    Some(match cmd {
        // ── KV ────────────────────────────────────────────────────────────────
        "kv.get" => ("GET", vec![field_str("key")]),

        "kv.set" => {
            let mut args = vec![field_str("key"), to_wire(&payload["value"])];
            if let Some(ttl) = payload["ttl"].as_u64() {
                args.push(WireValue::Str("EX".into()));
                args.push(WireValue::Int(ttl as i64));
            }
            ("SET", args)
        }

        "kv.del" => ("DEL", vec![field_str("key")]),
        "kv.exists" => ("EXISTS", vec![field_str("key")]),
        "kv.incr" => ("INCR", vec![field_str("key")]),
        "kv.decr" => ("DECR", vec![field_str("key")]),

        "kv.keys" => {
            let prefix = payload["prefix"].as_str().unwrap_or("");
            let pattern = if prefix.is_empty() {
                "*".to_string()
            } else {
                format!("{}*", prefix)
            };
            ("KEYS", vec![WireValue::Str(pattern)])
        }

        "kv.expire" => (
            "EXPIRE",
            vec![
                field_str("key"),
                WireValue::Int(payload["ttl"].as_u64().unwrap_or(0) as i64),
            ],
        ),

        "kv.ttl" => ("TTL", vec![field_str("key")]),

        // ── Hash ──────────────────────────────────────────────────────────────
        "hash.set" => (
            "HSET",
            vec![field_str("key"), field_str("field"), field_str("value")],
        ),
        "hash.get" => ("HGET", vec![field_str("key"), field_str("field")]),
        "hash.getall" => ("HGETALL", vec![field_str("key")]),
        "hash.del" => ("HDEL", vec![field_str("key"), field_str("field")]),
        "hash.exists" => ("HEXISTS", vec![field_str("key"), field_str("field")]),
        "hash.keys" => ("HKEYS", vec![field_str("key")]),
        "hash.values" => ("HVALS", vec![field_str("key")]),
        "hash.len" => ("HLEN", vec![field_str("key")]),

        "hash.mset" => {
            let mut args = vec![field_str("key")];
            if let Some(obj) = payload["fields"].as_object() {
                // HashMap format: {"field": "value", ...}
                for (k, v) in obj {
                    args.push(WireValue::Str(k.clone()));
                    args.push(to_wire(v));
                }
            } else if let Some(arr) = payload["fields"].as_array() {
                // Array format: [{"field": "...", "value": "..."}, ...]
                for item in arr {
                    if let Some(f) = item["field"].as_str() {
                        args.push(WireValue::Str(f.to_string()));
                        args.push(to_wire(&item["value"]));
                    }
                }
            }
            // Redis 4+ HSET handles multi-field: HSET key f1 v1 f2 v2 ...
            ("HSET", args)
        }

        "hash.mget" => {
            let mut args = vec![field_str("key")];
            if let Some(fields) = payload["fields"].as_array() {
                for f in fields {
                    if let Some(s) = f.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            ("HMGET", args)
        }

        "hash.incrby" => (
            "HINCRBY",
            vec![
                field_str("key"),
                field_str("field"),
                WireValue::Int(payload["increment"].as_i64().unwrap_or(0)),
            ],
        ),

        "hash.incrbyfloat" => (
            "HINCRBYFLOAT",
            vec![
                field_str("key"),
                field_str("field"),
                WireValue::Str(payload["increment"].as_f64().unwrap_or(0.0).to_string()),
            ],
        ),

        "hash.setnx" => (
            "HSETNX",
            vec![field_str("key"), field_str("field"), field_str("value")],
        ),

        // ── List ──────────────────────────────────────────────────────────────
        "list.lpush" | "list.lpushx" => {
            let cmd_name: &'static str = if cmd == "list.lpushx" {
                "LPUSHX"
            } else {
                "LPUSH"
            };
            let mut args = vec![field_str("key")];
            if let Some(vals) = payload["values"].as_array() {
                for v in vals {
                    args.push(WireValue::Str(v.as_str().unwrap_or("").to_string()));
                }
            }
            (cmd_name, args)
        }

        "list.rpush" | "list.rpushx" => {
            let cmd_name: &'static str = if cmd == "list.rpushx" {
                "RPUSHX"
            } else {
                "RPUSH"
            };
            let mut args = vec![field_str("key")];
            if let Some(vals) = payload["values"].as_array() {
                for v in vals {
                    args.push(WireValue::Str(v.as_str().unwrap_or("").to_string()));
                }
            }
            (cmd_name, args)
        }

        "list.lpop" => {
            let mut args = vec![field_str("key")];
            if let Some(c) = payload["count"].as_u64() {
                args.push(WireValue::Int(c as i64));
            }
            ("LPOP", args)
        }

        "list.rpop" => {
            let mut args = vec![field_str("key")];
            if let Some(c) = payload["count"].as_u64() {
                args.push(WireValue::Int(c as i64));
            }
            ("RPOP", args)
        }

        "list.range" => (
            "LRANGE",
            vec![
                field_str("key"),
                WireValue::Int(payload["start"].as_i64().unwrap_or(0)),
                WireValue::Int(payload["stop"].as_i64().unwrap_or(-1)),
            ],
        ),

        "list.len" => ("LLEN", vec![field_str("key")]),

        "list.index" => (
            "LINDEX",
            vec![
                field_str("key"),
                WireValue::Int(payload["index"].as_i64().unwrap_or(0)),
            ],
        ),

        "list.set" => (
            "LSET",
            vec![
                field_str("key"),
                WireValue::Int(payload["index"].as_i64().unwrap_or(0)),
                to_wire(&payload["value"]),
            ],
        ),

        "list.trim" => (
            "LTRIM",
            vec![
                field_str("key"),
                WireValue::Int(payload["start"].as_i64().unwrap_or(0)),
                WireValue::Int(payload["end"].as_i64().unwrap_or(-1)),
            ],
        ),

        "list.rem" => (
            "LREM",
            vec![
                field_str("key"),
                WireValue::Int(payload["count"].as_i64().unwrap_or(0)),
                to_wire(&payload["element"]),
            ],
        ),

        "list.insert" => {
            let before_after = if payload["before"].as_bool().unwrap_or(true) {
                "BEFORE"
            } else {
                "AFTER"
            };
            (
                "LINSERT",
                vec![
                    field_str("key"),
                    WireValue::Str(before_after.into()),
                    to_wire(&payload["pivot"]),
                    to_wire(&payload["value"]),
                ],
            )
        }

        "list.rpoplpush" => (
            "RPOPLPUSH",
            vec![field_str("key"), field_str("destination")],
        ),

        "list.pos" => ("LPOS", vec![field_str("key"), to_wire(&payload["element"])]),

        // ── Set ───────────────────────────────────────────────────────────────
        "set.add" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    args.push(WireValue::Str(m.as_str().unwrap_or("").to_string()));
                }
            }
            ("SADD", args)
        }

        "set.rem" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    args.push(WireValue::Str(m.as_str().unwrap_or("").to_string()));
                }
            }
            ("SREM", args)
        }

        "set.ismember" => ("SISMEMBER", vec![field_str("key"), field_str("member")]),

        "set.members" => ("SMEMBERS", vec![field_str("key")]),
        "set.card" => ("SCARD", vec![field_str("key")]),

        "set.pop" => (
            "SPOP",
            vec![
                field_str("key"),
                WireValue::Int(payload["count"].as_u64().unwrap_or(1) as i64),
            ],
        ),

        "set.randmember" => (
            "SRANDMEMBER",
            vec![
                field_str("key"),
                WireValue::Int(payload["count"].as_u64().unwrap_or(1) as i64),
            ],
        ),

        "set.move" => (
            "SMOVE",
            vec![
                field_str("key"),
                field_str("destination"),
                field_str("member"),
            ],
        ),

        "set.inter" | "set.union" | "set.diff" => {
            let raw: &'static str = match cmd {
                "set.inter" => "SINTER",
                "set.union" => "SUNION",
                _ => "SDIFF",
            };
            let mut args: Vec<WireValue> = vec![];
            if let Some(keys) = payload["keys"].as_array() {
                for k in keys {
                    if let Some(s) = k.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            (raw, args)
        }

        "set.interstore" | "set.unionstore" | "set.diffstore" => {
            let raw: &'static str = match cmd {
                "set.interstore" => "SINTERSTORE",
                "set.unionstore" => "SUNIONSTORE",
                _ => "SDIFFSTORE",
            };
            let mut args = vec![field_str("destination")];
            if let Some(keys) = payload["keys"].as_array() {
                for k in keys {
                    if let Some(s) = k.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            (raw, args)
        }

        // ── Sorted Set ────────────────────────────────────────────────────────
        "sortedset.zadd" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                // add_multiple format: [{member, score}, ...]
                for m in members {
                    args.push(WireValue::Str(
                        m["score"].as_f64().unwrap_or(0.0).to_string(),
                    ));
                    args.push(WireValue::Str(
                        m["member"].as_str().unwrap_or("").to_string(),
                    ));
                }
            } else {
                // add format: {member, score}
                args.push(WireValue::Str(
                    payload["score"].as_f64().unwrap_or(0.0).to_string(),
                ));
                args.push(to_wire(&payload["member"]));
            }
            ("ZADD", args)
        }

        "sortedset.zrem" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    args.push(WireValue::Str(m.as_str().unwrap_or("").to_string()));
                }
            } else {
                args.push(to_wire(&payload["member"]));
            }
            ("ZREM", args)
        }

        "sortedset.zscore" => (
            "ZSCORE",
            vec![field_str("key"), to_wire(&payload["member"])],
        ),

        "sortedset.zcard" => ("ZCARD", vec![field_str("key")]),

        "sortedset.zincrby" => (
            "ZINCRBY",
            vec![
                field_str("key"),
                WireValue::Str(payload["increment"].as_f64().unwrap_or(0.0).to_string()),
                to_wire(&payload["member"]),
            ],
        ),

        "sortedset.zrank" => ("ZRANK", vec![field_str("key"), to_wire(&payload["member"])]),

        "sortedset.zrevrank" => (
            "ZREVRANK",
            vec![field_str("key"), to_wire(&payload["member"])],
        ),

        "sortedset.zcount" => (
            "ZCOUNT",
            vec![
                field_str("key"),
                WireValue::Str(
                    payload["min"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["min"].as_str().unwrap_or("-inf").to_string()),
                ),
                WireValue::Str(
                    payload["max"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["max"].as_str().unwrap_or("+inf").to_string()),
                ),
            ],
        ),

        "sortedset.zrange" | "sortedset.zrevrange" => {
            let raw: &'static str = if cmd == "sortedset.zrevrange" {
                "ZREVRANGE"
            } else {
                "ZRANGE"
            };
            let mut args = vec![
                field_str("key"),
                WireValue::Int(payload["start"].as_i64().unwrap_or(0)),
                WireValue::Int(payload["stop"].as_i64().unwrap_or(-1)),
            ];
            if payload["withscores"].as_bool().unwrap_or(false) {
                args.push(WireValue::Str("WITHSCORES".into()));
            }
            (raw, args)
        }

        "sortedset.zrangebyscore" => {
            let mut args = vec![
                field_str("key"),
                WireValue::Str(
                    payload["min"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["min"].as_str().unwrap_or("-inf").to_string()),
                ),
                WireValue::Str(
                    payload["max"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["max"].as_str().unwrap_or("+inf").to_string()),
                ),
            ];
            if payload["withscores"].as_bool().unwrap_or(false) {
                args.push(WireValue::Str("WITHSCORES".into()));
            }
            ("ZRANGEBYSCORE", args)
        }

        "sortedset.zpopmin" | "sortedset.zpopmax" => {
            let raw: &'static str = if cmd == "sortedset.zpopmax" {
                "ZPOPMAX"
            } else {
                "ZPOPMIN"
            };
            (
                raw,
                vec![
                    field_str("key"),
                    WireValue::Int(payload["count"].as_u64().unwrap_or(1) as i64),
                ],
            )
        }

        "sortedset.zremrangebyrank" => (
            "ZREMRANGEBYRANK",
            vec![
                field_str("key"),
                WireValue::Int(payload["start"].as_i64().unwrap_or(0)),
                WireValue::Int(payload["stop"].as_i64().unwrap_or(-1)),
            ],
        ),

        "sortedset.zremrangebyscore" => (
            "ZREMRANGEBYSCORE",
            vec![
                field_str("key"),
                WireValue::Str(
                    payload["min"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["min"].as_str().unwrap_or("-inf").to_string()),
                ),
                WireValue::Str(
                    payload["max"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["max"].as_str().unwrap_or("+inf").to_string()),
                ),
            ],
        ),

        "sortedset.zinterstore" | "sortedset.zunionstore" | "sortedset.zdiffstore" => {
            let raw: &'static str = match cmd {
                "sortedset.zinterstore" => "ZINTERSTORE",
                "sortedset.zunionstore" => "ZUNIONSTORE",
                _ => "ZDIFFSTORE",
            };
            let mut args = vec![
                field_str("destination"),
                WireValue::Int(
                    payload["keys"]
                        .as_array()
                        .map(|a| a.len() as i64)
                        .unwrap_or(0),
                ),
            ];
            if let Some(keys) = payload["keys"].as_array() {
                for k in keys {
                    args.push(WireValue::Str(k.as_str().unwrap_or("").to_string()));
                }
            }
            (raw, args)
        }

        // ── Queue ─────────────────────────────────────────────────────────────
        "queue.create" => ("QCREATE", vec![field_str("name")]),
        "queue.delete" => ("QDELETE", vec![field_str("queue")]),
        "queue.list" => ("QLIST", vec![]),
        "queue.publish" => {
            let payload_bytes: WireValue = match &payload["payload"] {
                Value::Array(arr) => WireValue::Bytes(
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                        .collect(),
                ),
                Value::String(s) => WireValue::Bytes(s.as_bytes().to_vec()),
                other => WireValue::Bytes(other.to_string().into_bytes()),
            };
            let mut args = vec![field_str("queue"), payload_bytes];
            if let Some(p) = payload["priority"].as_u64() {
                args.push(WireValue::Int(p as i64));
            }
            if let Some(r) = payload["max_retries"].as_u64() {
                args.push(WireValue::Int(r as i64));
            }
            ("QPUBLISH", args)
        }
        "queue.consume" => (
            "QCONSUME",
            vec![field_str("queue"), field_str("consumer_id")],
        ),
        "queue.ack" => ("QACK", vec![field_str("queue"), field_str("message_id")]),
        "queue.nack" => (
            "QNACK",
            vec![
                field_str("queue"),
                field_str("message_id"),
                WireValue::Bool(payload["requeue"].as_bool().unwrap_or(true)),
            ],
        ),
        "queue.stats" => ("QSTATS", vec![field_str("queue")]),

        // ── Stream ────────────────────────────────────────────────────────────
        "stream.create" => ("SCREATE", vec![field_str("room")]),
        "stream.delete" => ("SDELETE", vec![field_str("room")]),
        "stream.list" => ("SLIST", vec![]),
        "stream.publish" => {
            let data_bytes: WireValue = match &payload["data"] {
                Value::String(s) => WireValue::Bytes(s.as_bytes().to_vec()),
                other => WireValue::Bytes(other.to_string().into_bytes()),
            };
            (
                "SPUBLISH",
                vec![field_str("room"), field_str("event"), data_bytes],
            )
        }
        "stream.consume" => {
            let from = WireValue::Int(payload["offset"].as_u64().unwrap_or(0) as i64);
            let limit = WireValue::Int(payload["limit"].as_u64().unwrap_or(100) as i64);
            (
                "SREAD",
                vec![
                    field_str("room"),
                    WireValue::Str("sdk-consumer".into()),
                    from,
                    limit,
                ],
            )
        }
        "stream.stats" => ("SSTATS", vec![field_str("room")]),

        // ── Pub/Sub ───────────────────────────────────────────────────────────
        "pubsub.publish" => {
            let payload_str = match &payload["payload"] {
                Value::String(s) => WireValue::Str(s.clone()),
                other => WireValue::Str(other.to_string()),
            };
            ("PUBLISH", vec![field_str("topic"), payload_str])
        }
        "pubsub.subscribe" => {
            let mut args: Vec<WireValue> = vec![];
            if let Some(topics) = payload["topics"].as_array() {
                for t in topics {
                    if let Some(s) = t.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            ("SUBSCRIBE", args)
        }
        "pubsub.unsubscribe" => {
            let mut args = vec![field_str("subscriber_id")];
            if let Some(topics) = payload["topics"].as_array() {
                for t in topics {
                    if let Some(s) = t.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            ("UNSUBSCRIBE", args)
        }
        "pubsub.topics" => ("TOPICS", vec![]),

        // ── Transactions ──────────────────────────────────────────────────────
        "transaction.multi" => (
            "MULTI",
            vec![WireValue::Str(
                payload["client_id"]
                    .as_str()
                    .unwrap_or("default")
                    .to_string(),
            )],
        ),
        "transaction.exec" => (
            "EXEC",
            vec![WireValue::Str(
                payload["client_id"]
                    .as_str()
                    .unwrap_or("default")
                    .to_string(),
            )],
        ),
        "transaction.discard" => (
            "DISCARD",
            vec![WireValue::Str(
                payload["client_id"]
                    .as_str()
                    .unwrap_or("default")
                    .to_string(),
            )],
        ),
        "transaction.watch" => {
            let client_id = payload["client_id"].as_str().unwrap_or("default");
            let mut args = vec![WireValue::Str(client_id.to_string())];
            if let Some(keys) = payload["keys"].as_array() {
                for k in keys {
                    if let Some(s) = k.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            ("WATCH", args)
        }
        "transaction.unwatch" => (
            "UNWATCH",
            vec![WireValue::Str(
                payload["client_id"]
                    .as_str()
                    .unwrap_or("default")
                    .to_string(),
            )],
        ),

        // ── Scripting ─────────────────────────────────────────────────────────
        "script.eval" => {
            let source = field_str("script");
            let keys = payload["keys"].as_array().cloned().unwrap_or_default();
            let numkeys = WireValue::Int(keys.len() as i64);
            let mut args = vec![source, numkeys];
            for k in &keys {
                args.push(to_wire(k));
            }
            if let Some(extra_args) = payload["args"].as_array() {
                for a in extra_args {
                    args.push(to_wire(a));
                }
            }
            ("EVAL", args)
        }
        "script.evalsha" => {
            let sha = field_str("sha1");
            let keys = payload["keys"].as_array().cloned().unwrap_or_default();
            let numkeys = WireValue::Int(keys.len() as i64);
            let mut args = vec![sha, numkeys];
            for k in &keys {
                args.push(to_wire(k));
            }
            if let Some(extra_args) = payload["args"].as_array() {
                for a in extra_args {
                    args.push(to_wire(a));
                }
            }
            ("EVALSHA", args)
        }
        "script.load" => ("SCRIPT.LOAD", vec![field_str("script")]),
        "script.exists" => {
            let hashes: Vec<WireValue> = payload["hashes"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|h| h.as_str().map(|s| WireValue::Str(s.to_string())))
                .collect();
            ("SCRIPT.EXISTS", hashes)
        }
        "script.flush" => ("SCRIPT.FLUSH", vec![]),
        "script.kill" => ("SCRIPT.KILL", vec![]),

        // ── HyperLogLog ───────────────────────────────────────────────────────
        "hyperloglog.pfadd" => {
            let mut args = vec![field_str("key")];
            if let Some(elements) = payload["elements"].as_array() {
                for e in elements {
                    args.push(to_wire(e));
                }
            }
            ("PFADD", args)
        }
        "hyperloglog.pfcount" => ("PFCOUNT", vec![field_str("key")]),
        "hyperloglog.pfmerge" => {
            let mut args = vec![field_str("destination")];
            if let Some(sources) = payload["sources"].as_array() {
                for s in sources {
                    args.push(to_wire(s));
                }
            }
            ("PFMERGE", args)
        }
        "hyperloglog.stats" => ("HLLSTATS", vec![]),

        // ── Geospatial ────────────────────────────────────────────────────────
        "geospatial.geoadd" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    // SDK sends {lat, lon, member}
                    let lat = m["lat"].as_f64().unwrap_or(0.0);
                    let lon = m["lon"].as_f64().unwrap_or(0.0);
                    let name = m["member"].as_str().unwrap_or("").to_string();
                    args.push(WireValue::Float(lat));
                    args.push(WireValue::Float(lon));
                    args.push(WireValue::Str(name));
                }
            }
            ("GEOADD", args)
        }
        "geospatial.geodist" => (
            "GEODIST",
            vec![
                field_str("key"),
                WireValue::Str(payload["member1"].as_str().unwrap_or("").to_string()),
                WireValue::Str(payload["member2"].as_str().unwrap_or("").to_string()),
                WireValue::Str(payload["unit"].as_str().unwrap_or("m").to_uppercase()),
            ],
        ),
        "geospatial.georadius" => {
            let mut args = vec![
                field_str("key"),
                WireValue::Float(payload["lat"].as_f64().unwrap_or(0.0)),
                WireValue::Float(payload["lon"].as_f64().unwrap_or(0.0)),
                WireValue::Float(payload["radius"].as_f64().unwrap_or(0.0)),
                WireValue::Str(payload["unit"].as_str().unwrap_or("m").to_uppercase()),
            ];
            if payload["with_distance"].as_bool().unwrap_or(false) {
                args.push(WireValue::Str("WITHCOORD".into()));
            }
            ("GEORADIUS", args)
        }
        "geospatial.georadiusbymember" => (
            "GEORADIUSBYMEMBER",
            vec![
                field_str("key"),
                WireValue::Str(payload["member"].as_str().unwrap_or("").to_string()),
                WireValue::Float(payload["radius"].as_f64().unwrap_or(0.0)),
                WireValue::Str(payload["unit"].as_str().unwrap_or("m").to_uppercase()),
            ],
        ),
        "geospatial.geopos" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    args.push(to_wire(m));
                }
            }
            ("GEOPOS", args)
        }
        "geospatial.geosearch" => (
            "GEOSEARCH",
            vec![
                field_str("key"),
                WireValue::Float(payload["lat"].as_f64().unwrap_or(0.0)),
                WireValue::Float(payload["lon"].as_f64().unwrap_or(0.0)),
                WireValue::Float(payload["radius"].as_f64().unwrap_or(0.0)),
                WireValue::Str(payload["unit"].as_str().unwrap_or("m").to_uppercase()),
            ],
        ),
        "geospatial.geohash" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    args.push(to_wire(m));
                }
            }
            ("GEOHASH", args)
        }
        "geospatial.stats" => ("GEOSTATS", vec![]),

        // Commands with no native mapping — caller returns UnsupportedCommand.
        _ => return None,
    })
}

// ── Response mapper ───────────────────────────────────────────────────────────

/// Convert a raw `WireValue` response into the JSON shape that SDK managers
/// expect from [`SynapClient::send_command`].
pub(crate) fn map_response(cmd: &str, wire: WireValue) -> Value {
    match cmd {
        // ── KV ────────────────────────────────────────────────────────────────
        // kv.get: managers do serde_json::from_value(response)? — pass through.
        "kv.get" => wire.to_json(),
        "kv.set" => json!({}),
        "kv.del" => {
            let n = wire.as_int().unwrap_or(0);
            let deleted = matches!(wire, WireValue::Bool(true)) || n > 0;
            json!({"deleted": deleted})
        }
        "kv.exists" => {
            let n = wire.as_int().unwrap_or(0);
            let exists = matches!(wire, WireValue::Bool(true)) || n > 0;
            json!({"exists": exists})
        }
        "kv.incr" | "kv.decr" => json!({"value": wire.as_int().unwrap_or(0)}),
        "kv.keys" => {
            let keys: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"keys": keys})
        }
        "kv.expire" => json!({}),
        "kv.ttl" => wire.to_json(),

        // ── Hash ──────────────────────────────────────────────────────────────
        "hash.set" => json!({"success": wire.as_int().map(|n| n >= 0).unwrap_or(true)}),
        "hash.get" => {
            let v = if wire.is_null() {
                Value::Null
            } else {
                wire.to_json()
            };
            json!({"value": v})
        }
        "hash.getall" => {
            // HGETALL returns a flat array: [field1, val1, field2, val2, ...]
            let mut fields = serde_json::Map::new();
            if let WireValue::Array(arr) = wire {
                for chunk in arr.chunks(2) {
                    if let (Some(k), Some(v)) = (chunk.first(), chunk.get(1)) {
                        if let Some(key_str) = k.as_str() {
                            fields.insert(key_str.to_string(), v.to_json());
                        }
                    }
                }
            }
            json!({"fields": Value::Object(fields)})
        }
        "hash.del" => {
            let n = wire.as_int().unwrap_or(0);
            let n = if matches!(wire, WireValue::Bool(true)) {
                1
            } else {
                n
            };
            json!({"deleted": n})
        }
        "hash.exists" => {
            let n = wire.as_int().unwrap_or(0);
            let exists = matches!(wire, WireValue::Bool(true)) || n > 0;
            json!({"exists": exists})
        }
        "hash.keys" => {
            let arr: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"fields": arr})
        }
        "hash.values" => {
            let arr: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"values": arr})
        }
        "hash.len" => json!({"length": wire.as_int().unwrap_or(0)}),
        "hash.mset" => json!({"success": !wire.is_null()}),
        "hash.mget" => {
            let values: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"values": values})
        }
        "hash.incrby" => json!({"value": wire.as_int().unwrap_or(0)}),
        "hash.incrbyfloat" => {
            let f = wire
                .as_float()
                .or_else(|| wire.as_str().and_then(|s| s.parse().ok()))
                .unwrap_or(0.0);
            json!({"value": f})
        }
        "hash.setnx" => json!({"created": wire.as_int().unwrap_or(0) > 0}),

        // ── List ──────────────────────────────────────────────────────────────
        "list.lpush" | "list.rpush" | "list.lpushx" | "list.rpushx" => {
            json!({"length": wire.as_int().unwrap_or(0)})
        }
        "list.lpop" | "list.rpop" => {
            let values: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                WireValue::Str(s) => vec![json!(s)],
                WireValue::Bytes(b) => match String::from_utf8(b.clone()) {
                    Ok(s) => vec![json!(s)],
                    Err(_) => vec![json!(b)],
                },
                WireValue::Null => vec![],
                _ => vec![],
            };
            json!({"values": values})
        }
        "list.range" => {
            let values: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"values": values})
        }
        "list.len" => json!({"length": wire.as_int().unwrap_or(0)}),
        "list.index" => wire.to_json(),
        "list.set" | "list.trim" => json!({}),
        "list.rem" => json!({"count": wire.as_int().unwrap_or(0)}),
        "list.insert" => json!({"length": wire.as_int().unwrap_or(-1)}),
        "list.rpoplpush" => wire.to_json(),
        "list.pos" => wire.to_json(),

        // ── Set ───────────────────────────────────────────────────────────────
        "set.add" => json!({"added": wire.as_int().unwrap_or(0)}),
        "set.rem" => json!({"removed": wire.as_int().unwrap_or(0)}),
        "set.ismember" => json!({"is_member": wire.as_int().unwrap_or(0) > 0}),
        "set.members" | "set.pop" | "set.randmember" => {
            let members: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                WireValue::Str(s) => vec![json!(s)],
                WireValue::Null => vec![],
                _ => vec![],
            };
            json!({"members": members})
        }
        "set.card" => json!({"cardinality": wire.as_int().unwrap_or(0)}),
        "set.move" => json!({"moved": wire.as_int().unwrap_or(0) > 0}),
        "set.inter" | "set.union" | "set.diff" => {
            let members: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"members": members})
        }
        "set.interstore" | "set.unionstore" | "set.diffstore" => {
            json!({"count": wire.as_int().unwrap_or(0)})
        }

        // ── Sorted Set ────────────────────────────────────────────────────────
        "sortedset.zadd" => json!({"added": wire.as_int().unwrap_or(0)}),
        "sortedset.zrem" => json!({"removed": wire.as_int().unwrap_or(0)}),
        "sortedset.zscore" => {
            let score = wire
                .as_float()
                .or_else(|| wire.as_str().and_then(|s| s.parse().ok()));
            json!({"score": score})
        }
        "sortedset.zcard" => json!({"count": wire.as_int().unwrap_or(0)}),
        "sortedset.zincrby" => {
            let score = wire
                .as_float()
                .or_else(|| wire.as_str().and_then(|s| s.parse().ok()))
                .unwrap_or(0.0);
            json!({"score": score})
        }
        "sortedset.zrank" | "sortedset.zrevrank" => {
            if wire.is_null() {
                json!({"rank": null})
            } else {
                json!({"rank": wire.as_int().unwrap_or(-1)})
            }
        }
        "sortedset.zcount" | "sortedset.zremrangebyrank" | "sortedset.zremrangebyscore" => {
            json!({"count": wire.as_int().unwrap_or(0)})
        }
        "sortedset.zrange" | "sortedset.zrevrange" | "sortedset.zrangebyscore" => {
            // ZRANGE … WITHSCORES returns interleaved [member, score, ...].
            // Without WITHSCORES returns plain [member, ...].
            // The SDK manager always requests with_scores, so we build ScoredMember objects.
            let members: Vec<Value> = match wire {
                WireValue::Array(arr) => {
                    // Check if interleaved (even count, alternating member/score strings).
                    if arr.len() % 2 == 0 && !arr.is_empty() {
                        arr.chunks(2)
                            .map(|chunk| {
                                let member = chunk[0].as_str().unwrap_or("").to_string();
                                let score = chunk[1]
                                    .as_float()
                                    .or_else(|| chunk[1].as_str().and_then(|s| s.parse().ok()))
                                    .unwrap_or(0.0);
                                json!({"member": member, "score": score})
                            })
                            .collect()
                    } else {
                        // Plain member list — score unknown, default to 0.
                        arr.iter()
                            .map(|v| json!({"member": v.as_str().unwrap_or(""), "score": 0.0}))
                            .collect()
                    }
                }
                _ => vec![],
            };
            json!({"members": members})
        }
        "sortedset.zpopmin" | "sortedset.zpopmax" => {
            // Returns interleaved [member, score, ...].
            let pairs: Vec<Value> = match wire {
                WireValue::Array(arr) => arr
                    .chunks(2)
                    .filter_map(|chunk| {
                        if chunk.len() == 2 {
                            let member = chunk[0].as_str().unwrap_or("").to_string();
                            let score = chunk[1]
                                .as_float()
                                .or_else(|| chunk[1].as_str().and_then(|s| s.parse().ok()))
                                .unwrap_or(0.0);
                            Some(json!({"member": member, "score": score}))
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => vec![],
            };
            json!({"members": pairs})
        }
        "sortedset.zinterstore" | "sortedset.zunionstore" | "sortedset.zdiffstore" => {
            json!({"count": wire.as_int().unwrap_or(0)})
        }

        // ── Queue ─────────────────────────────────────────────────────────────
        "queue.create" | "queue.delete" => json!({}),
        "queue.list" => {
            let queues: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"queues": queues})
        }
        "queue.publish" => {
            let id = wire.as_str().unwrap_or("").to_string();
            json!({"message_id": id})
        }
        "queue.consume" => {
            // Server returns Map with id/payload/priority/retry_count or Null.
            // On native transports the `payload` field arrives as a Str (bytes
            // decoded as UTF-8); convert it to a JSON byte array so that
            // `Message.payload` (Vec<u8>) deserialises correctly.
            if wire.is_null() {
                Value::Null
            } else {
                let mut json = wire.to_json();
                if let Some(s) = json.get("payload").and_then(|p| p.as_str()) {
                    let byte_arr: Vec<Value> = s.bytes().map(|b| json!(b as u64)).collect();
                    json["payload"] = Value::Array(byte_arr);
                }
                json
            }
        }
        "queue.ack" | "queue.nack" => json!({}),
        "queue.stats" => wire.to_json(),

        // ── Stream ────────────────────────────────────────────────────────────
        "stream.create" | "stream.delete" => json!({}),
        "stream.list" => {
            let rooms: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"rooms": rooms})
        }
        "stream.publish" => {
            let offset = wire.as_int().unwrap_or(0) as u64;
            json!({"offset": offset})
        }
        "stream.consume" => {
            let events: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"events": events})
        }
        "stream.stats" => wire.to_json(),

        // ── Pub/Sub ───────────────────────────────────────────────────────────
        "pubsub.publish" => {
            let matched = match &wire {
                WireValue::Map(pairs) => pairs
                    .iter()
                    .find(|(k, _)| k.as_str() == Some("subscribers_matched"))
                    .and_then(|(_, v)| v.as_int())
                    .unwrap_or(0),
                WireValue::Int(n) => *n,
                _ => 0,
            };
            json!({"subscribers_matched": matched})
        }
        "pubsub.subscribe" => {
            let sub_id = match &wire {
                WireValue::Map(pairs) => pairs
                    .iter()
                    .find(|(k, _)| k.as_str() == Some("subscriber_id"))
                    .and_then(|(_, v)| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            json!({"subscription_id": sub_id})
        }
        "pubsub.unsubscribe" => json!({}),
        "pubsub.topics" => {
            let topics: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"topics": topics})
        }

        // ── Transactions ──────────────────────────────────────────────────────
        "transaction.multi"
        | "transaction.discard"
        | "transaction.watch"
        | "transaction.unwatch" => json!({"success": true}),
        "transaction.exec" => {
            let results: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"results": results})
        }

        // ── Scripting ─────────────────────────────────────────────────────────
        "script.eval" | "script.evalsha" => {
            // Server returns a JSON string — parse it back to reconstruct
            // ScriptEvalResponse-compatible shape.
            let raw = wire.to_json();
            json!({"result": raw, "sha1": ""})
        }
        "script.load" => {
            let sha = wire.as_str().unwrap_or("").to_string();
            json!({"sha1": sha})
        }
        "script.exists" => {
            let exists: Vec<Value> = match wire {
                WireValue::Array(arr) => arr
                    .iter()
                    .map(|v| {
                        json!(v.as_int().unwrap_or(0) != 0 || matches!(v, WireValue::Bool(true)))
                    })
                    .collect(),
                _ => vec![],
            };
            json!({"exists": exists})
        }
        "script.flush" => {
            let cleared = wire.as_int().unwrap_or(0) as u64;
            json!({"cleared": cleared})
        }
        "script.kill" => {
            let terminated =
                matches!(wire, WireValue::Bool(true)) || wire.as_int().unwrap_or(0) != 0;
            json!({"terminated": terminated})
        }

        // ── HyperLogLog ───────────────────────────────────────────────────────
        "hyperloglog.pfadd" => json!({"modified": wire.as_int().unwrap_or(0) > 0}),
        "hyperloglog.pfcount" => json!({"count": wire.as_int().unwrap_or(0)}),
        "hyperloglog.pfmerge" => json!({}),
        "hyperloglog.stats" => wire.to_json(),

        // ── Geospatial ────────────────────────────────────────────────────────
        "geospatial.geoadd" => json!({"added": wire.as_int().unwrap_or(0)}),
        "geospatial.geodist" => {
            let dist = wire
                .as_float()
                .or_else(|| wire.as_str().and_then(|s| s.parse().ok()));
            json!({"distance": dist})
        }
        "geospatial.georadius" | "geospatial.georadiusbymember" | "geospatial.geosearch" => {
            let members: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"members": members})
        }
        "geospatial.geopos" => {
            let positions: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"positions": positions})
        }
        "geospatial.geohash" => {
            let hashes: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"hashes": hashes})
        }
        "geospatial.stats" => wire.to_json(),

        // Fallthrough: return the raw JSON representation.
        _ => wire.to_json(),
    }
}
