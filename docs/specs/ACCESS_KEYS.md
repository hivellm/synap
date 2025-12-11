# Access Keys Specification

## Overview

Access Keys are the primary authentication mechanism for HiveHub.Cloud SaaS mode. They enable secure, user-scoped access to Synap resources with Plan-based permissions and quota enforcement.

**Key Features**:
- Cryptographically secure keys (256-bit random)
- Bearer token or custom header authentication
- 60-second validation cache for performance
- Plan-based permission enforcement
- Automatic user context propagation
- Support for key rotation without downtime

## Access Key Format

### Key Structure

```
synap_live_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
│     │    │
│     │    └─ 64 hex characters (256 bits of entropy)
│     └────── Environment prefix (live, test, dev)
└────────────── Service prefix (synap = Synap Key)
```

**Components**:
1. **Service Prefix** (`synap`): Identifies the service (Synap)
2. **Environment** (`live`, `test`, `dev`): Deployment environment
3. **Key Material**: 64 hex characters = 256 bits of cryptographic randomness

**Example Keys** (FAKE - do not use in production):
```
synap_live_EXAMPLE1234567890abcdef1234567890abcdef1234567890abcdef123456
synap_test_EXAMPLE9876543210fedcba9876543210fedcba9876543210fedcba987654
synap_dev_EXAMPLE1111222233334444555566667777888899990000aaaabbbbccccdd
```

### Key Properties

| Property          | Value                     | Description                           |
|-------------------|---------------------------|---------------------------------------|
| Length            | 71 characters             | `sk_` (3) + env (4-5) + `_` (1) + key (64) |
| Entropy           | 256 bits                  | Cryptographically secure random       |
| Character Set     | Hex (0-9, a-f)            | Key material only                     |
| Prefix            | `synap_`                  | Identifies Synap keys                 |
| Environment       | `live`, `test`, `dev`     | Deployment target                     |
| Revocable         | Yes                       | Can be revoked via Hub API            |
| Rotatable         | Yes                       | New keys can be issued                |

## Authentication Flow

### Request Authentication

```
┌──────────────────────────────────────────────────────────────┐
│                  Access Key Authentication Flow              │
└──────────────────────────────────────────────────────────────┘

Client Request
    │
    ├─> Include Access Key in ONE of:
    │       1. Authorization: Bearer sk_live_...
    │       2. X-Hub-Access-Key: sk_live_...
    │
    ├─> [Hub Auth Middleware]
    │       │
    │       ├─> Extract Access Key from headers
    │       │       └─> Check Authorization header first
    │       │       └─> Fallback to X-Hub-Access-Key
    │       │
    │       ├─> Check Validation Cache (60s TTL)
    │       │       ├─> CACHE HIT: Use cached user context
    │       │       └─> CACHE MISS: Validate via Hub API
    │       │
    │       ├─> Validate Access Key (if not cached)
    │       │       └─> SDK: hub_client.access_keys()
    │       │                   .validate_access_key(key)
    │       │       └─> Returns: UserInfo {
    │       │               user_id: Uuid,
    │       │               plan: Plan,
    │       │               permissions: Vec<String>,
    │       │           }
    │       │
    │       ├─> Create HubUserContext
    │       │       └─> HubUserContext {
    │       │               user_id: Uuid,
    │       │               plan: Plan,
    │       │               access_key: String,
    │       │           }
    │       │
    │       ├─> Cache Validation Result (60s TTL)
    │       │
    │       └─> Insert Context into Request Extensions
    │               └─> request.extensions_mut()
    │                       .insert(HubUserContext)
    │
    ├─> [Rate Limit Middleware]
    │       └─> Uses HubUserContext.plan for rate limit
    │
    ├─> [Quota Middleware]
    │       └─> Uses HubUserContext.user_id for quota check
    │
    ├─> [Handler Execution]
    │       └─> Uses HubUserContext for multi-tenant scoping
    │
    └─> Response
```

### Header Formats

#### Option 1: Authorization Bearer Token (Recommended)

```http
GET /queue/publish HTTP/1.1
Host: synap.example.com
Authorization: Bearer synap_live_EXAMPLE1234567890abcdef1234567890abcdef1234567890abcdef123456
Content-Type: application/json
```

**Advantages**:
- Standard OAuth 2.0 convention
- Supported by most HTTP clients
- Automatic redaction in many logging systems

#### Option 2: Custom X-Hub-Access-Key Header

```http
GET /queue/publish HTTP/1.1
Host: synap.example.com
X-Hub-Access-Key: synap_live_EXAMPLE1234567890abcdef1234567890abcdef1234567890abcdef123456
Content-Type: application/json
```

