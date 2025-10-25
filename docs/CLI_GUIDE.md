# Synap CLI Guide

**Version**: 0.1.0-alpha  
**Interface**: Redis-compatible command-line interface

---

## Overview

`synap-cli` is a Redis-compatible command-line interface for interacting with Synap servers. It supports both interactive mode (like `redis-cli`) and command mode for scripting.

---

## Installation

### From Binary

```bash
# After building from source
cargo build --release

# Binary locations
./target/release/synap-server  # Server
./target/release/synap-cli      # CLI client
```

### Add to PATH

```bash
# Linux/macOS
export PATH="$PATH:/path/to/synap/target/release"

# Or copy to system path
sudo cp target/release/synap-cli /usr/local/bin/
```

---

## Usage Modes

### 1. Interactive Mode (Default)

Start interactive REPL session:

```bash
synap-cli
# or
synap-cli -h 127.0.0.1 -p 15500
```

**Interactive Session Example**:
```
Synap CLI v0.1.0-alpha
Connected to 127.0.0.1:15500
Type HELP for available commands

synap 127.0.0.1:15500> SET mykey "Hello World"
OK
(220.45µs)

synap 127.0.0.1:15500> GET mykey
"Hello World"
(185.32µs)

synap 127.0.0.1:15500> QUIT
Goodbye!
```

### 2. Command Mode

Execute single command and exit:

```bash
# SET command
synap-cli SET mykey "value"

# GET command
synap-cli GET mykey

# Multiple arguments
synap-cli MSET key1 val1 key2 val2

# With custom host/port
synap-cli -h 192.168.1.100 -p 15500 GET mykey
```

---

## CLI Options

```
Options:
  -h, --host <HOST>      Server host [default: 127.0.0.1]
  -p, --port <PORT>      Server port [default: 15500]
  --help                 Print help message
```

---

## Available Commands

### Basic Commands

#### SET - Set Key-Value

```bash
SET key value [ttl]
```

**Arguments**:
- `key` - Key name (string)
- `value` - Value to store (string)
- `ttl` - Optional: time-to-live in seconds (integer)

**Examples**:
```bash
# Simple set
SET user:1 "Alice"

# With TTL (expires in 3600 seconds)
SET session:abc "token123" 3600

# Numeric value
SET counter 42
```

**Returns**: `OK` on success

---

#### GET - Get Value

```bash
GET key
```

**Examples**:
```bash
GET user:1
# Output: "Alice"

GET nonexistent
# Output: (nil)
```

**Returns**: Value if found, `(nil)` if not found

---

#### DEL / DELETE - Delete Keys

```bash
DEL key [key ...]
```

**Examples**:
```bash
# Delete single key
DEL user:1

# Delete multiple keys
DEL user:1 user:2 user:3
```

**Returns**: `(integer) N` - number of keys deleted

---

#### EXISTS - Check if Key Exists

```bash
EXISTS key
```

**Examples**:
```bash
EXISTS user:1
# Output: (integer) 1  (exists)

EXISTS nonexistent
# Output: (integer) 0  (not found)
```

**Returns**: `(integer) 1` if exists, `(integer) 0` if not

---

### Counter Commands

#### INCR - Increment

```bash
INCR key [amount]
```

**Examples**:
```bash
# Increment by 1 (default)
INCR views

# Increment by custom amount
INCR views 5
```

**Returns**: `(integer) N` - new value after increment

---

#### DECR - Decrement

```bash
DECR key [amount]
```

**Examples**:
```bash
# Decrement by 1
DECR counter

# Decrement by custom amount
DECR counter 10
```

**Returns**: `(integer) N` - new value after decrement

---

### TTL Commands

#### EXPIRE - Set Expiration

```bash
EXPIRE key seconds
```

**Examples**:
```bash
# Expire in 60 seconds
EXPIRE session:123 60

# Expire in 1 hour
EXPIRE cache:data 3600
```

**Returns**: `(integer) 1` if TTL set, `(integer) 0` if key doesn't exist

---

#### TTL - Get Remaining Time

```bash
TTL key
```

**Examples**:
```bash
TTL session:123
# Output: (integer) 45  (45 seconds remaining)
```

**Returns**: 
- `(integer) N` - seconds remaining
- `(integer) -1` - key exists but no expiration
- `(integer) -2` - key doesn't exist

---

#### PERSIST - Remove Expiration

```bash
PERSIST key
```

**Examples**:
```bash
PERSIST session:123
```

**Returns**: `(integer) 1` if expiration removed, `(integer) 0` if key doesn't exist

---

### Key Discovery

#### KEYS - List All Keys

```bash
KEYS [pattern]
```

**Examples**:
```bash
# List all keys
KEYS

# List keys with prefix
KEYS user:
```

**Returns**: List of matching keys (numbered)
```
1) "user:1"
2) "user:2"
3) "user:3"
```

---

#### SCAN - Scan Keys with Prefix

```bash
SCAN [pattern] [count]
```

**Examples**:
```bash
# Scan all keys
SCAN

# Scan with prefix
SCAN user:

# Scan with limit
SCAN user: 100
```

**Returns**: List of matching keys

---

#### DBSIZE - Get Number of Keys

```bash
DBSIZE
```

