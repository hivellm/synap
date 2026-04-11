package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.net.http.HttpClient;
import java.util.HashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Main entry-point for the Synap Java SDK.
 *
 * <p>The transport is selected automatically from the URL scheme supplied to
 * {@link SynapConfig}:
 * <ul>
 *   <li>{@code synap://host:port} → SynapRPC (DEFAULT)</li>
 *   <li>{@code resp3://host:port} → RESP3</li>
 *   <li>{@code http://host:port}  → HTTP</li>
 * </ul>
 *
 * <p>The default URL is {@code synap://127.0.0.1:15501}.
 *
 * <p>A single client instance is thread-safe and should be shared across threads:
 * <pre>{@code
 * try (SynapClient client = new SynapClient(SynapConfig.builder().build())) {
 *     client.kv().set("key", "value");
 * }
 * }</pre>
 */
public final class SynapClient implements AutoCloseable {

    /** Shared Jackson mapper — thread-safe after construction. */
    final ObjectMapper mapper;

    private final SynapConfig config;
    private final Transport   transport;

    private volatile KVStore       kvStore;
    private volatile QueueManager  queueManager;
    private volatile StreamManager streamManager;
    private volatile PubSubManager pubSubManager;
    private volatile HashManager   hashManager;
    private volatile ListManager   listManager;
    private volatile SetManager    setManager;

    /**
     * Creates a new SynapClient with the given configuration.
     *
     * <p>The transport is chosen from the URL scheme in {@code config}.</p>
     *
     * @param config the client configuration
     * @throws NullPointerException if config is null
     */
    public SynapClient(SynapConfig config) {
        this.config    = Objects.requireNonNull(config, "config must not be null");
        this.mapper    = new ObjectMapper();
        this.transport = createTransport(config, mapper, null);
    }

    /**
     * Package-private constructor that accepts a custom {@link HttpClient} for testing.
     *
     * <p>When an explicit {@code HttpClient} is provided the client always uses the
     * HTTP transport, regardless of the URL scheme.</p>
     *
     * @param config     the client configuration
     * @param httpClient the HTTP client to use (forces HTTP transport)
     */
    SynapClient(SynapConfig config, HttpClient httpClient) {
        this.config    = Objects.requireNonNull(config,     "config must not be null");
        this.mapper    = new ObjectMapper();
        this.transport = new HttpTransport(config, mapper,
                Objects.requireNonNull(httpClient, "httpClient must not be null"));
    }

    // ── Transport factory ──────────────────────────────────────────────────────

    private static Transport createTransport(SynapConfig config, ObjectMapper mapper, HttpClient httpClient) {
        if (httpClient != null) {
            return new HttpTransport(config, mapper, httpClient);
        }
        return switch (config.getTransportMode()) {
            case SYNAP_RPC -> new SynapRpcTransport(
                    config.getHost(), config.getPort(), config.getTimeoutSeconds(), mapper);
            case RESP3     -> new Resp3Transport(
                    config.getHost(), config.getPort(), config.getTimeoutSeconds(), mapper);
            case HTTP      -> new HttpTransport(config, mapper);
        };
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
                if (kvStore == null) kvStore = new KVStore(this);
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
                if (queueManager == null) queueManager = new QueueManager(this);
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
                if (streamManager == null) streamManager = new StreamManager(this);
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
                if (pubSubManager == null) pubSubManager = new PubSubManager(this);
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
                if (hashManager == null) hashManager = new HashManager(this);
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
                if (listManager == null) listManager = new ListManager(this);
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
                if (setManager == null) setManager = new SetManager(this);
            }
        }
        return setManager;
    }

    // ── Core command dispatch ──────────────────────────────────────────────────

    /**
     * Sends a command to the Synap server and returns the parsed response payload.
     *
     * <p>This is the single dispatch point used by all manager classes.  Routing
     * to the active transport (SynapRPC, RESP3, or HTTP) is transparent.</p>
     *
     * @param command the command name (e.g. {@code "kv.set"})
     * @param payload key-value pairs to include; may be null
     * @return the response payload as a {@link JsonNode} (never null — an empty
     *         object node is returned when the server sends no payload)
     * @throws SynapException on network failure, protocol error, or server error
     */
    JsonNode sendCommand(String command, Map<String, Object> payload) {
        Objects.requireNonNull(command, "command must not be null");
        return transport.execute(command, payload);
    }

    /**
     * Convenience overload — sends a command with no payload.
     *
     * @param command the command name
     * @return the response payload node
     */
    JsonNode sendCommand(String command) {
        return sendCommand(command, null);
    }

    /**
     * Closes the client and releases any resources held by the underlying transport.
     */
    @Override
    public void close() {
        transport.close();
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
