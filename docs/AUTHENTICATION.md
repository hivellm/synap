# Synap Authentication & Authorization System

## Overview

Synap implements a comprehensive **rule-based authentication and authorization system** with support for:

✅ **User Management** with SHA512 password hashing  
✅ **Role-Based Access Control (RBAC)**  
✅ **API Keys** with expiration and IP filtering  
✅ **Access Control Lists (ACL)** for fine-grained permissions  
✅ **Basic Auth** and **Bearer Token** authentication  
✅ **Optional security** (off by default for development)  
✅ **Mandatory security** for production (binding to 0.0.0.0)  

---

## Features

### 1. User Management

#### Create Users

```rust
use synap_server::UserManager;

let user_manager = UserManager::new();

// Create admin user
user_manager.create_user("admin", "secure_password", true)?;

// Create regular user
user_manager.create_user("user1", "password123", false)?;
```

#### Password Management

- **SHA512** hashing (hexadecimal output, 128 characters)
- Secure password verification
- Password change capability

```rust
// Change password
user_manager.change_password("user1", "new_password")?;

// Authenticate
let user = user_manager.authenticate("user1", "new_password")?;
```

#### Enable/Disable Users

```rust
// Disable user
user_manager.set_user_enabled("user1", false)?;

// Disabled users cannot authenticate
```

---

### 2. Roles & Permissions

#### Built-in Roles

- **admin**: All permissions (`*`)
- **readonly**: Read-only access to all resources

#### Custom Roles

```rust
use synap_server::{Role, Permission, Action};

let role = Role::custom(
    "queue_manager",
    vec![
        Permission::new("queue:*", Action::All),
        Permission::new("kv:*", Action::Read),
    ]
);

user_manager.create_role(role)?;
```

#### Assign Roles to Users

```rust
user_manager.add_user_role("user1", "queue_manager")?;
user_manager.remove_user_role("user1", "readonly")?;
```

---

### 3. API Keys

#### Generate API Keys

```rust
use synap_server::{ApiKeyManager, Permission, Action};
use std::net::IpAddr;
use std::str::FromStr;

let api_key_manager = ApiKeyManager::new();

// Create API key with permissions
let api_key = api_key_manager.create(
    "service-key-1",  // Name
    Some("user1".to_string()),  // Associated user
    vec![Permission::new("queue:*", Action::Write)],  // Permissions
    vec![IpAddr::from_str("192.168.1.100").unwrap()],  // Allowed IPs
    Some(90),  // Expires in 90 days
)?;

// Key format: sk_XXXXXXXXXXXXXXXXXXXXXXXXXXXXX
tracing::info!("API Key: {}", api_key.key);
```

#### Features

- **Auto-generated** secure keys (`sk_` prefix)
- **Expiration** support (days from creation)
- **IP Filtering** (whitelist specific IPs)
- **Usage Tracking** (count + last used timestamp)
- **Enable/Disable** without deletion

#### Verify API Keys

```rust
use std::net::IpAddr;

let client_ip = IpAddr::from([192, 168, 1, 100]);
let verified_key = api_key_manager.verify("sk_XXXX...", client_ip)?;

tracing::info!("Key Name: {}", verified_key.name);
tracing::info!("Usage Count: {}", verified_key.usage_count);
```

#### Cleanup Expired Keys

```rust
// Remove all expired keys
let removed_count = api_key_manager.cleanup_expired();
tracing::info!("Removed {} expired keys", removed_count);
```

---

### 4. Access Control Lists (ACL)

#### Resource Types

- `Queue` - Message queues
- `KV` - Key-Value store
- `Stream` - Event streams
- `PubSub` - Pub/Sub topics
- `Admin` - Administrative operations

#### Create ACL Rules

```rust
use synap_server::{Acl, AclRule, ResourceType, Action};

let acl = Acl::new();

// Public queue (no auth required)
acl.add_rule(
    "queue:public",
    AclRule::public(ResourceType::Queue, "public")
);

// Private queue (auth required)
acl.add_rule(
    "queue:orders",
    AclRule::authenticated(
        ResourceType::Queue,
        "orders",
        vec![Action::Read, Action::Write]
    )
);
```

#### Check Access

```rust
use synap_server::AuthContext;
use std::net::IpAddr;

let ctx = AuthContext {
    user_id: Some("user1".to_string()),
    api_key_id: None,
    client_ip: IpAddr::from([127, 0, 0, 1]),
    permissions: vec![Permission::new("queue:*", Action::All)],
    is_admin: false,
};

// Check if user can write to queue
let result = acl.check_access(
    ResourceType::Queue,
    "orders",
    Action::Write,
    &ctx
);

if result.is_ok() {
    tracing::info!("Access granted!");
}
```

---

### 5. Authentication Methods

#### Basic Auth (HTTP)

