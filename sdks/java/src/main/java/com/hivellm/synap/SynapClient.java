package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.HashMap;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;

/**
 * Main entry-point for the Synap Java SDK.
 *
 * <p>All operations are executed synchronously over the StreamableHTTP endpoint
 * ({@code POST /api/v1/command}).  The client is thread-safe: a single instance
 * can be shared across threads.</p>
 *
 * <p>Always close the client when done to release the underlying HTTP connection
 * pool:
 * <pre>{@code
 * try (SynapClient client = new SynapClient(config)) {
 *     client.kv().set("key", "value");
 * }
 * }</pre>
 */
public final class SynapClient implements AutoCloseable {

    /** Shared Jackson mapper — thread-safe after construction. */
    final ObjectMapper mapper;

    private final SynapConfig config;
    private final HttpClient httpClient;

    private volatile KVStore kvStore;
    private volatile QueueManager queueManager;
    private volatile StreamManager streamManager;
    private volatile PubSubManager pubSubManager;
    private volatile HashManager hashManager;
    private volatile ListManager listManager;
    private volatile SetManager setManager;

    /**
     * Creates a new SynapClient with the given configuration.
     *
     * @param config the client configuration
     * @throws NullPointerException if config is null
     */
    public SynapClient(SynapConfig config) {
        this.config = Objects.requireNonNull(config, "config must not be null");
        this.mapper = new ObjectMapper();
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(config.getTimeout())
                .version(HttpClient.Version.HTTP_1_1)
                .build();
    }

    /**
     * Package-private constructor that accepts a custom HttpClient for testing.
     *
     * @param config     the client configuration
     * @param httpClient the HTTP client to use
     */
    SynapClient(SynapConfig config, HttpClient httpClient) {
        this.config = Objects.requireNonNull(config, "config must not be null");
        this.mapper = new ObjectMapper();
        this.httpClient = Objects.requireNonNull(httpClient, "httpClient must not be null");
    }

    // ── Sub-clients (lazy, thread-safe via double-checked locking) ─────────────

    /**
     * Returns the Key-Value store operations.
     *
     * @return KVStore instance
     */
    public KVStore kv() {
        if (kvStore == null) {
            synchronized (this) {
                if (kvStore == null) {
                    kvStore = new KVStore(this);
                }
            }
        }
        return kvStore;
    }

    /**
     * Returns the Queue operations.
     *
     * @return QueueManager instance
     */
    public QueueManager queue() {
        if (queueManager == null) {
            synchronized (this) {
                if (queueManager == null) {
                    queueManager = new QueueManager(this);
                }
            }
        }
        return queueManager;
    }

    /**
     * Returns the Stream operations.
     *
     * @return StreamManager instance
     */
    public StreamManager stream() {
        if (streamManager == null) {
            synchronized (this) {
                if (streamManager == null) {
                    streamManager = new StreamManager(this);
                }
            }
        }
        return streamManager;
    }

    /**
     * Returns the Pub/Sub operations.
     *
     * @return PubSubManager instance
     */
    public PubSubManager pubsub() {
        if (pubSubManager == null) {
            synchronized (this) {
                if (pubSubManager == null) {
                    pubSubManager = new PubSubManager(this);
                }
            }
        }
        return pubSubManager;
    }

    /**
     * Returns the Hash data-structure operations.
     *
     * @return HashManager instance
     */
    public HashManager hash() {
        if (hashManager == null) {
            synchronized (this) {
                if (hashManager == null) {
                    hashManager = new HashManager(this);
                }
            }
        }
        return hashManager;
    }

    /**
     * Returns the List data-structure operations.
     *
     * @return ListManager instance
     */
    public ListManager list() {
        if (listManager == null) {
            synchronized (this) {
                if (listManager == null) {
                    listManager = new ListManager(this);
                }
            }
        }
        return listManager;
    }

    /**
     * Returns the Set data-structure operations.
     *
     * @return SetManager instance
     */
    public SetManager set() {
        if (setManager == null) {
            synchronized (this) {
                if (setManager == null) {
                    setManager = new SetManager(this);
                }
            }
        }
        return setManager;
    }

    // ── Core HTTP transport ────────────────────────────────────────────────────

    /**
     * Sends a command to the Synap server and returns the parsed response payload.
     *
     * <p>This is the single transport method used by all manager classes.  The
     * {@code payload} map is serialised as the {@code "payload"} field in the
     * wire JSON envelope.  A unique {@code request_id} UUID is generated for
     * every call.</p>
     *
     * @param command the command name (e.g. {@code "kv.set"})
     * @param payload key-value pairs to include in the payload; may be null
     * @return the {@code payload} node from the server response (never null —
     *         an empty object node is returned when the server omits it)
     * @throws SynapException on network failure, HTTP error, or a server-side error
     */
    JsonNode sendCommand(String command, Map<String, Object> payload) {
        Objects.requireNonNull(command, "command must not be null");

        // Build the envelope.
        ObjectNode envelope = mapper.createObjectNode();
        envelope.put("command", command);
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

        HttpRequest.Builder requestBuilder = HttpRequest.newBuilder()
                .uri(URI.create(config.getBaseUrl() + "/api/v1/command"))
                .timeout(config.getTimeout())
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
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

    /**
     * Convenience helper — builds a mutable payload map and calls
     * {@link #sendCommand(String, Map)}.
     *
     * @param command the command name
     * @return the response payload node
     */
    JsonNode sendCommand(String command) {
        return sendCommand(command, null);
    }

    /**
     * Closes the client and releases resources held by the underlying HTTP client.
     *
     * <p>The Java {@link HttpClient} introduced in Java 21 implements
     * {@link AutoCloseable}; on Java 17 the client is not closeable and this
     * method is a no-op for that resource.  Sub-client managers hold no
     * independent resources and do not need disposal.</p>
     */
    @Override
    public void close() {
        // HttpClient does not implement Closeable on Java 17; nothing to close.
        // Included so the client works correctly in try-with-resources.
    }

    // ── Accessors ──────────────────────────────────────────────────────────────

    /**
     * Returns the configuration used to create this client.
     *
     * @return the SynapConfig
     */
    public SynapConfig getConfig() {
        return config;
    }

    // ── Internal helpers ───────────────────────────────────────────────────────

    /**
     * Creates a new mutable payload map with an initial capacity hint.
     */
    static Map<String, Object> newPayload() {
        return new HashMap<>();
    }
}
