# synap-cli

Interactive command-line client for the Synap server — a Redis-style REPL
and single-shot command executor over HTTP/REST.

## Building

```bash
# Debug
cargo build -p synap-cli

# Release
cargo build --release -p synap-cli
```

## Usage

```bash
# Interactive REPL (connects to localhost:15500)
./target/release/synap-cli

# Custom host / port
./target/release/synap-cli -h 10.0.0.1 -p 15500

# Single command, then exit
./target/release/synap-cli SET mykey "hello"
./target/release/synap-cli GET mykey
```

### Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--host` | `-h` | `127.0.0.1` | Server host |
| `--port` | `-p` | `15500` | Server port |

## Commands

### Basic

| Command | Description |
|---------|-------------|
| `SET key value [ttl]` | Set a key (optional TTL in seconds) |
| `GET key` | Get a value |
| `DEL key [key …]` | Delete one or more keys |
| `EXISTS key` | Check if a key exists |
| `INCR key` | Increment integer value |
| `DECR key` | Decrement integer value |

### TTL

| Command | Description |
|---------|-------------|
| `EXPIRE key seconds` | Set expiry on a key |
| `TTL key` | Get remaining TTL |
| `PERSIST key` | Remove expiry |

### Key discovery

| Command | Description |
|---------|-------------|
| `KEYS pattern` | List keys matching glob pattern |
| `SCAN cursor [pattern]` | Incremental key scan |
| `DBSIZE` | Number of keys in the database |

### Batch

| Command | Description |
|---------|-------------|
| `MSET key value [key value …]` | Set multiple keys at once |
| `MGET key [key …]` | Get values of multiple keys |

### Database

| Command | Description |
|---------|-------------|
| `FLUSHDB` | Remove all keys from current database |
| `FLUSHALL` | Remove all keys from all databases |

### Server

| Command | Description |
|---------|-------------|
| `INFO` / `STATS` | Server statistics |
| `PING` | Ping the server |
| `HELP` | Show command list |
| `QUIT` / `EXIT` | Exit the REPL |

## REPL prompt

```
synap 127.0.0.1:15500> SET foo bar
"OK"
(342.00µs)
synap 127.0.0.1:15500> GET foo
"bar"
(198.00µs)
```

Each response shows the result followed by the round-trip latency.
Command history is preserved across sessions via rustyline.

## Environment

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Log level (`info` by default) |

## Related crates

- [`synap-server`](../synap-server) — The server this CLI connects to
- [`synap-migrate`](../synap-migrate) — Data migration utility
- [`sdks/rust`](../sdks/rust) — Programmatic Rust SDK