```bash
# Via curl
curl -u username:password http://localhost:15500/queue/test

# URL format (Redis-style)
curl http://username:password@localhost:15500/queue/test
```

#### API Key (Bearer Token)

```bash
# Header
curl -H "Authorization: Bearer sk_XXXXX..." http://localhost:15500/queue/test

# Query parameter
curl http://localhost:15500/queue/test?api_key=sk_XXXXX...
```

---

## Configuration

### config.yml

```yaml
authentication:
  enabled: true
  require_auth: true  # Require auth for all endpoints
  
  default_admin:
    username: "admin"
    password: "CHANGE_ME_IN_PRODUCTION"
  
  api_key_default_expiration_days: 90
  api_key_allowed_ips: []

acl:
  enabled: true
  rules:
    - resource: "queue:public"
      require_auth: false
      actions: ["read", "write"]
    
    - resource: "queue:private"
      require_auth: true
      actions: ["read", "write"]
      allowed_roles: ["admin", "queue_user"]
```

---

## Security Best Practices

### 1. **Production Deployment**

✅ **ALWAYS enable authentication** when binding to `0.0.0.0`  
✅ **Change default root password** immediately after first login  
✅ **Use strong passwords** (minimum 12 characters, mix of letters, numbers, symbols)  
✅ **Enable `require_auth: true`** for production environments  
✅ **Disable root user** after creating admin users (optional, via `root.enabled: false`)  
✅ **Use environment variables** for sensitive credentials (never commit to git)  

### 2. **API Key Management**

