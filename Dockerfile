# ============================================================================
# Synap Server - Multi-stage Docker Build
# ============================================================================
# 
# This Dockerfile builds Synap server using a multi-stage build process:
# 1. Builder stage: Compile Rust binary with optimizations
# 2. Runtime stage: Minimal runtime image with compiled binary
#
# Features:
# - Optimized for size (< 50MB final image — fully static musl binary)
# - Runtime based on Docker Hardened Image (dhi.io/debian-base:trixie-dev)
#   for continuous CVE patching and minimized attack surface
# - Non-root user for security
# - Health checks enabled
# - Volume mounts for persistence
# - Ships with SBOM + provenance attestations when built via buildx
#
# Docker Commands:
#   Build image (single arch, with attestations — closes the
#   "Missing supply chain attestation(s)" Scout finding):
#     docker buildx build \
#       --sbom=true --provenance=mode=max \
#       -t hivehub/synap:1.0.0 -t hivehub/synap:latest --load .
#
#   Build multi-arch (AMD64 + ARM64) and push with attestations:
#     docker buildx build --platform linux/amd64,linux/arm64 \
#       --sbom=true --provenance=mode=max \
#       -t hivehub/synap:1.0.0 -t hivehub/synap:latest --push .
#
#   Run container (all three protocols):
#     docker run -d --name synap-server \
#       -p 15500:15500 -p 15501:15501 -p 6379:6379 \
#       -v synap-data:/data \
#       synap:latest
#
#   Run with authentication enabled:
#     docker run -d --name synap-server \
#       -p 15500:15500 -p 15501:15501 -p 6379:6379 \
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
#       -p 15500:15500 -p 15501:15501 -p 6379:6379 \
#       -v synap-data:/data \
#       -v /path/to/config.yml:/app/config.yml:ro \
#       synap:latest
#
#   View logs:
#     docker logs -f synap-server
#
#   Stop/remove:
#     docker stop synap-server && docker rm synap-server
# ============================================================================

# ============================================================================
# Stage 1: Builder
# ============================================================================
FROM rust:1-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    gcc \
    make \
    perl

# Install nightly toolchain + musl target in one layer to avoid
# overlayfs cross-device rename errors when rustup syncs channels.
ARG TARGETARCH
RUN case ${TARGETARCH} in \
    amd64) TARGET_TRIPLE=x86_64-unknown-linux-musl ;; \
    arm64) TARGET_TRIPLE=aarch64-unknown-linux-musl ;; \
    *) TARGET_TRIPLE=x86_64-unknown-linux-musl ;; \
    esac && \
    rustup toolchain install nightly --target ${TARGET_TRIPLE} && \
    rustup default nightly

# Set working directory
WORKDIR /usr/src/synap

# Configure Cargo for optimized builds
ENV CARGO_INCREMENTAL=1
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_NET_RETRY=2

# Copy Cargo configuration
COPY .cargo/config.toml ./.cargo/config.toml

# Copy manifest files first (for better Docker layer caching)
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/synap-core/Cargo.toml ./crates/synap-core/
COPY crates/synap-protocol/Cargo.toml ./crates/synap-protocol/
COPY crates/synap-server/Cargo.toml ./crates/synap-server/
COPY crates/synap-cli/Cargo.toml ./crates/synap-cli/
COPY crates/synap-migrate/Cargo.toml ./crates/synap-migrate/
COPY sdks/rust/Cargo.toml ./sdks/rust/

# Copy source code (needed for cargo to validate workspace)
COPY crates/synap-core/src ./crates/synap-core/src
COPY crates/synap-protocol/src ./crates/synap-protocol/src
COPY crates/synap-server/src ./crates/synap-server/src
COPY crates/synap-cli/src ./crates/synap-cli/src
COPY crates/synap-migrate/src ./crates/synap-migrate/src
COPY sdks/rust/src ./sdks/rust/src

