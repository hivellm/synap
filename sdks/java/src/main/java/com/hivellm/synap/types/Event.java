package com.hivellm.synap.types;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.JsonNode;

/**
 * Represents an event retrieved from a Synap stream.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public final class Event {

    private final long offset;
    private final String eventType;
    private final JsonNode data;
    private final long timestamp;
    private final String room;

    /**
     * Creates a new Event.
     *
     * @param offset    position in the stream
     * @param eventType the event type label
     * @param data      flexible JSON payload
     * @param timestamp Unix epoch milliseconds
     * @param room      the stream room this event belongs to
     */
    public Event(
            @JsonProperty("offset") long offset,
            @JsonProperty("event") String eventType,
            @JsonProperty("data") JsonNode data,
            @JsonProperty("timestamp") long timestamp,
            @JsonProperty("room") String room) {
        this.offset = offset;
        this.eventType = eventType;
        this.data = data;
        this.timestamp = timestamp;
        this.room = room;
    }

    /**
     * Returns the stream offset (position) of this event.
     *
     * @return offset
     */
    public long getOffset() {
        return offset;
    }

    /**
     * Returns the event type label.
     *
     * @return event type
     */
    public String getEventType() {
        return eventType;
    }

    /**
     * Returns the event data as a flexible JSON node.
     *
     * @return JSON data node
     */
    public JsonNode getData() {
        return data;
    }

    /**
     * Returns the event timestamp in Unix epoch milliseconds.
     *
     * @return timestamp
     */
    public long getTimestamp() {
        return timestamp;
    }

    /**
     * Returns the stream room this event belongs to.
     *
     * @return room name
     */
    public String getRoom() {
        return room;
    }
}
