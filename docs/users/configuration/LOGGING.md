---
title: Logging Configuration
module: configuration
id: logging-configuration
order: 3
description: Log levels, filtering, and output configuration
tags: [configuration, logging, logs, debug]
---

# Logging Configuration

Complete guide to configuring logging in Synap.

## Basic Configuration

### Log Level

```yaml
logging:
  level: "info"  # trace, debug, info, warn, error
```

### Log Format

```yaml
logging:
  level: "info"
  format: "json"  # json or text
```

## Log Levels

### Trace

Most verbose, includes all details:

```yaml
logging:
  level: "trace"
```

Use for: Deep debugging, development.

### Debug

Detailed information for debugging:

```yaml
logging:
  level: "debug"
```

Use for: Development, troubleshooting.

### Info

General informational messages:

```yaml
logging:
  level: "info"
```

Use for: Production (default).

### Warn

Warning messages:

```yaml
logging:
  level: "warn"
```

Use for: Production (minimal logging).

### Error

Only error messages:

```yaml
logging:
  level: "error"
```

Use for: Production (errors only).

## Log Formats

### JSON Format

```yaml
logging:
  level: "info"
  format: "json"
```

**Output:**
```json
{"level":"info","message":"Server started","timestamp":"2025-01-01T12:00:00Z"}
```

### Text Format

```yaml
logging:
  level: "info"
  format: "text"
```

**Output:**
```
2025-01-01 12:00:00 INFO Server started
```

## Environment Variables

### RUST_LOG

```bash
# Set log level
export RUST_LOG=info

# Set specific module
export RUST_LOG=synap_server=debug

# Multiple modules
export RUST_LOG=synap_server=debug,synap_kv=info
```

## Log Output

### Console Output

Logs go to stdout/stderr by default.

### File Output

```yaml
logging:
  level: "info"
  format: "json"
  file: "./logs/synap.log"
```

### Rotating Logs

Use external tools (logrotate, etc.) for log rotation:

```bash
# logrotate configuration
/path/to/synap.log {
    daily
    rotate 7
    compress
    missingok
    notifempty
}
```

## Filtering Logs

### By Module

```bash
# Filter by module
grep "synap_kv" synap.log

# Filter errors only
grep '"level":"error"' synap.log
```

### By Time

```bash
# Filter by timestamp
grep "2025-01-01" synap.log
```

## Docker Logging

### View Logs

```bash
# View logs
docker logs synap

# Follow logs
docker logs -f synap

# Last 100 lines
docker logs --tail 100 synap
```

### Log Driver

```yaml
services:
  synap:
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

## Systemd Logging

### View Logs

```bash
# View logs
journalctl -u synap

# Follow logs
journalctl -u synap -f

# Last 100 lines
journalctl -u synap -n 100
```

## Best Practices

### Production Settings

```yaml
logging:
  level: "info"
  format: "json"
```

### Development Settings

```yaml
logging:
  level: "debug"
  format: "text"
```

### Troubleshooting

```yaml
logging:
  level: "trace"
  format: "text"
```

## Related Topics

- [Configuration Overview](./CONFIGURATION.md) - General configuration
- [Server Configuration](./SERVER.md) - Server settings
- [Operations Guide](../operations/LOGS.md) - Log management

