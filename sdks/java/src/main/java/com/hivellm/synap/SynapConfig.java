package com.hivellm.synap;

import java.time.Duration;
import java.util.Objects;

/**
 * Immutable configuration for the Synap client.
 *
 * <p>The transport is determined automatically from the URL scheme:
 * <ul>
 *   <li>{@code synap://host:port} → SynapRPC (default)</li>
 *   <li>{@code resp3://host:port} → RESP3</li>
 *   <li>{@code http://host:port} or {@code https://host:port} → HTTP</li>
 * </ul>
 *
 * <p>The default URL is {@code synap://127.0.0.1:15501} (SynapRPC).
 *
 * <p>Create instances via the builder:
 * <pre>{@code
 * // Default (SynapRPC on 127.0.0.1:15501)
 * SynapConfig config = SynapConfig.builder().build();
 *
 * // Explicit SynapRPC
 * SynapConfig config = SynapConfig.builder("synap://localhost:15501").build();
 *
 * // HTTP fallback
 * SynapConfig config = SynapConfig.builder("http://localhost:15500").build();
 * }</pre>
 */
public final class SynapConfig {

    /** The default URL — SynapRPC on the standard port. */
    public static final String DEFAULT_URL = "synap://127.0.0.1:15501";

    /** HTTP port used when translating synap:// or resp3:// to an HTTP base URL. */
    private static final int DEFAULT_HTTP_PORT = 15500;

    // ── Transport modes ────────────────────────────────────────────────────────

    /** Identifies the active transport. */
    public enum TransportMode { SYNAP_RPC, RESP3, HTTP }

    // ── Fields ─────────────────────────────────────────────────────────────────

    private final String        rawUrl;        // the URL as supplied by the caller
    private final TransportMode transport;
    private final String        host;
    private final int           port;
    private final String        authToken;
    private final Duration      timeout;
    private final int           maxRetries;

    private SynapConfig(Builder builder) {
        this.rawUrl     = builder.rawUrl;
        this.transport  = builder.transport;
        this.host       = builder.host;
        this.port       = builder.port;
        this.authToken  = builder.authToken;
        this.timeout    = builder.timeout;
        this.maxRetries = builder.maxRetries;
    }

    // ── Accessors ──────────────────────────────────────────────────────────────

    /**
     * Returns the raw URL that was supplied to the builder.
     *
     * @return the raw URL string (e.g. {@code synap://127.0.0.1:15501})
     */
    public String getBaseUrl() {
        return rawUrl;
    }

    /**
     * Returns the active transport mode derived from the URL scheme.
     *
     * @return the transport mode
     */
    public TransportMode getTransportMode() {
        return transport;
    }

    /**
     * Returns the host portion of the configured URL.
     *
     * @return hostname or IP address
     */
    public String getHost() {
        return host;
    }

    /**
     * Returns the port portion of the configured URL.
     *
     * @return TCP port number
     */
    public int getPort() {
        return port;
    }

    /**
     * Returns an {@code http://host:port} base URL suitable for the HTTP transport.
     *
     * <p>When the original URL uses {@code http://} or {@code https://} this returns
     * the original URL.  For {@code synap://} and {@code resp3://} this synthesizes
     * an HTTP URL pointing to the default HTTP port ({@value DEFAULT_HTTP_PORT}).</p>
     *
     * @return the HTTP base URL (no trailing slash)
     */
    public String getHttpBaseUrl() {
        if (transport == TransportMode.HTTP) {
            return rawUrl;
        }
        // synap:// or resp3:// → use the HTTP port for the HTTP transport fallback.
        return "http://" + host + ":" + DEFAULT_HTTP_PORT;
    }

    /**
     * Returns the bearer auth token, or {@code null} if not configured.
     *
     * @return auth token or null
     */
    public String getAuthToken() {
        return authToken;
    }

    /**
     * Returns the per-request timeout.
     *
     * @return timeout duration
     */
    public Duration getTimeout() {
        return timeout;
    }

    /**
     * Returns the timeout in whole seconds (minimum 1).
     *
     * @return timeout seconds
     */
    public int getTimeoutSeconds() {
        return (int) Math.max(1L, timeout.toSeconds());
    }

    /**
     * Returns the maximum number of retries for transient failures.
     *
     * @return max retries
     */
    public int getMaxRetries() {
        return maxRetries;
    }

    // ── Builder factory ────────────────────────────────────────────────────────

    /**
     * Creates a builder with the default URL ({@value DEFAULT_URL}, SynapRPC).
     *
     * @return a new builder instance
     */
    public static Builder builder() {
        return new Builder(DEFAULT_URL);
    }

