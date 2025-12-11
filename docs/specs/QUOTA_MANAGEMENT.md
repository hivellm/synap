# Quota Management Specification

## Overview

The Quota Management system enforces Plan-based resource limits for users in HiveHub.Cloud SaaS mode. It ensures fair resource allocation, prevents abuse, and enables tiered service offerings through Free, Pro, and Enterprise plans.

**Key Features**:
- Plan-based quotas (storage, operations, connections)
- Pre-operation quota enforcement (fail-fast)
- Real-time usage tracking and reporting
- Graceful degradation (429 Too Many Requests)
- 60-second quota cache for performance
- Periodic sync with HiveHub API (5-minute intervals)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Quota Management Flow                    │
└─────────────────────────────────────────────────────────────┘

User Request
    │
    ├─> [Hub Auth Middleware]
    │       │
    │       ├─> Extract Access Key
    │       ├─> Validate via HubClient (cached 60s)
    │       └─> Create HubUserContext (user_id, plan)
    │
    ├─> [Quota Middleware]
    │       │
    │       ├─> Extract HubUserContext
    │       ├─> Fetch User Quotas via HubClient (cached 60s)
    │       │       ├─> SDK: hub_client.synap().get_quotas(user_id)
    │       │       └─> Cache: QuotaCache TTL=60s
    │       │
    │       ├─> Check Operation Against Quota
    │       │       ├─> Storage quota check (for writes)
    │       │       ├─> Operation quota check (monthly limit)
    │       │       └─> Connection quota check (active connections)
    │       │
    │       └─> Allow/Deny Request
    │               ├─> ALLOW: Continue to handler
    │               └─> DENY: Return 429 Too Many Requests
    │
    ├─> [Handler Execution]
    │       │
    │       └─> Perform operation (queue, stream, KV, etc.)
    │
    └─> [Usage Reporter]
            │
            ├─> Background task (every 5 minutes)
            ├─> Aggregate usage metrics
            │       ├─> Storage used (bytes)
            │       ├─> Operations performed
            │       └─> Peak connections
            │
            └─> Report to HubClient
                    └─> SDK: hub_client.synap().update_usage(user_id, metrics)
```

## Plan-Based Quotas

### Quota Comparison Table

| Quota Type              | Free Plan      | Pro Plan       | Enterprise Plan |
|-------------------------|----------------|----------------|-----------------|
| **Storage**             | 100 MB         | 10 GB          | 1 TB            |
| **Operations/Month**    | 100,000        | 10,000,000     | Unlimited       |
| **Active Connections**  | 10             | 100            | 1,000           |
| **Max TTL**             | 24 hours       | 30 days        | 365 days        |
| **Max Payload Size**    | 1 MB           | 10 MB          | 100 MB          |
| **Max Batch Size**      | 100            | 1,000          | 10,000          |
| **Rate Limit**          | 10 req/s       | 100 req/s      | 1,000 req/s     |

### Quota Types

#### 1. Storage Quota
**Purpose**: Limit total data stored across all resources (queues, streams, KV, etc.)

**Enforcement**:
- Checked BEFORE write operations (publish, set, push, etc.)
- Denies operation if `current_usage + operation_size > storage_quota`
- Returns `429 Too Many Requests` with error message

**Calculation**:
- Queue messages: Sum of all message payloads
- Stream events: Sum of all event payloads
- KV pairs: Sum of all key + value sizes
- Pub/Sub: Not counted (transient)

**Example**:
```rust
// Free plan user (100 MB quota)
let current_usage = 95_000_000; // 95 MB used
let operation_size = 10_000_000; // 10 MB message

// Check quota
if current_usage + operation_size > 100_000_000 {
    return Err(SynapError::QuotaExceeded(
        "Storage quota exceeded. Upgrade plan to increase limit."
    ));
}
```

#### 2. Operation Quota
**Purpose**: Limit total operations per month (read + write)

**Enforcement**:
- Checked BEFORE each operation
- Denies operation if `monthly_operations >= operation_quota`
- Resets monthly (1st of each month)

**Counted Operations**:
- Queue: publish, consume, ack, nack
- Stream: append, read, trim
- KV: get, set, delete, incr, etc.
- Pub/Sub: publish, subscribe (first subscription only)
- List/Set/Hash/HLL/Bitmap: All data structure operations

**Example**:
```rust
// Free plan user (100,000 ops/month quota)
let monthly_operations = 99_500; // 99,500 ops this month

