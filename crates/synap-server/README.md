# synap-server

The core Synap server binary — a high-performance in-memory data store and
message broker written in Rust.

## Building

```bash
# Debug
cargo build -p synap-server

# Release (optimised + stripped)
cargo build --release -p synap-server
```

## Running

```bash
# Default config
./target/release/synap-server

# Custom config file
./target/release/synap-server --config config.production.yml

# With environment overrides
SYNAP_PORT=15500 SYNAP_RPC_PORT=15501 ./synap-server
```

## Ports

| Port  | Protocol | Description |
|-------|----------|-------------|
| 15500 | HTTP/REST | JSON API, MCP, WebSocket |
| 15501 | SynapRPC  | MessagePack binary TCP (recommended) |
| 6379  | RESP3     | Redis-compatible text protocol |

## Configuration

See `config.example.yml` in the workspace root for all available options.
Key sections:

```yaml
server:
  host: "0.0.0.0"
  port: 15500
  rpc_port: 15501
  resp3_port: 6379

persistence:
  enabled: true
  data_dir: "./data"
  wal_sync_interval_ms: 100

cluster:
  enabled: false
  nodes: []
```

## Source layout

```
src/
├── core/               # Data store implementations
│   ├── kv_store/       # KV: 64-way sharded, TTL, LRU, persistence
│   ├── bitmap/         # Bitmap ops (SIMD-accelerated)
│   ├── hash.rs         # Hash maps (HSET/HGET/…)
│   ├── list.rs         # Doubly-linked lists (LPUSH/RPUSH/…)
│   ├── set.rs          # Hash sets (SADD/SMEMBERS/…)
│   ├── sorted_set.rs   # Skip-list sorted sets (ZADD/ZRANGE/…)
│   ├── hyperloglog.rs  # HyperLogLog (PFADD/PFCOUNT/PFMERGE)
│   └── geospatial.rs   # Geo index on top of sorted set
├── protocol/
│   ├── resp3/          # RESP3 TCP listener + command dispatcher
│   └── synap_rpc/      # SynapRPC TCP listener + dispatcher
├── server/
│   ├── handlers/       # Axum HTTP route handlers (17 modules)
│   └── router.rs       # Route registration
├── auth/               # API key + user authentication
├── monitoring/         # Metrics, INFO command, slow log
├── persistence/        # WAL + snapshot engine
├── scripting/          # Lua eval (mlua)
└── replication/        # Master/replica TCP sync
```

## Tests

```bash
# Unit + integration tests
cargo test -p synap-server

# With all features
cargo test -p synap-server --all-features

# S2S integration tests (requires running server)
cargo test -p synap-server --features s2s-tests
```

## Related crates

- [`synap-cli`](../synap-cli) — Interactive command-line client
- [`synap-migrate`](../synap-migrate) — Data migration utility
- [`sdks/rust`](../sdks/rust) — Official Rust SDK
