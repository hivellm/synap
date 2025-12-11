---
title: Docker Authentication
module: getting-started
id: docker-authentication
order: 4
description: Docker authentication and registry setup
tags: [getting-started, docker, authentication, registry]
---

# Docker Authentication

Complete guide to Docker authentication and registry access for Synap.

## Docker Hub

### Login

```bash
docker login
# Enter username and password
```

### Pull Image

```bash
docker pull hivellm/synap:latest
```

### Pull Specific Version

```bash
docker pull hivellm/synap:0.8.1
```

## Private Registry

### Login to Private Registry

```bash
docker login registry.example.com
# Enter credentials
```

### Pull from Private Registry

```bash
docker pull registry.example.com/synap:latest
```

### Configure Docker Compose

```yaml
version: '3.8'
services:
  synap:
    image: registry.example.com/synap:latest
    # Or use image pull secrets
    # image: hivellm/synap:latest
```

## Docker Compose with Authentication

### Using .env File

**`.env`:**
```
DOCKER_REGISTRY=registry.example.com
DOCKER_USERNAME=myuser
DOCKER_PASSWORD=mypassword
```

**`docker-compose.yml`:**
```yaml
version: '3.8'
services:
  synap:
    image: ${DOCKER_REGISTRY}/synap:latest
```

### Using Docker Credentials

```bash
# Create credentials helper
echo '{"auths":{"registry.example.com":{"username":"myuser","password":"mypassword"}}}' | \
  docker login registry.example.com --username myuser --password-stdin
```

## Kubernetes

### Create Secret

```bash
kubectl create secret docker-registry regcred \
  --docker-server=registry.example.com \
  --docker-username=myuser \
  --docker-password=mypassword \
  --docker-email=myuser@example.com
```

### Use in Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: synap
spec:
  template:
    spec:
      imagePullSecrets:
      - name: regcred
      containers:
      - name: synap
        image: registry.example.com/synap:latest
```

## Troubleshooting

### Authentication Failed

```bash
# Check login status
docker login registry.example.com

# Verify credentials
docker pull registry.example.com/synap:latest
```

### Permission Denied

```bash
# Check Docker group membership
groups

# Add user to docker group
sudo usermod -aG docker $USER
newgrp docker
```

## Related Topics

- [Docker Installation](./DOCKER.md) - Docker deployment guide
- [Installation Guide](./INSTALLATION.md) - General installation