**Advantages**:
- Explicit Hub-specific authentication
- Avoids conflicts with other auth mechanisms
- Easier to identify in logs

### Validation Caching

**Cache Key**: SHA-256 hash of access key

**Cache Entry**:
```rust
struct CachedValidation {
    user_id: Uuid,
    plan: Plan,
    cached_at: Instant,
}
```

**Cache TTL**: 60 seconds

**Cache Behavior**:
- **Cache Hit**: Return cached user context immediately (< 1ms)
- **Cache Miss**: Call Hub API to validate (50-200ms)
- **Cache Expiry**: Re-validate after 60 seconds
- **Cache Invalidation**: On key revocation (via Hub API callback)

**Performance Impact**:
- Without cache: Every request = Hub API call (50-200ms overhead)
- With cache (60s): 1 Hub API call per minute per key (< 1ms overhead)
- Typical improvement: **50-200x faster** for authenticated requests

## Plan-Based Permissions

### Permission Hierarchy

```
Enterprise Plan
    │
    ├─ ALL Pro Plan permissions
    ├─ Unlimited operations
    ├─ 1 TB storage
    ├─ 1,000 connections
    ├─ 365-day TTL
    └─ 100 MB payloads

Pro Plan
    │
    ├─ ALL Free Plan permissions
    ├─ 10M operations/month
    ├─ 10 GB storage
    ├─ 100 connections
    ├─ 30-day TTL
    └─ 10 MB payloads

Free Plan
    │
    ├─ Basic operations (queue, stream, KV, pub/sub)
    ├─ 100K operations/month
    ├─ 100 MB storage
    ├─ 10 connections
    ├─ 24-hour TTL
    └─ 1 MB payloads
```

### Permission Enforcement

**Enforced at Multiple Layers**:

1. **Rate Limiting**: Plan-based requests per second
   - Free: 10 req/s
   - Pro: 100 req/s
   - Enterprise: 1,000 req/s

2. **Quota Management**: Plan-based resource limits
   - Storage quotas (100 MB → 10 GB → 1 TB)
   - Operation quotas (100K → 10M → Unlimited)
   - Connection quotas (10 → 100 → 1,000)

3. **Payload Restrictions**: Plan-based size limits
   - Free: 1 MB max payload
   - Pro: 10 MB max payload
   - Enterprise: 100 MB max payload

4. **TTL Restrictions**: Plan-based expiration limits
   - Free: 24-hour max TTL
   - Pro: 30-day max TTL
   - Enterprise: 365-day max TTL

5. **Batch Restrictions**: Plan-based batch sizes
   - Free: 100 items per batch
   - Pro: 1,000 items per batch
   - Enterprise: 10,000 items per batch

6. **Dangerous Commands**: Always blocked in Hub mode
   - `FLUSHALL`, `FLUSHDB` - Data loss risk
   - `SCRIPT FLUSH`, `SCRIPT KILL` - Shared resource risk
   - `CLIENT LIST` - Privacy violation risk

### Permission Checking Example

```rust
// Extract Hub context (contains plan)
let hub_ctx = request.extensions()
    .get::<HubUserContext>()
    .ok_or(SynapError::Unauthorized)?;

// Check TTL limit for user's plan
if ttl_seconds > HubSaaSRestrictions::max_ttl_seconds(hub_ctx.plan) {
    return Err(SynapError::Forbidden(format!(
        "TTL of {}s exceeds {:?} plan limit of {}s",
        ttl_seconds,
        hub_ctx.plan,
        HubSaaSRestrictions::max_ttl_seconds(hub_ctx.plan)
    )));
}

// Check payload size limit
if payload.len() > HubSaaSRestrictions::max_payload_bytes(hub_ctx.plan) {
    return Err(SynapError::PayloadTooLarge(format!(
        "Payload of {} bytes exceeds {:?} plan limit of {} bytes",
        payload.len(),
        hub_ctx.plan,
        HubSaaSRestrictions::max_payload_bytes(hub_ctx.plan)
    )));
}

// Proceed with operation
Ok(())
```

## Access Key Lifecycle

### 1. Key Creation

**Via HiveHub.Cloud Dashboard**:
```
1. Navigate to https://hivehub.cloud/access-keys
2. Click "Create New Access Key"
3. Provide key name/description
4. Select environment (live, test, dev)
5. Copy key immediately (shown only once)
```

