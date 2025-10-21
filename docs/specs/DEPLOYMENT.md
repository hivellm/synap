# Deployment Guide

## Deployment Modes

### 1. Single Node (Development/Testing)

```
┌──────────────┐
│    Synap     │
│   (Master)   │
│  Port: 15500 │
└──────────────┘
```

**Use Cases**:
- Development
- Testing
- Low-traffic applications
- Non-critical services

**Configuration**: `config-standalone.yml`

```yaml
server:
  role: "master"
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: false
```

**Start Command**:
```bash
./synap-server --config config-standalone.yml
```

### 2. Master-Slave (Production)

```
        ┌──────────────┐
        │   Master     │
        │  (Write)     │
        │ :15500       │
        └──────────────┘
               │
       Replication Stream
               │
    ┌──────────┴──────────┐
    ▼                     ▼
┌──────────┐        ┌──────────┐
│Replica 1 │        │Replica 2 │
│ (Read)   │        │ (Read)   │
│ :15510   │        │ :15520   │
└──────────┘        └──────────┘
```

**Use Cases**:
- Production deployments
- High-availability requirements
- Read-heavy workloads
- Geographic distribution

**Start Master**:
```bash
./synap-server --config config-master.yml
```

**Start Replicas**:
```bash
# Replica 1
./synap-server --config config-replica-1.yml

# Replica 2  
./synap-server --config config-replica-2.yml
```

### 3. Load Balanced (High Availability)

```
        ┌──────────────────┐
        │  Load Balancer   │
        │   (HAProxy)      │
        └──────────────────┘
                  │
      ┌───────────┼───────────┐
      ▼           ▼           ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│Replica 1 │ │Replica 2 │ │Replica 3 │
│  :15510  │ │  :15520  │ │  :15530  │
└──────────┘ └──────────┘ └──────────┘
      ▲           ▲           ▲
      └───────────┴───────────┘
                  │
        ┌──────────────────┐
        │     Master       │
        │    (Write)       │
        │    :15500        │
        └──────────────────┘
```

**HAProxy Config**:
```haproxy
frontend synap_read
    bind *:15500
    default_backend synap_replicas

backend synap_replicas
    balance roundrobin
    option httpchk GET /health
    server replica1 replica1:15510 check
    server replica2 replica2:15520 check
    server replica3 replica3:15530 check

frontend synap_write
    bind *:15501
    default_backend synap_master

backend synap_master
    server master master:15500 check
```

## Docker Deployment

### Dockerfile

```dockerfile
FROM rust:1.75 AS builder

WORKDIR /app
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/synap-server /usr/local/bin/
COPY --from=builder /app/config.yml /etc/synap/config.yml

EXPOSE 15500

CMD ["synap-server", "--config", "/etc/synap/config.yml"]
```

### docker-compose.yml

```yaml
version: '3.8'

services:
  master:
    build: .
    ports:
      - "15500:15500"
      - "15501:15501"
    environment:
      - SYNAP_ROLE=master
      - RUST_LOG=info
    volumes:
      - ./config-master.yml:/etc/synap/config.yml
      - synap-data:/var/lib/synap
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15500/health"]
      interval: 10s
      timeout: 5s
      retries: 3
  
  replica1:
    build: .
    ports:
      - "15510:15500"
    environment:
      - SYNAP_ROLE=replica
      - SYNAP_MASTER=master:15501
    volumes:
      - ./config-replica.yml:/etc/synap/config.yml
    depends_on:
      - master
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15500/health"]
  
  replica2:
    build: .
    ports:
      - "15520:15500"
    environment:
      - SYNAP_ROLE=replica
      - SYNAP_MASTER=master:15501
    volumes:
      - ./config-replica.yml:/etc/synap/config.yml
    depends_on:
      - master

volumes:
  synap-data:
```

### Deploy

```bash
# Build and start
docker-compose up -d

# Check status
docker-compose ps

# View logs
docker-compose logs -f

# Scale replicas
docker-compose up -d --scale replica1=3
```

## Kubernetes Deployment

### Namespace

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: synap
```

### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: synap-config
  namespace: synap
data:
  master-config.yml: |
    server:
      role: master
      port: 15500
    replication:
      enabled: true
      listen_port: 15501
  
  replica-config.yml: |
    server:
      role: replica
      port: 15500
    replication:
      enabled: true
      master_host: synap-master
      master_port: 15501
```

### Master Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: synap-master
  namespace: synap
spec:
  replicas: 1  # Only 1 master
  selector:
    matchLabels:
      app: synap
      role: master
  template:
    metadata:
      labels:
        app: synap
        role: master
    spec:
      containers:
      - name: synap
        image: hivellm/synap:0.1.0
        ports:
        - containerPort: 15500
          name: http
        - containerPort: 15501
          name: replication
        volumeMounts:
        - name: config
          mountPath: /etc/synap
        resources:
          requests:
            memory: "4Gi"
            cpu: "2"
          limits:
            memory: "8Gi"
            cpu: "4"
        livenessProbe:
          httpGet:
            path: /health
            port: 15500
          initialDelaySeconds: 10
          periodSeconds: 10
      volumes:
      - name: config
        configMap:
          name: synap-config
          items:
          - key: master-config.yml
            path: config.yml