// Check quota
if monthly_operations >= 100_000 {
    return Err(SynapError::QuotaExceeded(
        "Monthly operation quota exceeded. Resets on 1st of next month."
    ));
}
```

#### 3. Connection Quota
**Purpose**: Limit concurrent active connections

**Enforcement**:
- Checked when new connection established
- Denies connection if `active_connections >= connection_quota`
- Decrements when connection closed

**Counted Connections**:
- HTTP/2 persistent connections
- WebSocket connections (future)
- gRPC streams (future)

**Example**:
```rust
// Free plan user (10 connections quota)
let active_connections = 10;

// Check quota on new connection
if active_connections >= 10 {
    return Err(SynapError::QuotaExceeded(
        "Connection quota exceeded. Close idle connections or upgrade plan."
    ));
}
```

## Quota Enforcement Workflow

### Pre-Operation Check

```rust
// 1. Extract Hub user context from request
let hub_ctx = request.extensions()
    .get::<HubUserContext>()
    .ok_or(SynapError::Unauthorized)?;

// 2. Fetch user quotas (cached 60s)
let quotas = hub_client.synap()
    .get_quotas(hub_ctx.user_id)
    .await?;

// 3. Check storage quota (for write operations)
if is_write_operation {
    let operation_size = calculate_operation_size(&payload);

    if quotas.current_storage + operation_size > quotas.storage_limit {
        return Err(SynapError::QuotaExceeded(
            "Storage quota exceeded"
        ));
    }
}

// 4. Check operation quota
if quotas.monthly_operations >= quotas.operation_limit {
    return Err(SynapError::QuotaExceeded(
        "Monthly operation quota exceeded"
    ));
}

// 5. Check connection quota (for new connections)
if is_new_connection {
    if quotas.active_connections >= quotas.connection_limit {
        return Err(SynapError::QuotaExceeded(
            "Connection quota exceeded"
        ));
    }
}

// 6. Allow operation
Ok(())
```

### Post-Operation Usage Tracking

```rust
// After successful operation, track usage
usage_reporter.record_operation(UsageMetric {
    user_id: hub_ctx.user_id,
    operation_type: OperationType::QueuePublish,
    bytes_written: payload.len(),
    timestamp: Utc::now(),
});
```

## Usage Tracking & Reporting

### UsageReporter Background Task

**Purpose**: Periodically report usage metrics to HiveHub API

**Frequency**: Every 5 minutes

**Metrics Reported**:
```rust
pub struct UsageMetrics {
    pub user_id: Uuid,
    pub storage_bytes: u64,          // Total storage used
    pub operations_count: u64,       // Operations in reporting period
    pub active_connections: u32,     // Peak connections in period
    pub timestamp: DateTime<Utc>,    // Report timestamp
}
```

**Implementation**:
```rust
// Background task loop
loop {
    tokio::time::sleep(Duration::from_secs(300)).await; // 5 minutes

    // Aggregate metrics for all users
    let metrics = aggregate_user_metrics().await;

    // Report to Hub API
    for (user_id, usage) in metrics {
        match hub_client.synap().update_usage(user_id, usage).await {
            Ok(_) => debug!("Usage reported for user {}", user_id),
            Err(e) => warn!("Failed to report usage: {}", e),
        }
    }
}
```

**Error Handling**:
- Non-blocking: Failures don't affect user operations
- Retry logic: 3 retries with exponential backoff
- Fallback: Local metrics preserved for next sync

## Quota API Endpoints

### GET /hub/quota

**Description**: Get current quota usage and limits for authenticated user

**Authentication**: Required (Hub access key)

**Response**:
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "plan": "Pro",
  "quotas": {
    "storage": {
      "limit": 10737418240,
      "used": 5368709120,
      "available": 5368709120,
      "percentage": 50.0
    },
    "operations": {
      "limit": 10000000,
      "used": 3456789,
      "available": 6543211,
      "percentage": 34.6,
      "resets_at": "2025-02-01T00:00:00Z"
    },
    "connections": {
      "limit": 100,
      "active": 12,
      "available": 88,
      "percentage": 12.0
    }
  },
  "restrictions": {
    "max_ttl_seconds": 2592000,
    "max_payload_bytes": 10485760,
    "max_batch_size": 1000
  }
}
```

