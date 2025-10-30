# Synap Authentication & Authorization System

## Overview

Synap implements a comprehensive **rule-based authentication and authorization system** with support for:

✅ **User Management** with bcrypt password hashing  
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

- **bcrypt** hashing (DEFAULT_COST = 12)
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
println!("API Key: {}", api_key.key);
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

println!("Key Name: {}", verified_key.name);
println!("Usage Count: {}", verified_key.usage_count);
```

#### Cleanup Expired Keys

```rust
// Remove all expired keys
let removed_count = api_key_manager.cleanup_expired();
println!("Removed {} expired keys", removed_count);
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
    println!("Access granted!");
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
✅ **Change default admin password** immediately  
✅ **Use strong passwords** (min 12 characters)  
✅ **Enable ACLs** for fine-grained control  

### 2. **API Key Management**

✅ **Set expiration** (recommended: 90 days)  
✅ **Use IP filtering** when possible  
✅ **Rotate keys** regularly  
✅ **Revoke unused keys** immediately  
✅ **Monitor usage** via `usage_count` and `last_used_at`  

### 3. **Password Security**

✅ **bcrypt hashing** with DEFAULT_COST (12)  
✅ **Never store plain-text** passwords  
✅ **Implement password rotation** policies  
✅ **Disable compromised accounts** immediately  

### 4. **Network Security**

✅ **Use TLS/SSL** for production (external proxy)  
✅ **Firewall rules** to limit access  
✅ **IP whitelisting** for sensitive keys  
✅ **Monitor failed auth** attempts  

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

println!("Service API Key: {}", api_key.key);
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

## Conclusion

Synap's authentication system provides **enterprise-grade security** with:

✅ Flexible permission model  
✅ Multiple authentication methods  
✅ IP-based access control  
✅ API key expiration  
✅ Role-based access  
✅ Production-ready security  

**Status**: 🟢 Core implementation complete  
**Next Steps**: CLI commands for user management (planned)

