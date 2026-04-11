package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;

import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/**
 * Set data-structure operations.
 *
 * <p>Obtain an instance via {@link SynapClient#set()}.
 *
 * <p>A set is an unordered collection of unique string members stored under a
 * top-level key, analogous to a Redis Set.  All operations are executed
 * synchronously.</p>
 */
public final class SetManager {

    private final SynapClient client;

    SetManager(SynapClient client) {
        this.client = client;
    }

    /**
     * Adds one or more members to the set stored at {@code key}.
     *
     * <p>If the key does not exist it is created.  Members that already belong to
     * the set are silently ignored.</p>
     *
     * @param key     the set key
     * @param members one or more string members to add
     * @return the number of members that were newly added (excluding duplicates)
     * @throws SynapException on network or server error
     */
    public int add(String key, String... members) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("members", Arrays.asList(members));
        JsonNode responsePayload = client.sendCommand("set.add", payload);

        JsonNode addedNode = responsePayload.get("added");
        return addedNode != null ? addedNode.asInt(0) : 0;
    }

    /**
     * Returns all members of the set stored at {@code key}.
     *
     * @param key the set key
     * @return an immutable set of all members; empty if the key does not exist
     * @throws SynapException on network or server error
     */
    public Set<String> members(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        JsonNode responsePayload = client.sendCommand("set.members", payload);

        JsonNode membersNode = responsePayload.get("members");
        if (membersNode == null || membersNode.isNull() || !membersNode.isArray()) {
            return Collections.emptySet();
        }

        Set<String> result = new HashSet<>(membersNode.size() * 2);
        for (JsonNode item : membersNode) {
            if (!item.isNull()) {
                result.add(item.asText());
            }
        }
        return Collections.unmodifiableSet(result);
    }

    /**
     * Determines whether {@code member} belongs to the set stored at {@code key}.
     *
     * @param key    the set key
     * @param member the member to test
     * @return {@code true} if the member is present, {@code false} otherwise
     * @throws SynapException on network or server error
     */
    public boolean isMember(String key, String member) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("member", member);
        JsonNode responsePayload = client.sendCommand("set.ismember", payload);

        JsonNode isMemberNode = responsePayload.get("is_member");
        return isMemberNode != null && isMemberNode.asBoolean(false);
    }

    /**
     * Removes one or more members from the set stored at {@code key}.
     *
     * <p>Members that do not exist in the set are silently ignored.</p>
     *
     * @param key     the set key
     * @param members one or more string members to remove
     * @return the number of members that were actually removed
     * @throws SynapException on network or server error
     */
    public int remove(String key, String... members) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        payload.put("members", Arrays.asList(members));
        JsonNode responsePayload = client.sendCommand("set.rem", payload);

        JsonNode removedNode = responsePayload.get("removed");
        return removedNode != null ? removedNode.asInt(0) : 0;
    }

    /**
     * Returns the cardinality (number of members) of the set stored at {@code key}.
     *
     * @param key the set key
     * @return the number of members, or {@code 0} if the key does not exist
     * @throws SynapException on network or server error
     */
    public int card(String key) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("key", key);
        JsonNode responsePayload = client.sendCommand("set.card", payload);

        JsonNode countNode = responsePayload.get("count");
        return countNode != null ? countNode.asInt(0) : 0;
    }
}
