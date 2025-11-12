# Migration Guide: Enabling Authentication

This guide helps you migrate from a Synap deployment without authentication to one with authentication enabled.

## Overview

Synap authentication is **optional by default** for backward compatibility. This guide covers:

1. **Planning the migration**
2. **Enabling authentication gradually**
3. **Creating users and API keys**
4. **Updating SDK clients**
5. **Verifying the migration**

---

## Prerequisites

- Synap server v0.8.0 or later
- Access to server configuration file (`config.yml`)
- Admin access to create users and API keys
- List of applications/services that connect to Synap

---

## Step 1: Plan Your Migration

### Identify All Clients

Before enabling authentication, identify all clients that connect to Synap:

```bash
# Check server logs for client IPs
grep "client_ip" /var/log/synap/server.log

# List active connections (if monitoring enabled)
curl http://localhost:15500/stats/connections
```

### Document Current Usage

Create a list of:
- **Applications/Services**: Name and purpose
- **Access Patterns**: Which resources they access (KV, queues, streams)
- **Required Permissions**: Read-only, write, or admin access
- **IP Addresses**: Source IPs for IP filtering

---

## Step 2: Enable Authentication (Optional Mode)

Start by enabling authentication **without requiring it**:

### Update config.yml

```yaml
auth:
  enabled: true
  require_auth: false  # Allow anonymous access during migration
  root:
    username: "root"
    password: "CHANGE_THIS_PASSWORD"  # Change immediately!
    enabled: true
```

### Restart Server

```bash
# Docker
docker restart synap-server

# Systemd
sudo systemctl restart synap
```

### Verify Backward Compatibility

All existing clients should continue working:

```bash
# Test anonymous access still works
curl http://localhost:15500/kv/test

# Should return 200 OK
```

---

## Step 3: Create Users and API Keys

### Create Root User (if not already created)

```bash
curl -X POST http://localhost:15500/auth/users \
  -u root:CHANGE_THIS_PASSWORD \
  -H "Content-Type: application/json" \
  -d '{
    "username": "root",
    "password": "NEW_SECURE_PASSWORD",
    "is_admin": true
  }'
```

### Create Service Users

For each application/service, create a dedicated user:

```bash
# Example: Queue worker service
curl -X POST http://localhost:15500/auth/users \
  -u root:NEW_SECURE_PASSWORD \
  -H "Content-Type: application/json" \
  -d '{
    "username": "queue_worker",
    "password": "secure_worker_password",
    "is_admin": false
  }'

# Assign role
curl -X POST http://localhost:15500/auth/users/queue_worker/roles \
  -u root:NEW_SECURE_PASSWORD \
  -H "Content-Type: application/json" \
  -d '{
    "role": "readonly"  # or create custom role
  }'
```

### Create API Keys

For each service, create an API key:

```bash
# Create API key for queue worker
curl -X POST http://localhost:15500/auth/keys \
  -u root:NEW_SECURE_PASSWORD \
  -H "Content-Type: application/json" \
  -d '{
    "name": "queue-worker-key",
    "expires_in_seconds": 2592000,  # 30 days
    "permissions": [
      {
        "resource": "queue:*",
        "actions": ["read", "write"]
      },
      {
        "resource": "kv:*",
        "actions": ["read"]
      }
    ],
    "allowed_ips": ["10.0.0.100"]  # Optional IP filtering
  }'

# Response includes the API key - SAVE IT SECURELY!
# {
#   "id": "key_123",
#   "key": "sk_XXXXX...",  # Only shown once!
#   ...
# }
```

---

## Step 4: Update SDK Clients

### Python SDK

**Before** (no auth):
```python
from synap_sdk import SynapClient, SynapConfig

config = SynapConfig.create("http://localhost:15500")
async with SynapClient(config) as client:
    await client.kv.set("key", "value")
```

**After** (with API key):
```python
from synap_sdk import SynapClient, SynapConfig

config = SynapConfig.create("http://localhost:15500")
config = config.with_auth_token("sk_XXXXX...")  # API key
async with SynapClient(config) as client:
    await client.kv.set("key", "value")
```

**Or with Basic Auth**:
```python
config = SynapConfig.create("http://localhost:15500")
config = config.with_basic_auth("username", "password")
async with SynapClient(config) as client:
    await client.kv.set("key", "value")
```

### TypeScript SDK

**Before**:
```typescript
import { Synap } from 'synap-sdk';

const synap = new Synap({
  url: 'http://localhost:15500',
});

await synap.kv.set('key', 'value');
```

**After**:
```typescript
import { Synap } from 'synap-sdk';

const synap = new Synap({
  url: 'http://localhost:15500',
  auth: {
    type: 'api_key',
    apiKey: 'sk_XXXXX...',
  },
});

await synap.kv.set('key', 'value');
```

### Rust SDK

**Before**:
```rust
use synap_sdk::client::{SynapClient, SynapConfig};

let config = SynapConfig::new("http://localhost:15500");
let client = SynapClient::new(config)?;
client.kv().set("key", b"value").await?;
```

