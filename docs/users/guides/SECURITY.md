---
title: Security Guide
module: guides
id: security-guide
order: 8
description: Security best practices and hardening
tags: [guides, security, authentication, encryption, hardening]
---

# Security Guide

Complete guide to securing Synap in production.

## Overview

Synap security features:
- **Authentication**: Users and API keys
- **RBAC**: Role-Based Access Control
- **TLS/SSL**: Encryption in transit
- **Network Security**: Firewall and access control

## Authentication

### Enable Authentication

```yaml
authentication:
  enabled: true
  
  users:
    - username: admin
      password_hash: "$2b$12$..."
      role: admin
    
    - username: user
      password_hash: "$2b$12$..."
      role: user
  
  api_keys:
    - key: "sk_live_abc123..."
      name: "Production API"
      role: admin
      expires_days: 365
```

### Generate Password Hash

```python
import bcrypt

password = "mypassword"
hashed = bcrypt.hashpw(password.encode('utf-8'), bcrypt.gensalt())
print(hashed.decode('utf-8'))
```

## API Keys

### Create API Key

```yaml
api_keys:
  - key: "sk_live_abc123def456..."
    name: "Production API"
    role: admin
    expires_days: 365
    allowed_ips: ["192.168.1.0/24"]  # Optional IP restriction
```

### Rotate API Keys

1. Create new key
2. Update applications
3. Revoke old key after migration

## TLS/SSL

### Using Reverse Proxy

Configure TLS in reverse proxy (recommended):

**Nginx:**
```nginx
server {
    listen 443 ssl http2;
    server_name synap.example.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    
    location / {
        proxy_pass http://localhost:15500;
    }
}
```

**Caddy:**
```caddy
synap.example.com {
    tls /path/to/cert.pem /path/to/key.pem
    reverse_proxy localhost:15500
}
```

## Network Security

### Firewall Configuration

**Linux (UFW):**
```bash
# Allow only specific IPs
sudo ufw allow from 192.168.1.0/24 to any port 15500

# Deny all other access
sudo ufw deny 15500
```

**Windows Firewall:**
```powershell
New-NetFirewallRule -DisplayName "Synap Server" `
    -Direction Inbound `
    -LocalPort 15500 `
    -Protocol TCP `
    -Action Allow `
    -RemoteAddress 192.168.1.0/24
```

### Bind to Specific Interface

```yaml
server:
  host: "127.0.0.1"  # Localhost only
  port: 15500
```

## Access Control

### IP Whitelisting

```yaml
server:
  allowed_ips:
    - "192.168.1.0/24"
    - "10.0.0.0/8"
```

### Rate Limiting

Configure in reverse proxy:

**Nginx:**
```nginx
limit_req_zone $binary_remote_addr zone=synap:10m rate=100r/s;

server {
    limit_req zone=synap burst=20;
    # ...
}
```

## Data Security

### Encryption at Rest

Use encrypted filesystem or disk encryption:

```bash
# Use LUKS for disk encryption
sudo cryptsetup luksFormat /dev/sdb1
sudo cryptsetup luksOpen /dev/sdb1 synap-data
```

### Secure Configuration

```yaml
# Store sensitive config in environment variables
authentication:
  enabled: true
  # Passwords and keys from environment
  users: ${SYNAP_USERS}
  api_keys: ${SYNAP_API_KEYS}
```

## Best Practices

### Use Strong Passwords

- Minimum 12 characters
- Mix of letters, numbers, symbols
- Use password manager

### Rotate Credentials

- Rotate API keys regularly
- Change passwords periodically
- Monitor for compromised credentials

### Monitor Access

```bash
# Check access logs
tail -f /var/log/synap/access.log

# Monitor failed authentication
grep "authentication failed" /var/log/synap/access.log
```

### Least Privilege

- Use appropriate roles (user vs admin)
- Limit API key permissions
- Restrict network access

## Related Topics

- [Authentication Guide](../api/AUTHENTICATION.md) - Authentication setup
- [Configuration Guide](../configuration/CONFIGURATION.md) - Server configuration

