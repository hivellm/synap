using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Net.Sockets;
using System.Text;
using Synap.SDK.Exceptions;

namespace Synap.SDK;

internal static class CommandMapper
{
    /// <summary>
    /// Maps a Synap operation + payload to a native command + args.
    /// Returns null for operations that must fall back to HTTP.
    /// </summary>
    internal static (string Command, object?[] Args)? MapCommand(
        string operation,
        Dictionary<string, object?> payload)
    {
        var key = payload.TryGetValue("key", out var k) ? k?.ToString() ?? string.Empty : string.Empty;

        return operation switch
        {
            // ---- KV ----
            "kv.get" => ("GET", [key]),
            "kv.delete" => ("DEL", [key]),
            "kv.exists" => ("EXISTS", [key]),
            "kv.ttl" => ("TTL", [key]),
            "kv.persist" => ("PERSIST", [key]),
            "kv.type" => ("TYPE", [key]),
            "kv.expire" => payload.TryGetValue("ttl", out var ttl)
                ? ("EXPIRE", new object?[] { key, ttl })
                : null,
            "kv.set" => BuildKvSet(key, payload),
            "kv.incr" => payload.TryGetValue("delta", out var d)
                ? ("INCRBY", new object?[] { key, d })
                : ("INCR", new object?[] { key }),
            "kv.decr" => payload.TryGetValue("delta", out var dd)
                ? ("DECRBY", new object?[] { key, dd })
                : ("DECR", new object?[] { key }),
            "kv.incr_float" => payload.TryGetValue("delta", out var df)
                ? ("INCRBYFLOAT", new object?[] { key, df })
                : null,
            "kv.rename" => payload.TryGetValue("new_key", out var nk)
                ? ("RENAME", new object?[] { key, nk })
                : null,

            // ---- Hash ----
            "hash.set" => BuildHSet(key, payload),
            "hash.get" => payload.TryGetValue("field", out var hf)
                ? ("HGET", new object?[] { key, hf })
                : null,
            "hash.getall" => ("HGETALL", [key]),
            "hash.delete" => payload.TryGetValue("field", out var hdf)
                ? ("HDEL", new object?[] { key, hdf })
                : null,
            "hash.exists" => payload.TryGetValue("field", out var hef)
                ? ("HEXISTS", new object?[] { key, hef })
                : null,
            "hash.len" => ("HLEN", [key]),
            "hash.keys" => ("HKEYS", [key]),
            "hash.values" => ("HVALS", [key]),
            "hash.incr" => payload.TryGetValue("field", out var hif) && payload.TryGetValue("delta", out var hid)
                ? ("HINCRBY", new object?[] { key, hif, hid })
                : null,

            // ---- List ----
            "list.push_left" => payload.TryGetValue("value", out var lpl)
                ? ("LPUSH", new object?[] { key, lpl })
                : null,
            "list.push_right" => payload.TryGetValue("value", out var lpr)
                ? ("RPUSH", new object?[] { key, lpr })
                : null,
            "list.pop_left" => ("LPOP", [key]),
            "list.pop_right" => ("RPOP", [key]),
            "list.len" => ("LLEN", [key]),
            "list.range" => BuildLRange(key, payload),
            "list.index" => payload.TryGetValue("index", out var li)
                ? ("LINDEX", new object?[] { key, li })
                : null,

            // ---- Set ----
            "set.add" => payload.TryGetValue("member", out var sm)
                ? ("SADD", new object?[] { key, sm })
                : null,
            "set.remove" => payload.TryGetValue("member", out var srm)
                ? ("SREM", new object?[] { key, srm })
                : null,
            "set.members" => ("SMEMBERS", [key]),
            "set.ismember" => payload.TryGetValue("member", out var sim)
                ? ("SISMEMBER", new object?[] { key, sim })
                : null,
            "set.card" => ("SCARD", [key]),

            // ---- Sorted Set ----
            "sorted_set.add" => BuildZAdd(key, payload),
            "sorted_set.remove" => payload.TryGetValue("member", out var zrm)
                ? ("ZREM", new object?[] { key, zrm })
                : null,
            "sorted_set.score" => payload.TryGetValue("member", out var zsm)
                ? ("ZSCORE", new object?[] { key, zsm })
                : null,
            "sorted_set.rank" => payload.TryGetValue("member", out var zrk)
                ? ("ZRANK", new object?[] { key, zrk })
                : null,
            "sorted_set.card" => ("ZCARD", [key]),
            "sorted_set.range" => BuildZRange(key, payload),

            // ---- Bitmap ----
            "bitmap.setbit" => payload.TryGetValue("offset", out var bo) && payload.TryGetValue("value", out var bv)
                ? ("SETBIT", new object?[] { key, bo, bv })
                : null,
            "bitmap.getbit" => payload.TryGetValue("offset", out var bgo)
                ? ("GETBIT", new object?[] { key, bgo })
                : null,
            "bitmap.bitcount" => ("BITCOUNT", [key]),
            "bitmap.bitop.and" => BuildBitOp("AND", payload),
            "bitmap.bitop.or" => BuildBitOp("OR", payload),
            "bitmap.bitop.xor" => BuildBitOp("XOR", payload),
            "bitmap.bitop.not" => BuildBitOp("NOT", payload),

            // ---- HyperLogLog ----
            "hyperloglog.add" => payload.TryGetValue("element", out var hle)
                ? ("PFADD", new object?[] { key, hle })
                : null,
            "hyperloglog.count" => ("PFCOUNT", [key]),
            "hyperloglog.pfmerge" => BuildPfMerge(payload),

            // ---- Geospatial ----
            "geospatial.add" => BuildGeoAdd(key, payload),
            "geospatial.dist" => BuildGeoDist(key, payload),
            "geospatial.pos" => payload.TryGetValue("member", out var gpm)
                ? ("GEOPOS", new object?[] { key, gpm })
                : null,
            "geospatial.radius" => BuildGeoRadius(key, payload),

            // ---- Queue ----
            "queue.create" => payload.TryGetValue("name", out var qn)
                ? ("QCREATE", new object?[] { qn,
                      payload.TryGetValue("max_depth", out var qmd) ? qmd : 0L,
                      payload.TryGetValue("ack_deadline_secs", out var qad) ? qad : 0L })
                : null,
            "queue.delete" => payload.TryGetValue("queue", out var qd)
                ? ("QDELETE", new object?[] { qd }) : null,
            "queue.publish" => BuildQPublish(payload),
            "queue.consume" => payload.TryGetValue("queue", out var qcq) && payload.TryGetValue("consumer_id", out var qcc)
                ? ("QCONSUME", new object?[] { qcq, qcc }) : null,
            "queue.ack" => payload.TryGetValue("queue", out var qaq) && payload.TryGetValue("message_id", out var qam)
                ? ("QACK", new object?[] { qaq, qam }) : null,
            "queue.nack" => payload.TryGetValue("queue", out var qnq) && payload.TryGetValue("message_id", out var qnm)
                ? ("QNACK", new object?[] { qnq, qnm, payload.TryGetValue("delay_secs", out var qnd) ? qnd : 0L }) : null,
            "queue.stats" => payload.TryGetValue("queue", out var qsq)
                ? ("QSTATS", new object?[] { qsq }) : null,
            "queue.purge" => payload.TryGetValue("queue", out var qpq)
                ? ("QPURGE", new object?[] { qpq }) : null,
            "queue.list" => ("QLIST", Array.Empty<object?>()),

            // ---- Stream ----
            "stream.create" => payload.TryGetValue("room", out var scr)
                ? ("SCREATE", new object?[] { scr }) : null,
            "stream.delete" => payload.TryGetValue("room", out var sdr)
                ? ("SDELETE", new object?[] { sdr }) : null,
            "stream.publish" => BuildSPublish(payload),
            "stream.consume" => payload.TryGetValue("room", out var srr)
                ? ("SREAD", new object?[] { srr,
                      payload.TryGetValue("subscriber_id", out var srid) ? srid : "sdk-reader",
                      payload.TryGetValue("from_offset", out var srfo) ? srfo?.ToString() ?? "0" : "0" })
                : null,
            "stream.list" => ("SLIST", Array.Empty<object?>()),

            // ---- Pub/Sub ----
            "pubsub.publish" => BuildPPublish(payload),
            "pubsub.subscribe" => BuildPSubscribe(payload),
            "pubsub.unsubscribe" => BuildPUnsubscribe(payload),
            "pubsub.topics" => ("PTOPICS", Array.Empty<object?>()),
            "pubsub.stats" => ("PSTATS", Array.Empty<object?>()),

            // ---- Transaction ----
            "transaction.multi" => payload.TryGetValue("client_id", out var tmid)
                ? ("MULTI", new object?[] { tmid }) : null,
            "transaction.exec" => payload.TryGetValue("client_id", out var teid)
                ? ("EXEC", new object?[] { teid }) : null,
            "transaction.discard" => payload.TryGetValue("client_id", out var tdid)
                ? ("DISCARD", new object?[] { tdid }) : null,
            "transaction.watch" => BuildTxWatch(payload),
            "transaction.unwatch" => payload.TryGetValue("client_id", out var tuid)
                ? ("UNWATCH", new object?[] { tuid }) : null,

            // ---- Scripting ----
            "script.eval" => BuildScriptEval(payload),
            "script.evalsha" => BuildScriptEvalSha(payload),

            _ => null,
        };
    }

