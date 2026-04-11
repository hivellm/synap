"""Command mapper: translates dotted SDK commands to native wire commands.

Maps SDK command names (e.g. ``kv.set``) and their JSON payload dicts into
``(wire_command, [args])`` tuples suitable for SynapRPC or RESP3 transports.
Raises :exc:`~synap_sdk.exceptions.UnsupportedCommandError` for commands that
have no native mapping.
"""

from __future__ import annotations

import json
from typing import Any

from synap_sdk.exceptions import UnsupportedCommandError


def map_command(cmd: str, payload: dict[str, Any]) -> tuple[str, list[Any]]:
    """Translate a dotted SDK command and JSON payload to a native wire command.

    Args:
        cmd: The dotted SDK command name (e.g. ``"kv.set"``).
        payload: The command payload dict containing operation arguments.

    Returns:
        A ``(wire_command, args)`` tuple ready to send over the wire.

    Raises:
        UnsupportedCommandError: If ``cmd`` has no native mapping.
    """
    result = _map_command_inner(cmd, payload)
    if result is None:
        raise UnsupportedCommandError(cmd, "native")
    return result


def map_command_optional(cmd: str, payload: dict[str, Any]) -> tuple[str, list[Any]] | None:
    """Translate a dotted SDK command to a native wire command, returning None if unmapped.

    Args:
        cmd: The dotted SDK command name.
        payload: The command payload dict.

    Returns:
        A ``(wire_command, args)`` tuple, or ``None`` if no native mapping exists.
    """
    return _map_command_inner(cmd, payload)


