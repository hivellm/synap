package com.hivellm.synap;

import java.time.Duration;
import java.util.Objects;

/**
 * Immutable configuration for the Synap client.
 *
 * <p>Create instances via the builder:
 * <pre>{@code
 * SynapConfig config = SynapConfig.builder("http://localhost:15500")
 *     .authToken("bearer-token")
 *     .timeout(Duration.ofSeconds(10))
 *     .build();
 * }</pre>
 */
public final class SynapConfig {

    private final String baseUrl;
    private final String authToken;
    private final Duration timeout;
    private final int maxRetries;

    private SynapConfig(Builder builder) {
        this.baseUrl = builder.baseUrl;
        this.authToken = builder.authToken;
        this.timeout = builder.timeout;
        this.maxRetries = builder.maxRetries;
    }

    /**
     * Returns the base URL of the Synap server.
     *
     * @return base URL (e.g. {@code http://localhost:15500})
     */
    public String getBaseUrl() {
        return baseUrl;
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
     * Returns the maximum number of retries for transient failures.
     *
     * @return max retries
     */
    public int getMaxRetries() {
        return maxRetries;
    }

    /**
     * Creates a new builder for the given base URL.
     *
     * @param baseUrl the Synap server base URL (e.g. {@code http://localhost:15500})
     * @return a new builder instance
     * @throws SynapException if baseUrl is null or empty
     */
    public static Builder builder(String baseUrl) {
        return new Builder(baseUrl);
    }

    /**
     * Builder for {@link SynapConfig}.
     */
    public static final class Builder {

        private final String baseUrl;
        private String authToken;
        private Duration timeout = Duration.ofSeconds(30);
        private int maxRetries = 3;

        private Builder(String baseUrl) {
            if (baseUrl == null || baseUrl.isBlank()) {
                throw SynapException.invalidConfig("Base URL cannot be null or empty");
            }
            this.baseUrl = baseUrl.stripTrailing().replaceAll("/$", "");
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
    }
}
