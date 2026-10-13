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
./target/release/synap-cli -H 10.0.0.1 -p 15500

# Other transports, with credentials
./target/release/synap-cli --url synap://10.0.0.1:15501 --user ops --password secret PING

# Single command, then exit
./target/release/synap-cli SET mykey "hello"
./target/release/synap-cli GET mykey
```

### Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--url` | `-u` | — | `http://`, `synap://` (SynapRPC) or `resp3://` URL; overrides host/port |
| `--host` | `-H` | `127.0.0.1` | Server host |
| `--port` | `-p` | `15500` | Server port |
| `--transport` | | `http` | Transport for host/port: `http`, `rpc`, `resp3` |
| `--user` | | — | Username (HTTP Basic auth, SynapRPC handshake); needs `--password` |
| `--password` | | — | Password for `--user` |
| `--timeout` | | `30` | Connection and request timeout, in seconds |

The RESP3 transport does not send credentials, so `--user`/`--password` with a
`resp3://` URL is refused rather than silently ignored.

## Commands

### Basic

| Command | Description |
|---------|-------------|
| `SET key value [ttl]` | Set a key (optional TTL in seconds) |
| `GET key` | Get a value |
| `DEL key [key …]` | Delete one or more keys |
| `EXISTS key` | Check if a key exists |
| `INCR key [amount]` | Increment integer value (by 1 without an amount) |
| `DECR key [amount]` | Decrement integer value (by 1 without an amount) |

An amount must be a 64-bit integer. A malformed amount (`INCR views ten`) is
refused before anything is sent, and the CLI exits non-zero. Zero and negative
amounts apply as given, identically over HTTP, SynapRPC and RESP3; a change
past the 64-bit range fails and leaves the counter unchanged.

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
| `PING` | Ping the server (protocol ping; see exit status below) |
| `HELP` | Show command list |
| `QUIT` / `EXIT` | Exit the REPL |

## Exit status

A single command exits `0` on success and `1` on failure, with the result on
stdout and the diagnostic on stderr. `PING` sends the protocol's own ping —
`PING` on SynapRPC/RESP3, the authenticated `ping` envelope command on HTTP —
and exits `0` only when the server answered `PONG`. A refused connection, a
timeout or rejected credentials exits non-zero:

```bash
if synap-cli --url http://127.0.0.1:15500 --timeout 5 PING >/dev/null; then
  echo healthy
else
  echo unhealthy   # the reason is on stderr
fi
```

Before 1.4.0 `PING` printed `Server unreachable` and exited `0` when the server
was down or the credentials were rejected.

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
