# HiveHub.Cloud Integration Specification

## Overview

This document specifies the integration between Synap server and HiveHub.Cloud, enabling Synap to operate as a multi-tenant SaaS service within the HiveHub ecosystem.

**Version**: 1.0
**Status**: Implemented (Phases 1-6, 9-10 complete)
**Feature Flag**: `hub-integration`

## Table of Contents

1. [Architecture](#architecture)
2. [Operating Modes](#operating-modes)
3. [Multi-Tenant Isolation](#multi-tenant-isolation)
4. [Authentication & Authorization](#authentication--authorization)
5. [Quota Management](#quota-management)
6. [Rate Limiting](#rate-limiting)
7. [Usage Tracking](#usage-tracking)
8. [Configuration](#configuration)
9. [Security](#security)
10. [Testing](#testing)

---

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     HiveHub.Cloud API                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Access Keys  │  │    Quotas    │  │    Usage     │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │ hivehub-internal-sdk
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                    Synap Server (Hub Mode)                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              HubClient (SDK Wrapper)                  │   │
│  │  • Access Key Validation (60s cache)                  │   │
│  │  • Quota Fetching & Enforcement                       │   │
│  │  • Usage Reporting (5min intervals)                   │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │            Hub Authentication Middleware              │   │
│  │  • Extract access keys from headers                   │   │
│  │  • Validate with HubClient                            │   │
│  │  • Create HubUserContext with Plan                    │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │         Multi-Tenant Resource Scoping                 │   │
│  │  • Scope: user_{user_id}:{resource_name}             │   │
│  │  • Applies to: Queues, Streams, KV, Pub/Sub, etc     │   │
│  │  • Ownership validation on all operations             │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │          Plan-Based Restrictions (SaaS)               │   │
│  │  • TTL limits (Free: 24h, Pro: 7d, Ent: 30d)        │   │
│  │  • Payload limits (Free: 256KB, Pro: 1MB, Ent: 10MB)│   │
│  │  • Rate limits (Free: 10/s, Pro: 100/s, Ent: 1000/s)│   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

### Component Breakdown

#### 1. HubClient (`synap-server/src/hub/client.rs`)
- **Purpose**: Wrapper around `hivehub-internal-sdk` with caching and quota integration
- **Key Features**:
  - Access key validation with 60-second cache
  - Quota fetching and caching
  - Usage reporting to Hub API
  - Error conversion from SDK to Synap errors

#### 2. Authentication (`synap-server/src/hub/hub_auth.rs`)
- **Purpose**: Extract and validate Hub access keys from requests
- **Key Structures**:
  - `HubUserContext`: Contains `user_id`, `plan`, `access_key`
  - Middleware: `hub_auth_middleware` (hybrid), `require_hub_auth_middleware` (strict)
- **Access Key Sources** (in priority order):
  1. `X-Hub-Access-Key` header
  2. `Authorization: Bearer <token>` header

#### 3. Multi-Tenant Scoping (`synap-server/src/hub/multi_tenant.rs`)
- **Purpose**: Isolate resources between users
- **Naming Convention**: `user_{user_id}:{resource_name}`
  - Example: `user_550e8400e29b41d4a716446655440000:my-queue`
- **Resource Types Scoped**:
  - Queues (Queue Manager)
  - Streams (Stream Manager)
  - KV Store (all data types: String, Hash, List, Set, Sorted Set, etc.)
  - Pub/Sub topics
  - HyperLogLog, Bitmaps, Geospatial

#### 4. Restrictions (`synap-server/src/hub/restrictions.rs`)
- **Purpose**: Enforce Plan-based limits for shared SaaS environment
- **Plan Tiers**:
  - **Free**: 10 req/s, 24h TTL, 256KB payload, 100MB storage
  - **Pro**: 100 req/s, 7d TTL, 1MB payload, 10GB storage
  - **Enterprise**: 1000 req/s, 30d TTL, 10MB payload, 100GB storage

#### 5. Quota Management (`synap-server/src/hub/quota.rs`)
- **Purpose**: Track and enforce storage and operation quotas
- **Features**:
  - Local quota cache (60s TTL)
  - Pre-operation quota checks
  - Post-operation quota updates

#### 6. Usage Tracking (`synap-server/src/hub/usage.rs`)
- **Purpose**: Report resource usage to Hub for billing
- **Metrics**:
  - Message count (queue publishes, stream appends)
  - Storage bytes (KV, hashes, lists, etc.)
- **Reporting**: Background task every 5 minutes

---

## Operating Modes

### 1. Standalone Mode (Default)
- **Enable**: `hub.enabled = false` in config OR compile without `hub-integration` feature
- **Behavior**:
  - No authentication required (unless local auth configured)
  - No resource scoping
  - No quotas or rate limits (except global config)
  - Full access to all commands including FLUSH, etc.

### 2. Hub Mode (SaaS)
- **Enable**: `hub.enabled = true` AND compile with `hub-integration` feature
- **Behavior**:
  - Authentication required via Hub access keys
  - Multi-tenant resource isolation
  - Plan-based quotas and rate limits
  - Dangerous commands blocked (FLUSH, script commands, client list)
  - Usage tracking and reporting

### 3. Hybrid Mode
- **Description**: Hub mode with optional local authentication fallback
- **Use Case**: Gradual migration from standalone to Hub
- **Middleware**: `hub_auth_middleware` allows requests without Hub keys to fall back to local auth

---

## Multi-Tenant Isolation

### Resource Naming Convention

**Format**: `user_{user_id}:{resource_name}`

Where:
- `{user_id}`: UUID in simple format (no hyphens)
- `{resource_name}`: User-provided resource name

**Example**:
```
User Input: "my-queue"
User ID: 550e8400-e29b-41d4-a716-446655440000
Scoped Name: user_550e8400e29b41d4a716446655440000:my-queue
```

### Scoping Implementation

#### Queues
```rust
// In queue handlers
let scoped_name = MultiTenant::scope_queue_name(
    hub_context.as_ref().map(|ctx| &ctx.user_id),
    &request_params.queue_name
);
```

#### Streams
```rust
// In stream handlers
let scoped_name = MultiTenant::scope_stream_name(
    hub_context.as_ref().map(|ctx| &ctx.user_id),
    &request_params.stream_name
);
```

#### KV Store (all data types)
```rust
// In KV, Hash, List, Set, etc. handlers
let scoped_key = MultiTenant::scope_kv_key(
    hub_context.as_ref().map(|ctx| &ctx.user_id),
    &request_params.key
);
```

### Cross-User Isolation

**Guarantee**: Users can ONLY access their own resources

**Mechanisms**:
1. **Scoping**: All resource names prefixed with `user_{user_id}:`
2. **Filtering**: LIST operations filter results by user prefix
3. **Ownership Validation**: All operations validate resource belongs to requesting user
4. **Response Integrity**: Original (unscoped) names returned to users

**Example Flow**:
```
User 1 creates: "my-queue"
→ Stored as: "user_111...111:my-queue"

User 2 creates: "my-queue" (same name!)
→ Stored as: "user_222...222:my-queue"

User 1 lists queues:
→ Sees: ["my-queue"] (their own queue only)

User 2 lists queues:
→ Sees: ["my-queue"] (their own queue only)
```

### Multi-Key Operations

For operations involving multiple resources (e.g., `SET_MOVE`, `SET_INTER`):
- **ALL keys are scoped** to the user's namespace
- Users cannot reference other users' resources
- Example:
  ```rust
  // SET_INTER key1 key2 key3
  let scoped_keys = vec![
      MultiTenant::scope_kv_key(user_id, "key1"),
      MultiTenant::scope_kv_key(user_id, "key2"),
      MultiTenant::scope_kv_key(user_id, "key3"),
  ];
  ```

---

## Authentication & Authorization

### Access Key Flow

```
1. Client Request
   ↓
2. Extract Access Key (hub_auth.rs::extract_access_key)
   • Check X-Hub-Access-Key header
   • Check Authorization: Bearer header
   ↓
3. Validate with HubClient (hub_auth.rs::hub_auth_middleware)
   • Check 60s cache first
   • If miss, call Hub API via SDK
   ↓
4. Create HubUserContext
   • user_id: UUID from Hub
   • plan: Plan enum (Free/Pro/Enterprise)
   • access_key: Original key for audit
   ↓
5. Store in Request Extensions
   • Available to all handlers via HubContextExtractor
```

### HubUserContext Structure

```rust
pub struct HubUserContext {
    pub user_id: Uuid,      // Unique user identifier
    pub plan: Plan,         // Subscription tier
    pub access_key: String, // For logging/audit
}
```

### Extracting Context in Handlers

```rust
use synap_server::hub::HubContextExtractor;

pub async fn my_handler(
    HubContextExtractor(hub_ctx): HubContextExtractor,
    // ... other parameters
) -> Result<Json<Response>, SynapError> {
    if let Some(ctx) = hub_ctx {
        // Hub mode - enforce restrictions
        let user_id = ctx.user_id;
        let plan = ctx.plan;
        // ... apply scoping and restrictions
    } else {
        // Standalone mode - no restrictions
    }
}
```

### Cache Strategy

**Access Key Cache**:
- **TTL**: 60 seconds
- **Key**: Access key string
- **Value**: `(user_id, plan)`
- **Invalidation**: Manual via `HubClient::invalidate_access_key_cache()`

**Benefits**:
- Reduces Hub API calls (from N req/s to ~1 req/min per user)
- Improves latency
- Resilience to temporary Hub API issues

---

## Quota Management

### Quota Types

1. **Storage Quota**: Total bytes stored (KV, queues, streams)
2. **Operation Quota**: Monthly operation count (publishes, appends, etc.)

### Plan-Based Quotas

| Plan       | Storage  | Monthly Ops | Resource Limits          |
|------------|----------|-------------|--------------------------|
| Free       | 100 MB   | Unlimited*  | 10 queues, 10 streams    |
| Pro        | 10 GB    | Unlimited*  | 100 queues, 100 streams  |
| Enterprise | 100 GB   | Unlimited*  | 1000 queues, 1000 streams|

*Operation quotas managed by Hub billing

### Quota Enforcement Flow

```
1. Resource Creation Request
   ↓
2. Check Quota (quota.rs::QuotaManager)
   • Fetch user quota (cached 60s)
   • Validate: current_usage + new_size <= quota_limit
   ↓
3. If Quota OK:
   • Proceed with operation
   • Update local usage tracking
   ↓
4. If Quota Exceeded:
   • Return 429 Too Many Requests
   • Error: "Storage quota exceeded"
```

### Quota Cache

```rust
pub struct UserQuota {
    pub storage_used: u64,
    pub storage_limit: u64,
    pub monthly_operations: u64,
    pub monthly_operations_limit: u64,
    pub plan: Plan,
    pub updated_at: Instant,
}
```

### Quota API Endpoint

**GET** `/hub/quota`

**Headers**: `X-Hub-Access-Key: <access_key>`

**Response**:
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "plan": "Pro",
  "storage": {
    "used_bytes": 1048576,
    "limit_bytes": 10737418240,
    "remaining_bytes": 10736369664,
    "usage_percent": 0.01
  },
  "operations": {
    "monthly_count": 50000,
    "monthly_limit": null,
    "remaining": null,
    "usage_percent": 0
  }
}
```

---

## Rate Limiting

### Plan-Based Rate Limits

| Plan       | Requests/Second | Burst Capacity |
|------------|-----------------|----------------|
| Free       | 10              | 20             |
| Pro        | 100             | 200            |
| Enterprise | 1000            | 2000           |

### Implementation

**Algorithm**: Token Bucket
- **Refill Rate**: Continuous (tokens per second)
- **Burst Capacity**: 2x rate limit
- **Bucket Key**: `user:{user_id}` (Hub mode) or `ip:{ip_address}` (standalone)

### Rate Limit Headers

All responses include standard rate limit headers:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 87
X-RateLimit-Reset: 1
```

### Rate Limit Exceeded Response

**Status**: `429 Too Many Requests`

**Body**:
```json
{
  "error": "Rate limit exceeded",
  "limit": 100,
  "retry_after": 1
}
```

---

## Usage Tracking

### Metrics Tracked

1. **Message Count**:
   - Queue publishes
   - Stream appends
   - Pub/Sub publishes

2. **Storage Bytes**:
   - KV values
   - Queue messages
   - Stream entries
   - Hash fields
   - List items
   - Set members

### Reporting Schedule

- **Interval**: 5 minutes (configurable via `hub.usage_report_interval`)
- **Method**: Background task (UsageReporter)
- **Endpoint**: `HubClient::update_usage()`

### Usage Aggregation

```rust
// Per-resource usage update
HubClient::update_usage(
    &user_id,
    ResourceType::Queue,
    "my-queue",
    Some(message_count),  // Messages published
    Some(storage_bytes),  // Total queue size
).await?;
```

### Error Handling

- **Network Errors**: Logged, retried on next interval
- **Invalid User**: Logged, no retry
- **Quota Exceeded**: Returned to user immediately

---

## Configuration

### Configuration File (`config.yml`)

```yaml
hub:
  enabled: true
  api_url: "https://api.hivehub.cloud"
  service_api_key: "your-service-api-key-here"
  usage_report_interval: 300  # seconds (5 minutes)
  cache_ttl: 60               # seconds
  timeout: 30                 # seconds
```

### Environment Variables

All Hub configuration can be overridden via environment variables:

```bash
SYNAP_HUB_ENABLED=true
SYNAP_HUB_API_URL=https://api.hivehub.cloud
SYNAP_HUB_SERVICE_API_KEY=sk_live_...
SYNAP_HUB_USAGE_REPORT_INTERVAL=300
SYNAP_HUB_CACHE_TTL=60
SYNAP_HUB_TIMEOUT=30
```

**Priority**: Environment Variables > Config File

### Compilation

**Enable Hub Integration**:
```bash
cargo build --features hub-integration
```

**Disable Hub Integration** (default):
```bash
cargo build
```

### Validation

On startup, Synap validates:
1. Hub configuration completeness
2. Service API key format
3. Cluster mode compatibility (warns if both enabled)

---

## Security

### Dangerous Command Blocking

In Hub mode, the following commands are **BLOCKED**:

| Command        | Reason                                    |
|----------------|-------------------------------------------|
| `FLUSH`        | Would delete all users' data              |
| `SCRIPT FLUSH` | Could affect other users' scripts         |
| `SCRIPT KILL`  | Could kill other users' scripts           |
| `CLIENT LIST`  | Exposes other users' connection info      |

**Response**: `403 Forbidden` - "This command is not available in Hub mode for security reasons"

### TTL Enforcement

**Mandatory TTL in SaaS Mode**:
- All resource operations MUST specify TTL
- Default TTL: 24 hours (if not specified)
- Minimum TTL: 5 minutes
- Maximum TTL: Plan-dependent (Free: 24h, Pro: 7d, Enterprise: 30d)

**Rationale**: Prevents unbounded resource growth on shared infrastructure

### Payload Size Limits

| Plan       | Max Payload |
|------------|-------------|
| Free       | 256 KB      |
| Pro        | 1 MB        |
| Enterprise | 10 MB       |

**Enforcement**: Pre-validation before operations

### Access Control

- **User Isolation**: Users can only access their own resources
- **Ownership Validation**: All operations validate resource ownership
- **Namespace Enforcement**: Scoping applied at all API layers

---

## Testing

### Test Coverage

- **Unit Tests**: 92 Hub module tests
- **Integration Tests**: Deferred (pending Hub API implementation)
- **Backward Compatibility**: 100% (430/430 core tests pass in both modes)

### Test Breakdown

| Module              | Tests | Coverage                                 |
|---------------------|-------|------------------------------------------|
| restrictions.rs     | 22    | Plan limits, standalone mode, edge cases |
| multi_tenant.rs     | 13    | Scoping, ownership, filtering            |
| hub_auth.rs         | 10    | Access key extraction, context           |
| client.rs           | 9     | SDK wrapper, caching, errors             |
| quota.rs            | ~10   | Quota management                         |
| naming.rs           | ~8    | Resource naming validation               |
| config.rs           | ~10   | Configuration validation                 |
| usage.rs            | ~10   | Usage tracking                           |

### Running Tests

```bash
# Without Hub integration (430 tests)
cargo test --lib

# With Hub integration (522 tests)
cargo test --lib --features hub-integration

# Hub module only
cargo test --lib --features hub-integration hub::
```

---

## Implementation Status

### Completed Phases

- ✅ **Phase 1**: HiveHub SDK Integration (7/7 tasks)
- ✅ **Phase 2**: Authentication (7/7 tasks)
- ✅ **Phase 3**: Multi-Tenant Data Isolation (9/9 tasks)
- ✅ **Phase 4**: Quota Management (7/7 tasks)
- ✅ **Phase 5**: Rate Limiting Enhancement (5/5 tasks)
- ✅ **Phase 6**: Usage Tracking (5/5 tasks)
- ✅ **Phase 9**: Configuration (7/7 tasks)
- ✅ **Phase 10**: Testing (6/9 tasks - 3 deferred)

### Pending Phases

- ⏸️ **Phase 7**: Cluster Support (0/4 tasks)
- ⏸️ **Phase 8**: Migration Tool (0/5 tasks)
- 🚧 **Phase 11**: Documentation (in progress)

---

## API Reference

### Hub-Specific Endpoints

#### GET `/hub/quota`
Get current quota usage and limits

**Auth**: Required (Hub access key)

**Response**: See [Quota API Endpoint](#quota-api-endpoint) above

---

## Troubleshooting

### Common Issues

**1. "Hub integration enabled but no Hub authentication found"**
- **Cause**: Hub mode enabled but no access key provided
- **Solution**: Include `X-Hub-Access-Key` header in requests

**2. "Storage quota exceeded"**
- **Cause**: User has reached their plan's storage limit
- **Solution**: Upgrade plan or delete unused resources

**3. "Rate limit exceeded"**
- **Cause**: User exceeded their plan's requests/second limit
- **Solution**: Implement client-side rate limiting or upgrade plan

**4. "This command is not available in Hub mode"**
- **Cause**: Attempting dangerous command (FLUSH, etc.) in Hub mode
- **Solution**: Use standalone mode for administrative commands

---

## Migration Guide

### From Standalone to Hub Mode

1. **Enable Hub integration**:
   ```yaml
   hub:
     enabled: true
     api_url: "https://api.hivehub.cloud"
     service_api_key: "your-key"
   ```

2. **Update client applications**:
   - Add `X-Hub-Access-Key` header to all requests
   - Handle 429 (rate limit) and 403 (quota exceeded) responses

3. **Test thoroughly**:
   - Verify resource isolation
   - Test quota enforcement
   - Validate rate limiting

4. **Monitor**:
   - Check usage metrics in Hub dashboard
   - Monitor quota consumption
   - Track rate limit hits

### Rollback Strategy

To revert to standalone mode:

1. Set `hub.enabled = false` in config
2. Restart Synap server
3. All resources remain accessible (no scoping applied)

---

## References

- [QUOTA_MANAGEMENT.md](./QUOTA_MANAGEMENT.md) - Detailed quota specification
- [ACCESS_KEYS.md](./ACCESS_KEYS.md) - Access key format and security
- [../AUTHENTICATION.md](../AUTHENTICATION.md) - Complete authentication guide

---

**Document Version**: 1.0
**Last Updated**: 2025-12-10
**Status**: Production Ready (Phases 1-6, 9-10)
