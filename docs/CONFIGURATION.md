# Synap Configuration Guide

**Version**: 0.1.0-alpha  
**Format**: YAML (Redis-compatible style)

---

## Overview

Synap uses YAML configuration files similar to Redis `redis.conf`. The configuration system supports:

- **Multiple config files** for different environments
- **CLI overrides** for quick changes
- **Environment variables** for secrets
- **Redis-compatible** naming and structure

---

## Configuration Files

### Available Configs

| File | Purpose | Use Case |
|------|---------|----------|
| `config.yml` | Default config | General usage |
| `config.example.yml` | Template | Copy to create custom configs |
| `config.development.yml` | Development | Local development with verbose logging |
| `config.production.yml` | Production | Optimized for production deployment |

---

## Loading Configuration

### From File

```bash
# Use default config.yml
synap-server

# Use specific config file
synap-server --config config.production.yml

# Use custom config
synap-server --config /etc/synap/custom.yml
```

### CLI Overrides

```bash
# Override host
synap-server --host 192.168.1.100

# Override port
synap-server --port 8080

# Override multiple settings
synap-server --config config.yml --host 0.0.0.0 --port 15500
```

### Priority Order

1. **CLI arguments** (highest priority)
2. **Configuration file**
3. **Default values** (lowest priority)

---

## Configuration Sections

### Network Configuration

```yaml
server:
  # Bind address (0.0.0.0 = all interfaces, 127.0.0.1 = localhost only)
  host: "0.0.0.0"
  
  # Port number (default: 15500)
  port: 15500
  
  # WebSocket support (Phase 2)
  websocket_enabled: false
```

**Examples**:

```yaml
# Localhost only (development)
host: "127.0.0.1"

# All interfaces (production)
host: "0.0.0.0"

# Specific interface
host: "192.168.1.100"
```

---

### Memory Management

```yaml
kv_store:
  # Maximum memory in MB (Redis maxmemory equivalent)
  max_memory_mb: 4096
  
  # Eviction policy (Redis maxmemory-policy equivalent)
  # Options: lru, lfu, ttl, none
  eviction_policy: "lru"
  
  # TTL cleanup interval in milliseconds
  ttl_cleanup_interval_ms: 100
```

#### Memory Limits

**Common Values**:
```yaml
# Small cache (256MB)
max_memory_mb: 256

# Medium cache (1GB)
max_memory_mb: 1024

# Large cache (4GB)
max_memory_mb: 4096

# Very large cache (8GB)
max_memory_mb: 8192

# No limit (use all available RAM)
max_memory_mb: 0  # Not recommended
```

#### Eviction Policies

**LRU (Least Recently Used)** - Recommended for caches
```yaml
eviction_policy: "lru"
```
- Evicts keys that haven't been accessed recently
- Good for caching scenarios
- Respects access patterns

**LFU (Least Frequently Used)**
```yaml
eviction_policy: "lfu"
```
- Evicts keys that are accessed least often
- Good for stable workloads
- Tracks access count

**TTL (Time-To-Live)**
```yaml
eviction_policy: "ttl"
```
- Evicts keys with shortest TTL first
- Good for time-sensitive data
- Respects expiration times

**None (No Eviction)**
```yaml
eviction_policy: "none"
```
- Returns error when memory limit reached
- No automatic eviction
- Strict memory control

---

### Logging Configuration

```yaml
logging:
  # Log level: trace, debug, info, warn, error
  level: "info"
  
  # Format: json (production) or pretty (development)
  format: "json"
```

#### Log Levels

**Development**:
```yaml
level: "debug"  # Verbose, shows all operations
```

**Production**:
```yaml
level: "info"  # Standard production logging
```

**Troubleshooting**:
```yaml
level: "trace"  # Very verbose, for debugging
```

**Quiet**:
```yaml
level: "warn"  # Only warnings and errors
```

#### Log Formats

**JSON Format** (Production):
```yaml
format: "json"
```

Output example:
```json
{
  "timestamp": "2025-10-21T14:06:19.509476Z",
  "level": "INFO",
  "target": "synap_server",
  "fields": {"message": "Starting Synap Server v0.1.0-alpha"},
  "file": "synap-server/src/main.rs",
  "line": 75,
  "thread_name": "main",
  "thread_id": 12345
}
```

**Pretty Format** (Development):
```yaml
format: "pretty"
```

Output example:
```
  2025-10-21T14:06:19.509476Z  INFO synap_server: Starting Synap Server v0.1.0-alpha
    at synap-server/src/main.rs:75 on main
```

---

### Protocol Configuration

```yaml
protocols:
  # StreamableHTTP Protocol
  streamable_http:
    enabled: true
    path: "/api/v1/command"
  
  # REST API
  rest:
    enabled: true
    prefix: "/kv"
```

**Enable/Disable Protocols**:
```yaml
# Disable StreamableHTTP
streamable_http:
  enabled: false

# Change REST prefix
rest:
  prefix: "/api/kv"
```

---

