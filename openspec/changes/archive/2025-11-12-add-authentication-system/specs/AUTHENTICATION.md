# Authentication & Authorization System Specification

## Overview

This document specifies the authentication and authorization system for Synap, providing secure access control for all resources and operations.

## Architecture

### Components

1. **UserManager**: Manages users, roles, and permissions
2. **ApiKeyManager**: Manages API keys with expiration and revocation
3. **AuthMiddleware**: HTTP middleware for authentication
4. **PermissionChecker**: Validates permissions for operations
5. **RootUserManager**: Special handling for root user

## Root User

### Configuration

Root user is configured at startup via:

1. **Environment Variables** (Docker):
   ```bash
   SYNAP_AUTH_ROOT_USERNAME=root
   SYNAP_AUTH_ROOT_PASSWORD=root
   SYNAP_AUTH_ROOT_ENABLED=true
   ```

2. **Config File**:
   ```yaml
   auth:
     root:
       username: "root"
       password: "root"
       enabled: true
   ```

### Properties

- **Full Permissions**: Root user has all permissions (`*:*`)
- **Protected**: Cannot be deleted
- **Disableable**: Can be disabled after initial setup via config
- **Password Change**: Can change password via API

### Initialization

1. On first startup, if no users exist, create root user
2. If root user exists, validate credentials from config/env
3. If `root.enabled: false`, root user cannot authenticate

## User Management

### User Model

```rust
pub struct User {
    pub username: String,
    pub password_hash: String,  // bcrypt hash
    pub roles: Vec<String>,
    pub enabled: bool,
    pub created_at: u64,
    pub last_login: Option<u64>,
}
```

### Operations

- **Create User**: `POST /auth/users`
  - Requires: Admin permission
  - Body: `{ username, password, roles: [] }`
  
- **Delete User**: `DELETE /auth/users/:username`
  - Requires: Admin permission
  - Cannot delete root user
  
- **Update User**: `PUT /auth/users/:username`
  - Requires: Admin permission or self
  - Body: `{ password?, enabled?, roles? }`
  
- **List Users**: `GET /auth/users`
  - Requires: Admin permission
  - Returns: Array of user objects (without password hashes)
  
- **Get User**: `GET /auth/users/:username`
  - Requires: Admin permission or self
  - Returns: User object (without password hash)

### Role Assignment

- **Grant Role**: `POST /auth/users/:username/roles`
  - Requires: Admin permission
  - Body: `{ role: "role_name" }`
  
- **Revoke Role**: `DELETE /auth/users/:username/roles/:role`
  - Requires: Admin permission

## API Key Management

### API Key Model

```rust
pub struct ApiKey {
    pub id: String,              // Unique identifier
    pub name: String,            // Human-readable name
    pub key_hash: String,        // Hashed key (never store plaintext)
    pub user_id: String,         // Owner user
    pub permissions: Vec<Permission>,
    pub created_at: u64,
    pub expires_at: Option<u64>, // None = no expiration
    pub last_used: Option<u64>,
    pub revoked: bool,
    pub allowed_ips: Vec<IpAddr>, // Empty = all IPs
}
```

### Operations

- **Create API Key**: `POST /auth/keys`
  - Requires: Authentication
  - Body: `{ name, expires_in?: number, permissions?: [], allowed_ips?: [] }`
  - Returns: `{ id, key, name, expires_at, created_at }` (key shown only once)
  
- **List API Keys**: `GET /auth/keys`
  - Requires: Authentication
  - Returns: Array of key objects (without key value)
  
- **Revoke API Key**: `DELETE /auth/keys/:id`
  - Requires: Authentication (owner or admin)
  - Sets `revoked: true`, invalidates immediately
  
- **Get API Key**: `GET /auth/keys/:id`
  - Requires: Authentication (owner or admin)
  - Returns: Key object (without key value)

### Key Generation

- Generate cryptographically secure random key (32+ bytes)
- Hash with bcrypt or Argon2
- Return plaintext key only once (in create response)
- Store only hash in database

### Expiration

- `expires_at`: Unix timestamp (seconds)
- Keys with `expires_at < now()` are invalid
- `expires_in`: Seconds from now (converted to `expires_at`)
- Temporary keys: `expires_in: 3600` (1 hour)

## Permission System

### Resource Types

```
kv:*              # Key-Value store
hash:*            # Hash operations
list:*            # List operations
set:*             # Set operations
sortedset:*       # Sorted Set operations
queue:*           # Queue operations
queue:jobs        # Specific queue
stream:*          # Stream/chatroom operations
stream:chat-*     # Pattern matching
pubsub:*          # Pub/Sub operations
transaction:*     # Transaction operations
script:*          # Lua scripting
admin:*           # Administrative operations
```

### Actions

- `read`: Read operations (GET, CONSUME, SUBSCRIBE, MEMBERS, etc.)
- `write`: Write operations (SET, PUBLISH, ADD, PUSH, etc.)
- `delete`: Delete operations (DEL, REMOVE, POP, etc.)
- `configure`: Configuration (CREATE, UPDATE, CONFIG)
- `admin`: Administrative (USER_MANAGE, SYSTEM_CONFIG)

### Permission Model

```rust
pub struct Permission {
    pub resource: String,  // e.g., "queue:jobs", "stream:*"
    pub actions: Vec<Action>,  // ["read", "write"]
}
```

### Permission Checking

1. **Admin Check**: If user is admin, allow all
2. **Exact Match**: Check if permission matches resource exactly
3. **Wildcard Match**: Check if permission matches resource pattern
4. **Action Check**: Verify requested action is in allowed actions

### Examples

