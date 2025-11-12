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
#
# Docker Commands:
#   Build image (AMD64):
#     docker build -t synap:0.8.0 -t synap:latest .
#     docker build -t hivellm/synap:0.8.0 -t hivellm/synap:latest .
#
#   Build for ARM64:
#     docker buildx build --platform linux/arm64 -t synap:0.8.0-arm64 .
#
#   Build multi-arch (AMD64 + ARM64):
#     docker buildx build --platform linux/amd64,linux/arm64 \
#       -t hivellm/synap:0.8.0 -t hivellm/synap:latest --push .
#
#   Build for pre-release testing:
#     docker build -t synap:0.8.0-rc -t synap:latest .
#
#   Run container:
#     docker run -d --name synap-server-0.8.0 \
#       -p 15500:15500 -p 15501:15501 \
#       -v synap-data:/data \
#       synap:0.8.0
#
#   Run with authentication enabled:
#     docker run -d --name synap-server \
#       -p 15500:15500 -p 15501:15501 \
#       -v synap-data:/data \
#       -e SYNAP_AUTH_ENABLED=true \
#       -e SYNAP_AUTH_REQUIRE_AUTH=true \
#       -e SYNAP_AUTH_ROOT_USERNAME=root \
#       -e SYNAP_AUTH_ROOT_PASSWORD=your_secure_password \
#       -e SYNAP_AUTH_ROOT_ENABLED=true \
#       synap:latest
#
#   Run with authentication and audit logging:
#     docker run -d --name synap-server \
#       -p 15500:15500 -p 15501:15501 \
#       -v synap-data:/data \
#       -e SYNAP_AUTH_ENABLED=true \
#       -e SYNAP_AUTH_REQUIRE_AUTH=true \
#       -e SYNAP_AUTH_ROOT_USERNAME=admin \
#       -e SYNAP_AUTH_ROOT_PASSWORD=SecurePassword123! \
#       -e SYNAP_AUTH_ROOT_ENABLED=true \
#       synap:latest
#
#   Run with custom config:
#     docker run -d --name synap-server \
#       -p 15500:15500 -p 15501:15501 \
#       -v synap-data:/data \
#       -v /path/to/config.yml:/app/config.yml:ro \
#       synap:latest
#
#   View logs:
#     docker logs -f synap-server-0.8.0
#
#   Check status:
#     docker ps --filter name=synap-server-0.8.0
#
#   Stop container:
#     docker stop synap-server-0.8.0
#
#   Remove container:
#     docker rm synap-server-0.8.0
#
#   Remove image:
#     docker rmi synap:0.8.0
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
    openssl-libs-static \
    gcc \
    musl-tools

# Install nightly toolchain for Rust Edition 2024
RUN rustup toolchain install nightly && \
    rustup default nightly

# Set working directory
WORKDIR /usr/src/synap

# Copy manifest files
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY synap-server/Cargo.toml ./synap-server/
COPY synap-cli/Cargo.toml ./synap-cli/
COPY sdks/rust/Cargo.toml ./sdks/rust/

# Copy source code
COPY synap-server/src ./synap-server/src
COPY synap-cli/src ./synap-cli/src
COPY sdks/rust/src ./sdks/rust/src

# Remove benchmark declarations from Cargo.toml for Docker build
# (benchmarks are not needed for production image)
RUN sed -i '/^# Configure benchmarks to use Criterion/,/^$/d' synap-server/Cargo.toml && \
    sed -i '/^\[\[bench\]\]/,/^$/d' synap-server/Cargo.toml

# Build release binary with optimizations
# - Static linking for portability
# - Strip symbols for smaller size
# - LTO for better optimization
# - --bins flag compiles only binaries (ignores benches, examples, tests)
# - Support multi-arch builds (AMD64 and ARM64)
ARG TARGETARCH
ARG TARGETPLATFORM
RUN case ${TARGETARCH} in \
    amd64) TARGET_TRIPLE=x86_64-unknown-linux-musl ;; \
    arm64) TARGET_TRIPLE=aarch64-unknown-linux-musl ;; \
    *) echo "Unsupported architecture: ${TARGETARCH}" && exit 1 ;; \
    esac && \
    rustup target add ${TARGET_TRIPLE} && \
    cargo build --release --bins \
    --target ${TARGET_TRIPLE} && \
    strip /usr/src/synap/target/${TARGET_TRIPLE}/release/synap-server && \
    cp /usr/src/synap/target/${TARGET_TRIPLE}/release/synap-server /usr/src/synap/synap-server

# ============================================================================
# Stage 2: Runtime
# ============================================================================
FROM alpine:3.19

# Install runtime dependencies (minimal)
RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    wget

# Create non-root user
RUN addgroup -g 1000 synap && \
    adduser -D -u 1000 -G synap synap

# Create directories for data persistence
RUN mkdir -p /data/wal /data/snapshots && \
    chown -R synap:synap /data

# Set working directory
WORKDIR /app

# Copy binary from builder (multi-arch support)
# Binary is copied to a fixed location in builder stage
COPY --from=builder --chown=synap:synap /usr/src/synap/synap-server /usr/local/bin/synap-server
RUN chmod +x /usr/local/bin/synap-server

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

