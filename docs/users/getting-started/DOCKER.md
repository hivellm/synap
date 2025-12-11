---
title: Docker Installation
module: docker
id: docker-installation
order: 2
description: Complete Docker deployment guide for Synap
tags: [docker, deployment, container, installation]
---

# Docker Installation

Complete guide for deploying Synap using Docker and Docker Compose.

## Quick Start

### Single Container

```bash
# Pull latest image
docker pull hivellm/synap:latest

# Run container
docker run -d \
  --name synap \
  -p 15500:15500 \
  -v synap-data:/data \
  hivellm/synap:latest

# Check status
curl http://localhost:15500/health
```

### Docker Compose

**Basic Setup:**
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
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15500/health"]
      interval: 10s
      timeout: 5s
      retries: 3
```

**Production Setup with Persistence:**
```yaml
version: '3.8'
services:
  synap:
    image: hivellm/synap:latest
    ports:
      - "15500:15500"
    volumes:
      - synap-data:/data
      - ./config.yml:/etc/synap/config.yml
    restart: unless-stopped
    environment:
      - RUST_LOG=info
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15500/health"]
      interval: 10s
      timeout: 5s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 2G
        reservations:
          memory: 512M

volumes:
  synap-data:
    driver: local
```

## Master-Replica Setup

**Master Node:**
```yaml
version: '3.8'
services:
  synap-master:
    image: hivellm/synap:latest
    ports:
      - "15500:15500"
      - "15501:15501"  # Replication port
    volumes:
      - master-data:/data
      - ./config-master.yml:/etc/synap/config.yml
    restart: unless-stopped
    command: ["--config", "/etc/synap/config.yml"]

volumes:
  master-data:
```

**Replica Node:**
```yaml
version: '3.8'
services:
  synap-replica:
    image: hivellm/synap:latest
    ports:
      - "15500:15500"
    volumes:
      - replica-data:/data
      - ./config-replica.yml:/etc/synap/config.yml
    restart: unless-stopped
    command: ["--config", "/etc/synap/config.yml"]
    depends_on:
      - synap-master

volumes:
  replica-data:
```

## Configuration

### Mount Configuration File

```yaml
services:
  synap:
    volumes:
      - ./config.yml:/etc/synap/config.yml:ro
    command: ["--config", "/etc/synap/config.yml"]
```

### Environment Variables

```yaml
services:
  synap:
    environment:
      - RUST_LOG=info
      - SYNAP_HOST=0.0.0.0
      - SYNAP_PORT=15500
```

## Networking

### Custom Network

```yaml
version: '3.8'
services:
  synap:
    image: hivellm/synap:latest
    networks:
      - synap-network

networks:
  synap-network:
    driver: bridge
```

### External Network

```yaml
services:
  synap:
    networks:
      - external-network

networks:
  external-network:
    external: true
```

## Resource Limits

```yaml
services:
  synap:
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M
```

## Health Checks

```yaml
services:
  synap:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15500/health"]
      interval: 10s
      timeout: 5s
      retries: 3
      start_period: 30s
```

## Data Persistence

### Named Volumes

```yaml
services:
  synap:
    volumes:
      - synap-data:/data

volumes:
  synap-data:
    driver: local
```

### Bind Mounts

```yaml
services:
  synap:
    volumes:
      - ./data:/data
```

## Logging

```yaml
services:
  synap:
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

## Security

### Read-only Root Filesystem

```yaml
services:
  synap:
    read_only: true
    tmpfs:
      - /tmp
    volumes:
      - synap-data:/data:rw
```

### Non-root User

```yaml
services:
  synap:
    user: "1000:1000"
    volumes:
      - synap-data:/data
```

## Docker Commands

### Start Services

```bash
docker-compose up -d
```

### Stop Services

```bash
docker-compose stop
```

### View Logs

```bash
# All services
docker-compose logs

# Specific service
docker-compose logs synap

# Follow logs
docker-compose logs -f synap
```

### Restart Services

```bash
docker-compose restart synap
```

### Remove Services

```bash
# Stop and remove containers
docker-compose down

# Remove volumes too
docker-compose down -v
```

## Troubleshooting

### Container Won't Start

```bash
# Check logs
docker logs synap

# Check container status
docker ps -a

# Inspect container
docker inspect synap
```

### Port Conflicts

```bash
# Check what's using the port
lsof -i :15500

# Change port in docker-compose.yml
ports:
  - "15501:15500"  # Host:Container
```

### Volume Issues

```bash
# List volumes
docker volume ls

# Inspect volume
docker volume inspect synap-data

# Remove volume (CAUTION: deletes data)
docker volume rm synap-data
```

## Related Topics

- [Installation Guide](./INSTALLATION.md) - General installation
- [Configuration Guide](../configuration/CONFIGURATION.md) - Server configuration
- [Operations Guide](../operations/SERVICE_MANAGEMENT.md) - Service management

