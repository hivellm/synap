---
title: Installation Guide
module: installation
id: installation-guide
order: 1
description: Complete guide for installing Synap on Linux, macOS, and Windows
tags: [installation, setup, linux, windows, macos]
---

# Installation Guide

This guide covers installing Synap on different platforms.

## Quick Installation

### Docker (Recommended)

**Single Instance:**
```bash
# Pull latest image
docker pull hivellm/synap:latest

# Run server
docker run -d \
  --name synap \
  -p 15500:15500 \
  -v synap-data:/data \
  hivellm/synap:latest

# Check status
curl http://localhost:15500/health
```

**With Docker Compose:**
```yaml
# docker-compose.yml
version: '3.8'
services:
  synap:
    image: hivellm/synap:latest
    ports:
      - "15500:15500"
    volumes:
      - ./data:/data
      - ./config.yml:/etc/synap/config.yml
    restart: unless-stopped
```

```bash
docker-compose up -d
```

### Kubernetes (Helm)

```bash
# Add Helm repository
helm repo add synap https://hivellm.github.io/synap-charts
helm repo update

# Install
helm install my-synap synap/synap

# With custom values
helm install my-synap synap/synap -f values.yaml
```

**Production Setup** (Master + Replicas):
```bash
# Master
helm install synap-master synap/synap \
  --set replication.master.enabled=true \
  --set config.replication.role=master

# Replicas
helm install synap-replica synap/synap \
  --set replication.replica.enabled=true \
  --set replication.replica.replicaCount=2 \
  --set config.replication.role=replica
```

### Binary Download

```bash
# Download from GitHub Releases
wget https://github.com/hivellm/synap/releases/download/v0.8.1/synap-linux-x64.tar.gz

# Extract
tar xzf synap-linux-x64.tar.gz
cd synap

# Run server
./synap-server --config config.example.yml

# In another terminal, use CLI
./synap-cli
```

## Installation Methods

- **[Docker Installation](./DOCKER.md)** - Complete Docker deployment guide
- **[Building from Source](./BUILD_FROM_SOURCE.md)** - Build Synap from source code

## Prerequisites

### System Requirements

- **OS**: Linux, macOS, or Windows
- **Memory**: Minimum 512MB, recommended 2GB+
- **Disk**: 100MB for binary, additional space for data
- **Network**: Port 15500 (configurable)

### Docker Requirements

- Docker 20.10+ or Docker Desktop
- 2GB+ available memory
- Port 15500 available

### Build Requirements (from source)

- Rust 1.85+ (Edition 2024, nightly)
- Cargo (comes with Rust)
- Git
- Build tools (gcc/clang)

## Verification

After installation, verify Synap is running:

```bash
# Health check
curl http://localhost:15500/health

# Expected output:
# {"status":"healthy","uptime_secs":5}

# Get server info
curl http://localhost:15500/info

# Get statistics
curl http://localhost:15500/kv/stats
```

## Next Steps

1. **[Quick Start Guide](./QUICK_START.md)** - Get up and running in minutes
2. **[First Steps](./FIRST_STEPS.md)** - Complete guide after installation
3. **[Configuration Guide](../configuration/CONFIGURATION.md)** - Configure Synap

## Troubleshooting

### Port Already in Use

```bash
# Linux/macOS
lsof -i :15500

# Windows
netstat -ano | findstr :15500
```

Change port in configuration or stop the conflicting service.

### Permission Denied

```bash
# Linux/macOS - make executable
chmod +x synap-server

# Or run with sudo (not recommended)
sudo ./synap-server
```

### Docker Issues

```bash
# Check Docker is running
docker ps

# Check logs
docker logs synap

# Restart container
docker restart synap
```

## Related Topics

- [Docker Installation](./DOCKER.md) - Complete Docker guide
- [Building from Source](./BUILD_FROM_SOURCE.md) - Build from source
- [Configuration Guide](../configuration/CONFIGURATION.md) - Server configuration