    /**
     * Creates a builder for the given URL.
     *
     * <p>The transport is selected from the URL scheme:
     * <ul>
     *   <li>{@code synap://} → SynapRPC</li>
     *   <li>{@code resp3://} → RESP3</li>
     *   <li>{@code http://} / {@code https://} → HTTP</li>
     * </ul>
     *
     * @param url the server URL (scheme determines transport)
     * @return a new builder instance
     * @throws SynapException if url is null, empty, or has an unrecognised scheme
     */
    public static Builder builder(String url) {
        return new Builder(url);
    }

    // ── Builder ────────────────────────────────────────────────────────────────

    /**
     * Builder for {@link SynapConfig}.
     */
    public static final class Builder {

        private final String        rawUrl;
        private final TransportMode transport;
        private final String        host;
        private final int           port;

        private String   authToken;
        private Duration timeout    = Duration.ofSeconds(30);
        private int      maxRetries = 3;

        private Builder(String url) {
            if (url == null || url.isBlank()) {
                throw SynapException.invalidConfig("URL cannot be null or empty");
            }
            String trimmed = url.strip().replaceAll("/$", "");
            this.rawUrl = trimmed;

            ParsedUrl parsed = parseUrl(trimmed);
            this.transport = parsed.mode;
            this.host      = parsed.host;
            this.port      = parsed.port;
        }

        /**
         * Sets the bearer authentication token.
         *
         * @param token the auth token
         * @return this builder
         */
        public Builder authToken(String token) {
            this.authToken = Objects.requireNonNull(token, "token must not be null");
            return this;
        }

        /**
         * Sets the per-request timeout.
         *
         * @param timeout the timeout duration (must be positive)
         * @return this builder
         */
        public Builder timeout(Duration timeout) {
            Objects.requireNonNull(timeout, "timeout must not be null");
            if (timeout.isNegative() || timeout.isZero()) {
                throw SynapException.invalidConfig("Timeout must be positive");
            }
            this.timeout = timeout;
            return this;
        }

        /**
         * Sets the maximum number of retries for transient failures.
         *
         * @param maxRetries number of retries (must be &gt;= 0)
         * @return this builder
         */
        public Builder maxRetries(int maxRetries) {
            if (maxRetries < 0) {
                throw SynapException.invalidConfig("maxRetries must be >= 0");
            }
            this.maxRetries = maxRetries;
            return this;
        }

        /**
         * Builds the {@link SynapConfig}.
         *
         * @return a new immutable SynapConfig
         */
        public SynapConfig build() {
            return new SynapConfig(this);
        }

        // ── URL parsing ────────────────────────────────────────────────────────

        private static ParsedUrl parseUrl(String url) {
            String lower = url.toLowerCase();

            if (lower.startsWith("synap://")) {
                HostPort hp = splitHostPort(url.substring("synap://".length()), 15501);
                return new ParsedUrl(TransportMode.SYNAP_RPC, hp.host, hp.port);
            }
            if (lower.startsWith("resp3://")) {
                HostPort hp = splitHostPort(url.substring("resp3://".length()), 6379);
                return new ParsedUrl(TransportMode.RESP3, hp.host, hp.port);
            }
            if (lower.startsWith("http://")) {
                HostPort hp = splitHostPort(url.substring("http://".length()), 15500);
                return new ParsedUrl(TransportMode.HTTP, hp.host, hp.port);
            }
            if (lower.startsWith("https://")) {
                HostPort hp = splitHostPort(url.substring("https://".length()), 443);
                return new ParsedUrl(TransportMode.HTTP, hp.host, hp.port);
            }
            throw SynapException.invalidConfig(
                    "Unrecognised URL scheme in '" + url
                    + "'. Use synap://, resp3://, or http://");
        }

        private static HostPort splitHostPort(String hostPort, int defaultPort) {
            // Strip any trailing path.
            int slashIdx = hostPort.indexOf('/');
            if (slashIdx != -1) hostPort = hostPort.substring(0, slashIdx);

            int colonIdx = hostPort.lastIndexOf(':');
            if (colonIdx == -1) {
                return new HostPort(hostPort, defaultPort);
            }
            String host = hostPort.substring(0, colonIdx);
            String portStr = hostPort.substring(colonIdx + 1);
            try {
                int port = Integer.parseInt(portStr);
                return new HostPort(host.isEmpty() ? "127.0.0.1" : host, port);
            } catch (NumberFormatException e) {
                throw SynapException.invalidConfig("Invalid port in URL: '" + portStr + "'");
            }
        }

        private record HostPort(String host, int port) {}
        private record ParsedUrl(TransportMode mode, String host, int port) {}
    }
}
