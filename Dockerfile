# ============================================================================
# Synap Server - Multi-stage Docker Build
# ============================================================================
# 
# This Dockerfile builds Synap server using a multi-stage build process:
# 1. Builder stage: Compile Rust binary with optimizations
# 2. Runtime stage: Minimal runtime image with compiled binary
#
# Features:
# - Optimized for size (< 50MB final image)
# - Based on Alpine Linux for security
# - Non-root user for security
# - Health checks enabled
# - Volume mounts for persistence
# ============================================================================

# ============================================================================
# Stage 1: Builder
# ============================================================================
FROM rust:1.85-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static

# Set working directory
WORKDIR /usr/src/synap

# Copy manifest files
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY synap-server/Cargo.toml ./synap-server/
COPY synap-cli/Cargo.toml ./synap-cli/

# Copy source code
COPY synap-server/src ./synap-server/src
COPY synap-cli/src ./synap-cli/src

# Build release binary with optimizations
# - Static linking for portability
# - Strip symbols for smaller size
# - LTO for better optimization
# - --bins flag compiles only binaries (ignores benches, examples, tests)
RUN cargo build --release --bins \
    --target x86_64-unknown-linux-musl && \
    strip /usr/src/synap/target/x86_64-unknown-linux-musl/release/synap-server

# ============================================================================
# Stage 2: Runtime
# ============================================================================
FROM alpine:3.19

# Install runtime dependencies (minimal)
RUN apk add --no-cache \
    ca-certificates \
    tzdata

# Create non-root user
RUN addgroup -g 1000 synap && \
    adduser -D -u 1000 -G synap synap

# Create directories for data persistence
RUN mkdir -p /data/wal /data/snapshots && \
    chown -R synap:synap /data

# Set working directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder /usr/src/synap/target/x86_64-unknown-linux-musl/release/synap-server /usr/local/bin/synap-server

# Copy default configuration
COPY config.yml /app/config.yml

# Fix permissions
RUN chown -R synap:synap /app

# Switch to non-root user
USER synap

# Expose ports
# 15500: HTTP/REST API + StreamableHTTP
# 15501: Replication TCP port
EXPOSE 15500 15501

# Health check
# Check if server responds to health endpoint
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:15500/health || exit 1

# Volume mounts for persistence
VOLUME ["/data"]

# Default command
CMD ["synap-server", "--config", "/app/config.yml"]