**Via Hub API** (Admin):
```bash
curl -X POST https://api.hivehub.cloud/v1/access-keys \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Production API Key",
    "environment": "live"
  }'

# Response:
{
  "access_key": "synap_live_EXAMPLE1234567890abcdef1234567890abcdef1234567890abcdef123456",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "plan": "Pro",
  "created_at": "2025-01-15T10:30:00Z"
}
```

**Important**:
- Access key shown **only once** at creation
- User must save key securely (password manager, secrets vault)
- Lost keys cannot be recovered (must be revoked and recreated)

### 2. Key Rotation

**Rotation Strategy** (Zero Downtime):

```
Step 1: Create new key
    └─> Issue: POST /v1/access-keys
    └─> Result: sk_live_NEW_KEY

Step 2: Update applications (gradual rollout)
    └─> Update 25% of servers with new key
    └─> Monitor for errors
    └─> Update remaining 75% if successful

Step 3: Verify new key usage
    └─> Check Hub dashboard for key activity
    └─> Ensure old key not in use

Step 4: Revoke old key
    └─> Issue: DELETE /v1/access-keys/{old_key_id}
    └─> Cached validations expire within 60s
```

**Rotation Schedule**:
- **Regular Rotation**: Every 90 days (recommended)
- **Compromised Key**: Immediately
- **Employee Offboarding**: Within 24 hours
- **Security Incident**: Immediately

### 3. Key Revocation

**Via Hub Dashboard**:
```
1. Navigate to Access Keys page
2. Locate key to revoke
3. Click "Revoke" button
4. Confirm revocation
5. Key invalidated immediately
```

**Via Hub API**:
```bash
curl -X DELETE https://api.hivehub.cloud/v1/access-keys/{key_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"

# Response:
{
  "status": "revoked",
  "revoked_at": "2025-01-15T14:45:00Z",
  "cache_expires_in_seconds": 60
}
```

**Revocation Effect**:
- **Immediate**: New requests with revoked key rejected
- **Cached**: Existing cached validations valid for up to 60 seconds
- **In-flight**: Active connections not terminated (complete naturally)

## Error Responses

### 401 Unauthorized

**Triggered When**: Access key missing, invalid, or revoked

**Response**:
```json
{
  "error": "Unauthorized",
  "message": "Invalid or missing access key",
  "code": "INVALID_ACCESS_KEY"
}
```

**Common Causes**:
- Access key not provided in request
- Access key format invalid (not `sk_*`)
- Access key revoked
- Access key expired
- Hub API validation failed

### 403 Forbidden

**Triggered When**: Valid key but operation not allowed for plan

**Response**:
```json
{
  "error": "Forbidden",
  "message": "TTL of 86400s exceeds Free plan limit of 3600s",
  "code": "PLAN_RESTRICTION",
  "restriction": "max_ttl_seconds",
  "limit": 3600,
  "requested": 86400,
  "plan": "Free",
  "upgrade_url": "https://hivehub.cloud/upgrade"
}
```

**Common Causes**:
- TTL exceeds plan limit
- Payload size exceeds plan limit
- Batch size exceeds plan limit
- Dangerous command attempted in Hub mode

### 429 Too Many Requests

**Triggered When**: Rate limit or quota exceeded

**Response**:
```json
{
  "error": "QuotaExceeded",
  "message": "Monthly operation quota exceeded (100,000 operations)",
  "code": "OPERATION_QUOTA_EXCEEDED",
  "quota_type": "operations",
  "current_usage": 100000,
  "limit": 100000,
  "resets_at": "2025-02-01T00:00:00Z"
}
```

## Security Considerations

### Key Storage Best Practices

**DO**:
- Store keys in environment variables (`$HIVEHUB_ACCESS_KEY`)
- Use secrets management systems (AWS Secrets Manager, HashiCorp Vault)
- Store keys in password managers (1Password, LastPass)
- Encrypt keys at rest
- Use different keys per environment (dev, test, prod)

**DON'T**:
- Hardcode keys in source code
- Commit keys to version control
- Share keys via email/Slack
- Log access keys in application logs
- Use same key across multiple projects

### Key Security Features

**Transport Security**:
- **HTTPS Required**: All access key transmissions over TLS 1.3
- **No Query Parameters**: Keys never in URLs (header-only)
- **Secure Storage**: Keys hashed (SHA-256) before storage

**Monitoring & Alerts**:
- **Unusual Activity**: Alert on geographic anomalies
- **Rate Limit Violations**: Alert on repeated 429 errors
- **Multiple Failed Auth**: Alert on repeated 401 errors
- **Key Usage Tracking**: Audit log of all key usage

**Automatic Protections**:
- **Rate Limiting**: Prevent brute-force validation attempts
- **Cache Poisoning Prevention**: 60s TTL limits exposure
- **Revocation**: Immediate key invalidation capability

