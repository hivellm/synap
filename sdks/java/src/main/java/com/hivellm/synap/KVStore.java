package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.hivellm.synap.types.KVStats;

import java.util.Map;

/**
 * Key-Value store operations.
 *
 * <p>Obtain an instance via {@link SynapClient#kv()}.
 */
public final class KVStore {

    private final SynapClient client;

    KVStore(SynapClient client) {
        this.client = client;
    }

    /**
     * Sets a key-value pair with no expiry.
     *
     * @param key   the key
     * @param value the value (serialized to JSON)
     * @throws SynapException on network or server error
     */
    public void set(String key, String value) {
        set(key, value, null);
    }

    /**
     * Sets a key-value pair with an optional time-to-live.
     *
     * @param key        the key
     * @param value      the value (serialized to JSON)
     * @param ttlSeconds TTL in seconds; {@code null} means no expiry
     * @throws SynapException on network or server error
     */
    public void set(String key, String value, Integer ttlSeconds) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("value", value);
        payload.put("ttl", ttlSeconds);
        client.sendCommand("kv.set", payload);
    }

    /**
     * Gets the value for the given key.
     *
     * @param key the key to retrieve
     * @return the string value, or {@code null} if the key does not exist
     * @throws SynapException on network or server error
     */
    public String get(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        JsonNode responsePayload = client.sendCommand("kv.get", payload);

        // Server returns either {"value": "..."} or {"found": false}
        JsonNode foundNode = responsePayload.get("found");
        if (foundNode != null && !foundNode.asBoolean(true)) {
            return null;
        }

        JsonNode valueNode = responsePayload.get("value");
        if (valueNode == null || valueNode.isNull()) {
            return null;
        }
        return valueNode.asText();
    }

    /**
     * Deletes the given key.
     *
     * @param key the key to delete
     * @return {@code true} if the key existed and was deleted, {@code false} otherwise
     * @throws SynapException on network or server error
     */
    public boolean delete(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        JsonNode responsePayload = client.sendCommand("kv.delete", payload);

        JsonNode deletedNode = responsePayload.get("deleted");
        return deletedNode != null && deletedNode.asBoolean(false);
    }

    /**
     * Checks whether the given key exists.
     *
     * @param key the key to check
     * @return {@code true} if the key exists, {@code false} otherwise
     * @throws SynapException on network or server error
     */
    public boolean exists(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        JsonNode responsePayload = client.sendCommand("kv.exists", payload);

        JsonNode existsNode = responsePayload.get("exists");
        return existsNode != null && existsNode.asBoolean(false);
    }

    /**
     * Atomically increments the integer value stored at the given key by 1.
     *
     * <p>If the key does not exist it is initialised to 0 before incrementing.</p>
     *
     * @param key the key
     * @return the new value after incrementing
     * @throws SynapException on network or server error
     */
    public long incr(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("delta", 1);
        JsonNode responsePayload = client.sendCommand("kv.incr", payload);

        JsonNode valueNode = responsePayload.get("value");
        return valueNode != null ? valueNode.asLong(0L) : 0L;
    }

    /**
     * Atomically decrements the integer value stored at the given key by 1.
     *
     * <p>If the key does not exist it is initialised to 0 before decrementing.</p>
     *
     * @param key the key
     * @return the new value after decrementing
     * @throws SynapException on network or server error
     */
    public long decr(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("delta", 1);
        JsonNode responsePayload = client.sendCommand("kv.decr", payload);

        JsonNode valueNode = responsePayload.get("value");
        return valueNode != null ? valueNode.asLong(0L) : 0L;
    }

    /**
     * Returns statistics for the key-value store.
     *
     * @return a {@link KVStats} snapshot
     * @throws SynapException on network or server error
     */
    public KVStats stats() {
        JsonNode responsePayload = client.sendCommand("kv.stats");
        try {
            return client.mapper.treeToValue(responsePayload, KVStats.class);
        } catch (Exception e) {
            throw SynapException.invalidResponse("Failed to deserialize KVStats: " + e.getMessage());
        }
    }
}