**Status Codes**:
- `200 OK`: Quota information returned
- `401 Unauthorized`: Invalid or missing access key
- `500 Internal Server Error`: Failed to fetch quota from Hub API

## Configuration

### Synap Configuration

```yaml
# synap.yaml
hub:
  enabled: true
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  base_url: "https://api.hivehub.cloud"

  # Quota settings
  quota:
    cache_ttl_seconds: 60          # How long to cache quota data
    usage_report_interval: 300     # Report usage every 5 minutes

  # Usage tracking
  usage:
    buffer_size: 10000             # Max metrics in memory before flush
    flush_interval_seconds: 300    # Force flush every 5 minutes
```

### Environment Variables

```bash
# Required
export HIVEHUB_SERVICE_API_KEY="sk_live_..."

# Optional
export HIVEHUB_BASE_URL="https://api.hivehub.cloud"
export HIVEHUB_QUOTA_CACHE_TTL="60"
export HIVEHUB_USAGE_REPORT_INTERVAL="300"
```

## Error Responses

### 429 Too Many Requests

**Triggered When**: Quota exceeded

**Response Headers**:
```
HTTP/1.1 429 Too Many Requests
Content-Type: application/json
X-RateLimit-Limit: 10000000
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1738368000
Retry-After: 3600
```

**Response Body**:
```json
{
  "error": "QuotaExceeded",
  "message": "Monthly operation quota exceeded (10,000,000 operations). Resets on 2025-02-01.",
  "quota_type": "operations",
  "current_usage": 10000000,
  "limit": 10000000,
  "resets_at": "2025-02-01T00:00:00Z",
  "upgrade_url": "https://hivehub.cloud/upgrade"
}
```

### Quota Error Types

| Error                    | Status Code | Description                           |
|--------------------------|-------------|---------------------------------------|
| `StorageQuotaExceeded`   | 429         | Storage limit reached                 |
| `OperationQuotaExceeded` | 429         | Monthly operation limit reached       |
| `ConnectionQuotaExceeded`| 429         | Concurrent connection limit reached   |
| `PayloadTooLarge`        | 413         | Payload exceeds plan limit            |
| `BatchTooLarge`          | 413         | Batch size exceeds plan limit         |
| `TTLTooLong`             | 400         | TTL exceeds plan limit                |

## Implementation Details

### Quota Caching Strategy

**Why Cache?**
- Reduce Hub API calls (avoid rate limiting)
- Improve response latency (60s cache vs API round-trip)
- Continue operating during Hub API outages

**Cache Implementation**:
```rust
pub struct QuotaCache {
    cache: Arc<RwLock<HashMap<Uuid, CachedQuota>>>,
    ttl: Duration,
}

struct CachedQuota {
    quotas: UserQuotas,
    cached_at: Instant,
}

impl QuotaCache {
    pub async fn get_or_fetch(&self, user_id: Uuid) -> Result<UserQuotas> {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(cached) = cache.get(&user_id) {
                if cached.cached_at.elapsed() < self.ttl {
                    return Ok(cached.quotas.clone());
                }
            }
        }

        // Cache miss or expired - fetch from Hub API
        let quotas = self.hub_client.synap()
            .get_quotas(user_id)
            .await?;

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.insert(user_id, CachedQuota {
                quotas: quotas.clone(),
                cached_at: Instant::now(),
            });
        }

        Ok(quotas)
    }
}
```

**Cache Invalidation**:
- Time-based: 60-second TTL
- Event-based: When quota update detected from Hub API
- Manual: Admin API to clear cache

### Usage Aggregation

