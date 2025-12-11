# Synap Administrator Guide

**Version**: 0.3.0  
**Last Updated**: October 22, 2025  
**Audience**: System Administrators, DevOps Engineers, SREs

---

## Table of Contents

1. [Deployment](#deployment)
2. [Configuration](#configuration)
3. [Monitoring & Observability](#monitoring--observability)
4. [Backup & Recovery](#backup--recovery)
5. [High Availability](#high-availability)
6. [Performance Tuning](#performance-tuning)
7. [Security](#security)
8. [Operations](#operations)
9. [Troubleshooting](#troubleshooting)

---

## Deployment

### Production Deployment Checklist

- [ ] Enable persistence (WAL + Snapshots)
- [ ] Configure authentication
- [ ] Setup monitoring (Prometheus + Grafana)
- [ ] Configure replication (1 master + N replicas)
- [ ] Setup reverse proxy (TLS/SSL)
- [ ] Configure resource limits
- [ ] Setup log aggregation
- [ ] Backup strategy defined
- [ ] Disaster recovery plan

### Deployment Options

#### Docker Production Setup

**docker-compose.yml** (Master + 2 Replicas):

```yaml
version: '3.8'

services:
  synap-master:
    image: hivellm/synap:latest
    container_name: synap-master
    ports:
      - "15500:15500"
      - "15501:15501"
    volumes:
      - ./config-master.yml:/etc/synap/config.yml
      - synap-master-data:/data
    restart: always
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15500/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  synap-replica-1:
    image: hivellm/synap:latest
    container_name: synap-replica-1
    ports:
      - "15502:15500"
    volumes:
      - ./config-replica.yml:/etc/synap/config.yml
      - synap-replica1-data:/data
    depends_on:
      - synap-master
    restart: always
    environment:
      - SYNAP_MASTER=synap-master:15501

  synap-replica-2:
    image: hivellm/synap:latest
    container_name: synap-replica-2
    ports:
      - "15503:15500"
    volumes:
      - ./config-replica.yml:/etc/synap/config.yml
      - synap-replica2-data:/data
    depends_on:
      - synap-master
    restart: always
    environment:
      - SYNAP_MASTER=synap-master:15501

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin

volumes:
  synap-master-data:
  synap-replica1-data:
  synap-replica2-data:
  prometheus-data:
  grafana-data:
```

#### Kubernetes Production Setup

**Using Helm**:

```bash
# Create namespace
kubectl create namespace synap

# Deploy master
helm install synap-master ./helm/synap \
  --namespace synap \
  --set replication.master.enabled=true \
  --set config.replication.role=master \
  --set persistence.size=50Gi \
  --set resources.limits.memory=8Gi \
  --set resources.limits.cpu=4000m

# Deploy replicas
helm install synap-replica ./helm/synap \
  --namespace synap \
  --set replication.replica.enabled=true \
  --set replication.replica.replicaCount=3 \
  --set config.replication.role=replica \
  --set persistence.size=50Gi
```

**With Ingress + TLS**:

```bash
helm install synap ./helm/synap \
  --set ingress.enabled=true \
  --set ingress.className=nginx \
  --set ingress.hosts[0].host=synap.example.com \
  --set ingress.tls[0].secretName=synap-tls \
  --set ingress.tls[0].hosts[0]=synap.example.com
```

#### Systemd Service (Linux)

**/etc/systemd/system/synap.service**:

```ini
[Unit]
Description=Synap High-Performance Data Store
After=network.target

[Service]
Type=simple
User=synap
Group=synap
WorkingDirectory=/opt/synap
ExecStart=/opt/synap/synap-server --config /etc/synap/config.yml
Restart=on-failure
RestartSec=5s

# Resource limits
LimitNOFILE=65536
MemoryLimit=8G
CPUQuota=400%

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/synap

[Install]
WantedBy=multi-user.target
```

```bash
# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable synap
sudo systemctl start synap

# Check status
sudo systemctl status synap
sudo journalctl -u synap -f
```

---

## Configuration

### Production Configuration

**config.production.yml**:

```yaml
server:
  host: "0.0.0.0"
  port: 15500
  websocket_enabled: true

kv_store:
  max_memory_mb: 8192  # 8 GB for KV data
  eviction_policy: "lru"
  ttl_cleanup_interval_ms: 100
  allow_flush_commands: false  # Disable in production

queue:
  enabled: true
  max_depth: 1000000  # 1M messages
  ack_deadline_secs: 60
  default_max_retries: 3
  default_priority: 5

rate_limit:
  enabled: true  # Enable for public APIs
  requests_per_second: 1000
  burst_size: 200

logging:
  level: "info"  # Use "warn" for less verbosity
  format: "json"  # For log aggregation

persistence:
  enabled: true
  wal:
    enabled: true
    path: "/var/lib/synap/wal/synap.wal"
    fsync_mode: "periodic"  # Balanced performance/safety
    fsync_interval_ms: 10
  snapshot:
    enabled: true
    directory: "/var/lib/synap/snapshots"
    interval_secs: 3600  # Hourly snapshots
    auto_snapshot: true

replication:
  enabled: true
  role: "master"  # or "replica"
  replica_listen_address: "0.0.0.0:15501"
  heartbeat_interval_ms: 1000
  max_lag_ms: 10000
  buffer_size_kb: 256
  replica_timeout_secs: 30

authentication:
  enabled: true
  require_auth_on_public_bind: true
```

### Environment Variables

Override config via environment:

```bash
export SYNAP_HOST="0.0.0.0"
export SYNAP_PORT="15500"
export SYNAP_LOG_LEVEL="info"
export SYNAP_LOG_FORMAT="json"
export SYNAP_PERSISTENCE_ENABLED="true"
export SYNAP_REPLICATION_ROLE="master"

synap-server --config config.yml
```

---

## Monitoring & Observability

### Prometheus Setup

**prometheus.yml**:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'production'

scrape_configs:
  - job_name: 'synap-master'
    static_configs:
      - targets: ['synap-master:15500']
        labels:
          role: 'master'

  - job_name: 'synap-replicas'
    static_configs:
      - targets: 
        - 'synap-replica-1:15500'
        - 'synap-replica-2:15500'
        - 'synap-replica-3:15500'
        labels:
          role: 'replica'

rule_files:
  - 'alerts.yml'

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']
```

### Prometheus Alerts

**alerts.yml**:

```yaml
groups:
  - name: synap
    interval: 30s
    rules:
      # High replication lag
      - alert: HighReplicationLag
        expr: synap_replication_lag_operations > 1000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High replication lag on {{ $labels.replica_id }}"
          description: "Replication lag is {{ $value }} operations"

      # High memory usage
      - alert: HighMemoryUsage
        expr: synap_kv_memory_bytes > 7516192768  # 7 GB
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage"
          description: "Memory usage is {{ $value | humanize }}B"

      # Queue depth too high
      - alert: HighQueueDepth
        expr: synap_queue_depth > 100000
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Queue {{ $labels.queue }} has high depth"
          description: "Queue depth is {{ $value }} messages"

      # High DLQ count
      - alert: HighDLQCount
        expr: synap_queue_dlq_messages > 1000
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High DLQ count in queue {{ $labels.queue }}"
          description: "DLQ has {{ $value }} failed messages"

      # Server down
      - alert: SynapDown
        expr: up{job=~"synap.*"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Synap instance {{ $labels.instance }} is down"
```

### Grafana Dashboards

**Key Panels to Create**:

1. **Overview Dashboard**
   - Total operations/sec (all systems)
   - Memory usage
   - CPU usage
   - Active connections

2. **KV Store Dashboard**
   - Operations rate by type (GET, SET, DELETE)
   - P50/P95/P99 latency
   - Key count by shard
   - Cache hit rate (if L1 enabled)

3. **Queue Dashboard**
   - Queue depth by queue
   - Publish/consume rate
   - DLQ count
   - Pending vs in-flight messages

4. **Replication Dashboard**
   - Replication lag by replica
   - Sync operations rate
   - Bytes transferred
   - Replica health status

5. **System Dashboard**
   - Process memory
   - CPU load
   - Network I/O
   - Disk I/O

### Log Aggregation

**JSON Logs** (for ELK/Loki):

```yaml
logging:
  level: "info"
  format: "json"
```

**Loki Configuration** (Docker):

```yaml
services:
  loki:
    image: grafana/loki:latest
    ports:
      - "3100:3100"

  promtail:
    image: grafana/promtail:latest
    volumes:
      - /var/log/synap:/var/log/synap
      - ./promtail-config.yml:/etc/promtail/config.yml
```

---

## Backup & Recovery

### Backup Strategy

#### 1. Snapshot Backups

```bash
# Trigger manual snapshot
curl -X POST http://localhost:15500/snapshot

# Snapshots stored in: /data/snapshots/
# Format: snapshot_<timestamp>_<offset>.snap

# Copy snapshots to backup location
cp -r /data/snapshots/* /backup/synap/snapshots/$(date +%Y%m%d)/
```

#### 2. WAL Backups

```bash
# WAL location: /data/wal/synap.wal

# Backup WAL (server must be stopped or use copy-on-write)
cp /data/wal/synap.wal /backup/synap/wal/synap_$(date +%Y%m%d_%H%M%S).wal
```

#### 3. Automated Backup Script

**backup.sh**:
```bash
#!/bin/bash

BACKUP_DIR="/backup/synap/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"

# Trigger snapshot
curl -X POST http://localhost:15500/snapshot

# Wait for snapshot completion
sleep 5

# Copy snapshots and WAL
cp -r /data/snapshots/* "$BACKUP_DIR/snapshots/"
cp /data/wal/synap.wal "$BACKUP_DIR/wal/"

# Compress
tar czf "$BACKUP_DIR.tar.gz" -C "$BACKUP_DIR" .

# Cleanup old backups (keep 7 days)
find /backup/synap -name "*.tar.gz" -mtime +7 -delete

echo "Backup completed: $BACKUP_DIR.tar.gz"
```

**Cron Schedule**:
```bash
# Daily backup at 2 AM
0 2 * * * /opt/synap/backup.sh >> /var/log/synap-backup.log 2>&1
```

### Recovery Procedures

#### Scenario 1: Server Crash

**Automatic Recovery**:
```bash
# Just restart server - automatic recovery on startup
systemctl restart synap

# Or Docker
docker restart synap
```

Recovery process:
1. Load latest snapshot
2. Replay WAL from snapshot offset
3. Server ready (1-10 seconds typical)

#### Scenario 2: Data Corruption

**Manual Recovery from Backup**:

```bash
# Stop server
systemctl stop synap

# Restore from backup
tar xzf /backup/synap/20251022_020000.tar.gz -C /tmp/restore

# Copy snapshot and WAL
cp -r /tmp/restore/snapshots/* /data/snapshots/
cp /tmp/restore/wal/synap.wal /data/wal/

# Start server
systemctl start synap

# Verify recovery
curl http://localhost:15500/kv/stats
```

#### Scenario 3: Complete Data Loss

**Options**:

1. **Restore from replica** (if replication enabled):
```bash
# Promote replica to master
# See High Availability section
```

2. **Restore from backup**:
```bash
# Follow Scenario 2 procedure
```

3. **Start fresh** (last resort):
```bash
# Clear data directory
rm -rf /data/wal/* /data/snapshots/*

# Start server
systemctl start synap
```

---

## High Availability

### Master-Slave Replication

#### Architecture

```
┌─────────────┐
│   Master    │ ← Writes only
│  (RW mode)  │
└──────┬──────┘
       │ Replication (TCP 15501)
       ├────────┬────────┬────────┐
       ▼        ▼        ▼        ▼
   ┌───────┐┌───────┐┌───────┐┌───────┐
   │Replica││Replica││Replica││Replica│
   │  #1   ││  #2   ││  #3   ││  #4   │
   │(R mode)││(R mode)││(R mode)││(R mode)│
   └───────┘└───────┘└───────┘└───────┘
```

#### Load Balancing

**Nginx Configuration**:

```nginx
upstream synap_read {
    # Read from replicas (round-robin)
    server synap-replica-1:15500;
    server synap-replica-2:15500;
    server synap-replica-3:15500;
}

upstream synap_write {
    # Write to master only
    server synap-master:15500;
}

server {
    listen 80;
    server_name synap.example.com;

    # Read operations → replicas
    location ~ ^/kv/get/ {
        proxy_pass http://synap_read;
    }

    location ~ ^/queue/.*/consume/ {
        proxy_pass http://synap_read;
    }

    # Write operations → master
    location ~ ^/kv/(set|del) {
        proxy_pass http://synap_write;
    }

    location ~ ^/queue/.*/publish {
        proxy_pass http://synap_write;
    }

    # Metrics from all nodes
    location /metrics {
        proxy_pass http://synap_read;
    }
}
```

#### Manual Failover

**When master fails**:

```bash
# 1. Stop failed master
docker stop synap-master

# 2. Promote replica to master
# Update replica config to role: "master"
docker exec synap-replica-1 kill -HUP 1

# 3. Point other replicas to new master
# Update their config: master_address: "synap-replica-1:15501"
docker restart synap-replica-2 synap-replica-3

# 4. Update load balancer
# Point writes to new master
```

**Automatic Failover** (Future):
- Sentinel nodes (like Redis Sentinel)
- Automatic promotion
- Coming in v1.1+

---

## Performance Tuning

### Hardware Recommendations

#### Small Deployment (< 1M keys)
- **CPU**: 2-4 cores
- **RAM**: 2-4 GB
- **Disk**: SSD recommended (for WAL)
- **Network**: 1 Gbps

#### Medium Deployment (1M - 10M keys)
- **CPU**: 4-8 cores
- **RAM**: 8-16 GB
- **Disk**: NVMe SSD (for snapshots/WAL)
- **Network**: 10 Gbps

#### Large Deployment (> 10M keys)
- **CPU**: 16+ cores
- **RAM**: 32-64 GB
- **Disk**: NVMe RAID for I/O
- **Network**: 10 Gbps bonded

### Configuration Tuning

#### For High Throughput

```yaml
kv_store:
  max_memory_mb: 32768  # 32 GB
  ttl_cleanup_interval_ms: 1000  # Less frequent cleanup

persistence:
  wal:
    fsync_mode: "periodic"  # Balance safety/performance
    fsync_interval_ms: 10

queue:
  max_depth: 10000000  # 10M messages
```

#### For Low Latency

```yaml
kv_store:
  ttl_cleanup_interval_ms: 50  # More frequent

persistence:
  wal:
    fsync_mode: "never"  # Fastest (risk on crash)
    
queue:
  ack_deadline_secs: 5  # Quick timeouts
```

#### For Maximum Safety

```yaml
persistence:
  wal:
    fsync_mode: "always"  # Safest (slower)
    
  snapshot:
    interval_secs: 900  # Snapshot every 15 min
    auto_snapshot: true

replication:
  max_lag_ms: 1000  # Alert if lag > 1s
```

### Operating System Tuning

**Linux**:

```bash
# Increase file descriptors
ulimit -n 65536
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# TCP tuning
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=8192

# Transparent Huge Pages (disable for better latency)
echo never > /sys/kernel/mm/transparent_hugepage/enabled

# I/O scheduler (deadline for SSD)
echo deadline > /sys/block/sda/queue/scheduler
```

**Persistence**:
```bash
# Disable CoW on Btrfs
chattr +C /data/synap

# Mount with noatime
mount -o noatime,nodiratime /dev/sda1 /data
```

---

## Security

### TLS/SSL (Reverse Proxy)

**Nginx with Let's Encrypt**:

```nginx
server {
    listen 443 ssl http2;
    server_name synap.example.com;

    ssl_certificate /etc/letsencrypt/live/synap.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/synap.example.com/privkey.pem;

    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://localhost:15500;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Authentication Setup

#### 1. Create Admin User

```bash
# Generate bcrypt hash (cost=12)
# Use: https://bcrypt-generator.com/ or bcrypt CLI

# Add to config.yml
users:
  - username: admin
    password_hash: "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyB3fH4hJ.ye"
    role: admin
    enabled: true
```

#### 2. Create API Keys

```bash
# Generate secure key
API_KEY=$(openssl rand -hex 16)
echo "sk_live_$API_KEY"

# Add to config.yml
api_keys:
  - key: "sk_live_abc123..."
    name: "Production App"
    role: admin
    expires_days: 365
    enabled: true
```

#### 3. Configure ACL

```yaml
acl:
  - name: "public_read"
    resource_type: "kv"
    resource_pattern: "public:*"
    actions: ["read"]
    authenticated: false

  - name: "admin_full"
    resource_type: "*"
    resource_pattern: "*"
    actions: ["all"]
    roles: ["admin"]
```

### Network Security

**Firewall Rules**:

```bash
# Allow Synap ports
ufw allow 15500/tcp  # API port
ufw allow 15501/tcp  # Replication port (master only)

# Restrict replication to private network
ufw allow from 10.0.0.0/8 to any port 15501
```

**IP Whitelisting** (in config.yml):

```yaml
api_keys:
  - key: "sk_live_..."
    allowed_ips:
      - "10.0.1.0/24"
      - "192.168.1.100"
```

---

## Operations

### Daily Operations

#### Health Checks

```bash
# System health
curl http://localhost:15500/health

# Replication health
curl http://localhost:15500/health/replication

# Queue health
curl http://localhost:15500/queue/list
```

#### Metrics Review

```bash
# Check key metrics
curl http://localhost:15500/metrics | grep -E '(kv_operations|queue_depth|replication_lag)'

# Monitor in Grafana
open http://grafana:3000/dashboards
```

#### Log Monitoring

```bash
# Follow logs
journalctl -u synap -f

# Search for errors
journalctl -u synap | grep -i error

# Docker logs
docker logs -f synap --tail 100
```

### Maintenance Tasks

#### Snapshot Management

```bash
# List snapshots
ls -lh /data/snapshots/

# Cleanup old snapshots (keep last 7 days)
find /data/snapshots -name "snapshot_*" -mtime +7 -delete

# Manual snapshot before major changes
curl -X POST http://localhost:15500/snapshot
```

#### Queue Maintenance

```bash
# Purge queue (removes all messages)
curl -X POST http://localhost:15500/queue/old-queue/purge

# Delete unused queue
curl -X DELETE http://localhost:15500/queue/old-queue

# Monitor DLQ
curl http://localhost:15500/queue/jobs/stats | jq .dlq_count
```

#### Memory Management

```bash
# Check memory usage
curl http://localhost:15500/kv/stats

# Force eviction if needed (LRU)
# Set max_memory_mb lower in config, restart
```

### Scaling Operations

#### Add Read Replica

```bash
# 1. Prepare new server
# 2. Configure as replica
# 3. Start server
synap-server --config config-replica-new.yml

# 4. Verify replication
curl http://new-replica:15500/health/replication

# 5. Add to load balancer
```

#### Increase Queue Capacity

```bash
# Update config
queue:
  max_depth: 2000000  # Double capacity

# Restart server
systemctl restart synap
```

---

## Troubleshooting

### Performance Issues

#### High Latency

**Diagnose**:
```bash
# Check metrics
curl http://localhost:15500/metrics | grep duration

# Profile with flamegraph
cargo flamegraph --bin synap-server
```

**Common Causes**:
1. Disk I/O bottleneck (slow SSD)
2. High GC pressure (too much data)
3. Network saturation

**Solutions**:
- Use NVMe SSD
- Increase RAM, enable eviction
- Upgrade network

#### Low Throughput

**Check**:
- CPU usage (`top`, `htop`)
- Network bandwidth (`iftop`, `nethogs`)
- Disk I/O (`iostat`)

**Solutions**:
- Scale horizontally (add replicas)
- Tune fsync_mode
- Use 64-way sharding (already default)

### Replication Issues

#### Replica Not Connecting

```bash
# Check replica logs
docker logs synap-replica

# Verify master is listening
netstat -tlnp | grep 15501

# Check network connectivity
telnet master-host 15501
```

#### High Replication Lag

**Check lag**:
```bash
curl http://localhost:15500/metrics | grep replication_lag
```

**Common causes**:
1. Network latency
2. Master overloaded
3. Replica disk slow

**Solutions**:
- Increase `max_lag_ms` threshold
- Add more replicas
- Optimize network path

### Data Issues

#### Keys Not Persisting

**Verify persistence**:
```bash
# Check config
grep -A5 "persistence:" config.yml

# Check WAL file
ls -lh /data/wal/synap.wal

# Check snapshots
ls -lh /data/snapshots/
```

**Solution**:
```yaml
persistence:
  enabled: true  # Must be true
  wal:
    enabled: true
```

#### Messages Stuck in DLQ

**Investigate**:
```bash
# Check DLQ count
curl http://localhost:15500/queue/jobs/stats

# Common reasons:
# - max_retries exceeded
# - Worker errors (not ACKing)
# - Invalid message format
```

**Solution**:
```bash
# Review DLQ messages (implement DLQ consume endpoint)
# Fix worker code
# Purge DLQ if needed
curl -X POST http://localhost:15500/queue/jobs/purge
```

---

## Best Practices

### 1. Capacity Planning

**Estimate memory usage**:
```
Memory = (avg_key_size + avg_value_size + 32 bytes overhead) × num_keys
```

Example:
- 1M keys
- 20 byte keys
- 100 byte values
- Total: ~152 MB

Add 50% buffer: **~230 MB**

### 2. Monitoring Strategy

**Must-Monitor Metrics**:
- `synap_kv_operations_total` - Detect traffic spikes
- `synap_queue_depth` - Prevent queue overflow
- `synap_replication_lag_operations` - Ensure consistency
- `synap_process_memory_bytes` - OOM prevention

**Alert Thresholds**:
- Replication lag > 1000 ops (warning)
- Queue depth > 100K (warning)
- Memory > 90% limit (critical)
- DLQ > 1000 messages (critical)

### 3. Backup Strategy

**Recommended**:
- Daily snapshots (automated)
- WAL backup every 6 hours
- Keep 7 days of backups
- Test recovery monthly

### 4. Security Checklist

- [x] Authentication enabled
- [x] TLS via reverse proxy
- [x] API keys rotated quarterly
- [x] Firewall configured
- [x] Logs monitored
- [x] Regular security updates

### 5. Upgrade Strategy

**Zero-Downtime Upgrade**:

```bash
# 1. Upgrade replicas one by one
helm upgrade synap-replica-1 synap/synap --set image.tag=0.4.0
# Wait for health check
helm upgrade synap-replica-2 synap/synap --set image.tag=0.4.0
# Wait for health check

# 2. Upgrade master (brief downtime for writes)
helm upgrade synap-master synap/synap --set image.tag=0.4.0

# 3. Verify replication
curl http://master:15500/health/replication
```

---

## Performance Benchmarks

### Expected Performance

| Operation | Throughput | Latency (P95) |
|-----------|------------|---------------|
| KV GET | 12M ops/s | 87 ns |
| KV SET (durable) | 44K ops/s | 22.5 µs |
| Queue Publish | 19.2K msg/s | 52 µs |
| Queue Consume+ACK | 1.6K msg/s | 607 µs |
| Stream Publish | 2.3 GiB/s | < 1 µs |
| Stream Consume | 12.5M msg/s | < 1 µs |

### Load Testing

**k6 Example**:

```javascript
// load-test.js
import http from 'k6/http';
import { check } from 'k6';

export let options = {
  vus: 100,  // 100 virtual users
  duration: '30s',
};

export default function() {
  // SET
  const setRes = http.post('http://localhost:15500/kv/set', 
    JSON.stringify({
      key: `test:${__VU}:${__ITER}`,
      value: 'test-value',
      ttl: 60
    }),
    { headers: { 'Content-Type': 'application/json' } }
  );
  
  check(setRes, { 'SET success': (r) => r.status === 200 });
  
  // GET
  const getRes = http.get(`http://localhost:15500/kv/get/test:${__VU}:${__ITER}`);
  check(getRes, { 'GET success': (r) => r.status === 200 });
}
```

```bash
k6 run load-test.js
```

---

## Configuration Reference

See [CONFIGURATION.md](../CONFIGURATION.md) for complete configuration documentation.

### Critical Settings

| Setting | Production Value | Notes |
|---------|------------------|-------|
| `server.host` | `0.0.0.0` | Listen on all interfaces |
| `persistence.enabled` | `true` | Always enable |
| `persistence.wal.fsync_mode` | `periodic` | Best balance |
| `replication.enabled` | `true` | For HA |
| `authentication.enabled` | `true` | If public |
| `rate_limit.enabled` | `true` | For public APIs |

---

## Support & Resources

### Documentation
- **Architecture**: [ARCHITECTURE.md](../ARCHITECTURE.md)
- **API Reference**: [REST_API.md](../api/REST_API.md)
- **Configuration**: [CONFIGURATION.md](../CONFIGURATION.md)

### Community
- **GitHub Issues**: https://github.com/hivellm/synap/issues
- **Discussions**: https://github.com/hivellm/synap/discussions

### Professional Support
- Enterprise support available
- Contact: support@hivellm.com

---

**Need help?** Check [Troubleshooting](#troubleshooting) or open an issue on GitHub!

