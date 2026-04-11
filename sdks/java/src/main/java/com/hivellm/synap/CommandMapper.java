package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

/**
 * Maps SDK dot-notation operations and their payloads to native server commands
 * and argument lists, and maps raw server responses back to the HTTP response shape.
 *
 * <p>This class is stateless and all methods are static; it is package-private.</p>
 */
final class CommandMapper {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    private CommandMapper() {}

    // ── Request mapping ────────────────────────────────────────────────────────

    /**
     * Maps a dot-notation operation + payload to a native {@code [COMMAND, arg…]} array.
     *
     * @param operation the dot-notation command (e.g. {@code "kv.set"})
     * @param payload   the SDK payload map (may be null)
     * @return a two-element array: {@code [0]} = command string, {@code [1]} = Object[] args
     * @throws UnsupportedCommandException if the operation has no mapping
     */
    static Object[] mapCommand(String operation, Map<String, Object> payload) {
        Map<String, Object> p = payload != null ? payload : Map.of();
        String key = p.containsKey("key") ? str(p.get("key")) : "";

        return switch (operation) {

            // ---- KV ----
            case "kv.get"    -> cmd("GET",  key);
            case "kv.delete" -> cmd("DEL",  key);
            case "kv.exists" -> cmd("EXISTS", key);
            case "kv.persist"-> cmd("PERSIST", key);
            case "kv.set"    -> buildKvSet(key, p);
            case "kv.incr"   -> p.containsKey("delta")
                                    ? cmd("INCRBY", key, p.get("delta"))
                                    : cmd("INCR", key);
            case "kv.decr"   -> p.containsKey("delta")
                                    ? cmd("DECRBY", key, p.get("delta"))
                                    : cmd("DECR", key);
            case "kv.dbsize" -> cmd("DBSIZE");
            case "kv.flushdb"-> cmd("FLUSHDB");
            case "kv.keys"   -> p.containsKey("pattern")
                                    ? cmd("KEYS", p.get("pattern"))
                                    : cmd("KEYS", "*");
            case "kv.mset"   -> buildMset(p);
            case "kv.mget"   -> buildMget(p);
            case "kv.ttl"    -> cmd("TTL", key);
            case "kv.expire" -> cmd("EXPIRE", key, p.get("ttl"));

            // ---- Hash ----
            case "hash.set"    -> cmd("HSET",    key, p.get("field"), p.get("value"));
            case "hash.get"    -> cmd("HGET",    key, p.get("field"));
            case "hash.getall" -> cmd("HGETALL", key);
            case "hash.del"    -> buildHdel(key, p);
            case "hash.exists" -> cmd("HEXISTS",  key, p.get("field"));

            // ---- List ----
            case "list.lpush" -> buildListPush("LPUSH", key, p);
            case "list.rpush" -> buildListPush("RPUSH", key, p);
            case "list.lpop"  -> buildListPop("LPOP",  key, p);
            case "list.rpop"  -> buildListPop("RPOP",  key, p);
            case "list.range" -> cmd("LRANGE", key,
                                     p.getOrDefault("start", 0L),
                                     p.getOrDefault("stop",  -1L));
            case "list.len"   -> cmd("LLEN", key);

            // ---- Set ----
            case "set.add"      -> buildSetAdd(key, p);
            case "set.members"  -> cmd("SMEMBERS",  key);
            case "set.ismember" -> cmd("SISMEMBER", key, p.get("member"));
            case "set.rem"      -> buildSetRem(key, p);
            case "set.card"     -> cmd("SCARD", key);

            // ---- Queue ----
            case "queue.create"  -> cmd("QCREATE",
                                        p.get("name"),
                                        p.getOrDefault("max_depth", 0L),
                                        p.getOrDefault("ack_deadline_secs", 0L));
            case "queue.delete"  -> cmd("QDELETE",  p.get("queue"));
            case "queue.publish" -> buildQPublish(p);
            case "queue.consume" -> cmd("QCONSUME", p.get("queue"), p.get("consumer_id"));
            case "queue.ack"     -> cmd("QACK",     p.get("queue"), p.get("message_id"));
            case "queue.nack"    -> cmd("QNACK",    p.get("queue"), p.get("message_id"),
                                        p.getOrDefault("delay_secs", 0L));
            case "queue.list"    -> cmd("QLIST");
            case "queue.stats"   -> cmd("QSTATS",   p.get("queue"));

            // ---- Stream ----
            case "stream.create"  -> cmd("SCREATE",  p.get("room"));
            case "stream.delete"  -> cmd("SDELETE",  p.get("room"));
            case "stream.publish" -> buildSPublish(p);
            case "stream.consume" -> cmd("SREAD",
                                         p.get("room"),
                                         p.getOrDefault("subscriber_id", "sdk-reader"),
                                         p.getOrDefault("from_offset", 0L));
            case "stream.list"    -> cmd("SLIST");
            case "stream.stats"   -> cmd("SSTATS",   p.get("room"));

            // ---- Pub/Sub ----
            case "pubsub.publish"     -> buildPPublish(p);
            case "pubsub.subscribe"   -> buildPSubscribe(p);
            case "pubsub.unsubscribe" -> buildPUnsubscribe(p);
            case "pubsub.topics"      -> cmd("TOPICS");

            default -> throw new UnsupportedCommandException(operation, "SynapRPC/RESP3");
        };
    }