```yaml
permissions:
  # Read-only access to all queues
  - resource: "queue:*"
    actions: ["read"]
  
  # Full access to specific queue
  - resource: "queue:jobs"
    actions: ["read", "write", "delete", "configure"]
  
  # Write access to all streams matching pattern
  - resource: "stream:chat-*"
    actions: ["write"]
  
  # Admin access
  - resource: "*"
    actions: ["admin"]
```

## Authentication Methods

### 1. Basic Auth

```
Authorization: Basic base64(username:password)
```

- Username/password authentication
- Validates against UserManager
- Returns AuthContext with user permissions

### 2. Bearer Token (API Key)

```
Authorization: Bearer <api_key>
```

- API key authentication
- Validates against ApiKeyManager
- Returns AuthContext with key permissions

### 3. Query Parameter

```
GET /api/v1/command?api_key=<api_key>
```

- For GET requests only
- Less secure (keys in URLs/logs)
- Same validation as Bearer token

## REST API Protection

### Middleware Flow

```
Request → AuthMiddleware → PermissionMiddleware → Handler
```

### AuthMiddleware

1. Extract credentials (Basic Auth, Bearer Token, Query Param)
2. Validate credentials
3. Create AuthContext
4. Attach to request extensions
5. Return 401 if required and not authenticated

### PermissionMiddleware

1. Extract AuthContext from request
2. Determine resource and action from route/command
3. Check permissions
4. Return 403 if insufficient permissions
5. Continue to handler if authorized

### Error Responses

- **401 Unauthorized**: Missing or invalid credentials
  ```json
  {
    "error": "Unauthorized",
    "message": "Invalid credentials"
  }
  ```

- **403 Forbidden**: Insufficient permissions
  ```json
  {
    "error": "Forbidden",
    "message": "Insufficient permissions for resource: queue:jobs, action: write"
  }
  ```

## SDK Authentication

### TypeScript SDK

```typescript
const synap = new Synap({
  url: 'http://localhost:15500',
  apiKey: 'your-api-key',
  // OR
  auth: {
    username: 'user',
    password: 'pass'
  }
});
```

### Python SDK

```python
client = SynapClient(
    url='http://localhost:15500',
    api_key='your-api-key'
    # OR
    # username='user',
    # password='pass'
)
```

### Rust SDK

```rust
let client = SynapClient::new("http://localhost:15500")
    .with_api_key("your-api-key")
    // OR
    // .with_basic_auth("user", "pass")
    .build()?;
```

## MCP Authentication

### MCP Server Auth

1. Extract API key from MCP request metadata
2. Validate key via ApiKeyManager
3. Attach AuthContext to request
4. Check permissions for MCP tool operations
5. Return error if unauthorized

### MCP Request Format

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "synap_kv_get",
    "arguments": {
      "key": "test"
    }
  },
  "metadata": {
    "api_key": "your-api-key"
  }
}
```

## Configuration

### Config File

```yaml
auth:
  enabled: true
  require_auth: true  # Mandatory authentication
  root:
    username: "root"
    password: "root"  # Must be changed
    enabled: true     # Can disable after setup
  default_key_ttl: 3600  # 1 hour
  password_min_length: 8
  password_require_complexity: true
  rate_limit_auth_endpoints: true
```

### Environment Variables

```bash
SYNAP_AUTH_ENABLED=true
SYNAP_AUTH_REQUIRE_AUTH=true
SYNAP_AUTH_ROOT_USERNAME=root
SYNAP_AUTH_ROOT_PASSWORD=root
SYNAP_AUTH_ROOT_ENABLED=true
SYNAP_AUTH_DEFAULT_KEY_TTL=3600
```

## Security Considerations

### Password Security

- **Hashing**: bcrypt with cost 12+
- **Requirements**: Min length 8, complexity (optional)
- **Storage**: Never store plaintext passwords
- **Change**: Require old password for change

### API Key Security

- **Generation**: Cryptographically secure random (32+ bytes)
- **Storage**: Hash with bcrypt or Argon2
- **Transmission**: HTTPS only in production
- **Rotation**: Support key rotation
- **Revocation**: Immediate invalidation

### Rate Limiting

- **Auth Endpoints**: Rate limit login/key creation
- **Failed Attempts**: Lock account after N failures
- **IP Filtering**: Optional IP whitelist for API keys

### Audit Logging

- Log all authentication events
- Log permission denials
- Log user management operations
- Log API key creation/revocation

## Migration Path

### Phase 1: Optional Auth (Default)

- Authentication disabled by default
- Existing deployments continue to work
- New deployments can enable auth

### Phase 2: Recommended Auth

- Documentation recommends enabling auth
- Examples show auth usage
- Migration guide provided

### Phase 3: Mandatory Auth (Future)

- Option to require auth in production
- Warning if auth disabled in production
- Best practices enforced

## Testing

### Unit Tests

- UserManager: 20+ tests
- ApiKeyManager: 15+ tests
- PermissionChecker: 25+ tests
- AuthMiddleware: 10+ tests

### Integration Tests

- REST endpoints: 30+ tests
- SDK authentication: 20+ tests
- MCP authentication: 10+ tests

### Security Tests

- Unauthorized access attempts
- Permission boundary tests
- Key expiration/revocation
- Password security tests

## Performance Targets

- Authentication overhead: <1ms per request
- Permission check: <100µs
- Key validation: <500µs
- User lookup: <200µs

## Future Enhancements

- OAuth2/OIDC integration
- JWT tokens
- Session management
- Two-factor authentication (2FA)
- Certificate-based authentication
- LDAP/Active Directory integration