    /// <summary>Maps the raw native response back to the Synap response shape.</summary>
    internal static Dictionary<string, object?> MapResponse(string operation, object? raw)
    {
        return operation switch
        {
            "kv.get" => new Dictionary<string, object?> { ["value"] = raw },
            "kv.exists" => new Dictionary<string, object?> { ["exists"] = raw is long l && l > 0 },
            "kv.incr" or "kv.decr" => new Dictionary<string, object?> { ["value"] = raw },
            "kv.incr_float" => new Dictionary<string, object?> { ["value"] = raw },
            "kv.ttl" => new Dictionary<string, object?> { ["ttl"] = raw },
            "kv.type" => new Dictionary<string, object?> { ["type"] = raw },

            "hash.get" => new Dictionary<string, object?> { ["value"] = raw },
            "hash.exists" => new Dictionary<string, object?> { ["exists"] = raw is long hl && hl > 0 },
            "hash.len" => new Dictionary<string, object?> { ["length"] = raw },
            "hash.keys" => new Dictionary<string, object?> { ["keys"] = RawToList(raw) },
            "hash.values" => new Dictionary<string, object?> { ["values"] = RawToList(raw) },
            "hash.getall" => new Dictionary<string, object?> { ["fields"] = FlatArrayToDict(raw) },
            "hash.incr" => new Dictionary<string, object?> { ["value"] = raw },

            "list.pop_left" or "list.pop_right" or "list.index" =>
                new Dictionary<string, object?> { ["value"] = raw },
            "list.len" => new Dictionary<string, object?> { ["length"] = raw },
            "list.range" => new Dictionary<string, object?> { ["items"] = RawToList(raw) },

            "set.ismember" => new Dictionary<string, object?> { ["is_member"] = raw is long sl && sl > 0 },
            "set.card" => new Dictionary<string, object?> { ["count"] = raw },
            "set.members" => new Dictionary<string, object?> { ["members"] = RawToList(raw) },

            "sorted_set.card" => new Dictionary<string, object?> { ["count"] = raw },
            "sorted_set.score" => new Dictionary<string, object?> { ["score"] = raw },
            "sorted_set.rank" => new Dictionary<string, object?> { ["rank"] = raw },
            "sorted_set.range" => new Dictionary<string, object?> { ["members"] = ZRangeToList(raw) },

            "bitmap.getbit" => new Dictionary<string, object?> { ["bit"] = raw },
            "bitmap.bitcount" => new Dictionary<string, object?> { ["count"] = raw },

            "hyperloglog.add" => new Dictionary<string, object?> { ["changed"] = raw is long hll && hll > 0 },
            "hyperloglog.count" => new Dictionary<string, object?> { ["count"] = raw },

            "geospatial.dist" => new Dictionary<string, object?> { ["distance"] = raw },
            "geospatial.pos" => new Dictionary<string, object?> { ["positions"] = RawToList(raw) },
            "geospatial.radius" => new Dictionary<string, object?> { ["members"] = RawToList(raw) },

            // ---- Queue responses ----
            "queue.create" => new Dictionary<string, object?> { ["success"] = IsOk(raw) },
            "queue.delete" or "queue.purge" => new Dictionary<string, object?> { ["success"] = raw is true or 1L },
            "queue.publish" => new Dictionary<string, object?> { ["message_id"] = raw?.ToString() ?? string.Empty },
            "queue.consume" => raw is null or "Null"
                ? new Dictionary<string, object?>()
                : new Dictionary<string, object?> { ["message"] = raw },
            "queue.ack" or "queue.nack" => new Dictionary<string, object?> { ["success"] = true },
            "queue.stats" => raw is Dictionary<string, object?> qsd ? qsd : new Dictionary<string, object?> { ["result"] = raw },
            "queue.list" => new Dictionary<string, object?> { ["queues"] = RawToList(raw) },

            // ---- Stream responses ----
            "stream.create" => new Dictionary<string, object?> { ["success"] = IsOk(raw) },
            "stream.delete" => new Dictionary<string, object?> { ["success"] = raw is true or 1L },
            "stream.publish" => new Dictionary<string, object?> { ["offset"] = raw is long sl ? sl : Convert.ToInt64(raw ?? 0L, System.Globalization.CultureInfo.InvariantCulture) },
            "stream.consume" => new Dictionary<string, object?> { ["events"] = RawToList(raw) },
            "stream.list" => new Dictionary<string, object?> { ["rooms"] = RawToList(raw) },

            // ---- Pub/Sub responses ----
            "pubsub.publish" => new Dictionary<string, object?> { ["subscribers_matched"] = raw is long pm ? pm : Convert.ToInt64(raw ?? 0L, System.Globalization.CultureInfo.InvariantCulture) },
            "pubsub.subscribe" or "pubsub.unsubscribe" => new Dictionary<string, object?> { ["success"] = true },
            "pubsub.topics" => new Dictionary<string, object?> { ["topics"] = RawToList(raw) },
            "pubsub.stats" => raw is Dictionary<string, object?> psd ? psd : new Dictionary<string, object?> { ["result"] = raw },

            // ---- Transaction responses ----
            "transaction.multi" or "transaction.discard" or "transaction.watch" or "transaction.unwatch"
                => new Dictionary<string, object?> { ["success"] = IsOk(raw) },
            "transaction.exec" => new Dictionary<string, object?> { ["success"] = true, ["results"] = RawToList(raw) },

            // ---- Scripting responses ----
            "script.eval" or "script.evalsha" => new Dictionary<string, object?> { ["result"] = raw },

            // For ops that return no meaningful value (set, delete, etc.)
            _ => new Dictionary<string, object?> { ["success"] = true },
        };
    }