✅ **Set expiration** (recommended: 30-90 days, shorter for high-security environments)  
✅ **Use IP filtering** when possible (restrict keys to specific source IPs)  
✅ **Rotate keys regularly** (create new keys before old ones expire)  
✅ **Revoke unused keys** immediately (don't leave orphaned keys)  
✅ **Monitor usage** via `usage_count` and `last_used_at` (detect suspicious activity)  
✅ **Use descriptive names** for keys (e.g., "production-worker-queue-2025-01")  
✅ **Store keys securely** (use secrets managers, never log or commit keys)  
✅ **One key per service** (don't share keys between services)  

### 3. **Password Security**

✅ **SHA512 hashing** (hexadecimal, 128 characters)  
✅ **Never store plain-text** passwords (always hash before storage)  
✅ **Implement password rotation** policies (force change every 90 days)  
✅ **Disable compromised accounts** immediately (set `enabled: false`)  
✅ **Use strong passwords** (minimum 12 characters, avoid common words)  
✅ **Don't reuse passwords** across different services  
✅ **Consider password complexity** (enforce via application logic if needed)  

### 4. **Network Security**

✅ **Use TLS/SSL** for production (set up reverse proxy with HTTPS)  
✅ **Firewall rules** to limit access (only allow necessary IPs/ports)  
✅ **IP whitelisting** for sensitive keys (restrict API keys to known IPs)  
✅ **Monitor failed auth** attempts (set up alerts for brute force attacks)  
✅ **Use VPN** for administrative access (don't expose admin endpoints publicly)  
✅ **Rate limiting** on auth endpoints (prevent brute force attacks)  

### 5. **Permission Management**

✅ **Principle of least privilege** (grant minimum permissions needed)  
✅ **Use wildcards carefully** (prefer specific resources over `*`)  
✅ **Review permissions regularly** (audit who has access to what)  
✅ **Use roles** for common permission sets (don't duplicate permissions)  
✅ **Test permissions** before deploying (verify access works as expected)  
✅ **Document permission rationale** (why each permission was granted)  

### 6. **Monitoring & Auditing**

✅ **Log all authentication events** (successful and failed attempts)  
✅ **Monitor API key usage** (detect unusual patterns)  
✅ **Set up alerts** for multiple failed auth attempts  
✅ **Review logs regularly** (weekly/monthly security audits)  
✅ **Track permission changes** (who granted/revoked what permissions)  
✅ **Monitor for privilege escalation** attempts  

### 7. **Development vs Production**

**Development**:
- `auth.enabled: false` or `require_auth: false` (for local testing)
- Use simple passwords (but still change defaults)
- Allow anonymous access for testing

**Production**:
- `auth.enabled: true` and `require_auth: true` (mandatory)
- Strong passwords (12+ characters, complex)
- No anonymous access
- IP filtering enabled
- API key expiration set
- Monitoring enabled

### 8. **Incident Response**

✅ **Have a plan** for compromised credentials  
✅ **Revoke compromised keys** immediately  
✅ **Disable compromised users** immediately  
✅ **Rotate all credentials** after security incident  
✅ **Review access logs** to identify scope of compromise  
✅ **Document incidents** for post-mortem analysis  

---

## Permission Model

### Resource Pattern Syntax

| Pattern | Matches | Example |
|---------|---------|---------|
| `*` | Everything | All resources |
| `queue:*` | All queues | `queue:orders`, `queue:payments` |
| `queue:orders` | Specific queue | Only `queue:orders` |
| `kv:users:*` | Prefix match | `kv:users:123`, `kv:users:456` |

### Actions

| Action | Description | Use Cases |
|--------|-------------|-----------|
| `Read` | GET, CONSUME operations | Read data, consume messages |
| `Write` | SET, PUBLISH operations | Write data, publish messages |
| `Delete` | DELETE, PURGE operations | Remove data, purge queues |
| `Admin` | Administrative operations | Manage users, configure system |
| `All` | All actions (wildcard) | Full access |

---

## Examples

### Example 1: Queue System with Auth

```rust
// Create user manager and API key manager
let user_manager = UserManager::new();
let api_key_manager = ApiKeyManager::new();

// Create admin user
user_manager.create_user("admin", "admin123", true)?;

// Create service user for queue processing
user_manager.create_user("queue_worker", "worker123", false)?;
user_manager.add_user_role("queue_worker", "queue_user")?;

// Generate API key for service
let api_key = api_key_manager.create(
    "worker-service-key",
    Some("queue_worker".to_string()),
    vec![
        Permission::new("queue:orders", Action::Read),
        Permission::new("queue:orders", Action::Write),
    ],
    vec![],  // No IP restrictions
    Some(365),  // 1 year
)?;

tracing::info!("Service API Key: {}", api_key.key);
```

### Example 2: Multi-Tenant Setup

```rust
// Tenant 1 - can only access their queues
let tenant1_key = api_key_manager.create(
    "tenant1-key",
    None,
    vec![Permission::new("queue:tenant1:*", Action::All)],
    vec![],
    Some(90),
)?;

// Tenant 2 - can only access their queues
let tenant2_key = api_key_manager.create(
    "tenant2-key",
    None,
    vec![Permission::new("queue:tenant2:*", Action::All)],
    vec![],
    Some(90),
)?;

// Admin - can access everything
let admin_key = api_key_manager.create(
    "admin-key",
    Some("admin".to_string()),
    vec![Permission::new("*", Action::All)],
    vec![IpAddr::from([192, 168, 1, 10])],  // Admin IP only
    None,  // Never expires
)?;
```

---

## Replication Support (Future)

The authentication system is designed with master/slave replication in mind:

- **User database** can be replicated to slaves
- **API keys** are synced across cluster
- **ACL rules** are consistent across nodes
- **Read replicas** can authenticate but not modify users

---

## Testing

```rust
#[test]
fn test_authentication_flow() {
    let manager = UserManager::new();
    
    // Create user
    manager.create_user("test", "password", false).unwrap();
    
    // Valid auth
    assert!(manager.authenticate("test", "password").is_ok());
    
    // Invalid auth
    assert!(manager.authenticate("test", "wrong").is_err());
}
```

---

## HiveHub.Cloud Authentication (SaaS Mode)

Synap can integrate with **HiveHub.Cloud** for multi-tenant SaaS authentication with Plan-based quotas and restrictions.

### Operating Modes

#### 1. **Standalone Mode** (Default)
- Local authentication (users, API keys, ACLs)
- No external dependencies
- Full control over security
- Suitable for self-hosted deployments

#### 2. **Hub Mode** (SaaS)
- HiveHub.Cloud access key authentication
- Multi-tenant resource isolation
- Plan-based quotas and rate limits
- Managed user accounts via Hub

#### 3. **Hybrid Mode**
- Hub authentication primary
- Local authentication fallback
- Best of both worlds for migration

### Hub Access Keys

**Format**: `sk_live_<64_hex_chars>` (71 characters total)

**Authentication Methods**:
```bash
# Option 1: Bearer Token (Recommended)
curl -H "Authorization: Bearer sk_live_..." \
  https://synap.example.com/queue/list

# Option 2: Custom Header
curl -H "X-Hub-Access-Key: sk_live_..." \
  https://synap.example.com/queue/list
```

**Key Features**:
- 256-bit cryptographic security
- 60-second validation cache
- Plan-based permissions (Free/Pro/Enterprise)
- Automatic user context propagation
- Key rotation support

### Configuration

```yaml
# Enable Hub integration
hub:
  enabled: true
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  base_url: "https://api.hivehub.cloud"

  # Access key validation
  access_key:
    cache_ttl_seconds: 60
    cache_max_entries: 10000

  # Authentication
  auth:
    require_hub_auth: true
    allow_local_auth_fallback: false  # true for hybrid mode
```

### Plan-Based Restrictions

| Feature              | Free        | Pro          | Enterprise   |
|----------------------|-------------|--------------|--------------|
| Rate Limit           | 10 req/s    | 100 req/s    | 1,000 req/s  |
| Storage              | 100 MB      | 10 GB        | 1 TB         |
| Operations/Month     | 100,000     | 10,000,000   | Unlimited    |
| Max TTL              | 24 hours    | 30 days      | 365 days     |
| Max Payload          | 1 MB        | 10 MB        | 100 MB       |
| Connections          | 10          | 100          | 1,000        |

### Multi-Tenant Isolation

In Hub mode, resources are automatically scoped by user:

**Resource Naming**: `user_{user_id}:{resource_name}`

**Example**:
```
User Input:    "my-queue"
User ID:       550e8400-e29b-41d4-a716-446655440000
Scoped Name:   user_550e8400e29b41d4a716446655440000:my-queue
```

**Automatic Scoping**:
- ✅ Queues: `user_{user_id}:{queue_name}`
- ✅ Streams: `user_{user_id}:{stream_name}`
- ✅ KV Keys: `user_{user_id}:{key}`
- ✅ Pub/Sub Topics: `user_{user_id}:{topic}`
- ✅ All data structures (Hash, List, Set, etc.)

### Authentication Flow (Hub Mode)

```
1. Client sends request with access key
   └─> Header: Authorization: Bearer sk_live_...

2. Hub Auth Middleware
   ├─> Extract access key from headers
   ├─> Check validation cache (60s TTL)
   │   ├─> CACHE HIT: Use cached user context
   │   └─> CACHE MISS: Validate via Hub API
   ├─> Create HubUserContext (user_id, plan)
   └─> Insert into request extensions

3. Rate Limit Middleware
   └─> Apply Plan-based rate limits

4. Quota Middleware
   └─> Check storage/operation quotas

5. Handler Execution
   └─> Apply multi-tenant scoping
```

### Security Differences

**Standalone Mode**:
- Local user database
- API keys stored locally
- No quotas or rate limits by default
- Full access to dangerous commands (FLUSHALL, etc.)

**Hub Mode**:
- HiveHub.Cloud manages users
- Access keys validated via Hub API
- Automatic Plan-based quotas and rate limits
- Dangerous commands blocked (FLUSHALL, SCRIPT FLUSH, etc.)
- Multi-tenant isolation enforced

### Error Responses

**401 Unauthorized**: Invalid or missing access key
```json
{
  "error": "Unauthorized",
  "message": "Invalid or missing access key"
}
```

**403 Forbidden**: Valid key but operation not allowed for plan
```json
{
  "error": "Forbidden",
  "message": "TTL of 86400s exceeds Free plan limit of 3600s",
  "plan": "Free"
}
```

**429 Too Many Requests**: Quota or rate limit exceeded
```json
{
  "error": "QuotaExceeded",
  "message": "Monthly operation quota exceeded",
  "limit": 100000,
  "current_usage": 100000
}
```

### Migration: Standalone → Hub Mode

**Step 1**: Configure Hub integration
```yaml
hub:
  enabled: true
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  auth:
    allow_local_auth_fallback: true  # Enable hybrid mode
```

**Step 2**: Gradually migrate clients to access keys
- Existing clients: Continue using local API keys
- New clients: Use Hub access keys

**Step 3**: Disable local auth fallback when ready
```yaml
hub:
  auth:
    allow_local_auth_fallback: false  # Hub-only mode
```

### Best Practices (Hub Mode)

✅ **Store access keys securely** (environment variables, secrets managers)
✅ **Use HTTPS** for all API communication
✅ **Rotate keys every 90 days** via Hub dashboard
✅ **Monitor quota usage** via `/hub/quota` endpoint
✅ **One key per application** for isolation
✅ **Separate keys per environment** (dev/test/prod)
✅ **Never log access keys** in application logs
✅ **Revoke compromised keys immediately** via Hub API

### Additional Documentation

For detailed Hub integration documentation, see:

- **[HUB_INTEGRATION.md](./specs/HUB_INTEGRATION.md)** - Complete integration specification
- **[QUOTA_MANAGEMENT.md](./specs/QUOTA_MANAGEMENT.md)** - Quota and usage tracking details
- **[ACCESS_KEYS.md](./specs/ACCESS_KEYS.md)** - Access key authentication specification

---

## Conclusion

Synap's authentication system provides **enterprise-grade security** with:

✅ Flexible permission model
✅ Multiple authentication methods
✅ IP-based access control
✅ API key expiration
✅ Role-based access
✅ Production-ready security
✅ **HiveHub.Cloud SaaS integration** (multi-tenant, quotas, rate limits)

**Status**: 🟢 Core implementation complete
**Hub Integration**: 🟢 Phases 1-6, 9-10 complete (Testing 6/9, Documentation in progress)
**Next Steps**: Phase 11 - Documentation, Phase 7 - Cluster support, Phase 8 - Migration tool