    // ── Response mapping ───────────────────────────────────────────────────────

    /**
     * Maps the raw value returned by the native transport back to the HTTP-style
     * JSON payload shape expected by the manager classes.
     *
     * @param operation the original dot-notation operation
     * @param raw       the raw value decoded from the wire
     * @return a {@link JsonNode} mirroring the HTTP payload shape
     */
    static JsonNode mapResponse(String operation, Object raw) {
        ObjectMapper m = MAPPER;
        return switch (operation) {

            // ---- KV ----
            case "kv.get"    -> obj(m, "value", raw);
            case "kv.exists" -> obj(m, "exists", isNonZero(raw));
            case "kv.incr", "kv.decr" -> obj(m, "value", raw);
            case "kv.delete" -> obj(m, "deleted", isNonZero(raw));
            case "kv.persist", "kv.flushdb", "kv.expire" -> obj(m, "success", isOk(raw));
            case "kv.dbsize" -> obj(m, "count", raw);
            case "kv.keys"   -> obj(m, "keys",  toArrayNode(m, raw));
            case "kv.mget"   -> obj(m, "values", toArrayNode(m, raw));
            case "kv.mset"   -> obj(m, "success", isOk(raw));
            case "kv.ttl"    -> obj(m, "ttl", raw);
            case "kv.set"    -> m.createObjectNode();

            // ---- Hash ----
            case "hash.set"    -> obj(m, "created", isNonZero(raw));
            case "hash.get"    -> obj(m, "value",   raw);
            case "hash.getall" -> buildHgetallResponse(m, raw);
            case "hash.del"    -> obj(m, "removed", raw);
            case "hash.exists" -> obj(m, "exists",  isNonZero(raw));

            // ---- List ----
            case "list.lpush", "list.rpush" -> obj(m, "len", raw);
            case "list.lpop", "list.rpop"   -> obj(m, "values", toArrayNode(m, raw));
            case "list.range"               -> obj(m, "values", toArrayNode(m, raw));
            case "list.len"                 -> obj(m, "len", raw);

            // ---- Set ----
            case "set.add"      -> obj(m, "added",    raw);
            case "set.members"  -> obj(m, "members",  toArrayNode(m, raw));
            case "set.ismember" -> obj(m, "is_member", isNonZero(raw));
            case "set.rem"      -> obj(m, "removed",   raw);
            case "set.card"     -> obj(m, "count",     raw);

            // ---- Queue ----
            case "queue.create"  -> obj(m, "success",    isOk(raw));
            case "queue.delete"  -> obj(m, "success",    isOk(raw));
            case "queue.publish" -> obj(m, "message_id", raw != null ? raw.toString() : "");
            case "queue.consume" -> buildQueueConsumeResponse(m, raw);
            case "queue.ack"     -> obj(m, "success", true);
            case "queue.nack"    -> obj(m, "success", true);
            case "queue.list"    -> obj(m, "queues",  toArrayNode(m, raw));
            case "queue.stats"   -> rawToNode(m, raw);

            // ---- Stream ----
            case "stream.create"  -> obj(m, "success", isOk(raw));
            case "stream.delete"  -> obj(m, "success", isOk(raw));
            case "stream.publish" -> obj(m, "offset",  toLong(raw));
            case "stream.consume" -> obj(m, "events",  toArrayNode(m, raw));
            case "stream.list"    -> obj(m, "rooms",   toArrayNode(m, raw));
            case "stream.stats"   -> rawToNode(m, raw);

            // ---- Pub/Sub ----
            case "pubsub.publish"     -> obj(m, "subscribers_matched", toLong(raw));
            case "pubsub.subscribe"   -> obj(m, "success", true);
            case "pubsub.unsubscribe" -> obj(m, "success", true);
            case "pubsub.topics"      -> obj(m, "topics",  toArrayNode(m, raw));

            default -> m.createObjectNode();
        };
    }

