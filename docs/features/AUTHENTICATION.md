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

#### Issuing Keys over REST (`POST /auth/keys`)

A key can never carry more authority than the identity that creates it:

- **Authenticated issuer.** With authentication enabled, an anonymous request
  is rejected with `401`, even when `require_auth` is `false`.
- **Bounded delegation.** Every requested grant must be covered by one
  permission the issuer holds: the held action must include the requested one
  (`all` ⊇ everything; `configure` ⊇ `configure`, `read`, `write`), and every
  resource the requested pattern can match must be matched by the held
  pattern. `kv:orders:*` covers `kv:orders:eu:*` and `kv:orders:1`, but not
  `kv:*`. Admin users may grant anything. A grant that is only covered by the
  union of several held permissions is refused; ask for one grant per held
  permission instead. Violations answer `403`.
- **Keys issued by keys.** A request authenticated with an API key is bounded
  by that key's permissions, and when that key expires, the new key must set
  `expires_in_seconds` and expire no later than it.
- **Strict validation.** An unknown action (anything other than `read`,
  `write`, `delete`, `configure`, `admin`, `all`), an empty resource, or an
  unparsable `allowed_ips` entry answers `400`. An empty `allowed_ips` list
  allows every address, so a typo is never silently dropped.

A rejected request stores nothing.

```bash
curl -X POST http://localhost:15500/auth/keys \
  -u orders:secret \
  -H 'Content-Type: application/json' \
  -d '{"name":"eu-reader","expires_in_seconds":86400,
       "permissions":[{"resource":"kv:orders:eu:*","action":"read"}]}'
```

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

### One Policy on Every Transport

Since 1.4.0, REST, the `/api/v1/command` envelope, RESP3 and SynapRPC all
authorize a command against the same `(resource, action)` pairs, so a role
grants exactly the same access whichever transport a client uses. A request is
allowed only when the caller holds every pair the command touches.

| Data | Resource | Notes |
|------|----------|-------|
| Strings / KV | `kv:<key>` | `KEYS`, `SCAN`, `DBSIZE` and KV stats need `kv:*` read |
| Hashes | `hash:<key>` | |
| Lists | `list:<key>` | pops are `write`; `RPOPLPUSH` needs write on both lists |
| Sets | `set:<key>` | `SREM` is `delete` |
| Sorted sets | `sortedset:<key>` | geospatial indexes are sorted sets, so `GEO*` use it too |
| HyperLogLog | `hyperloglog:<key>` | `PFMERGE`: write the destination, read the sources |
| Bitmaps | `bitmap:<key>` | |
| Queues | `queue:<name>` | create is `configure`; ACK/NACK are `write`; delete and purge are `delete` |
| Streams | `stream:<room>` | partitioned topics use `stream:<topic>`, consumer groups `stream:group:<id>` |
| Pub/Sub | `pubsub:<topic>` | subscribe is `read`, publish is `write` |
| Scripts | `script:*` | `SCRIPT LOAD` is `write`, `SCRIPT EXISTS` is `read` |
| Server | `admin:*` | `FLUSHALL`/`FLUSHDB`, `SCRIPT FLUSH`/`KILL`, `CONFIG`, `INFO`, slow log, client list: `admin` |

Rules that follow from it:

- **Every key a command touches is checked.** Multi-key commands check each
  key; commands with a destination check it for `write` and the sources for
  `read`.
- **Scripts run as their caller.** `EVAL`/`EVALSHA` need no permission of their
  own, but every `redis.call` inside is authorized against the caller, so a
  script cannot reach a key its caller could not.
- **Transactions are all-or-nothing.** Each queued command is authorized when
  it is queued; if one is refused, the transaction is aborted and `EXEC` fails
  with `EXECABORT` without applying anything. `MULTI`, `EXEC`, `DISCARD` and
  `WATCH` need no permission.
- **Internal channels carry key data.** Subscribing to `__watch@0__:<key>` or
  `__keyspace@0__:<key>` also needs `kv:<key>` read, `__keyevent@0__:*` needs
  `kv:*` read, and a wildcard subscription that could match one of them
  (`*`, `#`, `__*`) needs `kv:*` read. Publishing to them needs the matching
  `write`. `KV.WATCH` and `GET /kv/ws` need read on the watched keys.
- **Unknown commands are denied.** A command without an access policy is
  refused while authentication is enabled; a test enforces that every command
  in every dispatcher has one.
- **Anonymous means no permissions.** With authentication enabled but
  `require_auth: false`, a REST request without credentials and a RESP3 or
  SynapRPC connection that never sent `AUTH` hold no permissions: only
  connection commands (`PING`, `HELLO`, `QUIT`) succeed.
- **Authentication disabled means unrestricted**, decided by configuration.