**In-Memory Metrics**:
```rust
pub struct UsageAggregator {
    metrics: Arc<RwLock<HashMap<Uuid, UserMetrics>>>,
}

struct UserMetrics {
    storage_bytes: AtomicU64,
    operations_count: AtomicU64,
    peak_connections: AtomicU32,
}

impl UsageAggregator {
    pub fn record_operation(&self, user_id: Uuid, bytes: u64) {
        let metrics = self.metrics.read();
        if let Some(user_metrics) = metrics.get(&user_id) {
            user_metrics.operations_count.fetch_add(1, Ordering::Relaxed);
            user_metrics.storage_bytes.fetch_add(bytes, Ordering::Relaxed);
        }
    }

    pub fn snapshot_and_reset(&self) -> HashMap<Uuid, UsageMetrics> {
        let mut metrics = self.metrics.write();
        let snapshot = metrics.iter().map(|(user_id, m)| {
            (*user_id, UsageMetrics {
                storage_bytes: m.storage_bytes.swap(0, Ordering::Relaxed),
                operations_count: m.operations_count.swap(0, Ordering::Relaxed),
                active_connections: m.peak_connections.swap(0, Ordering::Relaxed),
                timestamp: Utc::now(),
            })
        }).collect();

        snapshot
    }
}
```

## Testing

### Unit Tests

**Coverage**: 92 Hub module tests including quota enforcement

**Test Files**:
- `synap-server/src/hub/quota.rs` - Quota management tests
- `synap-server/src/hub/client.rs` - SDK quota fetching tests
- `synap-server/src/hub/restrictions.rs` - Plan-based limit tests
- `synap-server/src/hub/usage.rs` - Usage tracking tests

**Example Tests**:
```rust
#[tokio::test]
async fn test_storage_quota_enforcement() {
    let hub_client = create_test_hub_client();
    let user_id = Uuid::new_v4();

    // Set quota: 100 MB
    hub_client.set_test_quota(user_id, QuotaType::Storage, 100_000_000);

    // Current usage: 95 MB
    hub_client.set_test_usage(user_id, 95_000_000);

    // Try to publish 10 MB message (would exceed quota)
    let result = publish_message(user_id, vec![0u8; 10_000_000]).await;

    // Should be denied
    assert!(matches!(result, Err(SynapError::QuotaExceeded(_))));
}

#[tokio::test]
async fn test_operation_quota_enforcement() {
    let hub_client = create_test_hub_client();
    let user_id = Uuid::new_v4();

    // Set quota: 100,000 ops/month
    hub_client.set_test_quota(user_id, QuotaType::Operations, 100_000);

    // Current usage: 99,999 ops
    hub_client.set_test_usage(user_id, 99_999);

    // Try one more operation (would reach quota)
    let result = perform_operation(user_id).await;
    assert!(result.is_ok()); // Should be allowed (at quota)

    // Try another operation (would exceed quota)
    let result = perform_operation(user_id).await;
    assert!(matches!(result, Err(SynapError::QuotaExceeded(_))));
}
```

### Integration Tests

**Status**: DEFERRED pending Hub API implementation

**Required Tests**:
- End-to-end quota enforcement with real Hub API
- Usage reporting verification
- Quota cache expiration behavior
- Error handling for Hub API failures

## Security Considerations

### Quota Bypass Prevention

**Threat**: User attempts to bypass quota checks

**Mitigations**:
1. **Server-side enforcement**: All quota checks on server, never trust client
2. **Pre-operation checks**: Deny before resource consumption
3. **Idempotent operations**: Usage tracking prevents double-counting
4. **Audit logging**: All quota denials logged with user_id

### Resource Exhaustion

**Threat**: User attempts to exhaust server resources

**Mitigations**:
1. **Plan-based limits**: TTL, payload, batch size restrictions
2. **Rate limiting**: Token bucket algorithm per user
3. **Connection limits**: Max concurrent connections per plan
4. **Operation timeouts**: Prevent long-running operations

### Quota Cache Poisoning

**Threat**: Attacker manipulates cached quota data