### Threat Model

| Threat                        | Mitigation                                          |
|-------------------------------|-----------------------------------------------------|
| **Key Exposure**              | Immediate revocation, rotation, monitoring          |
| **Key Theft**                 | HTTPS only, no logging, secure storage              |
| **Replay Attacks**            | Time-based cache invalidation, HTTPS               |
| **Brute Force**               | Rate limiting on validation endpoint                |
| **Privilege Escalation**      | Plan-based restrictions enforced server-side        |
| **Cache Poisoning**           | 60s TTL, signed responses (future)                  |

## Configuration

### Synap Server Configuration

```yaml
# synap.yaml
hub:
  enabled: true
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  base_url: "https://api.hivehub.cloud"

  # Access key validation
  access_key:
    cache_ttl_seconds: 60           # Cache validated keys for 60s
    cache_max_entries: 10000        # Max cached keys in memory

  # Authentication
  auth:
    require_hub_auth: true          # Require Hub auth in SaaS mode
    allow_local_auth_fallback: false # Disable local auth (Hub only)
```

### Environment Variables

```bash
# Required - Synap service API key
export HIVEHUB_SERVICE_API_KEY="sk_service_..."

# Optional - Override defaults
export HIVEHUB_BASE_URL="https://api.hivehub.cloud"
export HIVEHUB_ACCESS_KEY_CACHE_TTL="60"
export HIVEHUB_ACCESS_KEY_CACHE_MAX="10000"
```

### Client Configuration

**Rust Client**:
```rust
use synap::client::SynapClient;

let client = SynapClient::builder()
    .url("https://synap.example.com")
    .access_key("synap_live_EXAMPLE123...")
    .build()?;

let result = client.queue_publish("my-queue", b"hello").await?;
```

**HTTP Client** (curl):
```bash
export ACCESS_KEY="synap_live_EXAMPLE123..."

# Using Bearer token
curl -X POST https://synap.example.com/queue/publish \
  -H "Authorization: Bearer $ACCESS_KEY" \
  -H "Content-Type: application/json" \
  -d '{"queue": "my-queue", "message": "hello"}'

# Using custom header
curl -X POST https://synap.example.com/queue/publish \
  -H "X-Hub-Access-Key: $ACCESS_KEY" \
  -H "Content-Type: application/json" \
  -d '{"queue": "my-queue", "message": "hello"}'
```

**Python Client**:
```python
import requests
import os

access_key = os.environ['HIVEHUB_ACCESS_KEY']

headers = {
    'Authorization': f'Bearer {access_key}',
    'Content-Type': 'application/json'
}

response = requests.post(
    'https://synap.example.com/queue/publish',
    headers=headers,
    json={'queue': 'my-queue', 'message': 'hello'}
)
```

## Testing

### Unit Tests

**Coverage**: 10 tests in `hub/hub_auth.rs`

**Test Examples**:
```rust
#[test]
fn test_extract_access_key_from_bearer_token() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        "Bearer sk_live_test123".parse().unwrap()
    );

    let key = extract_access_key(&headers);
    assert_eq!(key, Some("sk_live_test123".to_string()));
}

#[test]
fn test_extract_access_key_from_custom_header() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-hub-access-key",
        "sk_live_test456".parse().unwrap()
    );

    let key = extract_access_key(&headers);
    assert_eq!(key, Some("sk_live_test456".to_string()));
}

#[test]
fn test_bearer_token_priority_over_custom_header() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer sk_live_bearer".parse().unwrap());
    headers.insert("x-hub-access-key", "sk_live_custom".parse().unwrap());

    // Should prefer Bearer token
    let key = extract_access_key(&headers);
    assert_eq!(key, Some("sk_live_bearer".to_string()));
}

#[tokio::test]
async fn test_access_key_validation_caching() {
    let hub_client = create_test_hub_client();
    let access_key = "sk_live_cache_test";

    // First validation - cache miss (calls Hub API)
    let result1 = hub_client.validate_access_key(access_key).await;
    assert!(result1.is_ok());

    // Second validation - cache hit (no Hub API call)
    let start = Instant::now();
    let result2 = hub_client.validate_access_key(access_key).await;
    let elapsed = start.elapsed();

    assert!(result2.is_ok());
    assert!(elapsed < Duration::from_millis(1)); // Cached response is instant
}
```

### Integration Tests

**Status**: DEFERRED pending Hub API implementation

**Required Tests**:
- End-to-end access key validation with real Hub API
- Cache expiration and re-validation behavior
- Key revocation propagation (invalidates cache)
- Multiple concurrent validations (cache contention)
- Plan-based permission enforcement
- Error handling for Hub API failures

