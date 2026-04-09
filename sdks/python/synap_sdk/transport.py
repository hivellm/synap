"""Binary TCP transports for Synap SDK.

Implements SynapRPC (MessagePack-framed binary) and RESP3 (Redis-compatible text)
alongside the original HTTP transport.

Wire encoding mirrors Rust's rmp_serde externally-tagged enum format:
  - Null       → bare msgpack string "Null"
  - Str(s)     → {"Str": s}
  - Int(n)     → {"Int": n}
  - Float(f)   → {"Float": f}
  - Bool(b)    → {"Bool": b}
  - Bytes(b)   → {"Bytes": b}
Structs (Request, Response) are encoded as msgpack arrays.
"""

from __future__ import annotations

import asyncio
import json
import struct
from typing import Any, Literal

import msgpack

# ── Transport mode ─────────────────────────────────────────────────────────────

TransportMode = Literal["synaprpc", "resp3", "http"]

# ── Wire value helpers ─────────────────────────────────────────────────────────


def _to_wire(v: Any) -> Any:  # noqa: ANN401
    """Wrap a Python value in the externally-tagged WireValue envelope."""
    if v is None:
        return "Null"
    if isinstance(v, bool):
        return {"Bool": v}
    if isinstance(v, int):
        return {"Int": v}
    if isinstance(v, float):
        return {"Float": v}
    if isinstance(v, (bytes, bytearray)):
        return {"Bytes": bytes(v)}
    # Default: stringify
    return {"Str": str(v)}


def _from_wire(wire: Any) -> Any:  # noqa: ANN401
    """Unwrap a WireValue envelope back to a Python value."""
    if wire == "Null" or wire is None:
        return None
    if isinstance(wire, dict):
        if "Str" in wire:
            return wire["Str"]
        if "Int" in wire:
            return wire["Int"]
        if "Float" in wire:
            return wire["Float"]
        if "Bool" in wire:
            return wire["Bool"]
        if "Bytes" in wire:
            return wire["Bytes"]
        if "Array" in wire:
            return [_from_wire(x) for x in wire["Array"]]
        if "Map" in wire:
            return {str(_from_wire(k)): _from_wire(v) for k, v in wire["Map"]}
    return wire


# ── Command → native protocol mapping ─────────────────────────────────────────


def _map_command(cmd: str, payload: dict[str, Any]) -> tuple[str, list[Any]] | None:
    """Translate a dotted SDK command + JSON payload into a native wire command + args.

    Returns None for commands that have no native mapping (fall back to HTTP).
    """
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
            keys = payload.get("keys", [])
            args: list[Any] = payload.get("args", [])
            return "EVAL", [payload.get("script", ""), str(len(keys)), *keys, *[str(a) for a in args]]
        case "script.evalsha":
            keys = payload.get("keys", [])
            args = payload.get("args", [])
            return "EVALSHA", [payload.get("sha1", ""), str(len(keys)), *keys, *[str(a) for a in args]]
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
                geo_args.extend([str(loc.get("lon", 0)), str(loc.get("lat", 0)), str(loc.get("member", ""))])
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
            geo_args = [key]
            if "member" in payload:
                geo_args.extend(["FROMMEMBER", payload["member"]])
            elif "longitude" in payload:
                geo_args.extend(["FROMLONLAT", str(payload["longitude"]), str(payload["latitude"])])
            geo_args.extend(["BYRADIUS", str(payload.get("radius", 0)), payload.get("unit", "m")])
            geo_args.extend(["WITHCOORD", "WITHDIST"])
            return "GEOSEARCH", geo_args
        case "geospatial.stats":
            return "GEOINFO", []

        # ── kv.stats ──────────────────────────────────────────────────────────
        case "kv.stats":
            return "KVSTATS", []

        case _:
            return None


def _map_response(cmd: str, raw: Any) -> dict[str, Any]:  # noqa: ANN401
    """Convert a raw wire response to the JSON shape that SDK managers expect."""
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

        # ── kv.stats ──────────────────────────────────────────────────────────
        case "kv.stats":
            return raw if isinstance(raw, dict) else {}

        case _:
            if isinstance(raw, dict):
                return raw
            return {"result": raw}


# ── SynapRPC transport ─────────────────────────────────────────────────────────


