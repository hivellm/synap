package com.hivellm.synap.types;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Represents a message retrieved from a Synap queue.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public final class Message {

    private final String id;
    private final byte[] payload;
    private final int priority;
    private final int retryCount;
    private final int maxRetries;

    /**
     * Creates a new Message (used by deserialization and managers).
     *
     * @param id         unique message identifier
     * @param payload    raw message bytes
     * @param priority   message priority (0-9, higher is more important)
     * @param retryCount number of delivery attempts so far
     * @param maxRetries maximum allowed delivery attempts
     */
    public Message(
            @JsonProperty("id") String id,
            @JsonProperty("payload") byte[] payload,
            @JsonProperty("priority") int priority,
            @JsonProperty("retry_count") int retryCount,
            @JsonProperty("max_retries") int maxRetries) {
        this.id = id;
        this.payload = payload;
        this.priority = priority;
        this.retryCount = retryCount;
        this.maxRetries = maxRetries;
    }

    /**
     * Returns the unique message identifier.
     *
     * @return message ID
     */
    public String getId() {
        return id;
    }

    /**
     * Returns the raw message payload bytes.
     *
     * @return payload bytes
     */
    public byte[] getPayload() {
        return payload;
    }

    /**
     * Returns the message priority (0-9).
     *
     * @return priority
     */
    public int getPriority() {
        return priority;
    }

    /**
     * Returns the number of delivery attempts so far.
     *
     * @return retry count
     */
    public int getRetryCount() {
        return retryCount;
    }

    /**
     * Returns the maximum allowed delivery attempts.
     *
     * @return max retries
     */
    public int getMaxRetries() {
        return maxRetries;
    }
}