def _map_command_inner(cmd: str, payload: dict[str, Any]) -> tuple[str, list[Any]] | None:  # noqa: C901, PLR0912
    """Internal implementation — returns None for unmapped commands."""
    key = payload.get("key", "")
    value = payload.get("value", "")
    field = payload.get("field", "")
    fields = payload.get("fields", {})
    ttl = payload.get("ttl")

    match cmd:
        # ── KV ──────────────────────────────────────────────────────────────
        case "kv.get":
            return "GET", [key]
        case "kv.set":
            if ttl is not None:
                return "SET", [key, value, "EX", ttl]
            return "SET", [key, value]
        case "kv.del":
            return "DEL", [key]
        case "kv.exists":
            return "EXISTS", [key]
        case "kv.expire":
            return "EXPIRE", [key, payload.get("seconds", 0)]
        case "kv.ttl":
            return "TTL", [key]
        case "kv.persist":
            return "PERSIST", [key]
        case "kv.incr":
            return "INCR", [key]
        case "kv.incrby":
            return "INCRBY", [key, payload.get("amount", 1)]
        case "kv.decr":
            return "DECR", [key]
        case "kv.decrby":
            return "DECRBY", [key, payload.get("amount", 1)]
        case "kv.append":
            return "APPEND", [key, value]
        case "kv.strlen":
            return "STRLEN", [key]
        case "kv.getset":
            return "GETSET", [key, value]
        case "kv.setnx":
            return "SETNX", [key, value]
        case "kv.scan":
            pattern = payload.get("pattern", "*")
            count = payload.get("count", 100)
            cursor = payload.get("cursor", 0)
            return "SCAN", [cursor, "MATCH", pattern, "COUNT", count]
        case "kv.keys":
            return "KEYS", [payload.get("pattern", "*")]
        case "kv.type":
            return "TYPE", [key]
        case "kv.rename":
            return "RENAME", [key, payload.get("new_key", "")]
        case "kv.copy":
            return "COPY", [key, payload.get("destination", "")]
        case "kv.dump":
            return "DUMP", [key]
        case "kv.object_encoding":
            return "OBJECT", ["ENCODING", key]
        case "kv.debug_sleep":
            return "DEBUG", ["SLEEP", payload.get("seconds", 0)]
        case "kv.stats":
            return "KVSTATS", []

        # ── Hash ─────────────────────────────────────────────────────────────
        case "hash.get":
            return "HGET", [key, field]
        case "hash.set":
            return "HSET", [key, field, value]
        case "hash.del":
            return "HDEL", [key, field]
        case "hash.exists":
            return "HEXISTS", [key, field]
        case "hash.getall":
            return "HGETALL", [key]
        case "hash.keys":
            return "HKEYS", [key]
        case "hash.values":
            return "HVALS", [key]
        case "hash.len":
            return "HLEN", [key]
        case "hash.mget":
            return "HMGET", [key, *payload.get("fields", [])]
        case "hash.mset":
            args: list[Any] = [key]
            for k, v in fields.items():
                args.extend([k, v])
            return "HMSET", args
        case "hash.incrby":
            return "HINCRBY", [key, field, payload.get("amount", 1)]
        case "hash.incrbyfloat":
            return "HINCRBYFLOAT", [key, field, payload.get("amount", 1.0)]
        case "hash.setnx":
            return "HSETNX", [key, field, value]

        # ── List ─────────────────────────────────────────────────────────────
        case "list.lpush":
            return "LPUSH", [key, value]
        case "list.rpush":
            return "RPUSH", [key, value]
        case "list.lpop":
            return "LPOP", [key]
        case "list.rpop":
            return "RPOP", [key]
        case "list.lrange":
            return "LRANGE", [key, payload.get("start", 0), payload.get("stop", -1)]
        case "list.llen":
            return "LLEN", [key]
        case "list.lindex":
            return "LINDEX", [key, payload.get("index", 0)]
        case "list.lset":
            return "LSET", [key, payload.get("index", 0), value]
        case "list.linsert":
            return "LINSERT", [
                key,
                payload.get("position", "BEFORE"),
                payload.get("pivot", ""),
                value,
            ]
        case "list.lrem":
            return "LREM", [key, payload.get("count", 0), value]
        case "list.ltrim":
            return "LTRIM", [key, payload.get("start", 0), payload.get("stop", -1)]
        case "list.lpos":
            return "LPOS", [key, value]
        case "list.lmove":
            return "LMOVE", [
                key,
                payload.get("destination", ""),
                payload.get("src", "LEFT"),
                payload.get("dst", "RIGHT"),
            ]

        # ── Set ──────────────────────────────────────────────────────────────
        case "set.add":
            return "SADD", [key, value]
        case "set.remove":
            return "SREM", [key, value]
        case "set.members":
            return "SMEMBERS", [key]
        case "set.ismember":
            return "SISMEMBER", [key, value]
        case "set.card":
            return "SCARD", [key]
        case "set.pop":
            return "SPOP", [key]
        case "set.randmember":
            return "SRANDMEMBER", [key, payload.get("count", 1)]
        case "set.union":
            return "SUNION", [key, *payload.get("keys", [])]
        case "set.inter":
            return "SINTER", [key, *payload.get("keys", [])]
        case "set.diff":
            return "SDIFF", [key, *payload.get("keys", [])]
        case "set.unionstore":
            return "SUNIONSTORE", [payload.get("destination", ""), key, *payload.get("keys", [])]
        case "set.interstore":
            return "SINTERSTORE", [payload.get("destination", ""), key, *payload.get("keys", [])]
        case "set.diffstore":
            return "SDIFFSTORE", [payload.get("destination", ""), key, *payload.get("keys", [])]
        case "set.move":
            return "SMOVE", [key, payload.get("destination", ""), value]

        # ── Sorted Set ───────────────────────────────────────────────────────
        case "sorted_set.add":
            return "ZADD", [key, payload.get("score", 0.0), value]
        case "sorted_set.score":
            return "ZSCORE", [key, value]
        case "sorted_set.rank":
            return "ZRANK", [key, value]
        case "sorted_set.revrank":
            return "ZREVRANK", [key, value]
        case "sorted_set.range":
            return "ZRANGE", [
                key,
                payload.get("start", 0),
                payload.get("stop", -1),
                "WITHSCORES",
            ]
        case "sorted_set.revrange":
            return "ZREVRANGE", [
                key,
                payload.get("start", 0),
                payload.get("stop", -1),
                "WITHSCORES",
            ]
        case "sorted_set.rangebyscore":
            return "ZRANGEBYSCORE", [
                key,
                payload.get("min", "-inf"),
                payload.get("max", "+inf"),
                "WITHSCORES",
            ]
        case "sorted_set.card":
            return "ZCARD", [key]
        case "sorted_set.count":
            return "ZCOUNT", [key, payload.get("min", "-inf"), payload.get("max", "+inf")]
        case "sorted_set.rem":
            return "ZREM", [key, value]
        case "sorted_set.incrby":
            return "ZINCRBY", [key, payload.get("increment", 1.0), value]
        case "sorted_set.remrangebyrank":
            return "ZREMRANGEBYRANK", [key, payload.get("start", 0), payload.get("stop", -1)]
        case "sorted_set.remrangebyscore":
            return "ZREMRANGEBYSCORE", [
                key,
                payload.get("min", "-inf"),
                payload.get("max", "+inf"),
            ]
        case "sorted_set.unionstore":
            return "ZUNIONSTORE", [
                payload.get("destination", ""),
                len(payload.get("keys", [])) + 1,
                key,
                *payload.get("keys", []),
            ]
        case "sorted_set.interstore":
            return "ZINTERSTORE", [
                payload.get("destination", ""),
                len(payload.get("keys", [])) + 1,
                key,
                *payload.get("keys", []),
            ]

        # ── Queue ─────────────────────────────────────────────────────────────
        case "queue.create":
            return "QCREATE", [
                payload.get("name", ""),
                str(payload.get("max_depth", 0)),
                str(payload.get("ack_deadline_secs", 30)),
            ]
        case "queue.delete":
            return "QDELETE", [payload.get("queue", "")]
        case "queue.list":
            return "QLIST", []
        case "queue.purge":
            return "QPURGE", [payload.get("queue", "")]
        case "queue.publish":
            pl = payload.get("payload", "")
            payload_arg = pl if isinstance(pl, (bytes, bytearray)) else str(pl)
            return "QPUBLISH", [
                payload.get("queue", ""),
                payload_arg,
                str(payload.get("priority", 0)),
                str(payload.get("max_retries", 3)),
            ]
        case "queue.consume":
            return "QCONSUME", [payload.get("queue", ""), payload.get("consumer_id", "")]
        case "queue.ack":
            return "QACK", [payload.get("queue", ""), payload.get("message_id", "")]
        case "queue.nack":
            return "QNACK", [
                payload.get("queue", ""),
                payload.get("message_id", ""),
                str(payload.get("requeue", True)),
            ]
        case "queue.stats":
            return "QSTATS", [payload.get("queue", "")]

        # ── Stream ────────────────────────────────────────────────────────────
        case "stream.create" | "stream.create_room":
            return "SCREATE", [payload.get("room", ""), str(payload.get("max_events", 0))]
        case "stream.delete" | "stream.delete_room":
            return "SDELETE", [payload.get("room", "")]
        case "stream.list" | "stream.list_rooms":
            return "SLIST", []
        case "stream.publish":
            return "SPUBLISH", [
                payload.get("room", ""),
                payload.get("event", ""),
                json.dumps(payload.get("data", {})),
            ]
        case "stream.consume" | "stream.read":
            return "SREAD", [
                payload.get("room", ""),
                payload.get("subscriber_id", payload.get("consumer_id", "")),
                str(payload.get("from_offset", payload.get("offset", 0))),
            ]
        case "stream.stats":
            return "SSTATS", [payload.get("room", "")]

        # ── Pub/Sub ───────────────────────────────────────────────────────────
        case "pubsub.publish":
            pl = payload.get("payload", payload.get("data", ""))
            return "PUBLISH", [payload.get("topic", ""), json.dumps(pl)]
        case "pubsub.subscribe":
            topics: list[Any] = payload.get("topics", [])
            return "SUBSCRIBE", [*topics]
        case "pubsub.unsubscribe":
            topics = payload.get("topics", [])
            return "UNSUBSCRIBE", [payload.get("subscriber_id", ""), *topics]
        case "pubsub.topics" | "pubsub.list":
            return "TOPICS", []

        # ── Transactions ──────────────────────────────────────────────────────
        case "transaction.multi":
            return "MULTI", [payload.get("client_id", "")]
        case "transaction.exec":
            return "EXEC", [payload.get("client_id", "")]
        case "transaction.discard":
            return "DISCARD", [payload.get("client_id", "")]
        case "transaction.watch":
            keys: list[Any] = payload.get("keys", [])
            return "WATCH", [payload.get("client_id", ""), *keys]
        case "transaction.unwatch":
            return "UNWATCH", [payload.get("client_id", "")]

        # ── Scripts ───────────────────────────────────────────────────────────
        case "script.eval":
            s_keys: list[Any] = payload.get("keys", [])
            s_args: list[Any] = payload.get("args", [])
            return "EVAL", [
                payload.get("script", ""),
                str(len(s_keys)),
                *s_keys,
                *[str(a) for a in s_args],
            ]
        case "script.evalsha":
            s_keys = payload.get("keys", [])
            s_args = payload.get("args", [])
            return "EVALSHA", [
                payload.get("sha1", ""),
                str(len(s_keys)),
                *s_keys,
                *[str(a) for a in s_args],
            ]
        case "script.load":
            return "SCRIPT", ["LOAD", payload.get("script", "")]
        case "script.exists":
            hashes: list[Any] = payload.get("hashes", [])
            return "SCRIPT", ["EXISTS", *hashes]
        case "script.flush":
            return "SCRIPT", ["FLUSH"]
        case "script.kill":
            return "SCRIPT", ["KILL"]

        # ── HyperLogLog ───────────────────────────────────────────────────────
        case "hyperloglog.pfadd":
            elements: list[Any] = payload.get("elements", [])
            return "PFADD", [key, *elements]
        case "hyperloglog.pfcount":
            keys_arg: list[Any] = payload.get("keys", [key] if key else [])
            return "PFCOUNT", [*keys_arg]
        case "hyperloglog.pfmerge":
            sources: list[Any] = payload.get("sources", [])
            return "PFMERGE", [payload.get("destination", ""), *sources]
        case "hyperloglog.stats":
            return "HLLSTATS", []

        # ── Geospatial ────────────────────────────────────────────────────────
        case "geospatial.geoadd":
            locations: list[Any] = payload.get("locations", [])
            geo_args: list[Any] = [key]
            for loc in locations:
                geo_args.extend([
                    str(loc.get("lon", 0)),
                    str(loc.get("lat", 0)),
                    str(loc.get("member", "")),
                ])
            return "GEOADD", geo_args
        case "geospatial.geopos":
            members: list[Any] = payload.get("members", [])
            return "GEOPOS", [key, *members]
        case "geospatial.geodist":
            return "GEODIST", [
                key,
                payload.get("member1", ""),
                payload.get("member2", ""),
                payload.get("unit", "m"),
            ]
        case "geospatial.geohash":
            members = payload.get("members", [])
            return "GEOHASH", [key, *members]
        case "geospatial.georadius":
            return "GEORADIUS", [
                key,
                str(payload.get("longitude", 0)),
                str(payload.get("latitude", 0)),
                str(payload.get("radius", 0)),
                payload.get("unit", "m"),
                "WITHCOORD",
                "WITHDIST",
            ]
        case "geospatial.georadiusbymember":
            return "GEORADIUSBYMEMBER", [
                key,
                payload.get("member", ""),
                str(payload.get("radius", 0)),
                payload.get("unit", "m"),
                "WITHCOORD",
                "WITHDIST",
            ]
        case "geospatial.geosearch":
            g_args: list[Any] = [key]
            if "member" in payload:
                g_args.extend(["FROMMEMBER", payload["member"]])
            elif "longitude" in payload:
                g_args.extend([
                    "FROMLONLAT",
                    str(payload["longitude"]),
                    str(payload["latitude"]),
                ])
            g_args.extend(["BYRADIUS", str(payload.get("radius", 0)), payload.get("unit", "m")])
            g_args.extend(["WITHCOORD", "WITHDIST"])
            return "GEOSEARCH", g_args
        case "geospatial.stats":
            return "GEOINFO", []

        case _:
            return None


def map_response(cmd: str, raw: Any) -> dict[str, Any]:  # noqa: ANN401
    """Convert a raw wire response to the JSON shape each SDK module expects.

    Args:
        cmd: The dotted SDK command name (e.g. ``"kv.get"``).
        raw: The raw value returned by the transport layer.

    Returns:
        A dict shaped exactly as the HTTP REST response payload for ``cmd``.
    """
    match cmd:
        case "kv.get":
            return {"value": raw}
        case "kv.set" | "kv.setnx" | "kv.getset":
            if cmd == "kv.getset":
                return {"old_value": raw}
            return {"success": raw == "OK" or raw is True}
        case "kv.del":
            return {"deleted": bool(raw) if not isinstance(raw, bool) else raw}
        case "kv.exists":
            return {"exists": bool(raw) if isinstance(raw, int) else raw}
        case "kv.expire" | "kv.persist":
            return {"success": bool(raw)}
        case "kv.ttl":
            return {"ttl": raw}
        case "kv.incr" | "kv.incrby" | "kv.decr" | "kv.decrby":
            return {"value": raw}
        case "kv.append" | "kv.strlen":
            return {"length": raw}
        case "kv.scan":
            if isinstance(raw, (list, tuple)) and len(raw) >= 2:
                return {"cursor": raw[0], "keys": list(raw[1])}
            return {"cursor": 0, "keys": []}
        case "kv.keys":
            return {"keys": list(raw) if raw else []}
        case "kv.type":
            return {"type": raw}
        case "kv.rename" | "kv.copy":
            return {"success": raw == "OK" or raw is True}
        case "kv.stats":
            return raw if isinstance(raw, dict) else {}
        case "hash.get":
            return {"value": raw}
        case "hash.set" | "hash.setnx":
            return {"success": bool(raw)}
        case "hash.del":
            return {"deleted": bool(raw)}
        case "hash.exists":
            return {"exists": bool(raw)}
        case "hash.getall":
            if isinstance(raw, (list, tuple)):
                pairs = list(raw)
                flds: dict[str, Any] = {}
                for i in range(0, len(pairs) - 1, 2):
                    flds[str(pairs[i])] = pairs[i + 1]
                return {"fields": flds}
            if isinstance(raw, dict):
                return {"fields": raw}
            return {"fields": {}}
        case "hash.keys":
            return {"keys": list(raw) if raw else []}
        case "hash.values":
            return {"values": list(raw) if raw else []}
        case "hash.len":
            return {"length": raw}
        case "hash.mget":
            return {"values": list(raw) if raw else []}
        case "hash.mset":
            return {"success": raw == "OK" or raw is True}
        case "hash.incrby" | "hash.incrbyfloat":
            return {"value": raw}
        case "list.lpush" | "list.rpush":
            return {"length": raw}
        case "list.lpop" | "list.rpop":
            return {"value": raw}
        case "list.lrange":
            return {"values": list(raw) if raw else []}
        case "list.llen":
            return {"length": raw}
        case "list.lindex":
            return {"value": raw}
        case "list.lset" | "list.ltrim":
            return {"success": raw == "OK" or raw is True}
        case "list.linsert":
            return {"length": raw}
        case "list.lrem":
            return {"removed": raw}
        case "list.lpos":
            return {"index": raw}
        case "list.lmove":
            return {"value": raw}
        case "set.add":
            return {"added": raw}
        case "set.remove":
            return {"removed": raw}
        case "set.members":
            return {"members": list(raw) if raw else []}
        case "set.ismember":
            return {"is_member": bool(raw)}
        case "set.card":
            return {"cardinality": raw}
        case "set.pop":
            return {"value": raw}
        case "set.randmember":
            return {"members": [raw] if not isinstance(raw, list) else raw}
        case "set.union" | "set.inter" | "set.diff":
            return {"members": list(raw) if raw else []}
        case "set.unionstore" | "set.interstore" | "set.diffstore":
            return {"count": raw}
        case "set.move":
            return {"success": bool(raw)}
        case "sorted_set.add":
            return {"added": raw}
        case "sorted_set.rem":
            return {"removed": raw}
        case "sorted_set.score":
            return {"score": float(raw) if raw is not None else None}
        case "sorted_set.rank" | "sorted_set.revrank":
            return {"rank": raw}
        case "sorted_set.range" | "sorted_set.revrange" | "sorted_set.rangebyscore":
            if isinstance(raw, (list, tuple)):
                items = list(raw)
                members = []
                for i in range(0, len(items) - 1, 2):
                    members.append({"member": str(items[i]), "score": float(items[i + 1])})
                return {"members": members}
            return {"members": []}
        case "sorted_set.card":
            return {"cardinality": raw}
        case "sorted_set.count":
            return {"count": raw}
        case "sorted_set.incrby":
            return {"score": float(raw)}
        case "sorted_set.remrangebyrank" | "sorted_set.remrangebyscore":
            return {"removed": raw}
        case "sorted_set.unionstore" | "sorted_set.interstore":
            return {"count": raw}

        # ── Queue ─────────────────────────────────────────────────────────────
        case "queue.create" | "queue.delete" | "queue.purge":
            return {}
        case "queue.list":
            if isinstance(raw, list):
                return {"queues": raw}
            return {"queues": [] if raw is None else [raw]}
        case "queue.publish":
            if isinstance(raw, dict) and "message_id" in raw:
                return raw
            return {"message_id": str(raw or "")}
        case "queue.consume":
            return raw if isinstance(raw, dict) else {}
        case "queue.ack" | "queue.nack":
            return {"success": bool(raw)}
        case "queue.stats":
            return raw if isinstance(raw, dict) else {}

        # ── Stream ────────────────────────────────────────────────────────────
        case "stream.create" | "stream.create_room" | "stream.delete" | "stream.delete_room":
            return {}
        case "stream.list" | "stream.list_rooms":
            if isinstance(raw, list):
                return {"rooms": raw}
            return {"rooms": [] if raw is None else [raw]}
        case "stream.publish":
            if isinstance(raw, dict) and "offset" in raw:
                return raw
            return {"offset": int(raw or 0)}
        case "stream.consume" | "stream.read":
            if isinstance(raw, dict) and "events" in raw:
                return raw
            if isinstance(raw, list):
                return {"events": raw}
            return {"events": []}
        case "stream.stats":
            return raw if isinstance(raw, dict) else {}

        # ── Pub/Sub ───────────────────────────────────────────────────────────
        case "pubsub.publish":
            if isinstance(raw, dict):
                return raw
            return {"subscribers_matched": int(raw or 0)}
        case "pubsub.subscribe" | "pubsub.unsubscribe":
            return {"success": True}
        case "pubsub.topics" | "pubsub.list":
            if isinstance(raw, list):
                return {"topics": raw}
            return {"topics": [] if raw is None else [raw]}

        # ── Transactions ──────────────────────────────────────────────────────
        case "transaction.multi" | "transaction.discard" | "transaction.watch" | "transaction.unwatch":
            if isinstance(raw, dict):
                return raw
            return {"success": raw == "OK" or raw is True, "message": str(raw or "")}
        case "transaction.exec":
            if isinstance(raw, dict):
                return raw
            if isinstance(raw, list):
                return {"results": raw}
            return {"results": []}

        # ── Scripts ───────────────────────────────────────────────────────────
        case "script.eval" | "script.evalsha":
            if isinstance(raw, dict):
                return raw
            return {"result": raw, "sha1": ""}
        case "script.load":
            return {"sha1": str(raw or "")}
        case "script.exists":
            if isinstance(raw, list):
                return {"exists": [bool(x) for x in raw]}
            return {"exists": [bool(raw)]}
        case "script.flush":
            return {"cleared": 1 if raw == "OK" or raw is True else 0}
        case "script.kill":
            return {"terminated": raw == "OK" or raw is True}

        # ── HyperLogLog ───────────────────────────────────────────────────────
        case "hyperloglog.pfadd":
            return {"updated": bool(raw)}
        case "hyperloglog.pfcount":
            return {"count": int(raw or 0)}
        case "hyperloglog.pfmerge":
            return {"success": raw == "OK" or raw is True}
        case "hyperloglog.stats":
            return raw if isinstance(raw, dict) else {}

        # ── Geospatial ────────────────────────────────────────────────────────
        case "geospatial.geoadd":
            return {"added": int(raw or 0)}
        case "geospatial.geopos":
            if isinstance(raw, list):
                return {"positions": raw}
            return {"positions": []}
        case "geospatial.geodist":
            return {"distance": float(raw) if raw is not None else None}
        case "geospatial.geohash":
            if isinstance(raw, list):
                return {"hashes": raw}
            return {"hashes": []}
        case "geospatial.georadius" | "geospatial.georadiusbymember" | "geospatial.geosearch":
            if isinstance(raw, list):
                return {"results": raw}
            return {"results": []}
        case "geospatial.stats":
            return raw if isinstance(raw, dict) else {}

        case _:
            if isinstance(raw, dict):
                return raw
            return {"result": raw}


__all__ = [
    "map_command",
    "map_command_optional",
    "map_response",
]
