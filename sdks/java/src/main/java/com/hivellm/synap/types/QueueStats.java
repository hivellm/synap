package com.hivellm.synap.types;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Statistics for a Synap queue.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public final class QueueStats {

    private final String name;
    private final long depth;
    private final long maxDepth;
    private final long totalPublished;
    private final long totalConsumed;
    private final long totalAcked;
    private final long totalNacked;

    /**
     * Creates a new QueueStats instance.
     *
     * @param name           queue name
     * @param depth          current number of messages in the queue
     * @param maxDepth       maximum allowed queue depth
     * @param totalPublished total messages published since creation
     * @param totalConsumed  total messages consumed since creation
     * @param totalAcked     total messages acknowledged since creation
     * @param totalNacked    total messages negatively acknowledged since creation
     */
    public QueueStats(
            @JsonProperty("name") String name,
            @JsonProperty("depth") long depth,
            @JsonProperty("max_depth") long maxDepth,
            @JsonProperty("total_published") long totalPublished,
            @JsonProperty("total_consumed") long totalConsumed,
            @JsonProperty("total_acked") long totalAcked,
            @JsonProperty("total_nacked") long totalNacked) {
        this.name = name;
        this.depth = depth;
        this.maxDepth = maxDepth;
        this.totalPublished = totalPublished;
        this.totalConsumed = totalConsumed;
        this.totalAcked = totalAcked;
        this.totalNacked = totalNacked;
    }

    /** Returns the queue name. */
    public String getName() { return name; }

    /** Returns the current depth (number of pending messages). */
    public long getDepth() { return depth; }

    /** Returns the maximum allowed depth. */
    public long getMaxDepth() { return maxDepth; }

    /** Returns the total number of published messages. */
    public long getTotalPublished() { return totalPublished; }

    /** Returns the total number of consumed messages. */
    public long getTotalConsumed() { return totalConsumed; }

    /** Returns the total number of acknowledged messages. */
    public long getTotalAcked() { return totalAcked; }

    /** Returns the total number of negatively acknowledged messages. */
    public long getTotalNacked() { return totalNacked; }
}
