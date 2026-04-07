---
title: Server Configuration
module: configuration
id: server-configuration
order: 2
description: Network, ports, and server settings
tags: [configuration, server, network, ports]
---

# Server Configuration

Complete guide to configuring Synap server network and ports.

## Basic Server Settings

### Host and Port

```yaml
server:
  host: "0.0.0.0"  # Bind to all interfaces
  port: 15500      # HTTP port
```

### Command Line

```bash
synap-server --host 0.0.0.0 --port 15500
```

### Environment Variables

```bash
export SYNAP_HOST=0.0.0.0
export SYNAP_PORT=15500
synap-server
```

## Network Configuration

### Bind to Specific Interface

```yaml
server:
  host: "127.0.0.1"  # Localhost only
  port: 15500
```

### Bind to All Interfaces

```yaml
server:
  host: "0.0.0.0"  # All interfaces
  port: 15500
```

### IPv6 Support

```yaml
server:
  host: "::"  # All IPv6 interfaces
  port: 15500
```

## Reverse Proxy

### Nginx Configuration

```nginx
upstream synap {
    server localhost:15500;
}

server {
    listen 80;
    server_name synap.example.com;
    
    location / {
        proxy_pass http://synap;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Caddy Configuration

```caddy
synap.example.com {
    reverse_proxy localhost:15500
}
```

### Traefik Configuration

```yaml
http:
  routers:
    synap:
      rule: "Host(`synap.example.com`)"
      service: synap
  services:
    synap:
      loadBalancer:
        servers:
          - url: "http://localhost:15500"
```

## TLS/SSL

### Using Reverse Proxy

Configure TLS in reverse proxy (recommended):

```nginx
server {
    listen 443 ssl http2;
    server_name synap.example.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    location / {
        proxy_pass http://localhost:15500;
    }
}
```

## Firewall Configuration

### Linux (UFW)

```bash
# Allow port 15500
sudo ufw allow 15500/tcp

# Allow from specific IP
sudo ufw allow from 192.168.1.0/24 to any port 15500
```

### Linux (firewalld)

```bash
# Allow port 15500
sudo firewall-cmd --permanent --add-port=15500/tcp
sudo firewall-cmd --reload
```

### Windows Firewall

```powershell
New-NetFirewallRule -DisplayName "Synap Server" `
    -Direction Inbound `
    -LocalPort 15500 `
    -Protocol TCP `
    -Action Allow
```

## Health Checks

### HTTP Health Check

```bash
curl http://localhost:15500/health
```

**Response:**
```json
{
  "status": "healthy",
  "uptime_secs": 12345
}
```

### Docker Health Check

```yaml
services:
  synap:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:15500/health"]
      interval: 10s
      timeout: 5s
      retries: 3
```

### Kubernetes Liveness Probe

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 15500
  initialDelaySeconds: 30
  periodSeconds: 10
```

## Performance Tuning

### Connection Limits

Configure in reverse proxy:

```nginx
upstream synap {
    server localhost:15500;
    keepalive 100;
}

server {
    # ...
    keepalive_timeout 65;
    keepalive_requests 1000;
}
```

### Timeouts

```yaml
server:
  host: "0.0.0.0"
  port: 15500
  read_timeout_secs: 30
  write_timeout_secs: 30
```

## Related Topics

- [Configuration Overview](./CONFIGURATION.md) - General configuration
- [Logging Configuration](./LOGGING.md) - Log settings
- [Performance Tuning](./PERFORMANCE_TUNING.md) - Performance optimization

