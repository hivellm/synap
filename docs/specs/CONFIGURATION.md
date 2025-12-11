# Configuration Reference

## Configuration File

Synap uses YAML configuration files. Default: `config.yml`

## Complete Configuration Example

```yaml
# Server Configuration
server:
  role: "master"              # "master" or "replica"
  host: "0.0.0.0"            # Bind address
  port: 15500                # HTTP API port
  workers: 0                 # Tokio worker threads (0 = auto)
  max_connections: 100000    # Maximum concurrent connections
  
  # TLS/SSL (optional)
  tls:
    enabled: false
    cert_path: "/etc/synap/tls/cert.pem"
    key_path: "/etc/synap/tls/key.pem"

# Protocol Configuration
protocol:
  http2: true                     # Enable HTTP/2
  http2_max_concurrent_streams: 1000
  websocket_enabled: true         # Enable WebSocket upgrade
  max_request_size_mb: 10         # Maximum request size
  max_response_size_mb: 10        # Maximum response size
  request_timeout_secs: 30        # Request timeout
  websocket_idle_timeout_secs: 300
  compression: false              # Gzip compression
  compression_threshold: 1024     # Compress if > 1KB
  format: "json"                  # "json" or "msgpack"

# Authentication
auth:
  enabled: true
  api_keys:
    - key: "synap_production_key"
      name: "production-app"
      role: "admin"
      rate_limit: 10000  # requests per minute
    - key: "synap_readonly_key"
      name: "analytics"
      role: "readonly"
      rate_limit: 50000
  
  jwt:
    enabled: false
    secret: "your-jwt-secret"
    expiration_secs: 3600

# Key-Value Store
kv_store:
  enabled: true
  max_keys: 10000000              # Maximum number of keys
  max_memory_mb: 4096             # Memory limit
  eviction_policy: "lru"          # "none", "lru", "lfu", "ttl"
  eviction_sample_size: 100       # Sample size for eviction
  ttl_cleanup_interval_ms: 100    # TTL cleanup frequency
  
  # Compression
  compression:
    enabled: false
    threshold_bytes: 1024
    algorithm: "lz4"  # "lz4", "zstd"

# Queue System
queue:
  enabled: true
  max_queues: 10000               # Maximum number of queues
  default_max_depth: 100000       # Default queue capacity
  default_ack_deadline_secs: 30   # Default ACK deadline
  default_max_retries: 3          # Default max retries
  default_priority: 5             # Default priority (0-9)
  
  # Dead Letter Queue
  dead_letter:
    enabled: true
    max_size: 10000
    retention_hours: 24
  
  # Prefetch
  prefetch:
    default: 10
    max: 1000

# Event Stream
event_stream:
  enabled: true
  retention_mode: "hybrid"        # "time", "count", "hybrid"
  retention_hours: 24             # Time-based retention
  max_events_per_room: 100000     # Count-based retention
  max_rooms: 100000               # Maximum concurrent rooms
  room_cleanup_interval_mins: 60  # Cleanup frequency
  room_inactive_timeout_hours: 24 # Remove inactive rooms
  max_subscribers_per_room: 10000
  broadcast_timeout_ms: 1000      # Broadcast timeout

# Pub/Sub
pubsub:
  enabled: true
  max_topics: 1000000             # Maximum topics
  max_subscribers_per_topic: 10000
  max_wildcard_subscriptions: 10000
  delivery_timeout_ms: 1000
  topic_cleanup_interval_mins: 60
  inactive_topic_timeout_hours: 24

# Persistence
persistence:
  enabled: true
  
  # Write-Ahead Log
  wal:
    enabled: true
    path: "/var/lib/synap/wal/synap.wal"
    buffer_size_kb: 64
    fsync_mode: "periodic"        # "always", "periodic", "never"
    fsync_interval_ms: 1000
    max_size_mb: 1024
  
  # Snapshots
  snapshot:
    enabled: true
    directory: "/var/lib/synap/snapshots"
    interval_secs: 300            # Every 5 minutes
    operation_threshold: 10000    # Or every 10K operations
    max_snapshots: 10             # Keep last 10 snapshots
    compression: true
    compression_level: 6
  
  # Recovery
  recovery:
    verify_checksums: true
    skip_corrupted: false         # Fail on corruption
    repair_mode: false            # Try to repair if true

# Replication
replication:
  enabled: true
  mode: "master"                  # "master" or "replica"
  
  # Master settings
  listen_host: "0.0.0.0"
  listen_port: 15501
  log_retention_hours: 24
  log_max_entries: 1000000
  min_replicas_for_write: 0       # Sync replication threshold
  replica_timeout_secs: 30
  heartbeat_interval_secs: 5
  
  # Replica settings
  master_host: "master.synap.local"
  master_port: 15501
  sync_interval_ms: 100
  reconnect_interval_secs: 5
  reconnect_max_attempts: 0       # 0 = infinite
  full_sync_threshold: 10000      # Full sync if lag > 10K
  
  # Performance
  batch_size: 100
  batch_timeout_ms: 10
  compression: true

# Monitoring
monitoring:
  enabled: true
  prometheus_port: 9090
  health_check_interval_secs: 10
  
  metrics:
    - operations_total
    - operation_duration
    - memory_usage
    - queue_depth
    - replication_lag

# Logging
logging:
  level: "info"  # "trace", "debug", "info", "warn", "error"
  format: "json" # "json" or "text"
  output: "stdout"  # "stdout", "stderr", or file path
  
  # Structured logging
  structured: true
  include_timestamps: true
  include_thread_ids: false

# Resource Limits
limits:
  max_memory_gb: 8
  max_cpu_percent: 80
  max_connections: 100000
  max_request_size_mb: 10
  rate_limit:
    enabled: true
    requests_per_minute: 60000
    burst: 1000
```

