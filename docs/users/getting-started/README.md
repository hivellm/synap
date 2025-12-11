---
title: Getting Started
module: getting-started
id: getting-started-index
order: 0
description: Installation and quick start guides
tags: [getting-started, installation, quick-start, tutorial]
---

# Getting Started

Complete guides to install Synap and get started quickly.

## Installation Guides

### [Installation Guide](./INSTALLATION.md)
Quick installation and overview:
- Quick installation scripts (Linux/macOS, Windows)
- Manual installation
- Verification steps

### [Docker Installation](./DOCKER.md)
Complete Docker deployment guide:
- Docker Compose examples
- Volumes and networking
- Health checks and resource limits
- Backup and restore

### [Building from Source](./BUILD_FROM_SOURCE.md)
Build Synap from source code:
- Prerequisites and dependencies
- Build process and optimization
- Feature flags and cross-compilation
- Development workflow

## Quick Start Guides

### [Quick Start Guide](./QUICK_START.md)
Get up and running in minutes:
- Start Synap server
- Your first key-value operations
- Your first queue message
- Your first stream event

### [First Steps](./FIRST_STEPS.md)
Complete guide after installation:
- Verify installation
- Create first key-value entry
- Publish first queue message
- Consume first stream event
- Next steps

### [Quick Start (Windows)](./QUICK_START_WINDOWS.md)
Windows-specific guide:
- Windows installation
- PowerShell commands
- Windows service management

### [Docker Authentication](./DOCKER_AUTHENTICATION.md)
Docker authentication guide:
- Docker Hub login
- Private registry setup
- Kubernetes secrets
- Troubleshooting

## Quick Installation

**Linux/macOS:**
```bash
# Using Docker (recommended)
docker run -d -p 15500:15500 --name synap hivellm/synap:latest
```

**Windows:**
```powershell
# Using Docker
docker run -d -p 15500:15500 --name synap hivellm/synap:latest
```

## Next Steps

After installation:
1. **[First Steps](./FIRST_STEPS.md)** - Verify and setup
2. **[Basic KV Operations](../kv-store/BASIC.md)** - Key-value operations
3. **[Message Queues](../queues/CREATING.md)** - Create queues
4. **[Use Cases](../use-cases/)** - See examples

## Related Topics

- [KV Store Guide](../kv-store/KV_STORE.md) - Key-value operations
- [Queues Guide](../queues/QUEUES.md) - Message queue operations
- [Streams Guide](../streams/STREAMS.md) - Event stream operations
- [SDKs Guide](../sdks/README.md) - Client SDKs

