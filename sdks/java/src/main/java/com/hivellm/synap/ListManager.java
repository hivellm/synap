package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;

/**
 * List data-structure operations.
 *
 * <p>Obtain an instance via {@link SynapClient#list()}.
 *
 * <p>A list is an ordered sequence of strings stored under a top-level key,
 * analogous to a Redis List.  Elements can be pushed and popped from either
 * end.  All operations are executed synchronously.</p>
 */
public final class ListManager {

    private final SynapClient client;

    ListManager(SynapClient client) {
        this.client = client;
    }

    /**
     * Prepends one or more values to the head of the list stored at {@code key}.
     *
     * <p>If the key does not exist it is created as an empty list first.</p>
     *
     * @param key    the list key
     * @param values one or more values to prepend (left-most first)
     * @return the length of the list after the push
     * @throws SynapException on network or server error
     */
    public int lpush(String key, String... values) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("values", Arrays.asList(values));
        JsonNode responsePayload = client.sendCommand("list.lpush", payload);

        JsonNode lenNode = responsePayload.get("len");
        return lenNode != null ? lenNode.asInt(0) : 0;
    }

    /**
     * Appends one or more values to the tail of the list stored at {@code key}.
     *
     * <p>If the key does not exist it is created as an empty list first.</p>
     *
     * @param key    the list key
     * @param values one or more values to append (left-most first)
     * @return the length of the list after the push
     * @throws SynapException on network or server error
     */
    public int rpush(String key, String... values) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("values", Arrays.asList(values));
        JsonNode responsePayload = client.sendCommand("list.rpush", payload);

        JsonNode lenNode = responsePayload.get("len");
        return lenNode != null ? lenNode.asInt(0) : 0;
    }

    /**
     * Removes and returns up to {@code count} elements from the head of the list.
     *
     * @param key   the list key
     * @param count maximum number of elements to pop
     * @return a list of popped values (may be shorter than {@code count} if the
     *         list has fewer elements); never null; empty if the key does not exist
     * @throws SynapException on network or server error
     */
    public List<String> lpop(String key, int count) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("count", count);
        JsonNode responsePayload = client.sendCommand("list.lpop", payload);

        return extractValues(responsePayload);
    }

    /**
     * Removes and returns up to {@code count} elements from the tail of the list.
     *
     * @param key   the list key
     * @param count maximum number of elements to pop
     * @return a list of popped values (may be shorter than {@code count} if the
     *         list has fewer elements); never null; empty if the key does not exist
     * @throws SynapException on network or server error
     */
    public List<String> rpop(String key, int count) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("count", count);
        JsonNode responsePayload = client.sendCommand("list.rpop", payload);

        return extractValues(responsePayload);
    }

    /**
     * Returns a sub-range of the list stored at {@code key}.
     *
     * <p>Both {@code start} and {@code stop} are zero-based, inclusive indices.
     * Negative indices can be used to refer to elements from the tail of the list
     * (e.g., {@code -1} is the last element).</p>
     *
     * @param key   the list key
     * @param start start index (inclusive)
     * @param stop  stop index (inclusive)
     * @return the requested range of elements; never null; empty if the key does
     *         not exist or the range is out of bounds
     * @throws SynapException on network or server error
     */
    public List<String> range(String key, int start, int stop) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("start", start);
        payload.put("stop", stop);
        JsonNode responsePayload = client.sendCommand("list.range", payload);

        return extractValues(responsePayload);
    }

    /**
     * Returns the number of elements in the list stored at {@code key}.
     *
     * @param key the list key
     * @return the list length, or {@code 0} if the key does not exist
     * @throws SynapException on network or server error
     */
    public int len(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        JsonNode responsePayload = client.sendCommand("list.len", payload);

        JsonNode lenNode = responsePayload.get("len");
        return lenNode != null ? lenNode.asInt(0) : 0;
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    private List<String> extractValues(JsonNode responsePayload) {
        JsonNode valuesNode = responsePayload.get("values");
        if (valuesNode == null || valuesNode.isNull() || !valuesNode.isArray()) {
            return Collections.emptyList();
        }

        List<String> result = new ArrayList<>(valuesNode.size());
        for (JsonNode item : valuesNode) {
            if (!item.isNull()) {
                result.add(item.asText());
            }
        }
        return result;
    }
}