## Environment Variables

Override config with environment variables:

```bash
# Server settings
export SYNAP_HOST=0.0.0.0
export SYNAP_PORT=15500
export SYNAP_ROLE=master

# Authentication
export SYNAP_API_KEY=synap_your_key

# Replication
export SYNAP_MASTER_HOST=master.synap.local
export SYNAP_MASTER_PORT=15501

# Logging
export RUST_LOG=synap=info
export SYNAP_LOG_FORMAT=json

# Tokio
export TOKIO_WORKER_THREADS=16
```

## Configuration Precedence

1. Command-line arguments (highest)
2. Environment variables
3. Configuration file
4. Default values (lowest)

**Example**:
```bash
./synap-server \
  --config config.yml \       # Config file
  --port 15500 \              # CLI override
  --role master
  
# SYNAP_PORT env var would be ignored (CLI takes precedence)
```

## Component Configuration

### Minimal Configuration (Development)

```yaml
server:
  port: 15500

kv_store:
  enabled: true

queue:
  enabled: true

event_stream:
  enabled: true

pubsub:
  enabled: true
```

### Production Configuration

```yaml
server:
  role: "master"
  host: "0.0.0.0"
  port: 15500
  workers: 16
  max_connections: 100000
  
  tls:
    enabled: true
    cert_path: "/etc/synap/tls/cert.pem"
    key_path: "/etc/synap/tls/key.pem"

protocol:
  http2: true
  compression: true
  compression_threshold: 1024

auth:
  enabled: true
  api_keys:
    - key: "${SYNAP_API_KEY}"
      role: "admin"

kv_store:
  max_memory_mb: 32768  # 32 GB
  eviction_policy: "lru"

replication:
  enabled: true
  mode: "master"
  listen_port: 15501
  log_retention_hours: 24

monitoring:
  enabled: true
  prometheus_port: 9090

logging:
  level: "info"
  format: "json"
  structured: true
```

### Replica Configuration

```yaml
server:
  role: "replica"
  host: "0.0.0.0"
  port: 15500
  read_only: true

replication:
  enabled: true
  mode: "replica"
  master_host: "master.synap.internal"
  master_port: 15501
  sync_interval_ms: 100

# Same settings for other components
kv_store:
  max_memory_mb: 32768

monitoring:
  enabled: true
```

## Validation

### Check Configuration

```bash
# Validate config file
synap-server --config config.yml --validate

# Show effective configuration
synap-server --config config.yml --show-config
```

### Configuration Errors

Common validation errors:

```yaml
# Error: Invalid port
server:
  port: 70000  # Must be 1-65535

# Error: Invalid eviction policy
kv_store:
  eviction_policy: "invalid"  # Must be: none, lru, lfu, ttl

# Error: Role mismatch
server:
  role: "replica"
replication:
  mode: "master"  # Must match server.role
```

## Performance Tuning

### For High Throughput

```yaml
server:
  workers: 16
  max_connections: 100000

protocol:
  http2: true
  http2_max_concurrent_streams: 1000

replication:
  batch_size: 200
  batch_timeout_ms: 5
```

### For Low Latency

```yaml
protocol:
  http2: false  # HTTP/1.1 has lower latency
  compression: false

replication:
  batch_size: 10
  batch_timeout_ms: 1
  
kv_store:
  ttl_cleanup_interval_ms: 50
```

### For High Availability

```yaml
replication:
  enabled: true
  min_replicas_for_write: 1  # Wait for 1 replica ACK
  replica_timeout_secs: 10

monitoring:
  enabled: true
  health_check_interval_secs: 5
```

## See Also

- [DEPLOYMENT.md](DEPLOYMENT.md) - Deployment strategies
- [PERFORMANCE.md](PERFORMANCE.md) - Performance tuning
- [DEVELOPMENT.md](DEVELOPMENT.md) - Development setup

