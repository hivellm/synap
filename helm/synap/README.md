# Synap Helm Chart

High-Performance In-Memory Key-Value Store & Message Broker for Kubernetes.

## Prerequisites

- Kubernetes 1.19+
- Helm 3.0+
- PersistentVolume provisioner support in the underlying infrastructure

## Installing the Chart

```bash
# Add the Synap Helm repository
helm repo add synap https://hivellm.github.io/synap-charts
helm repo update

# Install Synap
helm install my-synap synap/synap

# Or install from local directory
helm install my-synap ./helm/synap
```

## Uninstalling the Chart

```bash
helm uninstall my-synap
```

## Configuration

### Basic Configuration

```yaml
# values-custom.yaml
replicaCount: 1

resources:
  limits:
    memory: "4Gi"
    cpu: "2000m"
  requests:
    memory: "1Gi"
    cpu: "500m"

persistence:
  enabled: true
  size: 20Gi
```

Install with custom values:

```bash
helm install my-synap synap/synap -f values-custom.yaml
```

### Master-Replica Setup

**Master Node:**

```yaml
# values-master.yaml
replicaCount: 1

config:
  replication:
    enabled: true
    role: "master"

replication:
  master:
    enabled: true
```

**Replica Nodes:**

```yaml
# values-replica.yaml
replicaCount: 2

config:
  replication:
    enabled: true
    role: "replica"

replication:
  replica:
    enabled: true
    masterAddress: "my-synap-master:15501"
```

Deploy:

```bash
# Deploy master
helm install synap-master synap/synap -f values-master.yaml

# Deploy replicas
helm install synap-replica synap/synap -f values-replica.yaml
```

### Enable Ingress

```yaml
ingress:
  enabled: true
  className: "nginx"
  annotations:
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
  hosts:
    - host: synap.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: synap-tls
      hosts:
        - synap.example.com
```

### Prometheus Monitoring

```yaml
serviceMonitor:
  enabled: true
  interval: 30s

podAnnotations:
  prometheus.io/scrape: "true"
  prometheus.io/port: "15500"
  prometheus.io/path: "/metrics"
```

### Autoscaling

```yaml
autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 80
  targetMemoryUtilizationPercentage: 80
```

## Parameters

### Global Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Number of Synap replicas | `1` |
| `image.repository` | Synap image repository | `hivehub/synap` |
| `image.tag` | Synap image tag | `""` (uses appVersion) |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |

### Service Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `service.type` | Kubernetes service type | `ClusterIP` |
| `service.port` | Service port | `15500` |
| `service.annotations` | Service annotations | `{}` |

### Persistence Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `persistence.enabled` | Enable persistence | `true` |
| `persistence.storageClass` | Storage class | `""` |
| `persistence.accessMode` | Access mode | `ReadWriteOnce` |
| `persistence.size` | Volume size | `10Gi` |

### Resource Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `resources.limits.cpu` | CPU limit | `2000m` |
| `resources.limits.memory` | Memory limit | `4Gi` |
| `resources.requests.cpu` | CPU request | `500m` |
| `resources.requests.memory` | Memory request | `1Gi` |

### Configuration Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `config.server.port` | Server port | `15500` |
| `config.kv_store.max_memory_mb` | Max memory in MB | `4096` |
| `config.queue.enabled` | Enable queue system | `true` |
| `config.persistence.enabled` | Enable WAL/snapshots | `true` |
| `config.replication.enabled` | Enable replication | `false` |
| `config.rate_limit.enabled` | Enable rate limiting | `false` |

## Examples

### Development Setup

```bash
helm install synap-dev synap/synap \
  --set persistence.enabled=false \
  --set resources.requests.memory=512Mi \
  --set resources.requests.cpu=250m
```

### Production Setup with Replication

```bash
# Master
helm install synap-master synap/synap \
  --set replication.master.enabled=true \
  --set config.replication.enabled=true \
  --set config.replication.role=master \
  --set persistence.size=50Gi \
  --set resources.limits.memory=8Gi

# Replicas
helm install synap-replica synap/synap \
  --set replication.replica.enabled=true \
  --set replication.replica.replicaCount=3 \
  --set config.replication.enabled=true \
  --set config.replication.role=replica \
  --set persistence.size=50Gi
```

### With Prometheus Operator

```bash
helm install synap synap/synap \
  --set serviceMonitor.enabled=true \
  --set podAnnotations."prometheus\.io/scrape"=true
```

## Accessing Synap

### Inside Kubernetes

```bash
# Service name: my-synap
# Port: 15500

# Test connection
kubectl run -it --rm test --image=curlimages/curl --restart=Never -- \
  curl http://my-synap:15500/health
```

### Port Forward

```bash
kubectl port-forward svc/my-synap 15500:15500

# Access locally
curl http://localhost:15500/health
curl http://localhost:15500/metrics
```

### Using synap-cli

```bash
kubectl exec -it deployment/my-synap -- synap-cli
```

## Upgrading

```bash
# Update repository
helm repo update

# Upgrade release
helm upgrade my-synap synap/synap

# Or with custom values
helm upgrade my-synap synap/synap -f values-custom.yaml
```

## Troubleshooting

### Check Pod Status

```bash
kubectl get pods -l app.kubernetes.io/name=synap
kubectl describe pod <pod-name>
kubectl logs <pod-name>
```

### Check Configuration

```bash
kubectl get configmap my-synap -o yaml
```

### Check Persistence

```bash
kubectl get pvc
kubectl describe pvc my-synap
```

### Common Issues

1. **Pod not starting**: Check resource limits and PVC availability
2. **Connection refused**: Verify service and network policies
3. **Replication lag**: Check master/replica connectivity and network latency

## Development

### Testing Locally

```bash
# Lint chart
helm lint ./helm/synap

# Dry run install
helm install --dry-run --debug my-synap ./helm/synap

# Template rendering
helm template my-synap ./helm/synap
```

## License

MIT License - See [LICENSE](../../../LICENSE) for details.

## Support

- GitHub Issues: https://github.com/hivellm/synap/issues
- Documentation: https://github.com/hivellm/synap/blob/main/README.md