class SynapRpcTransport:
    """Persistent async TCP connection to the SynapRPC listener.

    Requests are multiplexed by ID; responses are dispatched to waiting coroutines.
    """

    def __init__(self, host: str, port: int, timeout: float) -> None:
        self._host = host
        self._port = port
        self._timeout = timeout
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None
        self._next_id = 1
        self._pending: dict[int, asyncio.Future[Any]] = {}
        self._recv_task: asyncio.Task[None] | None = None
        self._lock = asyncio.Lock()

    async def _connect(self) -> None:
        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(self._host, self._port),
            timeout=self._timeout,
        )
        self._reader = reader
        self._writer = writer
        loop = asyncio.get_event_loop()
        self._recv_task = loop.create_task(self._recv_loop())

    async def _recv_loop(self) -> None:
        assert self._reader is not None  # noqa: S101
        try:
            while True:
                len_bytes = await self._reader.readexactly(4)
                frame_len = struct.unpack_from("<I", len_bytes)[0]
                body = await self._reader.readexactly(frame_len)
                decoded = msgpack.unpackb(body, raw=False)
                req_id, result_env = decoded[0], decoded[1]
                fut = self._pending.pop(req_id, None)
                if fut is None:
                    continue
                if "Ok" in result_env:
                    fut.set_result(_from_wire(result_env["Ok"]))
                else:
                    fut.set_exception(Exception(str(result_env.get("Err", "unknown error"))))
        except Exception as exc:  # noqa: BLE001
            # Propagate error to all waiters.
            for fut in self._pending.values():
                if not fut.done():
                    fut.set_exception(exc)
            self._pending.clear()
            self._reader = None
            self._writer = None

    async def _ensure_connected(self) -> None:
        async with self._lock:
            if self._writer is None or self._writer.is_closing():
                await self._connect()

    async def execute(self, cmd: str, args: list[Any]) -> Any:  # noqa: ANN401
        """Send a command and await its response."""
        await self._ensure_connected()
        assert self._writer is not None  # noqa: S101

        loop = asyncio.get_event_loop()
        req_id = self._next_id
        self._next_id += 1

        wire_args = [_to_wire(a) for a in args]
        body = msgpack.packb([req_id, cmd.upper(), wire_args], use_bin_type=True)
        frame = struct.pack("<I", len(body)) + body

        fut: asyncio.Future[Any] = loop.create_future()
        self._pending[req_id] = fut
        self._writer.write(frame)
        await self._writer.drain()

        return await asyncio.wait_for(fut, timeout=self._timeout)

    async def subscribe_push(
        self,
        topics: list[str],
        on_message: Any,  # Callable[[dict], None]
    ) -> tuple[str, Any]:
        """Open a dedicated push connection for pub/sub subscriptions.

        Sends a SUBSCRIBE command on a fresh TCP socket, reads the initial
        response to extract the subscriber_id, then starts a background task
        that reads incoming push frames (id == 0xFFFFFFFF) and calls
        ``on_message`` with ``{topic, payload, id, timestamp}`` dicts.

        Returns:
            A ``(subscriber_id, cancel_fn)`` tuple. Call ``cancel_fn()`` to stop.
        """
        PUSH_ID = 0xFFFF_FFFF

        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(self._host, self._port),
            timeout=self._timeout,
        )

        # Send SUBSCRIBE frame
        wire_args = [_to_wire(t) for t in topics]
        body = msgpack.packb([1, "SUBSCRIBE", wire_args], use_bin_type=True)
        frame = struct.pack("<I", len(body)) + body
        writer.write(frame)
        await writer.drain()

        # Read the initial SUBSCRIBE response (not a push frame)
        len_bytes = await asyncio.wait_for(reader.readexactly(4), timeout=self._timeout)
        frame_len = struct.unpack_from("<I", len_bytes)[0]
        init_body = await asyncio.wait_for(reader.readexactly(frame_len), timeout=self._timeout)
        init_decoded = msgpack.unpackb(init_body, raw=False)
        subscriber_id = ""
        if len(init_decoded) >= 2:
            result_env = init_decoded[1]
            if isinstance(result_env, dict) and "Ok" in result_env:
                val = _from_wire(result_env["Ok"])
                if isinstance(val, dict) and "subscriber_id" in val:
                    subscriber_id = str(val["subscriber_id"])

        cancelled = False

        async def _push_loop() -> None:
            try:
                while not cancelled:
                    len_b = await asyncio.wait_for(reader.readexactly(4), timeout=self._timeout)
                    f_len = struct.unpack_from("<I", len_b)[0]
                    f_body = await asyncio.wait_for(reader.readexactly(f_len), timeout=self._timeout)
                    decoded = msgpack.unpackb(f_body, raw=False)
                    if len(decoded) < 2:
                        continue
                    frame_id, result_env = decoded[0], decoded[1]
                    if frame_id != PUSH_ID:
                        continue
                    if isinstance(result_env, dict) and "Ok" in result_env:
                        val = _from_wire(result_env["Ok"])
                        if isinstance(val, dict):
                            on_message({
                                "topic": str(val.get("topic", "")),
                                "payload": val.get("payload"),
                                "id": str(val.get("id", "")),
                                "timestamp": int(val.get("timestamp", 0)),
                            })
            except (asyncio.CancelledError, Exception):
                pass
            finally:
                writer.close()

        loop = asyncio.get_event_loop()
        push_task = loop.create_task(_push_loop())

        def cancel() -> None:
            nonlocal cancelled
            cancelled = True
            push_task.cancel()

        return subscriber_id, cancel

    async def close(self) -> None:
        """Close the TCP connection."""
        if self._recv_task:
            self._recv_task.cancel()
            try:
                await self._recv_task
            except (asyncio.CancelledError, Exception):
                pass
            self._recv_task = None
        if self._writer:
            self._writer.close()
            try:
                await self._writer.wait_closed()
            except Exception:  # noqa: BLE001
                pass
            self._writer = None
        self._reader = None