## Environment-Specific Configs

### Development Configuration

`config.development.yml`:
```yaml
server:
  host: "127.0.0.1"  # Localhost only
  port: 15500

kv_store:
  max_memory_mb: 512  # Lower memory

logging:
  level: "debug"     # Verbose
  format: "pretty"   # Readable

# Use: synap-server --config config.development.yml
```

### Production Configuration

`config.production.yml`:
```yaml
server:
  host: "0.0.0.0"    # All interfaces
  port: 15500

kv_store:
  max_memory_mb: 8192  # 8GB

logging:
  level: "info"      # Standard
  format: "json"     # Structured

# Use: synap-server --config config.production.yml
```

---

## Environment Variables

### RUST_LOG

Override log level at runtime:

```bash
# Set specific level
RUST_LOG=debug synap-server

# Module-specific logging
RUST_LOG=synap_server=trace,axum=info synap-server

# Multiple targets
RUST_LOG=synap_server::core=debug,synap_server::server=info synap-server
```

### Configuration Priority

```
CLI Args > Environment Variables > Config File > Defaults
```

---

## Complete Configuration Reference

```yaml
# Network
server:
  host: "0.0.0.0"
  port: 15500
  websocket_enabled: false

# Memory (Redis-compatible)
kv_store:
  max_memory_mb: 4096
  eviction_policy: "lru"  # lru, lfu, ttl, none
  ttl_cleanup_interval_ms: 100

# Logging
logging:
  level: "info"    # trace, debug, info, warn, error
  format: "json"   # json, pretty

# Protocols
protocols:
  streamable_http:
    enabled: true
    path: "/api/v1/command"
  rest:
    enabled: true
    prefix: "/kv"

# Future configurations (Phase 2+)
# persistence:
#   enabled: false
#   wal:
#     path: "./data/wal"
#     fsync_mode: "periodic"
#   snapshot:
#     path: "./data/snapshots"
#     interval_secs: 300

# replication:
#   mode: "master"
#   master_host: null
#   sync_interval_ms: 100

# security:
#   require_auth: false
#   tls:
#     enabled: false
```

---

## Usage Examples

### Localhost Development

```bash
synap-server --config config.development.yml
```

Features:
- Binds to 127.0.0.1 only
- 512MB memory limit
- Debug logging (verbose)
- Pretty format (colored, readable)

### Production Deployment

```bash
synap-server --config config.production.yml
```

Features:
- Binds to all interfaces (0.0.0.0)
- 8GB memory limit
- Info logging (balanced)
- JSON format (structured)

### Custom Port

```bash
synap-server --port 8080
```

### Custom Memory Limit

Edit `config.yml`:
```yaml
kv_store:
  max_memory_mb: 2048  # 2GB
```

---

## Tracing Features

### JSON Logging Features

When `format: "json"`, logs include:

- ✅ **Timestamp**: ISO 8601 format
- ✅ **Level**: TRACE, DEBUG, INFO, WARN, ERROR
- ✅ **Target**: Module path (e.g., `synap_server::core`)
- ✅ **Message**: Log message
- ✅ **File**: Source file path
- ✅ **Line**: Line number
- ✅ **Thread ID**: System thread ID
- ✅ **Thread Name**: Thread name (e.g., "main", "tokio-runtime-worker")
- ✅ **Span Context**: Current tracing span

### Pretty Logging Features

When `format: "pretty"`, logs include:

- ✅ **Colored output**: Level-based colors
- ✅ **Timestamp**: Human-readable
- ✅ **Target**: Module name
- ✅ **File/Line**: Source location
- ✅ **Thread**: Thread name
- ✅ **Indentation**: Span hierarchy

---

## Performance Tuning

### High-Performance Setup

```yaml
logging:
  level: "warn"  # Minimal logging
  format: "json"  # Fast serialization

kv_store:
  max_memory_mb: 16384  # 16GB
  eviction_policy: "lru"
  ttl_cleanup_interval_ms: 1000  # Less frequent cleanup
```

### Debug Setup

```yaml
logging:
  level: "trace"  # Everything
  format: "pretty"  # Readable

kv_store:
  max_memory_mb: 256  # Small for testing
  ttl_cleanup_interval_ms: 10  # Frequent cleanup
```

---

## Validation

### Test Configuration

```bash
# Dry-run to test config
synap-server --config config.yml --help

# Test with timeout
timeout 5s synap-server --config config.yml
```

### Check Logs

```bash
# JSON format (pipe to jq)
RUST_LOG=info synap-server --config config.production.yml 2>&1 | jq

# Pretty format (colored output)
RUST_LOG=debug synap-server --config config.development.yml
```

---

## See Also

- [Development Guide](DEVELOPMENT.md)
- [CLI Guide](CLI_GUIDE.md)
- [Architecture](ARCHITECTURE.md)
- [Redis Configuration Reference](https://redis.io/docs/management/config/)

---

**Last Updated**: October 21, 2025  
**Status**: Configuration system fully implemented

