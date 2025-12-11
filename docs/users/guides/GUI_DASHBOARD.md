---
title: GUI Dashboard
module: guides
id: gui-dashboard
order: 9
description: Synap Desktop GUI for monitoring and management
tags: [guides, gui, dashboard, desktop, monitoring]
---

# GUI Dashboard

Complete guide to Synap Desktop - the graphical user interface for monitoring and managing Synap.

## Overview

Synap Desktop is an Electron-based graphical user interface that provides:
- **Real-time Monitoring**: Live metrics and performance graphs
- **Multi-instance Management**: Monitor multiple Synap servers
- **Configuration Editor**: Visual configuration management
- **Performance Analytics**: CPU, memory, throughput, latency tracking
- **Log Viewer**: Real-time log streaming and filtering

## Installation

### Download

Download Synap Desktop from:
- GitHub Releases: https://github.com/hivellm/synap/releases
- Available for Windows, macOS, and Linux

### Windows

```powershell
# Download installer
# Run Synap-Desktop-Setup.exe
```

### macOS

```bash
# Download DMG
# Mount and drag to Applications
```

### Linux

```bash
# Download AppImage
chmod +x Synap-Desktop-*.AppImage
./Synap-Desktop-*.AppImage
```

## Getting Started

### Connect to Server

1. Launch Synap Desktop
2. Click "Add Server"
3. Enter server details:
   - **Name**: My Synap Server
   - **Host**: localhost (or remote IP)
   - **Port**: 15500
   - **Authentication**: Optional (if enabled)

### Dashboard View

The dashboard shows:
- **Operations per Second**: Real-time throughput
- **Memory Usage**: Current memory consumption
- **Active Connections**: Number of connected clients
- **Cache Hit Rates**: L1/L2 cache performance
- **Server Health**: Status indicators

## Features

### Real-Time Monitoring

- Live metrics updates via WebSocket
- Interactive charts and graphs
- Customizable time ranges
- Export data (CSV, JSON)

### KV Store Inspector

- Browse keys with search
- View key values and TTL
- Statistics and memory usage
- Bulk operations

### Queue System Viewer

- List all queues
- View message counts (pending, in-flight, DLQ)
- Monitor queue depth
- Consumer information

### Event Streams Monitor

- List all streams
- View subscribers
- Monitor offsets
- Stream statistics

### Pub/Sub Viewer

- List topics
- View subscriptions
- Monitor message rates
- Wildcard pattern testing

### Replication Monitor

- Master-slave topology
- Replication lag visualization
- Sync status
- Failover controls

### Configuration Editor

- Visual YAML editor
- Syntax highlighting
- Validation
- Apply changes with preview
- Rollback support

### Log Viewer

- Real-time log streaming
- Log level filtering
- Search and filter
- Export logs

## Best Practices

### Monitor Key Metrics

- Operations per second
- Memory usage trends
- Cache hit rates
- Replication lag

### Set Up Alerts

Configure alerts for:
- High memory usage
- High queue depth
- Replication lag
- Server errors

### Use Multi-Instance Management

Monitor multiple servers:
- Development
- Staging
- Production

## Related Topics

- [Monitoring Guide](../operations/MONITORING.md) - Monitoring and metrics
- [Configuration Guide](../configuration/CONFIGURATION.md) - Server configuration