```

### Replica Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: synap-replica
  namespace: synap
spec:
  replicas: 3  # Multiple replicas
  selector:
    matchLabels:
      app: synap
      role: replica
  template:
    metadata:
      labels:
        app: synap
        role: replica
    spec:
      containers:
      - name: synap
        image: hivellm/synap:0.1.0
        ports:
        - containerPort: 15500
        volumeMounts:
        - name: config
          mountPath: /etc/synap
        resources:
          requests:
            memory: "4Gi"
            cpu: "2"
          limits:
            memory: "8Gi"
            cpu: "4"
      volumes:
      - name: config
        configMap:
          name: synap-config
          items:
          - key: replica-config.yml
            path: config.yml
```

### Services

```yaml
apiVersion: v1
kind: Service
metadata:
  name: synap-master
  namespace: synap
spec:
  selector:
    app: synap
    role: master
  ports:
  - name: http
    port: 15500
    targetPort: 15500
  - name: replication
    port: 15501
    targetPort: 15501
  type: ClusterIP

---
apiVersion: v1
kind: Service
metadata:
  name: synap-replica
  namespace: synap
spec:
  selector:
    app: synap
    role: replica
  ports:
  - name: http
    port: 15500
    targetPort: 15500
  type: ClusterIP
```

### Deploy to Kubernetes

```bash
# Create namespace
kubectl apply -f k8s/namespace.yml

# Deploy config
kubectl apply -f k8s/configmap.yml

# Deploy master
kubectl apply -f k8s/master-deployment.yml
kubectl apply -f k8s/master-service.yml

# Deploy replicas
kubectl apply -f k8s/replica-deployment.yml
kubectl apply -f k8s/replica-service.yml

# Check status
kubectl get pods -n synap
kubectl logs -n synap synap-master-xxx
```

## Systemd Service

### synap.service

```ini
[Unit]
Description=Synap Server
After=network.target

[Service]
Type=simple
User=synap
Group=synap
WorkingDirectory=/opt/synap
ExecStart=/usr/local/bin/synap-server --config /etc/synap/config.yml
Restart=always
RestartSec=10
LimitNOFILE=1000000

[Install]
WantedBy=multi-user.target
```

### Installation

```bash
# Copy binary
sudo cp target/release/synap-server /usr/local/bin/

# Copy config
sudo mkdir -p /etc/synap
sudo cp config.yml /etc/synap/

# Create user
sudo useradd -r -s /bin/false synap

# Install service
sudo cp synap.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable synap
sudo systemctl start synap

# Check status
sudo systemctl status synap
```

## Monitoring

### Prometheus Integration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'synap'
    static_configs:
      - targets:
        - 'localhost:15500'
        - 'replica1:15510'
        - 'replica2:15520'
    metrics_path: '/metrics'
    scrape_interval: 10s
```

### Health Checks

```bash
# Application health
curl http://localhost:15500/health

# Component health
curl http://localhost:15500/health/kv
curl http://localhost:15500/health/queue
curl http://localhost:15500/health/replication
```

## Backup & Recovery

### Data Backup

```bash
# Export all data (not needed if using replication)
synap-admin export --output backup-2025-10-15.json

# Restore
synap-admin import --input backup-2025-10-15.json
```

### Disaster Recovery

```bash
# 1. Promote replica to master
synap-admin promote-replica --replica replica-1 --force

# 2. Update DNS/load balancer

# 3. Restart old master as replica
```

## Security

### TLS/SSL

```yaml
server:
  tls:
    enabled: true
    cert_path: "/etc/synap/tls/cert.pem"
    key_path: "/etc/synap/tls/key.pem"
```

### Firewall

```bash
# Allow Synap ports
sudo ufw allow 15500/tcp  # HTTP API
sudo ufw allow 15501/tcp  # Replication
```

### API Keys

```bash
# Generate API key
synap-admin create-api-key --name "production-app" --role admin

# Revoke API key
synap-admin revoke-api-key --key synap_abc123...
```

## Scaling Guidelines

### Vertical Scaling

Add more CPU/RAM to single node:
- **4 → 8 cores**: ~2x throughput
- **8 → 16 GB RAM**: ~2x capacity
- **Diminishing returns** beyond 16 cores

### Horizontal Scaling (Replicas)

Add read replicas for read scaling:
- **Linear scaling** for read operations
- **No scaling** for write operations (single master)
- **Minimal overhead** (< 5%) for replication

### When to Scale

**Add RAM** when:
- Memory usage > 80%
- Frequent evictions
- Keys approaching limits

**Add CPU** when:
- CPU usage > 70%
- Latency increasing
- Queue depths growing

**Add Replicas** when:
- Read traffic > 100K rps
- Geographic distribution needed
- High availability required

## Troubleshooting

### High Latency

```bash
# Check CPU usage
top

# Check network
netstat -an | grep 15500

# Check logs
tail -f /var/log/synap/server.log

# Check replication lag
curl http://localhost:15500/health/replication
```

### Memory Issues

```bash
# Check memory usage
free -h

# Check Synap memory
curl http://localhost:15500/metrics | grep synap_memory

# Enable memory profiling
RUST_LOG=synap=debug ./synap-server
```

### Connection Issues

```bash
# Check open connections
netstat -an | grep ESTABLISHED | wc -l

# Check file descriptors
lsof -p $(pgrep synap-server) | wc -l

# Increase limits
ulimit -n 1000000
```

## See Also

- [CONFIGURATION.md](CONFIGURATION.md) - Full configuration reference
- [PERFORMANCE.md](PERFORMANCE.md) - Performance tuning
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture

