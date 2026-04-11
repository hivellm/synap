package com.hivellm.synap.types;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Statistics for a Synap stream room.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public final class StreamStats {

    private final String room;
    private final long eventCount;
    private final long maxEvents;
    private final long firstOffset;
    private final long lastOffset;

    /**
     * Creates a new StreamStats instance.
     *
     * @param room        room name
     * @param eventCount  number of events currently retained
     * @param maxEvents   maximum events the room retains
     * @param firstOffset offset of the oldest retained event
     * @param lastOffset  offset of the newest retained event
     */
    public StreamStats(
            @JsonProperty("room") String room,
            @JsonProperty("event_count") long eventCount,
            @JsonProperty("max_events") long maxEvents,
            @JsonProperty("first_offset") long firstOffset,
            @JsonProperty("last_offset") long lastOffset) {
        this.room = room;
        this.eventCount = eventCount;
        this.maxEvents = maxEvents;
        this.firstOffset = firstOffset;
        this.lastOffset = lastOffset;
    }

    /** Returns the stream room name. */
    public String getRoom() { return room; }

    /** Returns the current number of retained events. */
    public long getEventCount() { return eventCount; }

    /** Returns the maximum number of events the room retains. */
    public long getMaxEvents() { return maxEvents; }

    /** Returns the offset of the oldest retained event. */
    public long getFirstOffset() { return firstOffset; }

    /** Returns the offset of the newest retained event. */
    public long getLastOffset() { return lastOffset; }
}
