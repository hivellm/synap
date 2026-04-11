package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.hivellm.synap.types.Event;
import com.hivellm.synap.types.StreamStats;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Event stream operations.
 *
 * <p>Obtain an instance via {@link SynapClient#stream()}.
 */
public final class StreamManager {

    private final SynapClient client;

    StreamManager(SynapClient client) {
        this.client = client;
    }

    /**
     * Creates a new stream room.
     *
     * @param room      room name
     * @param maxEvents maximum number of events the room retains (0 = unlimited)
     * @throws SynapException on network or server error
     */
    public void create(String room, long maxEvents) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("room", room);
        payload.put("max_events", maxEvents);
        client.sendCommand("stream.create", payload);
    }

    /**
     * Publishes an event to the named stream room.
     *
     * @param room      room name
     * @param eventType event type label (e.g. {@code "message"}, {@code "click"})
     * @param data      event data (any JSON-serializable object)
     * @return the stream offset assigned to this event
     * @throws SynapException on network or server error
     */
    public long publish(String room, String eventType, Object data) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("room", room);
        payload.put("event", eventType);
        payload.put("data", data);

        JsonNode responsePayload = client.sendCommand("stream.publish", payload);

        JsonNode offsetNode = responsePayload.get("offset");
        return offsetNode != null ? offsetNode.asLong(0L) : 0L;
    }

    /**
     * Consumes events from a stream starting at the given offset.
     *
     * @param room   room name
     * @param offset starting offset (inclusive); use 0 to read from the beginning
     * @param limit  maximum number of events to return
     * @return list of events (never null; may be empty)
     * @throws SynapException on network or server error
     */
    public List<Event> consume(String room, long offset, int limit) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("room", room);
        payload.put("subscriber_id", "sdk-default");
        payload.put("from_offset", offset);
        payload.put("limit", limit);

        JsonNode responsePayload = client.sendCommand("stream.consume", payload);

        List<Event> result = new ArrayList<>();
        JsonNode eventsNode = responsePayload.get("events");
        if (eventsNode != null && eventsNode.isArray()) {
            for (JsonNode eventNode : eventsNode) {
                try {
                    result.add(client.mapper.treeToValue(eventNode, Event.class));
                } catch (Exception e) {
                    throw SynapException.invalidResponse("Failed to deserialize Event: " + e.getMessage());
                }
            }
        }
        return result;
    }

    /**
     * Returns statistics for the named stream room.
     *
     * @param room room name
     * @return a {@link StreamStats} snapshot
     * @throws SynapException on network or server error
     */
    public StreamStats stats(String room) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("room", room);
        JsonNode responsePayload = client.sendCommand("stream.stats", payload);

        try {
            return client.mapper.treeToValue(responsePayload, StreamStats.class);
        } catch (Exception e) {
            throw SynapException.invalidResponse("Failed to deserialize StreamStats: " + e.getMessage());
        }
    }

    /**
     * Lists the names of all stream rooms currently registered on the server.
     *
     * @return list of room names (never null; may be empty)
     * @throws SynapException on network or server error
     */
    public List<String> list() {
        JsonNode responsePayload = client.sendCommand("stream.list");

        List<String> result = new ArrayList<>();
        JsonNode roomsNode = responsePayload.get("rooms");
        if (roomsNode != null && roomsNode.isArray()) {
            for (JsonNode item : roomsNode) {
                if (!item.isNull()) {
                    result.add(item.asText());
                }
            }
        }
        return result;
    }

    /**
     * Deletes the named stream room and all its retained events.
     *
     * @param room room name
     * @throws SynapException on network or server error
     */
    public void delete(String room) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("room", room);
        client.sendCommand("stream.delete", payload);
    }
}
