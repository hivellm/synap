package com.hivellm.synap.types;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Statistics for the Synap key-value store.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public final class KVStats {

    private final long totalKeys;
    private final long memoryUsageBytes;
    private final long totalGets;
    private final long totalSets;
    private final long totalDeletes;

    /**
     * Creates a new KVStats instance.
     *
     * @param totalKeys        total number of keys currently stored
     * @param memoryUsageBytes approximate memory used by the store in bytes
     * @param totalGets        cumulative number of GET operations
     * @param totalSets        cumulative number of SET operations
     * @param totalDeletes     cumulative number of DELETE operations
     */
    public KVStats(
            @JsonProperty("total_keys") long totalKeys,
            @JsonProperty("memory_usage") long memoryUsageBytes,
            @JsonProperty("total_gets") long totalGets,
            @JsonProperty("total_sets") long totalSets,
            @JsonProperty("total_deletes") long totalDeletes) {
        this.totalKeys = totalKeys;
        this.memoryUsageBytes = memoryUsageBytes;
        this.totalGets = totalGets;
        this.totalSets = totalSets;
        this.totalDeletes = totalDeletes;
    }

    /** Returns the total number of keys currently stored. */
    public long getTotalKeys() { return totalKeys; }

    /** Returns the approximate memory used by the store in bytes. */
    public long getMemoryUsageBytes() { return memoryUsageBytes; }

    /** Returns the cumulative number of GET operations. */
    public long getTotalGets() { return totalGets; }

    /** Returns the cumulative number of SET operations. */
    public long getTotalSets() { return totalSets; }

    /** Returns the cumulative number of DELETE operations. */
    public long getTotalDeletes() { return totalDeletes; }
}