    // ── Request builder helpers ────────────────────────────────────────────────

    /** Returns {@code [commandName, arg0, arg1, …]} as an Object array. */
    private static Object[] cmd(String command, Object... args) {
        Object[] result = new Object[1 + args.length];
        result[0] = command;
        System.arraycopy(args, 0, result, 1, args.length);
        return result;
    }

    /** {@code SET key value [EX ttl]} */
    private static Object[] buildKvSet(String key, Map<String, Object> p) {
        Object value = p.get("value");
        Object ttl   = p.get("ttl");
        if (ttl != null) {
            return cmd("SET", key, value, "EX", ttl);
        }
        return cmd("SET", key, value);
    }

    /** {@code MSET k1 v1 k2 v2 …} */
    private static Object[] buildMset(Map<String, Object> p) {
        @SuppressWarnings("unchecked")
        Map<String, Object> entries = (Map<String, Object>) p.getOrDefault("entries", Map.of());
        List<Object> args = new ArrayList<>();
        args.add("MSET");
        for (Map.Entry<String, Object> e : entries.entrySet()) {
            args.add(e.getKey());
            args.add(e.getValue());
        }
        return args.toArray();
    }

    /** {@code MGET k1 k2 …} */
    private static Object[] buildMget(Map<String, Object> p) {
        @SuppressWarnings("unchecked")
        List<Object> keys = (List<Object>) p.getOrDefault("keys", List.of());
        List<Object> args = new ArrayList<>();
        args.add("MGET");
        args.addAll(keys);
        return args.toArray();
    }

    /** {@code HDEL key field [field …]} — payload "fields" is a list */
    private static Object[] buildHdel(String key, Map<String, Object> p) {
        @SuppressWarnings("unchecked")
        List<Object> fields = (List<Object>) p.getOrDefault("fields", List.of());
        List<Object> args = new ArrayList<>();
        args.add("HDEL");
        args.add(key);
        args.addAll(fields);
        return args.toArray();
    }

    /** {@code LPUSH/RPUSH key val [val …]} */
    private static Object[] buildListPush(String cmd, String key, Map<String, Object> p) {
        @SuppressWarnings("unchecked")
        List<Object> values = (List<Object>) p.getOrDefault("values", List.of());
        List<Object> args = new ArrayList<>();
        args.add(cmd);
        args.add(key);
        args.addAll(values);
        return args.toArray();
    }

    /** {@code LPOP/RPOP key [count]} */
    private static Object[] buildListPop(String cmd, String key, Map<String, Object> p) {
        Object count = p.get("count");
        if (count != null) {
            return new Object[]{cmd, key, count};
        }
        return new Object[]{cmd, key};
    }