# Remove benchmark declarations from Cargo.toml for Docker build
# (benchmarks are not needed for production image)
RUN sed -i '/^# Configure benchmarks to use Criterion/,/^$/d' crates/synap-server/Cargo.toml && \
    sed -i '/^\[\[bench\]\]/,/^$/d' crates/synap-server/Cargo.toml

# Build release binary with optimizations
# - Static linking for portability (musl target)
# - LTO + strip for smallest binary
# - --bins flag skips benches/examples/tests
# - BuildKit cache mounts for incremental compilation
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/src/synap/target \
    case ${TARGETARCH} in \
    amd64) TARGET_TRIPLE=x86_64-unknown-linux-musl ;; \
    arm64) TARGET_TRIPLE=aarch64-unknown-linux-musl ;; \
    esac && \
    cargo build --release --bins \
    --target ${TARGET_TRIPLE} && \
    strip /usr/src/synap/target/${TARGET_TRIPLE}/release/synap-server && \
    cp /usr/src/synap/target/${TARGET_TRIPLE}/release/synap-server /usr/src/synap/synap-server-binary

# ============================================================================
# Stage 2: Root filesystem prep
# ============================================================================
# Assembles everything the scratch image needs with the right ownership:
# passwd/group entries for the non-root user, the CA bundle for outbound TLS,
# and the /data tree. Done here because scratch has no shell to run RUN steps.
FROM alpine:3.22 AS rootfs
RUN apk add --no-cache ca-certificates && \
    echo 'synap:x:1000:' > /rootfs-group && \
    echo 'synap:x:1000:1000::/app:/sbin/nologin' > /rootfs-passwd && \
    mkdir -p /rootfs-data/wal /rootfs-data/snapshots && \
    chown -R 1000:1000 /rootfs-data

# ============================================================================
# Stage 3: Runtime — FROM scratch
# ============================================================================
# The Synap binary is a fully static musl build, so the runtime image carries
# NOTHING but the binary, its config, the CA bundle and the user database:
# no distro packages, no shell, no package manager — zero distro CVE surface
# (previously the debian-base:trixie-dev base shipped 114 packages / 38 CVEs).
# The HEALTHCHECK runs the server binary itself (`--health-check`, a plain-std
# HTTP probe) since there is no wget/shell.
FROM scratch

# Build metadata
ARG BUILD_DATE
ARG VERSION
LABEL org.opencontainers.image.title="Synap"
LABEL org.opencontainers.image.description="High-performance in-memory key-value store and message broker"
LABEL org.opencontainers.image.version="${VERSION}"
LABEL org.opencontainers.image.created="${BUILD_DATE}"
LABEL org.opencontainers.image.source="https://github.com/hivellm/synap"
LABEL org.opencontainers.image.vendor="HiveLLM"
LABEL org.opencontainers.image.licenses="Apache-2.0"

# Minimal user database (non-root) + CA bundle + data tree.
COPY --from=rootfs /rootfs-passwd /etc/passwd
COPY --from=rootfs /rootfs-group /etc/group
COPY --from=rootfs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=rootfs --chown=1000:1000 /rootfs-data /data

# Binary + default configuration.
COPY --from=builder --chown=1000:1000 /usr/src/synap/synap-server-binary /usr/local/bin/synap-server
COPY --chown=1000:1000 config/config.docker.yml /app/config.yml

WORKDIR /app
USER synap

# Expose ports
# 15500: HTTP/REST API + StreamableHTTP + WebSocket
# 15501: SynapRPC binary protocol (MessagePack/TCP)
# 6379:  RESP3 protocol (Redis-compatible wire protocol)
EXPOSE 15500 15501 6379

# Health check via the server binary's built-in probe (no shell/wget needed).
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/synap-server", "--health-check"]

# Volume mounts for persistence
VOLUME ["/data"]

# Default command
CMD ["/usr/local/bin/synap-server", "--config", "/app/config.yml"]

