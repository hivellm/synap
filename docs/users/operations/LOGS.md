---
title: Log Management
module: operations
id: log-management
order: 3
description: Viewing, filtering, and analyzing logs
tags: [operations, logs, logging, analysis]
---

# Log Management

Complete guide to viewing, filtering, and analyzing Synap logs.

## Viewing Logs

### Docker Logs

```bash
# View all logs
docker logs synap

# Follow logs (real-time)
docker logs -f synap

# Last 100 lines
docker logs --tail 100 synap

# Since timestamp
docker logs --since "2025-01-01T00:00:00" synap

# Last 10 minutes
docker logs --since 10m synap
```

### systemd Logs (journalctl)

```bash
# View all logs
journalctl -u synap

# Follow logs
journalctl -u synap -f

# Last 100 lines
journalctl -u synap -n 100

# Since today
journalctl -u synap --since today

# Since specific time
journalctl -u synap --since "2025-01-01 00:00:00"

# Between times
journalctl -u synap --since "2025-01-01 00:00:00" --until "2025-01-01 23:59:59"
```

### File Logs

```bash
# View logs
tail -f /var/log/synap/synap.log

# Last 100 lines
tail -n 100 /var/log/synap/synap.log

# Search logs
grep "error" /var/log/synap/synap.log
```

## Filtering Logs

### By Level

```bash
# Errors only
grep '"level":"error"' synap.log

# Warnings and errors
grep -E '"level":"(warn|error)"' synap.log
```

### By Module

```bash
# KV store logs
grep "synap_kv" synap.log

# Queue logs
grep "synap_queue" synap.log

# Stream logs
grep "synap_stream" synap.log
```

### By Time

```bash
# Today's logs
grep "2025-01-01" synap.log

# Specific hour
grep "2025-01-01T12:" synap.log
```

### By Message

```bash
# Search for specific message
grep "Server started" synap.log

# Case insensitive
grep -i "error" synap.log

# Multiple patterns
grep -E "(error|warning)" synap.log
```

## JSON Log Parsing

### Using jq

```bash
# Parse JSON logs
cat synap.log | jq 'select(.level == "error")'

# Filter by module
cat synap.log | jq 'select(.module == "synap_kv")'

# Extract fields
cat synap.log | jq '.message, .timestamp'
```

### Using Python

```python
import json

with open('synap.log', 'r') as f:
    for line in f:
        log = json.loads(line)
        if log['level'] == 'error':
            print(f"{log['timestamp']}: {log['message']}")
```

## Log Analysis

### Count Errors

```bash
# Count errors
grep -c '"level":"error"' synap.log

# Count by module
grep '"level":"error"' synap.log | grep -o 'synap_[^"]*' | sort | uniq -c
```

### Find Patterns

```bash
# Most common errors
grep '"level":"error"' synap.log | grep -o '"message":"[^"]*"' | sort | uniq -c | sort -rn
```

### Performance Analysis

```bash
# Find slow operations
grep "duration" synap.log | jq 'select(.duration > 1.0)'
```

## Log Rotation

### logrotate Configuration

**`/etc/logrotate.d/synap`:**

```
/var/log/synap/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 synap synap
    sharedscripts
    postrotate
        systemctl reload synap > /dev/null 2>&1 || true
    endscript
}
```

### Manual Rotation

```bash
# Rotate logs
logrotate -f /etc/logrotate.d/synap
```

## Centralized Logging

### Send to Syslog

```yaml
logging:
  level: "info"
  format: "json"
  syslog:
    enabled: true
    host: "log-server.example.com"
    port: 514
```

### Send to Log Aggregation

Use tools like:
- **ELK Stack** (Elasticsearch, Logstash, Kibana)
- **Loki** (Grafana)
- **Fluentd**
- **Datadog**
- **Splunk**

## Best Practices

### Log Levels

- **Production**: `info` or `warn`
- **Development**: `debug`
- **Troubleshooting**: `trace`

### Log Retention

- Keep logs for 7-30 days locally
- Archive important logs
- Use centralized logging for long-term storage

### Monitor Logs

Set up alerts for:
- Error rate spikes
- Critical errors
- Performance degradation

## Related Topics

- [Logging Configuration](../configuration/LOGGING.md) - Log configuration
- [Monitoring](./MONITORING.md) - Monitoring and metrics
- [Troubleshooting](./TROUBLESHOOTING.md) - Common problems

