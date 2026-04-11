package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;

import java.util.Map;

/**
 * Common interface for all Synap transport implementations.
 *
 * <p>A Transport is responsible for sending a logical command to the server
 * and returning the parsed response payload as a {@link JsonNode}.  The
 * returned node must match the HTTP response shape so that all manager
 * classes remain transport-agnostic.</p>
 *
 * <p>Implementations must be thread-safe.</p>
 */
interface Transport extends AutoCloseable {

    /**
     * Executes a command against the server and returns the response payload.
     *
     * @param command the dot-notation command (e.g. {@code "kv.set"})
     * @param payload the key-value arguments for the command; may be null or empty
     * @return the response payload as a {@link JsonNode} (never null — an empty
     *         object node is returned when the server sends no payload)
     * @throws SynapException if the command fails for any reason
     */
    JsonNode execute(String command, Map<String, Object> payload) throws SynapException;

    /**
     * Closes the underlying connection/socket and releases resources.
     */
    @Override
    void close();
}
