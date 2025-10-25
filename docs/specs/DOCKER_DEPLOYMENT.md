# Docker Deployment Guide

Complete guide for deploying Synap replication cluster using Docker.

## Overview

This guide covers deploying a production-ready Synap cluster with:
- 1 Master node (write operations)
- 3 Replica nodes (read operations, high availability)
- Persistent data volumes
- Health monitoring
- Automatic reconnection

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Load Balancer (Optional)                 │
│                    (Read Traffic Distribution)              │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│  Replica 1   │      │  Replica 2   │      │  Replica 3   │
│ Port: 15510  │      │ Port: 15520  │      │ Port: 15530  │
│  (Read Only) │      │  (Read Only) │      │  (Read Only) │
└──────────────┘      └──────────────┘      └──────────────┘
        ▲                     ▲                     ▲
        │                     │                     │
        └─────────────────────┴─────────────────────┘
                              │
                    Replication Stream
                              │
                    ┌──────────────────┐
                    │      Master      │
                    │   Port: 15500    │
                    │   (Read/Write)   │
                    └──────────────────┘
                              ▲
                              │
                    ┌──────────────────┐
                    │  Write Clients   │
                    └──────────────────┘
```

## Quick Start

### Prerequisites

- Docker Engine 20.10+
- Docker Compose 2.0+
- 4GB+ available RAM
- 10GB+ available disk space

### 1. Clone Repository

```bash
git clone https://github.com/hivellm/synap.git
cd synap
```

### 2. Build Docker Image

**Linux/macOS**:
```bash
chmod +x scripts/docker-build.sh
./scripts/docker-build.sh 0.3.0
```

**Windows (PowerShell)**:
```powershell
.\scripts\docker-build.ps1 0.3.0
```

### 3. Deploy Cluster

**Linux/macOS**:
```bash
chmod +x scripts/docker-deploy.sh
./scripts/docker-deploy.sh start
```

**Windows (PowerShell)**:
```powershell
.\scripts\docker-deploy.ps1 start
```

### 4. Verify Deployment

```bash
# Check cluster status
./scripts/docker-deploy.sh status

# Check health
./scripts/docker-deploy.sh health
```

## Service Endpoints

| Service | Port | Purpose | Access |
|---------|------|---------|--------|
| Master | 15500 | Read/Write operations | `http://localhost:15500` |
| Replica 1 | 15510 | Read-only operations | `http://localhost:15510` |
| Replica 2 | 15520 | Read-only operations | `http://localhost:15520` |
| Replica 3 | 15530 | Read-only operations | `http://localhost:15530` |

## Configuration Files

### Master Node (`config-master.yml`)

```yaml
replication:
  enabled: true
  role: "master"
  replica_listen_address: "0.0.0.0:15501"
  heartbeat_interval_ms: 1000
  max_lag_ms: 10000
```

### Replica Node (`config-replica.yml`)

```yaml
replication:
  enabled: true
  role: "replica"
  master_address: "master:15501"
  auto_reconnect: true
  reconnect_delay_ms: 5000
```

## Usage Examples

### Write to Master

```bash
# Set key-value
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key": "user:100", "value": "Alice", "ttl": 3600}'
```

### Read from Replicas (Load Balancing)

```bash
# Read from replica 1
curl http://localhost:15510/kv/get/user:100

# Read from replica 2
curl http://localhost:15520/kv/get/user:100

# Read from replica 3
curl http://localhost:15530/kv/get/user:100
```

### Monitor Replication

```bash
# Master replication status
curl http://localhost:15500/health/replication

# Replica 1 status
curl http://localhost:15510/health/replication
```

## Management Commands

### Start Cluster

```bash
./scripts/docker-deploy.sh start
```

### Stop Cluster

```bash
./scripts/docker-deploy.sh stop
```

### Restart Cluster

```bash
./scripts/docker-deploy.sh restart
```

### View Logs

```bash
# All nodes
./scripts/docker-deploy.sh logs

# Specific node
docker logs -f synap-master
docker logs -f synap-replica1
```

### Check Status

```bash
./scripts/docker-deploy.sh status
```

### Health Check

```bash
./scripts/docker-deploy.sh health
```

### Clean (Remove All Data)

```bash
# WARNING: This deletes all data!
./scripts/docker-deploy.sh clean
```

## Data Persistence

### Volume Mounts

Each node has a dedicated volume for data persistence:

- `master_data` → `/data` (master node)
- `replica1_data` → `/data` (replica 1)
- `replica2_data` → `/data` (replica 2)
- `replica3_data` → `/data` (replica 3)

### Data Structure