# ── RESP3 transport ────────────────────────────────────────────────────────────


class Resp3Transport:
    """Persistent async TCP connection to a RESP3 (Redis-compatible) listener.

    Requests are serialised (one at a time) with a per-request lock to keep
    the parser simple without buffering complexity.
    """

    def __init__(self, host: str, port: int, timeout: float) -> None:
        self._host = host
        self._port = port
        self._timeout = timeout
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None
        self._req_lock = asyncio.Lock()
        self._conn_lock = asyncio.Lock()

    async def _connect(self) -> None:
        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(self._host, self._port),
            timeout=self._timeout,
        )
        self._reader = reader
        self._writer = writer
        # Send HELLO 3 to enable RESP3
        hello_cmd = b"*2\r\n$5\r\nHELLO\r\n$1\r\n3\r\n"
        writer.write(hello_cmd)
        await writer.drain()
        # Drain the HELLO response (a map, starts with %)
        await self._read_value()

    async def _ensure_connected(self) -> None:
        async with self._conn_lock:
            if self._writer is None or self._writer.is_closing():
                await self._connect()

    async def _read_line(self) -> str:
        assert self._reader is not None  # noqa: S101
        line = await asyncio.wait_for(self._reader.readline(), timeout=self._timeout)
        return line.decode("utf-8").rstrip("\r\n")

    async def _read_value(self) -> Any:  # noqa: ANN401
        """Recursively parse one RESP3 value from the stream."""
        assert self._reader is not None  # noqa: S101
        line = await self._read_line()
        if not line:
            return None
        prefix, rest = line[0], line[1:]
        match prefix:
            case "+":
                return rest
            case "-":
                raise Exception(rest)  # noqa: TRY002
            case ":":
                return int(rest)
            case ",":
                return float(rest)
            case "#":
                return rest.lower() == "t"
            case "_":
                return None
            case "$":
                # Bulk string: next line is the data
                length = int(rest)
                if length == -1:
                    return None
                data = await asyncio.wait_for(
                    self._reader.readexactly(length + 2), timeout=self._timeout
                )
                return data[:-2].decode("utf-8")
            case "*":
                count = int(rest)
                if count == -1:
                    return None
                return [await self._read_value() for _ in range(count)]
            case "%":
                # Map type (RESP3)
                count = int(rest)
                result: dict[str, Any] = {}
                for _ in range(count):
                    k = await self._read_value()
                    v = await self._read_value()
                    result[str(k)] = v
                return result
            case "~":
                # Set type (RESP3)
                count = int(rest)
                return [await self._read_value() for _ in range(count)]
            case _:
                return rest

    def _encode_command(self, cmd: str, args: list[Any]) -> bytes:
        """Encode a command as a RESP3 array."""
        parts = [cmd, *[str(a) for a in args]]
        out = [f"*{len(parts)}\r\n".encode()]
        for part in parts:
            enc = str(part).encode("utf-8")
            out.append(f"${len(enc)}\r\n".encode())
            out.append(enc)
            out.append(b"\r\n")
        return b"".join(out)

    async def execute(self, cmd: str, args: list[Any]) -> Any:  # noqa: ANN401
        """Send a command and return the parsed response."""
        await self._ensure_connected()
        async with self._req_lock:
            assert self._writer is not None  # noqa: S101
            frame = self._encode_command(cmd, args)
            self._writer.write(frame)
            await self._writer.drain()
            return await self._read_value()

    async def close(self) -> None:
        """Close the TCP connection."""
        if self._writer:
            self._writer.close()
            try:
                await self._writer.wait_closed()
            except Exception:  # noqa: BLE001
                pass
            self._writer = None
        self._reader = None


__all__ = [
    "TransportMode",
    "SynapRpcTransport",
    "Resp3Transport",
    "map_command",
    "map_response",
]

# Public aliases used by client.py
map_command = _map_command
map_response = _map_response
