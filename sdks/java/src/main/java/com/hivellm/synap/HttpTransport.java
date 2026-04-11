package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;

/**
 * HTTP transport implementation.
 *
 * <p>Sends every command as {@code POST /api/v1/command} with a JSON body and
 * returns the {@code "payload"} node from the response envelope.  This is the
 * original Synap transport and is used when the configured URL uses the
 * {@code http://} scheme.</p>
 *
 * <p>Thread-safe: the underlying {@link HttpClient} is shared and stateless.</p>
 */
final class HttpTransport implements Transport {

    private final SynapConfig config;
    private final HttpClient  httpClient;
    private final ObjectMapper mapper;

    /**
     * Creates an HttpTransport using a freshly-built {@link HttpClient}.
     *
     * @param config the client configuration (URL, timeout, auth token)
     * @param mapper the shared Jackson mapper
     */
    HttpTransport(SynapConfig config, ObjectMapper mapper) {
        this.config = Objects.requireNonNull(config, "config must not be null");
        this.mapper = Objects.requireNonNull(mapper, "mapper must not be null");
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(config.getTimeout())
                .version(HttpClient.Version.HTTP_1_1)
                .build();
    }

    /**
     * Creates an HttpTransport with a caller-supplied {@link HttpClient} (for testing).
     *
     * @param config     the client configuration
     * @param mapper     the shared Jackson mapper
     * @param httpClient the HTTP client to use
     */
    HttpTransport(SynapConfig config, ObjectMapper mapper, HttpClient httpClient) {
        this.config     = Objects.requireNonNull(config,     "config must not be null");
        this.mapper     = Objects.requireNonNull(mapper,     "mapper must not be null");
        this.httpClient = Objects.requireNonNull(httpClient, "httpClient must not be null");
    }

    /** {@inheritDoc} */
    @Override
    public JsonNode execute(String command, Map<String, Object> payload) {
        // Build the HTTP envelope.
        ObjectNode envelope = mapper.createObjectNode();
        envelope.put("command",    command);
        envelope.put("request_id", UUID.randomUUID().toString());

        if (payload != null && !payload.isEmpty()) {
            envelope.set("payload", mapper.valueToTree(payload));
        } else {
            envelope.set("payload", mapper.createObjectNode());
        }

        String requestBody;
        try {
            requestBody = mapper.writeValueAsString(envelope);
        } catch (IOException e) {
            throw SynapException.networkError("Failed to serialize request: " + e.getMessage(), e);
        }

        // Determine the HTTP base URL from config.
        // The config stores the original synap://… URL; we translate to http:// here.
        String baseUrl = config.getHttpBaseUrl();

        HttpRequest.Builder requestBuilder = HttpRequest.newBuilder()
                .uri(URI.create(baseUrl + "/api/v1/command"))
                .timeout(config.getTimeout())
                .header("Content-Type", "application/json")
                .header("Accept",       "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(requestBody));

        if (config.getAuthToken() != null && !config.getAuthToken().isBlank()) {
            requestBuilder.header("Authorization", "Bearer " + config.getAuthToken());
        }

        HttpResponse<String> response;
        try {
            response = httpClient.send(requestBuilder.build(), HttpResponse.BodyHandlers.ofString());
        } catch (IOException e) {
            throw SynapException.networkError("Request failed: " + e.getMessage(), e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw SynapException.networkError("Request interrupted", e);
        }

        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            throw SynapException.httpError("Unexpected status", response.statusCode());
        }

        String body = response.body();
        if (body == null || body.isBlank()) {
            return mapper.createObjectNode();
        }

        JsonNode root;
        try {
            root = mapper.readTree(body);
        } catch (IOException e) {
            throw SynapException.invalidResponse("Failed to parse JSON response: " + e.getMessage());
        }

        // Check the "success" field.
        JsonNode successNode = root.get("success");
        if (successNode != null && !successNode.asBoolean(true)) {
            JsonNode errorNode = root.get("error");
            String errorMessage = (errorNode != null && !errorNode.isNull())
                    ? errorNode.asText()
                    : "Unknown server error";
            throw SynapException.serverError(errorMessage);
        }

        // Return the inner payload node (or an empty object if absent).
        JsonNode payloadNode = root.get("payload");
        if (payloadNode == null || payloadNode.isNull()) {
            return mapper.createObjectNode();
        }
        return payloadNode;
    }

    /** {@inheritDoc} */
    @Override
    public void close() {
        // HttpClient does not implement Closeable on Java 17; no-op.
    }
}
