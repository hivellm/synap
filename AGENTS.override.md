<!-- OVERRIDE:START -->
# Project-Specific Overrides

Project-specific directives that take precedence over the generated AGENTS.md.
This file is never overwritten by `rulebook init` or `rulebook update`.

# Synap Project Guide

Synap is a high-performance in-memory key-value store and message broker built in Rust Edition 2024.

## Build Commands

```bash
# Build (requires Rust nightly 1.85+)
cargo build --release

# Tests (528+ tests)
cargo test
cargo test test_name                          # single test
cargo test --package synap-server test_name   # scoped
cargo test --features s2s-tests               # S2S (needs running server)

# Lint / format
cargo fmt
cargo clippy -- -D warnings

# Run server
./target/release/synap-server --config config.yml

# Benchmarks
cargo bench --bench kv_bench
```

## Docker

```bash
docker build -t synap:latest .
docker run -d -p 15500:15500 -p 15501:15501 -v synap-data:/data synap:latest
docker push hivehub/synap:latest
```

## Workspace Structure

```
synap/
├── synap-server/     # Main server binary and library
├── synap-cli/        # Command-line client
├── synap-migrate/    # Migration utilities
├── sdks/             # Client SDKs (rust, typescript, python, php, csharp)
└── gui/              # Electron-based dashboard
```

## synap-server Module Layers

```
server/          HTTP/WebSocket handlers, MCP/UMICP protocol servers
    router.rs        Main Axum router with all endpoints
    handlers.rs      Request handlers (11K+ lines)
    mcp_*.rs         Model Context Protocol implementation

core/            Data structures and business logic
    kv_store.rs      Radix-trie key-value store
    queue.rs         ACK-based message queues
    stream.rs        Kafka-style event streams
    pubsub.rs        Topic-based pub/sub
    hash.rs, list.rs, set.rs, sorted_set.rs   Redis-compatible structures
    bitmap.rs, hyperloglog.rs, geospatial.rs

persistence/     WAL + Snapshots for durability
    wal_async.rs     Async WAL with group commit
    snapshot.rs      Point-in-time snapshots

replication/     Master-slave replication
    master.rs        Accepts replica connections
    replica.rs       Connects to master

auth/            Authentication and authorization
    user.rs          User management (SHA512 passwords)
    api_key.rs       API key management
    acl.rs           Access control lists

hub/             HiveHub.Cloud integration (multi-tenant SaaS)
    client.rs        Hub API client
    multi_tenant.rs  Resource scoping
    quota.rs         Quota enforcement
```

## Key Patterns

**State Management**: shared state uses `Arc<RwLock<T>>` or `Arc<parking_lot::RwLock<T>>`.

**Error Handling**: use `Result<T, SynapError>` consistently. `SynapError` in `core/error.rs` covers all error cases.

**Handler Pattern**:
```rust
pub async fn handler(
    State(state): State<AppState>,
    Json(req): Json<Request>,
) -> Result<Json<Response>, SynapError> {
    let result = state.kv_store.operation(&req.key).await?;
    Ok(Json(Response { success: true, data: result }))
}
```

**Hub Integration**: when `hub.enabled=true`, resources are scoped per-user via `MultiTenant::scope_*()`. Hub auth uses access keys validated against HiveHub.Cloud API.

## Configuration

Server config in `config.yml`. Key sections:
- `server` — host, port
- `auth` — user / API key authentication
- `persistence` — WAL and snapshot settings
- `replication` — master-slave
- `hub` — HiveHub.Cloud integration (multi-tenant mode)

Environment variables override config: `SYNAP_AUTH_ENABLED=true`.

## Testing

- Unit tests: `#[cfg(test)]` modules in same file
- Integration tests: `synap-server/tests/`
- S2S tests (require running server): `cargo test --features s2s-tests`
- Benchmarks: `synap-server/benches/`

## Protocols

- **REST API** — HTTP endpoints at port 15500
- **MCP** — Model Context Protocol at `/mcp` for AI assistants
- **UMICP** — Universal Matrix Inter-Communication Protocol
- **WebSocket** — real-time subscriptions

## SDKs

Client libraries in `sdks/` for TypeScript, Python, Rust, PHP, and C#. Each has its own README.

<!-- OVERRIDE:END -->