    /** {@code SADD key member [member …]} */
    private static Object[] buildSetAdd(String key, Map<String, Object> p) {
        @SuppressWarnings("unchecked")
        List<Object> members = (List<Object>) p.getOrDefault("members", List.of());
        List<Object> args = new ArrayList<>();
        args.add("SADD");
        args.add(key);
        args.addAll(members);
        return args.toArray();
    }

    /** {@code SREM key member [member …]} */
    private static Object[] buildSetRem(String key, Map<String, Object> p) {
        @SuppressWarnings("unchecked")
        List<Object> members = (List<Object>) p.getOrDefault("members", List.of());
        List<Object> args = new ArrayList<>();
        args.add("SREM");
        args.add(key);
        args.addAll(members);
        return args.toArray();
    }

    /** {@code QPUBLISH queue payloadJson priority maxRetries} */
    private static Object[] buildQPublish(Map<String, Object> p) {
        Object queue      = p.get("queue");
        Object payload    = p.get("payload");
        Object priority   = p.getOrDefault("priority",   0);
        Object maxRetries = p.getOrDefault("max_retries", 3);
        String payloadJson;
        try {
            payloadJson = MAPPER.writeValueAsString(payload);
        } catch (Exception e) {
            payloadJson = "null";
        }
        return cmd("QPUBLISH", queue, payloadJson, priority, maxRetries);
    }

    /** {@code SPUBLISH room event dataJson} */
    private static Object[] buildSPublish(Map<String, Object> p) {
        Object room  = p.get("room");
        Object event = p.get("event");
        Object data  = p.get("data");
        String dataJson;
        try {
            dataJson = MAPPER.writeValueAsString(data);
        } catch (Exception e) {
            dataJson = "null";
        }
        return cmd("SPUBLISH", room, event, dataJson);
    }

    /** {@code PPUBLISH topic payloadJson} */
    private static Object[] buildPPublish(Map<String, Object> p) {
        Object topic   = p.get("topic");
        Object payload = p.get("payload");
        String payloadJson;
        try {
            payloadJson = MAPPER.writeValueAsString(payload);
        } catch (Exception e) {
            payloadJson = "null";
        }
        return cmd("PPUBLISH", topic, payloadJson);
    }

    /** {@code PSUBSCRIBE subscriberId topic [topic …]} */
    private static Object[] buildPSubscribe(Map<String, Object> p) {
        Object sid = p.get("subscriber_id");
        @SuppressWarnings("unchecked")
        List<Object> topics = (List<Object>) p.getOrDefault("topics", List.of());
        List<Object> args = new ArrayList<>();
        args.add("PSUBSCRIBE");
        args.add(sid);
        args.addAll(topics);
        return args.toArray();
    }

    /** {@code PUNSUBSCRIBE subscriberId topic [topic …]} */
    private static Object[] buildPUnsubscribe(Map<String, Object> p) {
        Object sid = p.get("subscriber_id");
        @SuppressWarnings("unchecked")
        List<Object> topics = (List<Object>) p.getOrDefault("topics", List.of());
        List<Object> args = new ArrayList<>();
        args.add("PUNSUBSCRIBE");
        args.add(sid);
        args.addAll(topics);
        return args.toArray();
    }

    // ── Response builder helpers ───────────────────────────────────────────────

    /** Creates a single-field object node. */
    private static ObjectNode obj(ObjectMapper m, String field, Object value) {
        ObjectNode node = m.createObjectNode();
        putValue(node, field, value);
        return node;
    }

    private static void putValue(ObjectNode node, String field, Object value) {
        if (value == null) {
            node.putNull(field);
        } else if (value instanceof Boolean b) {
            node.put(field, b);
        } else if (value instanceof Long l) {
            node.put(field, l);
        } else if (value instanceof Integer i) {
            node.put(field, i);
        } else if (value instanceof Double d) {
            node.put(field, d);
        } else if (value instanceof String s) {
            node.put(field, s);
        } else if (value instanceof ArrayNode an) {
            node.set(field, an);
        } else if (value instanceof JsonNode jn) {
            node.set(field, jn);
        } else {
            node.put(field, value.toString());
        }
    }