    private static bool IsOk(object? raw) => raw is "OK" or true or 1L;

    // -- Helpers --

    private static (string, object?[])? BuildKvSet(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("value", out var value))
        {
            return null;
        }

        if (payload.TryGetValue("ttl", out var ttl))
        {
            return ("SET", [key, value, "EX", ttl]);
        }

        return ("SET", [key, value]);
    }

    private static (string, object?[])? BuildHSet(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("field", out var field) || !payload.TryGetValue("value", out var value))
        {
            return null;
        }

        return ("HSET", [key, field, value]);
    }

    private static (string, object?[])? BuildLRange(string key, Dictionary<string, object?> payload)
    {
        var start = payload.TryGetValue("start", out var s) ? s : (object?)0L;
        var stop = payload.TryGetValue("stop", out var e) ? e : (object?)-1L;
        return ("LRANGE", [key, start, stop]);
    }

    private static (string, object?[])? BuildZAdd(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("member", out var member) || !payload.TryGetValue("score", out var score))
        {
            return null;
        }

        return ("ZADD", [key, score, member]);
    }

    private static (string, object?[])? BuildZRange(string key, Dictionary<string, object?> payload)
    {
        var start = payload.TryGetValue("start", out var s) ? s : (object?)0L;
        var stop = payload.TryGetValue("stop", out var e) ? e : (object?)-1L;
        var withScores = payload.TryGetValue("with_scores", out var ws) && ws is bool b && b;
        if (withScores)
        {
            return ("ZRANGE", [key, start, stop, "WITHSCORES"]);
        }

        return ("ZRANGE", [key, start, stop]);
    }

    private static (string, object?[])? BuildBitOp(string op, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("destination", out var dest))
        {
            return null;
        }

        if (!payload.TryGetValue("keys", out var keysObj))
        {
            return null;
        }

        var keys = keysObj as object?[] ?? Array.Empty<object?>();
        var args = new List<object?> { op, dest };
        args.AddRange(keys);
        return ("BITOP", args.ToArray());
    }

    private static (string, object?[])? BuildPfMerge(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("destination", out var dest))
        {
            return null;
        }

        if (!payload.TryGetValue("keys", out var keysObj))
        {
            return null;
        }

        var keys = keysObj as object?[] ?? Array.Empty<object?>();
        var args = new List<object?> { dest };
        args.AddRange(keys);
        return ("PFMERGE", args.ToArray());
    }

    private static (string, object?[])? BuildGeoAdd(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("member", out var member))
        {
            return null;
        }

        if (!payload.TryGetValue("longitude", out var lon))
        {
            return null;
        }

        if (!payload.TryGetValue("latitude", out var lat))
        {
            return null;
        }

        return ("GEOADD", [key, lon, lat, member]);
    }

    private static (string, object?[])? BuildGeoDist(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("member1", out var m1))
        {
            return null;
        }

        if (!payload.TryGetValue("member2", out var m2))
        {
            return null;
        }

        var unit = payload.TryGetValue("unit", out var u) ? u?.ToString() ?? "m" : "m";
        return ("GEODIST", [key, m1, m2, unit]);
    }

    private static (string, object?[])? BuildGeoRadius(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("longitude", out var lon))
        {
            return null;
        }

        if (!payload.TryGetValue("latitude", out var lat))
        {
            return null;
        }

        if (!payload.TryGetValue("radius", out var radius))
        {
            return null;
        }

        var unit = payload.TryGetValue("unit", out var u) ? u?.ToString() ?? "m" : "m";
        return ("GEORADIUS", [key, lon, lat, radius, unit]);
    }

    private static List<object?> RawToList(object? raw)
    {
        if (raw is object?[] arr)
        {
            return new List<object?>(arr);
        }

        return new List<object?>();
    }

    private static Dictionary<string, object?> FlatArrayToDict(object? raw)
    {
        var result = new Dictionary<string, object?>();
        if (raw is not object?[] arr)
        {
            return result;
        }

        for (var i = 0; i + 1 < arr.Length; i += 2)
        {
            var k = arr[i]?.ToString();
            if (k is not null)
            {
                result[k] = arr[i + 1];
            }
        }

        return result;
    }

    private static List<object?> ZRangeToList(object? raw)
    {
        if (raw is not object?[] arr)
        {
            return new List<object?>();
        }
        // If WITHSCORES, arr is [member, score, member, score, ...]
        // Return as list of { member, score } dicts
        if (arr.Length > 0 && arr.Length % 2 == 0)
        {
            var hasScores = true;
            for (var i = 1; i < arr.Length; i += 2)
            {
                if (arr[i] is not (double or long or float))
                {
                    hasScores = false;
                    break;
                }
            }

            if (hasScores)
            {
                var list = new List<object?>();
                for (var i = 0; i + 1 < arr.Length; i += 2)
                {
                    list.Add(new Dictionary<string, object?>
                    {
                        ["member"] = arr[i],
                        ["score"] = arr[i + 1],
                    });
                }

                return list;
            }
        }

        return new List<object?>(arr);
    }

    // ---- Queue helpers ----

    private static (string, object?[])? BuildQPublish(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("queue", out var queue))
        {
            return null;
        }

        var payloadVal = payload.TryGetValue("payload", out var p) ? p : null;
        var priority   = payload.TryGetValue("priority", out var pr) ? pr : (object?)0L;
        var maxRetries = payload.TryGetValue("max_retries", out var mr) ? mr : (object?)3L;
        return ("QPUBLISH", [queue, System.Text.Json.JsonSerializer.Serialize(payloadVal), priority, maxRetries]);
    }

    // ---- Stream helpers ----

    private static (string, object?[])? BuildSPublish(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("room", out var room) || !payload.TryGetValue("event", out var evt))
        {
            return null;
        }

        var data = payload.TryGetValue("data", out var d) ? d : null;
        return ("SPUBLISH", [room, evt, System.Text.Json.JsonSerializer.Serialize(data)]);
    }

    // ---- Pub/Sub helpers ----

    private static (string, object?[])? BuildPPublish(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("topic", out var topic))
        {
            return null;
        }

        var msg = payload.TryGetValue("payload", out var p) ? p : null;
        return ("PPUBLISH", [topic, System.Text.Json.JsonSerializer.Serialize(msg)]);
    }

    private static (string, object?[])? BuildPSubscribe(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("subscriber_id", out var sid))
        {
            return null;
        }

        var topics = payload.TryGetValue("topics", out var t) && t is object?[] arr
            ? arr
            : Array.Empty<object?>();
        return ("PSUBSCRIBE", new object?[] { sid }.Concat(topics).ToArray());
    }

    private static (string, object?[])? BuildPUnsubscribe(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("subscriber_id", out var sid))
        {
            return null;
        }

        var topics = payload.TryGetValue("topics", out var t) && t is object?[] arr
            ? arr
            : Array.Empty<object?>();
        return ("PUNSUBSCRIBE", new object?[] { sid }.Concat(topics).ToArray());
    }

    // ---- Transaction helpers ----

    private static (string, object?[])? BuildTxWatch(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("client_id", out var clientId))
        {
            return null;
        }

        var keys = payload.TryGetValue("keys", out var k) && k is object?[] arr
            ? arr
            : Array.Empty<object?>();
        return ("WATCH", new object?[] { clientId }.Concat(keys).ToArray());
    }

    // ---- Script helpers ----

    private static (string, object?[])? BuildScriptEval(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("script", out var script))
        {
            return null;
        }

        var keys = payload.TryGetValue("keys", out var k) && k is object?[] kArr ? kArr : Array.Empty<object?>();
        var args = payload.TryGetValue("args", out var a) && a is object?[] aArr ? aArr : Array.Empty<object?>();
        return ("EVAL", new object?[] { script, (long)keys.Length }.Concat(keys).Concat(args).ToArray());
    }

    private static (string, object?[])? BuildScriptEvalSha(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("sha", out var sha))
        {
            return null;
        }

        var keys = payload.TryGetValue("keys", out var k) && k is object?[] kArr ? kArr : Array.Empty<object?>();
        var args = payload.TryGetValue("args", out var a) && a is object?[] aArr ? aArr : Array.Empty<object?>();
        return ("EVALSHA", new object?[] { sha, (long)keys.Length }.Concat(keys).Concat(args).ToArray());
    }
}

// ---------------------------------------------------------------------------
// SynapRPC transport — MessagePack over TCP with multiplexing
// ---------------------------------------------------------------------------