**Mitigations**:
1. **Short TTL**: 60-second cache expiration
2. **Immutable cache entries**: No direct modification
3. **Hub API as source of truth**: Cache is optimization only
4. **Signed responses**: Hub API responses include signature (future)

## Troubleshooting

### Quota Exceeded Unexpectedly

**Symptom**: User gets 429 errors despite not reaching quota

**Possible Causes**:
1. **Stale quota cache**: Cached quota doesn't reflect recent upgrade
   - Solution: Wait 60s for cache expiration or clear cache
2. **Usage reporting delay**: Recent usage not yet reported
   - Solution: Wait 5 minutes for next usage sync
3. **Shared resources**: Multiple clients using same access key
   - Solution: Review all active connections

**Debug Steps**:
```bash
# Check current quota via API
curl -H "X-Hub-Access-Key: $ACCESS_KEY" \
  https://synap.example.com/hub/quota

# Review recent operations
grep "quota_check" synap-server.log | tail -n 50
```

### Usage Not Reporting

**Symptom**: Hub dashboard shows zero usage despite activity

**Possible Causes**:
1. **UsageReporter not running**: Background task crashed
   - Solution: Check logs for task errors, restart server
2. **Hub API unreachable**: Network connectivity issues
   - Solution: Verify `hub.base_url` configuration, check firewall
3. **Invalid service API key**: Authentication failure
   - Solution: Verify `HIVEHUB_SERVICE_API_KEY` environment variable

**Debug Steps**:
```bash
# Check UsageReporter logs
grep "UsageReporter" synap-server.log | tail -n 20

# Test Hub API connectivity
curl -H "Authorization: Bearer $HIVEHUB_SERVICE_API_KEY" \
  https://api.hivehub.cloud/v1/health
```

## Migration Guide

### Adding Quotas to Existing Installation

**Before Migration**:
- Standalone mode: No quotas, unlimited resources
- Local authentication only

**After Migration**:
- Hub mode: Plan-based quotas enforced
- Multi-tenant isolation enabled

**Migration Steps**:

1. **Backup existing data**:
   ```bash
   # Backup Synap data directory
   cp -r /var/lib/synap /var/lib/synap.backup
   ```

2. **Configure Hub integration**:
   ```yaml
   # synap.yaml
   hub:
     enabled: true
     service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
   ```

3. **Assign users to plans**:
   - All existing users default to **Free** plan
   - Manually upgrade specific users via Hub dashboard

4. **Monitor quota usage**:
   ```bash
   # Watch for quota exceeded errors
   tail -f synap-server.log | grep "QuotaExceeded"
   ```

5. **Gradual rollout**:
   - Enable Hub mode on staging environment first
   - Test quota enforcement with sample users
   - Monitor for unexpected quota denials
   - Roll out to production

### Handling Legacy Data

**Issue**: Existing resources don't have user namespaces

**Solution**: Migration tool (Phase 8) will:
1. Scan all existing resources
2. Assign ownership to default user
3. Apply namespace prefixes: `user_{default_id}:{resource}`
4. Preserve data integrity with validation

**Rollback Plan**:
```bash
# Disable Hub mode (revert to standalone)
hub:
  enabled: false

# Restore from backup if needed
cp -r /var/lib/synap.backup /var/lib/synap
```

## Future Enhancements

1. **Dynamic quota adjustment**: Real-time quota updates without cache expiration
2. **Quota alerts**: Notify users at 80%, 90%, 100% usage thresholds
3. **Usage analytics**: Detailed breakdowns by operation type
4. **Burst quotas**: Temporary quota increases for spikes
5. **Custom quotas**: Per-user overrides for enterprise customers
6. **Quota reservations**: Pre-allocate quota for critical operations

## References

- [HUB_INTEGRATION.md](./HUB_INTEGRATION.md) - Complete Hub integration specification
- [ACCESS_KEYS.md](./ACCESS_KEYS.md) - Access key authentication specification
- [AUTHENTICATION.md](../AUTHENTICATION.md) - General authentication documentation
- Phase 4 tasks: `rulebook/tasks/hub-integration/tasks.md` (Tasks 4.1-4.7)
- Implementation: `synap-server/src/hub/quota.rs`
