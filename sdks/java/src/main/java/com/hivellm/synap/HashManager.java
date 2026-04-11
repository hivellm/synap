package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;

import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Hash data-structure operations.
 *
 * <p>Obtain an instance via {@link SynapClient#hash()}.
 *
 * <p>A hash is a map of string field-value pairs stored under a top-level key,
 * analogous to a Redis Hash.  All operations are executed synchronously.</p>
 */
public final class HashManager {

    private final SynapClient client;

    HashManager(SynapClient client) {
        this.client = client;
    }

    /**
     * Sets a field in the hash stored at {@code key} to {@code value}.
     *
     * @param key   the hash key
     * @param field the field name within the hash
     * @param value the value to store
     * @return {@code true} if the field was newly created, {@code false} if it was updated
     * @throws SynapException on network or server error
     */
    public boolean set(String key, String field, String value) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("field", field);
        payload.put("value", value);
        JsonNode responsePayload = client.sendCommand("hash.set", payload);

        JsonNode createdNode = responsePayload.get("created");
        return createdNode != null && createdNode.asBoolean(false);
    }

    /**
     * Returns the value associated with {@code field} in the hash stored at {@code key}.
     *
     * @param key   the hash key
     * @param field the field name within the hash
     * @return the field value, or {@code null} if the key or field does not exist
     * @throws SynapException on network or server error
     */
    public String get(String key, String field) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("field", field);
        JsonNode responsePayload = client.sendCommand("hash.get", payload);

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
     * Returns all field-value pairs in the hash stored at {@code key}.
     *
     * @param key the hash key
     * @return an immutable map of all fields to their values; empty if the key does not exist
     * @throws SynapException on network or server error
     */
    public Map<String, String> getAll(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        JsonNode responsePayload = client.sendCommand("hash.getall", payload);

        JsonNode fieldsNode = responsePayload.get("fields");
        if (fieldsNode == null || fieldsNode.isNull() || !fieldsNode.isObject()) {
            return Collections.emptyMap();
        }

        Map<String, String> result = new HashMap<>();
        fieldsNode.fields().forEachRemaining(entry -> result.put(entry.getKey(), entry.getValue().asText()));
        return Collections.unmodifiableMap(result);
    }

    /**
     * Removes the specified field from the hash stored at {@code key}.
     *
     * @param key   the hash key
     * @param field the field name to remove
     * @return the number of fields that were removed (0 or 1)
     * @throws SynapException on network or server error
     */
    public long del(String key, String field) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("fields", List.of(field));
        JsonNode responsePayload = client.sendCommand("hash.del", payload);

        JsonNode removedNode = responsePayload.get("removed");
        return removedNode != null ? removedNode.asLong(0L) : 0L;
    }

    /**
     * Determines whether the given field exists in the hash stored at {@code key}.
     *
     * @param key   the hash key
     * @param field the field name to check
     * @return {@code true} if the field exists, {@code false} otherwise
     * @throws SynapException on network or server error
     */
    public boolean exists(String key, String field) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("field", field);
        JsonNode responsePayload = client.sendCommand("hash.exists", payload);

        JsonNode existsNode = responsePayload.get("exists");
        return existsNode != null && existsNode.asBoolean(false);
    }
}