```
/data/
  wal/
    synap.wal           # Write-Ahead Log
  snapshots/
    snapshot_*.bin      # Periodic snapshots
```

### Backup Strategy

```bash
# Backup master data
docker run --rm \
  -v synap_master_data:/data \
  -v $(pwd)/backup:/backup \
  alpine tar czf /backup/master-backup.tar.gz /data

# Restore master data
docker run --rm \
  -v synap_master_data:/data \
  -v $(pwd)/backup:/backup \
  alpine tar xzf /backup/master-backup.tar.gz -C /
```

## Scaling

### Add More Replicas

1. Edit `docker-compose.yml`:

```yaml
replica4:
  build:
    context: .
    dockerfile: Dockerfile
  container_name: synap-replica4
  hostname: replica4
  ports:
    - "15540:15500"
  environment:
    - RUST_LOG=info
    - SYNAP_ROLE=replica
    - SYNAP_MASTER_ADDRESS=master:15501
  volumes:
    - replica4_data:/data
    - ./config-replica.yml:/app/config.yml:ro
  networks:
    - synap-network
  depends_on:
    master:
      condition: service_healthy
```

2. Add volume:

```yaml
volumes:
  replica4_data:
    driver: local
```

3. Restart cluster:

```bash
./scripts/docker-deploy.sh restart
```

## Production Best Practices

### 1. Resource Limits

Add resource constraints to `docker-compose.yml`:

```yaml
services:
  master:
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 4G
        reservations:
          cpus: '1'
          memory: 2G
```

### 2. Restart Policy

Already configured in `docker-compose.yml`:

```yaml
restart: unless-stopped
```

### 3. Monitoring

Use health checks and metrics:

```bash
# Health endpoint
curl http://localhost:15500/health

# Replication metrics
curl http://localhost:15500/health/replication

# Stats
curl http://localhost:15500/stats
```

### 4. Logging

Configure log rotation:

```yaml
logging:
  driver: "json-file"
  options:
    max-size: "100m"
    max-file: "10"
```

### 5. Network Isolation

Use internal networks for replication:

```yaml
networks:
  synap-network:
    driver: bridge
    internal: false  # Set to true for production
```

### 6. Security

- Enable authentication in config files
- Use TLS for production
- Restrict network access
- Use secrets for sensitive data

## Troubleshooting

### Master Not Starting

```bash
# Check logs
docker logs synap-master

# Common issues:
# - Port already in use
# - Insufficient memory
# - Invalid configuration
```

### Replica Not Connecting

```bash
# Check replica logs
docker logs synap-replica1

# Verify master is running
curl http://localhost:15500/health

# Check network connectivity
docker exec synap-replica1 ping master
```

### Replication Lag

```bash
# Check replication status
curl http://localhost:15500/health/replication

# Common causes:
# - Network latency
# - High write volume
# - Insufficient replica resources
```

### Data Loss After Restart

```bash
# Verify volumes are mounted
docker volume ls
docker volume inspect synap_master_data

# Check persistence is enabled in config
grep -A 5 "persistence:" config-master.yml
```

## Performance Tuning

### Master Node

```yaml
kv_store:
  max_memory_mb: 8192  # Increase for more data

persistence:
  wal:
    fsync_mode: "periodic"  # Best for performance
    fsync_interval_ms: 1000
```

### Replica Nodes

```yaml
replication:
  buffer_size_kb: 512  # Increase for high throughput
  heartbeat_interval_ms: 500  # Faster detection
```

### Docker Resources

Increase container limits:

```yaml
deploy:
  resources:
    limits:
      cpus: '4'
      memory: 8G
```

## Monitoring Dashboard

### Prometheus + Grafana (Optional)

Add to `docker-compose.yml`:

```yaml
prometheus:
  image: prom/prometheus
  ports:
    - "9090:9090"
  volumes:
    - ./prometheus.yml:/etc/prometheus/prometheus.yml

grafana:
  image: grafana/grafana
  ports:
    - "3000:3000"
  environment:
    - GF_SECURITY_ADMIN_PASSWORD=admin
```

## Migration Guide

### From Standalone to Cluster

1. **Backup existing data**:
```bash
docker cp synap-standalone:/data ./backup
```

2. **Stop standalone**:
```bash
docker stop synap-standalone
```

3. **Deploy cluster**:
```bash
./scripts/docker-deploy.sh start
```

4. **Restore data to master**:
```bash
docker cp ./backup/. synap-master:/data
docker restart synap-master
```

## See Also

- [Configuration Guide](CONFIGURATION.md)
- [Replication Spec](specs/REPLICATION.md)
- [Production Deployment](DEPLOYMENT.md)
- [Performance Tuning](PERFORMANCE.md)