Denials answer `403` on REST, `{"success": false, "error": "Forbidden: NOPERM ..."}`
on the envelope, and a `-NOPERM ...` error on RESP3 and SynapRPC, naming the
missing grant (e.g. `NOPERM no 'write' permission on 'kv:a'`).

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

#### 2. **Hub Mode** (SaaS, `hub.enabled: true`)
- Every data request is authenticated with a HiveHub.Cloud access key
- Each tenant is confined to its own namespace on every transport
- Local users and local API keys are not accepted: there is no hybrid mode
- Plan-based quotas and restrictions

### Current Status: Access-Key Validation Is Unavailable

hivehub-internal-sdk 1.0.0 has no endpoint that validates an access-key secret
(it can generate, list, fetch by id and revoke keys only), so Synap cannot tell
which tenant a key belongs to. Hub mode therefore **fails closed**: every request
that needs a key answers **503** with an error naming
[hivellm/synap#230](https://github.com/hivellm/synap/issues/230), and the server
logs a warning at startup. `/health` and `/metrics` keep answering. Accepting a
key it cannot check would let any caller pick a tenant, so Synap refuses
instead. Validation sits behind the `AccessKeyValidator` trait
(`hub/client.rs`); when the SDK gains the endpoint, only the production
implementation changes, and the policy below applies unchanged.

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

`X-Hub-Access-Key` wins when both are sent. A validated key is cached for
`hub.cache_ttl` seconds (default 60); a rejected key is never cached, so a
revoked key stops working within one cache period.

### Configuration

```yaml
hub:
  enabled: true
  api_url: "https://api.hivehub.cloud"
  service_api_key: ""            # set SYNAP_HUB_SERVICE_API_KEY instead
  cache_ttl: 60                  # seconds a validated key is cached (10-300)
  usage_report_interval: 300     # seconds between usage reports (>= 60)
  timeout: 30                    # Hub API timeout in seconds (5-120)

# Required in Hub mode: these listeners have no tenant authentication.
resp3:
  enabled: false
synap_rpc:
  enabled: false
```

Environment overrides: `SYNAP_HUB_ENABLED`, `SYNAP_HUB_API_URL`,
`SYNAP_HUB_SERVICE_API_KEY`, `SYNAP_HUB_CACHE_TTL`,
`SYNAP_HUB_USAGE_REPORT_INTERVAL`, `SYNAP_HUB_TIMEOUT`.

### Hub Mode Policy

**Credentials.** In Hub mode the router installs Hub authentication *instead of*
local authentication. Every route except `/health` and `/metrics` needs a Hub
access key:

| Request | Answer |
|---------|--------|
| No `X-Hub-Access-Key` / `Authorization: Bearer` header | 401 |
| Key the Hub rejects (unknown, revoked, expired) | 401 |
| Local credentials only (Basic auth, a local API key) | 401 |
| Key that could not be checked (see the status above) | 503 |
| Valid key, route not in the tenant allow-list | 403 |

**Tenant identity.** A request with a valid key runs as `hub:<user id>`: never an
admin, with full access to `kv`, `hash`, `list`, `set`, `sortedset`,
`hyperloglog`, `bitmap`, `queue`, `stream` and `pubsub` resources and no
`admin`, `script` or cluster grants. Every existing authorization check applies
to it; the tenant's namespace is enforced by scoping, below.

**Tenant routes (allow-list).** Only routes whose handlers confine every
resource to the caller's tenant answer in Hub mode; a route added later is
refused until it is reviewed for scoping.

| Allowed | Refused with 403 ("`<path>` is not available in Hub mode") |
|---------|----------------------------------------------------------|
| `/api/v1/command`, `/mcp`, `/umicp`, `/umicp/discover`, `/hub/quota` | `/kv/stats` and every other `/<type>/stats` |
| `/kv/set`, `/kv/get/{key}`, `/kv/del/{key}`, `/kv/{key}/...`, `/kv/msetnx`, `/kv/ws` | `/info`, `/slowlog`, `/clients`, `/memory/{key}/usage` |
| `/key/{key}/...`, `/key/randomkey` (picks among the tenant's keys) | `/snapshot`, `/script/*` |
| `/hash/*`, `/set/*`, `/sortedset/*`, `/list/*`, `/hyperloglog/*`, `/bitmap/*`, `/geospatial/*` | `/transaction/*` (one transaction id shared by every REST caller; use the envelope's `transaction.*` with a `client_id`) |
| `/stream/*`, `/queue/*` (`/stream/list`, `/queue/list` list the tenant's own) | `/topics/*`, `/consumer-groups/*` (partitioned topics have no tenant scoping) |
| `/pubsub/*` except `/pubsub/stats` (`/pubsub/topics` lists the tenant's own) | `/cluster/*`, `/auth/*` (local users and keys) |

**Envelope, MCP and UMICP.** After authorization and before the command runs,
the resource fields of the `/api/v1/command` payload (and of MCP and UMICP tool
arguments, which use the same names) are rewritten into the tenant's namespace:
`key`, `keys`, `source`, `destination`, `sources`, `source_keys`,
`pairs[].key`, `queue`, `name`, `room`, `topic`, `topics`, and the transaction
`client_id`, so two tenants' transactions never meet. A command is refused with
`NOPERM '<command>' is not available in Hub mode` when it cannot be confined:

- keyspace-wide or server-wide commands (`kv.keys`, `kv.scan`, `kv.dbsize`,
  `key.randomkey`, every `*.stats`, `queue.list`, `stream.list`,
  `pubsub.topics`); admin commands (`info`, `kv.flushall`, `script.flush`, ...)
  are already refused because a tenant holds no admin grant;
- `script.eval` and `script.evalsha`, which run commands their payload does not
  name;
- transaction commands without a string `client_id` (the envelope uses the
  request's `request_id`, as it does outside Hub mode; MCP transaction tools
  must pass one);
- a resource name given as anything but a string.

**WebSockets.** `/kv/ws` watches `__watch@0__:user_<id>:<key>`, `/pubsub/ws`
subscribes to the tenant's topics (patterns too, so `*` and `#` match only the
tenant's own topics), and `/queue/{name}/ws/...` and `/stream/{room}/ws/...` use
the tenant's queue and room. Frames carry the scoped names.

**Fail closed.** Scoping never falls back to a raw name: a Hub-mode request that
reaches a scoped surface without a tenant is refused.

**Native listeners.** RESP3 and SynapRPC authenticate local users only, so the
server refuses to start in Hub mode while either is enabled, and the error names
the listener (`resp3.enabled`, `synap_rpc.enabled`). Both default to `true`, so
a Hub deployment must disable them explicitly.

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

**Automatic Scoping** (REST, envelope, MCP, UMICP and WebSockets):
- ✅ KV keys, including key management (rename, copy, type)
- ✅ Hashes, lists, sets, sorted sets, geospatial indexes, HyperLogLogs, bitmaps
  (source and destination keys of the multi-key commands too)
- ✅ Queues and streams
- ✅ Pub/Sub topics and KV watch channels
- ✅ Transaction ids (`client_id`)

### Authentication Flow (Hub Mode)

```
1. Client sends request with access key
   └─> Header: X-Hub-Access-Key or Authorization: Bearer sk_live_...

2. Hub authentication layer (replaces local authentication)
   ├─> /health, /metrics: pass through
   ├─> Extract access key (missing -> 401)
   ├─> Check validation cache (hub.cache_ttl)
   │   ├─> CACHE HIT: use the cached tenant
   │   └─> CACHE MISS: validate (rejected -> 401, unavailable -> 503)
   ├─> Route allow-list (not a tenant route -> 403)
   └─> Insert HubUserContext and the tenant's AuthContext

3. Handler execution
   ├─> Authorize against the tenant's AuthContext
   └─> Scope every resource name by the tenant
```

### Security Differences

**Standalone Mode**:
- Local user database
- API keys stored locally
- No quotas or rate limits by default
- Full access to dangerous commands (FLUSHALL, etc.)

**Hub Mode**:
- HiveHub.Cloud manages users; local credentials are not accepted
- Access keys validated through the Hub (currently unavailable: every key is
  refused, see above)
- Admin, scripting, cluster, snapshot and server-introspection routes refused
- RESP3 and SynapRPC listeners must be disabled
- Multi-tenant isolation enforced on every HTTP transport

### Error Responses

**401 Unauthorized**: missing or rejected access key
```json
{
  "error": "Unauthorized: Hub mode: a Hub access key is required",
  "code": 401
}
```

**403 Forbidden**: valid key, route not available to tenants
```json
{
  "error": "Forbidden: /info is not available in Hub mode",
  "code": 403
}
```

**503 Service Unavailable**: the key could not be checked
```json
{
  "error": "Internal server error: Hub access-key validation is unavailable: the HiveHub SDK has no endpoint that validates an access-key secret (hivellm/synap#230)",
  "code": 503
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

There is no hybrid mode: once `hub.enabled` is true, local credentials stop
working on every route.

1. Disable the native listeners (`resp3.enabled: false`,
   `synap_rpc.enabled: false`); clients move to the HTTP transports.
2. Enable Hub integration and issue a Hub access key to every client.
3. Data written in standalone mode keeps its raw names, which no tenant can
   reach; copy what a tenant needs under its own key.

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

- **[HUB_INTEGRATION.md](../specs/HUB_INTEGRATION.md)** - Complete integration specification
- **[QUOTA_MANAGEMENT.md](../specs/QUOTA_MANAGEMENT.md)** - Quota and usage tracking details
- **[ACCESS_KEYS.md](../specs/ACCESS_KEYS.md)** - Access key authentication specification

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