    /**
     * Converts the raw wire value (List, array, or any Object) to an ArrayNode.
     * Returns an empty array node if the value is null or not list-shaped.
     */
    @SuppressWarnings("unchecked")
    static ArrayNode toArrayNode(ObjectMapper m, Object raw) {
        ArrayNode arr = m.createArrayNode();
        if (raw == null) {
            return arr;
        }
        if (raw instanceof List<?> list) {
            for (Object item : list) {
                addToArray(arr, item);
            }
        } else if (raw instanceof Object[] objs) {
            for (Object item : objs) {
                addToArray(arr, item);
            }
        } else {
            addToArray(arr, raw);
        }
        return arr;
    }

    private static void addToArray(ArrayNode arr, Object item) {
        if (item == null) {
            arr.addNull();
        } else if (item instanceof Boolean b) {
            arr.add(b);
        } else if (item instanceof Long l) {
            arr.add(l);
        } else if (item instanceof Integer i) {
            arr.add(i);
        } else if (item instanceof Double d) {
            arr.add(d);
        } else if (item instanceof String s) {
            arr.add(s);
        } else if (item instanceof JsonNode jn) {
            arr.add(jn);
        } else {
            arr.add(item.toString());
        }
    }

    /** Converts a flat alternating [key, val, key, val, …] list to {"fields": {k: v, …}}. */
    private static ObjectNode buildHgetallResponse(ObjectMapper m, Object raw) {
        ObjectNode outer  = m.createObjectNode();
        ObjectNode fields = m.createObjectNode();
        outer.set("fields", fields);

        if (raw instanceof List<?> list) {
            for (int i = 0; i + 1 < list.size(); i += 2) {
                Object k = list.get(i);
                Object v = list.get(i + 1);
                if (k != null) {
                    fields.put(k.toString(), v != null ? v.toString() : "");
                }
            }
        }
        return outer;
    }

    /** Wraps the raw queue.consume result: an empty node when the queue is empty. */
    private static ObjectNode buildQueueConsumeResponse(ObjectMapper m, Object raw) {
        ObjectNode node = m.createObjectNode();
        if (raw == null || "Null".equals(raw)) {
            return node; // empty — no "message" field means null to the caller
        }
        node.set("message", rawToNode(m, raw));
        return node;
    }

    /** Converts any raw value to a JsonNode. */
    static JsonNode rawToNode(ObjectMapper m, Object raw) {
        if (raw == null) {
            return m.nullNode();
        }
        try {
            return m.valueToTree(raw);
        } catch (Exception e) {
            return m.createObjectNode();
        }
    }

    // ── Misc helpers ───────────────────────────────────────────────────────────

    private static String str(Object o) {
        return o != null ? o.toString() : "";
    }

    /** Returns true if the raw value indicates "OK" from the server. */
    static boolean isOk(Object raw) {
        if (raw == null) return false;
        if (raw instanceof Boolean b) return b;
        if (raw instanceof Long l) return l > 0;
        if (raw instanceof Integer i) return i > 0;
        return "OK".equalsIgnoreCase(raw.toString());
    }

    /** Returns true if the raw value is a non-zero integer (EXISTS-style responses). */
    static boolean isNonZero(Object raw) {
        if (raw == null) return false;
        if (raw instanceof Boolean b) return b;
        if (raw instanceof Long l) return l > 0;
        if (raw instanceof Integer i) return i > 0;
        return false;
    }

    /** Converts to long, defaulting to 0. */
    static long toLong(Object raw) {
        if (raw == null) return 0L;
        if (raw instanceof Long l) return l;
        if (raw instanceof Integer i) return i.longValue();
        if (raw instanceof Double d) return d.longValue();
        try {
            return Long.parseLong(raw.toString());
        } catch (NumberFormatException e) {
            return 0L;
        }
    }
}