**After**:
```rust
use synap_sdk::client::{SynapClient, SynapConfig};

let config = SynapConfig::new("http://localhost:15500")
    .with_auth_token("sk_XXXXX...");
let client = SynapClient::new(config)?;
client.kv().set("key", b"value").await?;
```

### PHP SDK

**Before**:
```php
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

$config = SynapConfig::create('http://localhost:15500');
$client = new SynapClient($config);
$client->kv()->set('key', 'value');
```

**After**:
```php
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

$config = SynapConfig::create('http://localhost:15500')
    ->withAuthToken('sk_XXXXX...');
$client = new SynapClient($config);
$client->kv()->set('key', 'value');
```

### C# SDK

**Before**:
```csharp
using Synap.SDK;

var config = SynapConfig.Create("http://localhost:15500");
using var client = new SynapClient(config);
await client.KV.SetAsync("key", Encoding.UTF8.GetBytes("value"));
```

**After**:
```csharp
using Synap.SDK;

var config = SynapConfig.Create("http://localhost:15500")
    .WithAuthToken("sk_XXXXX...");
using var client = new SynapClient(config);
await client.KV.SetAsync("key", Encoding.UTF8.GetBytes("value"));
```

---

## Step 5: Test with Authentication

### Test Each Client

Update one client at a time and verify it works:

```bash
# Test with API key
curl -H "Authorization: Bearer sk_XXXXX..." \
  http://localhost:15500/kv/test

# Test with Basic Auth
curl -u username:password \
  http://localhost:15500/kv/test
```

### Monitor Logs

Watch for authentication failures:

```bash
# Monitor auth failures
tail -f /var/log/synap/server.log | grep "UNAUTHORIZED\|FORBIDDEN"
```

---

## Step 6: Enable Required Authentication

Once all clients are updated, enable mandatory authentication:

### Update config.yml

```yaml
auth:
  enabled: true
  require_auth: true  # Now require auth for all requests
  root:
    username: "root"
    password: "SECURE_PASSWORD"
    enabled: true
```

### Restart Server

```bash
docker restart synap-server
# or
sudo systemctl restart synap
```

### Verify Anonymous Access is Blocked

```bash
# Should return 401 Unauthorized
curl http://localhost:15500/kv/test

# With auth should work
curl -H "Authorization: Bearer sk_XXXXX..." \
  http://localhost:15500/kv/test
```

---

## Step 7: Post-Migration Checklist

- [ ] All clients updated with authentication
- [ ] All API keys created and stored securely
- [ ] Root password changed from default
- [ ] `require_auth: true` enabled
- [ ] Anonymous access blocked (401 responses)
- [ ] Monitoring enabled for auth failures
- [ ] Documentation updated with new credentials
- [ ] Backup of user/API key data

---

## Troubleshooting

### Client Gets 401 Unauthorized

**Problem**: Client receives 401 even with credentials

**Solutions**:
1. Verify API key is correct (check for typos)
2. Check if API key has expired
3. Verify IP address is allowed (if IP filtering enabled)
4. Check server logs for specific error

### Client Gets 403 Forbidden

**Problem**: Client authenticated but operation denied

**Solutions**:
1. Check user/API key permissions
2. Verify resource pattern matches (e.g., `kv:*` vs `kv:users:*`)
3. Check if action is allowed (read vs write)
4. Verify user has required role

### Migration Rollback

If you need to rollback:

```yaml
auth:
  enabled: false  # Disable authentication
```

Restart server and all clients will work without auth again.

---

## Best Practices

1. **Gradual Migration**: Enable `require_auth: false` first, update clients, then enable `require_auth: true`
2. **Test Each Client**: Update and test one client at a time
3. **Monitor Logs**: Watch for authentication failures during migration
4. **Backup Credentials**: Store API keys securely (password manager, secrets manager)
5. **Document Changes**: Keep track of which clients use which credentials
6. **Set Expiration**: Use temporary API keys during migration, rotate after completion

---

## Example Migration Timeline

**Day 1**: Enable auth (optional mode)
- Update config.yml
- Restart server
- Verify backward compatibility

**Day 2-3**: Create users and API keys
- Create service users
- Generate API keys
- Document credentials

**Day 4-7**: Update clients (one per day)
- Update SDK clients
- Test each client
- Monitor for issues

**Day 8**: Enable required auth
- Set `require_auth: true`
- Restart server
- Verify all clients work

**Day 9+**: Post-migration
- Monitor logs
- Rotate API keys
- Review permissions

---

## Support

For issues during migration:

1. Check server logs: `/var/log/synap/server.log`
2. Review authentication documentation: `docs/AUTHENTICATION.md`
3. Test with curl to isolate SDK issues
4. Enable debug logging: `SYNAP_LOG=debug`

---

**Last Updated**: January 2025  
**Version**: 0.8.0+

