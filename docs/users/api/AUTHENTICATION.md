---
title: Authentication
module: api
id: authentication
order: 2
description: Authentication, authorization, and security
tags: [authentication, security, api-keys, rbac]
---

# Authentication

Complete guide to authentication and security in Synap.

## Overview

Synap supports multiple authentication methods:
- **User Authentication** - Username/password with bcrypt
- **API Keys** - Bearer tokens for programmatic access
- **RBAC** - Role-Based Access Control

## Configuration

### Enable Authentication

```yaml
authentication:
  enabled: true
  
  users:
    - username: admin
      password_hash: "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyY5Y5Y5Y5Y5"
      role: admin
    
    - username: user
      password_hash: "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyY5Y5Y5Y5Y5"
      role: user
  
  api_keys:
    - key: "sk_live_abc123def456ghi789"
      name: "Production API"
      role: admin
      expires_days: 365
    
    - key: "sk_test_xyz789uvw456rst123"
      name: "Development API"
      role: user
      expires_days: 30
```

## User Authentication

### Basic Auth

```bash
# Using curl
curl -u admin:password http://localhost:15500/kv/stats

# Or with header
curl -H "Authorization: Basic YWRtaW46cGFzc3dvcmQ=" \
  http://localhost:15500/kv/stats
```

### Generate Password Hash

```python
import bcrypt

password = "mypassword"
hashed = bcrypt.hashpw(password.encode('utf-8'), bcrypt.gensalt())
print(hashed.decode('utf-8'))
```

## API Keys

### Bearer Token

```bash
curl -H "Authorization: Bearer sk_live_abc123..." \
  http://localhost:15500/kv/stats
```

### Query Parameter

```bash
curl "http://localhost:15500/kv/stats?api_key=sk_live_abc123..."
```

## Roles

### Admin Role

Full access to all operations:
- All KV operations
- Queue management
- Stream management
- Cluster management
- System administration

### User Role

Limited access:
- KV operations (read/write)
- Queue consume/publish
- Stream consume/publish
- No administrative operations

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

# With API key
client = SynapClient(
    "http://localhost:15500",
    api_key="sk_live_abc123..."
)

# With basic auth
client = SynapClient(
    "http://localhost:15500",
    username="admin",
    password="password"
)
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

// With API key
const client = new SynapClient("http://localhost:15500", {
  apiKey: "sk_live_abc123..."
});

// With basic auth
const client = new SynapClient("http://localhost:15500", {
  username: "admin",
  password: "password"
});
```

### Rust

```rust
use synap_sdk::SynapClient;

// With API key
let client = SynapClient::new_with_auth(
    "http://localhost:15500",
    Some("sk_live_abc123...".to_string()),
    None,
    None
)?;

// With basic auth
let client = SynapClient::new_with_auth(
    "http://localhost:15500",
    None,
    Some("admin".to_string()),
    Some("password".to_string())
)?;
```

## Security Best Practices

### Use API Keys for Applications

- Generate unique keys per application
- Rotate keys regularly
- Use different keys for different environments

### Use Strong Passwords

- Minimum 12 characters
- Mix of letters, numbers, symbols
- Use password manager

### Enable TLS

Use reverse proxy (Nginx, Caddy) for TLS:

```nginx
server {
    listen 443 ssl;
    server_name synap.example.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    location / {
        proxy_pass http://localhost:15500;
    }
}
```

### Rate Limiting

Configure rate limiting in reverse proxy:

```nginx
limit_req_zone $binary_remote_addr zone=synap:10m rate=100r/s;

server {
    limit_req zone=synap burst=20;
    # ...
}
```

## Related Topics

- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [Configuration Guide](../configuration/CONFIGURATION.md) - Server configuration

