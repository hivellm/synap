---
title: Troubleshooting
module: operations
id: troubleshooting
order: 4
description: Common problems and solutions
tags: [troubleshooting, problems, solutions, debugging]
---

# Troubleshooting

Common problems and solutions when using Synap.

## Server Won't Start

### Port Already in Use

**Problem:** Port 15500 is already in use.

**Solution:**

```bash
# Linux/macOS - Check what's using the port
lsof -i :15500

# Windows
netstat -ano | findstr :15500

# Kill the process or change port in config
```

**Change port:**
```yaml
server:
  port: 15501  # Use different port
```

### Permission Denied

**Problem:** Cannot bind to port (permission denied).

**Solution:**

```bash
# Linux - Use port > 1024 or run with sudo (not recommended)
# Better: Use port > 1024
synap-server --port 15500

# Or configure systemd service with proper permissions
```

### Configuration Error

**Problem:** Server fails to start due to config error.

**Solution:**

```bash
# Validate configuration
synap-server --config config.yml --validate

# Check logs
tail -f server.log
# or
docker logs synap
```

## Connection Refused

### Server Not Running

**Problem:** Cannot connect to server.

**Solution:**

```bash
# Check if server is running
curl http://localhost:15500/health

# Check process
ps aux | grep synap-server
# or
docker ps | grep synap
```

### Firewall Blocking

**Problem:** Firewall blocking connections.

**Solution:**

```bash
# Linux - Allow port
sudo ufw allow 15500/tcp

# Check listening
netstat -tlnp | grep 15500
```

### Wrong Host/Port

**Problem:** Connecting to wrong address.

**Solution:**

```bash
# Verify server address
curl http://localhost:15500/health

# Check configuration
cat config.yml | grep -A 2 server
```

## High Memory Usage

### Check Statistics

```bash
# Get KV store stats
curl http://localhost:15500/kv/stats

# Get memory metrics
curl http://localhost:15500/metrics | grep memory
```

### Configure Eviction

```yaml
kv_store:
  max_memory_mb: 4096
  eviction_policy: "lru"  # or "lfu"
```

### Monitor Memory

```bash
# Prometheus metrics
curl http://localhost:15500/metrics | grep synap_process_memory_bytes
```

## Replication Lag

### Check Lag Metrics

```bash
# Get replication metrics
curl http://localhost:15500/metrics | grep replication_lag
```

### Common Causes

1. **Network latency** between master/replica
2. **Replica overloaded** (too many reads)
3. **Disk I/O bottleneck**

### Solutions

```yaml
# Increase max lag threshold
replication:
  max_lag_ms: 20000  # Increase from default 10000

# Add more replicas for read scaling
# Use faster network/disk
```

## Messages Stuck in Queue

### Check Queue Stats

```bash
curl http://localhost:15500/queue/jobs/stats
```

### Common Issues

1. **No consumers connected**
2. **All messages in DLQ** (exceeded retries)
3. **ACK deadline too short**

### Solutions

```bash
# Check if consumers are active
# Monitor queue stats regularly

# Purge queue (CAUTION: deletes all messages)
curl -X POST http://localhost:15500/queue/jobs/purge

# Adjust ACK deadline
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{"ack_deadline_secs": 60}'
```

## Performance Issues

### Slow Operations

**Check metrics:**
```bash
curl http://localhost:15500/metrics | grep duration
```

**Solutions:**
- Enable persistence with `periodic` fsync mode
- Use batch operations (MSET, MGET)
- Monitor and optimize hot keys

### High Latency

**Check:**
```bash
# Get operation latency
curl http://localhost:15500/metrics | grep latency
```

**Solutions:**
- Use local network for replication
- Optimize configuration
- Check system resources (CPU, memory, disk)

## Data Loss

### Check Persistence

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    fsync_mode: "periodic"  # or "always" for maximum safety
```

### Recovery

```bash
# Check if snapshots exist
ls -la data/snapshots/

# Server automatically recovers on startup
# Loads latest snapshot + replays WAL
```

## Authentication Issues

### Check Configuration

```yaml
authentication:
  enabled: true
  users:
    - username: admin
      password_hash: "$2b$12$..."
```

### Test Authentication

```bash
# Basic auth
curl -u admin:password http://localhost:15500/kv/stats

# API key
curl -H "Authorization: Bearer sk_live_abc123..." \
  http://localhost:15500/kv/stats
```

## Getting Help

### Check Logs

```bash
# Docker
docker logs synap

# Systemd
journalctl -u synap -f

# File logs
tail -f server.log
```

### Enable Debug Logging

```yaml
logging:
  level: "debug"  # or "trace" for more detail
```

### Collect Information

When reporting issues, include:
- Server version
- Configuration (sanitized)
- Error logs
- Steps to reproduce
- System information

## Related Topics

- [Service Management](./SERVICE_MANAGEMENT.md) - Service management
- [Monitoring](./MONITORING.md) - Monitoring and metrics
- [Log Management](./LOGS.md) - Log viewing and analysis

