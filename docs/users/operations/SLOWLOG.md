---
title: Slow Query Log
module: operations
id: slowlog
order: 6
description: Monitor and analyze slow queries
tags: [operations, slowlog, monitoring, performance, debugging]
---

# Slow Query Log

Monitor and analyze slow queries in Synap to identify performance bottlenecks.

## Overview

Slow query log tracks commands that exceed a configurable time threshold:

- **Automatic tracking** - Commands exceeding threshold are logged
- **Configurable threshold** - Set minimum duration to log (default: 10ms)
- **Limited history** - Keeps last N entries (default: 128)
- **Detailed information** - Command, arguments, duration, timestamp

## Configuration

### Enable Slow Log

```yaml
monitoring:
  slowlog:
    # Threshold in milliseconds (default: 10ms)
    threshold_ms: 10
    
    # Maximum entries to keep (default: 128)
    max_entries: 128
```

### Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `threshold_ms` | `10` | Minimum duration to log (milliseconds) |
| `max_entries` | `128` | Maximum entries to keep in memory |

### Recommended Settings

**Development:**
```yaml
monitoring:
  slowlog:
    threshold_ms: 1  # Log queries > 1ms
    max_entries: 256
```

**Production:**
```yaml
monitoring:
  slowlog:
    threshold_ms: 10  # Log queries > 10ms
    max_entries: 128
```

**Performance Tuning:**
```yaml
monitoring:
  slowlog:
    threshold_ms: 5  # Log queries > 5ms
    max_entries: 512
```

## Querying Slow Log

### Get Slow Log Entries

**REST API:**
```bash
# Get all slow log entries
curl http://localhost:15500/slowlog

# Get last N entries
curl http://localhost:15500/slowlog?limit=10
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "timestamp": 1699123456,
      "duration_us": 15234,
      "command": "kv.set",
      "args": ["large-key", "very-large-value..."]
    },
    {
      "id": 2,
      "timestamp": 1699123457,
      "duration_us": 23456,
      "command": "kv.get",
      "args": ["key-with-ttl"]
    }
  ]
}
```

### Using SDKs

**Python:**
```python
from synap import SynapClient

client = SynapClient("http://localhost:15500")

# Get slow log entries
entries = client.slowlog.get(limit=10)

for entry in entries:
    print(f"{entry['command']} took {entry['duration_us']}μs")
```

**TypeScript:**
```typescript
import { Synap } from '@hivehub/synap';

const client = new SynapClient('http://localhost:15500');

// Get slow log entries
const entries = await client.slowlog.get({ limit: 10 });

entries.forEach(entry => {
  console.log(`${entry.command} took ${entry.duration_us}μs`);
});
```

**Rust:**
```rust
use synap_client::SynapClient;

let client = SynapClient::new("http://localhost:15500").await?;

// Get slow log entries
let entries = client.slowlog.get(Some(10)).await?;

for entry in entries {
    println!("{} took {}μs", entry.command, entry.duration_us);
}
```

## Slow Log Entry Format

### Entry Structure

```json
{
  "id": 1,                    // Unique entry ID
  "timestamp": 1699123456,     // Unix timestamp (seconds)
  "duration_us": 15234,        // Duration in microseconds
  "command": "kv.set",         // Command name
  "args": ["key", "value"]     // Command arguments
}
```

### Field Descriptions

- **id**: Unique identifier (incremental)
- **timestamp**: Unix timestamp when command executed
- **duration_us**: Command duration in microseconds
- **command**: Command name (e.g., `kv.set`, `queue.publish`)
- **args**: Command arguments (may be truncated for large values)

## Use Cases

### 1. Performance Monitoring

Monitor slow queries in production:

```python
import time
from synap import SynapClient

client = SynapClient("http://localhost:15500")

def monitor_slow_queries():
    while True:
        entries = client.slowlog.get(limit=10)
        
        for entry in entries:
            if entry['duration_us'] > 10000:  # > 10ms
                print(f"SLOW: {entry['command']} took {entry['duration_us']}μs")
        
        time.sleep(60)  # Check every minute
```

### 2. Debugging Performance Issues

Identify problematic commands:

```bash
# Get slowest queries
curl http://localhost:15500/slowlog | jq '.data | sort_by(.duration_us) | reverse | .[0:5]'
```

### 3. Alerting

Set up alerts for slow queries:

```python
from synap import SynapClient
import smtplib

client = SynapClient("http://localhost:15500")

def check_slow_queries():
    entries = client.slowlog.get(limit=1)
    
    if entries and entries[0]['duration_us'] > 100000:  # > 100ms
        send_alert(f"Very slow query detected: {entries[0]}")
```

### 4. Performance Tuning

Analyze slow queries to optimize:

```python
from collections import Counter
from synap import SynapClient

client = SynapClient("http://localhost:15500")

# Get slow queries
entries = client.slowlog.get(limit=100)

# Count slow commands
slow_commands = Counter(entry['command'] for entry in entries)
print("Slowest commands:", slow_commands.most_common(5))
```

## Best Practices

### 1. Set Appropriate Threshold

**Too Low:**
- Too many entries
- Noise in logs
- Performance impact

**Too High:**
- Miss important slow queries
- Less useful for debugging

**Recommended:**
- Start with 10ms
- Adjust based on your use case
- Use 1ms for development, 10ms for production

### 2. Monitor Regularly

**Automated Monitoring:**
```python
# Check slow log every 5 minutes
import schedule

def check_slowlog():
    entries = client.slowlog.get(limit=10)
    # Process entries...

schedule.every(5).minutes.do(check_slowlog)
```

### 3. Analyze Patterns

**Common Patterns:**
- Large values causing slow SET operations
- Complex queries (SINTER, ZUNION)
- TTL operations on large datasets
- Persistence operations

### 4. Clear Slow Log

Clear slow log when needed:

```bash
# Clear slow log (restart server or use API if available)
curl -X POST http://localhost:15500/slowlog/clear
```

## Troubleshooting

### No Slow Log Entries

**Problem:** Slow log is empty but queries are slow.

**Solution:**
1. Check threshold - may be too high
2. Verify slow log is enabled
3. Check if queries are actually slow (use metrics)

### Too Many Entries

**Problem:** Slow log fills up quickly.

**Solution:**
1. Increase threshold
2. Increase `max_entries`
3. Investigate why queries are slow

### Missing Entries

**Problem:** Some slow queries not appearing.

**Solution:**
1. Check `max_entries` - old entries are removed
2. Verify threshold is appropriate
3. Check if command is being logged

## Related Topics

- [Monitoring Guide](./MONITORING.md) - Complete monitoring guide
- [Performance Tuning](../configuration/PERFORMANCE_TUNING.md) - Optimize performance
- [Troubleshooting](./TROUBLESHOOTING.md) - Common problems
- [Performance Guide](../guides/PERFORMANCE.md) - Advanced performance tips