## Troubleshooting

### Access Key Not Working

**Symptom**: `401 Unauthorized` on all requests

**Possible Causes**:

1. **Access key not provided**
   ```bash
   # Check request headers
   curl -v https://synap.example.com/queue/list

   # Should include:
   # Authorization: Bearer sk_live_...
   # OR
   # X-Hub-Access-Key: sk_live_...
   ```

2. **Access key format invalid**
   ```bash
   # Valid format: sk_{env}_{64_hex_chars}
   echo "synap_live_EXAMPLE..." | wc -c  # Should be ~71 characters
   ```

3. **Access key revoked**
   ```bash
   # Check Hub dashboard for key status
   # Revoked keys cannot be unrevoked - must create new key
   ```

4. **Hub API unreachable**
   ```bash
   # Test Hub API connectivity
   curl -v https://api.hivehub.cloud/v1/health
   ```

5. **Service API key invalid**
   ```bash
   # Check Synap server logs
   grep "HubClient" synap-server.log | tail -n 20

   # Verify service API key in config
   echo $HIVEHUB_SERVICE_API_KEY
   ```

### Cache Not Invalidating After Revocation

**Symptom**: Revoked key still works for ~60 seconds

**Explanation**: This is expected behavior due to 60-second cache TTL

**Workaround**:
```bash
# Option 1: Wait for cache expiration (max 60 seconds)
# Option 2: Restart Synap server (clears cache)
# Option 3: Implement cache invalidation webhook (future feature)
```

### Permission Denied Despite Valid Key

**Symptom**: `403 Forbidden` with valid access key

**Possible Causes**:

1. **Plan restriction exceeded**
   ```json
   {
     "error": "Forbidden",
     "message": "TTL of 86400s exceeds Free plan limit of 3600s"
   }
   ```
   **Solution**: Reduce TTL or upgrade plan

2. **Dangerous command attempted**
   ```json
   {
     "error": "Forbidden",
     "message": "FLUSHALL command not allowed in Hub mode"
   }
   ```
   **Solution**: Use standalone mode or alternative approach

3. **Quota exceeded**
   ```json
   {
     "error": "QuotaExceeded",
     "message": "Storage quota exceeded"
   }
   ```
   **Solution**: Delete old data or upgrade plan

## Best Practices

### Key Management

1. **One Key Per Application**: Isolate blast radius
2. **Environment Separation**: Use `sk_dev_*`, `sk_test_*`, `sk_live_*`
3. **Regular Rotation**: Rotate keys every 90 days
4. **Least Privilege**: Use plan-appropriate keys
5. **Monitoring**: Track key usage and anomalies

### Development Workflow

```bash
# Development
export HIVEHUB_ACCESS_KEY="sk_dev_..."

# Testing
export HIVEHUB_ACCESS_KEY="sk_test_..."

# Production (via secrets manager)
export HIVEHUB_ACCESS_KEY="$(aws secretsmanager get-secret-value \
  --secret-id prod/synap/access-key \
  --query SecretString \
  --output text)"
```

### Security Checklist

- [ ] Access keys stored in environment variables or secrets manager
- [ ] Keys never committed to version control
- [ ] HTTPS enforced for all API calls
- [ ] Separate keys per environment (dev, test, prod)
- [ ] Key rotation scheduled every 90 days
- [ ] Monitoring alerts configured for key usage
- [ ] Revoked keys documented and tracked
- [ ] Access key permissions documented in runbook

## Future Enhancements

1. **Scoped Access Keys**: Limit keys to specific resources/operations
2. **Temporary Keys**: Time-limited keys for short-lived tasks
3. **Key Metadata**: Labels, descriptions, last-used timestamps
4. **Webhook Invalidation**: Real-time cache invalidation on revocation
5. **Key Rotation API**: Automated key rotation support
6. **Multi-Factor Authentication**: Require MFA for key creation
7. **IP Whitelisting**: Restrict keys to specific IP ranges
8. **Audit Logs**: Detailed access key usage logs

## References

- [HUB_INTEGRATION.md](./HUB_INTEGRATION.md) - Complete Hub integration specification
- [QUOTA_MANAGEMENT.md](./QUOTA_MANAGEMENT.md) - Quota management specification
- [AUTHENTICATION.md](../AUTHENTICATION.md) - General authentication documentation
- Phase 2 tasks: `rulebook/tasks/hub-integration/tasks.md` (Tasks 2.1-2.7)
- Implementation: `synap-server/src/hub/hub_auth.rs`