**Returns**: `(integer) N` - total number of keys

---

### Batch Commands

#### MSET - Set Multiple Keys

```bash
MSET key1 value1 [key2 value2 ...]
```

**Examples**:
```bash
# Set multiple keys atomically
MSET user:1 "Alice" user:2 "Bob" user:3 "Charlie"
```

**Returns**: `OK`

---

#### MGET - Get Multiple Values

```bash
MGET key [key ...]
```

**Examples**:
```bash
MGET user:1 user:2 user:3
```

**Returns**:
```
1) "Alice"
2) "Bob"
3) "Charlie"
```

---

### Database Commands

#### FLUSHDB - Clear Database

```bash
FLUSHDB
```

**Warning**: Deletes ALL keys from the database!

**Returns**: `OK`

---

#### FLUSHALL - Clear All Databases

```bash
FLUSHALL
```

**Warning**: Deletes ALL keys from all databases!

**Returns**: `OK`

---

### Server Commands

#### PING - Test Connection

```bash
PING
```

**Returns**: `PONG` if server is healthy

---

#### INFO / STATS - Get Server Statistics

```bash
INFO
# or
STATS
```

**Returns**:
```
# Keyspace
keys: 1000
memory: 524288 bytes

# Operations
gets: 5000
sets: 1000
dels: 50
hits: 4500
misses: 500
hit_rate: 90.00%
```

---

#### HELP - Show Available Commands

```bash
HELP
```

**Returns**: List of all available commands with descriptions

---

#### QUIT / EXIT - Exit CLI

```bash
QUIT
# or
EXIT
```

---

## Interactive Mode Features

### Command History

- **Up/Down arrows**: Navigate command history
- **Ctrl+R**: Reverse search in history
- **History persistence**: Commands saved between sessions

### Auto-completion (Planned)

Future versions will support TAB completion for:
- Command names
- Key names
- Common patterns

### Shortcuts

- **Ctrl+C**: Interrupt current command
- **Ctrl+D**: Exit (same as QUIT)

---

## Scripting Examples

### Batch Operations

```bash
#!/bin/bash

# Load test data
for i in {1..1000}; do
    synap-cli SET user:$i "User $i"
done

# Query data
synap-cli KEYS user:
synap-cli DBSIZE
```

### Health Check Script

```bash
#!/bin/bash

if synap-cli PING | grep -q "PONG"; then
    echo "Synap server is healthy"
    exit 0
else
    echo "Synap server is down"
    exit 1
fi
```

### Backup Script (Planned)

```bash
#!/bin/bash

# Get all keys
keys=$(synap-cli KEYS | grep -E '^[0-9]+\)')

# Export to file
for key in $keys; do
    value=$(synap-cli GET "$key")
    echo "SET $key \"$value\"" >> backup.txt
done
```

---

## Performance

### Timing Information

Each command displays execution time:

```bash
synap> GET mykey
"value"
(185.32µs)  ← Execution time
```

Typical latencies:
- Local connection: 100-500µs (includes network)
- Core operations: 200-300ns (benchmark)

---

## Configuration

### Connection Settings

Connect to specific server:

```bash
synap-cli -h 192.168.1.100 -p 15500
```

### Environment Variables

```bash
# Set default host
export SYNAP_HOST=127.0.0.1

# Set default port
export SYNAP_PORT=15500
```

---

## Comparison with redis-cli

### Compatible Commands

✅ Supported and compatible:
- SET, GET, DEL, EXISTS
- INCR, DECR
- EXPIRE, TTL, PERSIST
- KEYS, SCAN, DBSIZE
- MSET, MGET
- FLUSHDB, FLUSHALL
- PING, INFO

⏳ Planned for Phase 2:
- LPUSH, RPUSH, LPOP, RPOP (Lists)
- PUBLISH, SUBSCRIBE (Pub/Sub)
- MULTI, EXEC (Transactions)

❌ Not planned:
- Redis-specific: BGSAVE, SAVE, SHUTDOWN
- Cluster commands: CLUSTER, READONLY

---

## Troubleshooting

### Connection Issues

**Error**: Connection refused

```bash
# Check if server is running
ps aux | grep synap-server

# Start server
synap-server
```

**Error**: Wrong host/port

```bash
# Verify server address
synap-cli -h 127.0.0.1 -p 15500 PING
```

### Command Errors

**Error**: Unknown command

- Check spelling (commands are case-insensitive)
- Type `HELP` for available commands
- Some Redis commands not yet implemented

**Error**: Wrong number of arguments

- Check command syntax with `HELP`
- Example: `SET` requires at least 2 arguments

---

## Advanced Usage

### Pipeline (Future)

Not yet implemented, but planned:

```bash
# Future: pipe commands
echo -e "SET key1 val1\nGET key1" | synap-cli
```

### Output Formats (Future)

```bash
# Future: JSON output for scripting
synap-cli --output json GET mykey
```

---

## See Also

- [Development Guide](DEVELOPMENT.md)
- [API Reference](api/REST_API.md)
- [Configuration](specs/CONFIGURATION.md)
- [Benchmarks](BENCHMARK_RESULTS.md)

---

**Last Updated**: October 21, 2025  
**Status**: Phase 1 Complete - CLI Fully Functional

