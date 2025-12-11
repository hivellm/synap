# Technical Design: HiveHub.Cloud Integration

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Client                               │
│                    (Access Key)                             │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                   Synap Server                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │           Hub Auth Middleware                         │  │
│  │  1. Extract access key                                │  │
│  │  2. Validate via Hub SDK                             │  │
│  │  3. Store UserContext                                 │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │         SaaS Restrictions Middleware                  │  │
│  │  - TTL enforcement                                    │  │
│  │  - Size limits                                        │  │
│  │  - Rate limiting                                      │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │         Resource Operations                           │  │
│  │  - Namespace: user_{user_id}:{resource}              │  │
│  │  - Quota checking via Hub SDK                        │  │
│  │  - Usage tracking                                     │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              HiveHub.Cloud API                              │
│         (via hivehub-cloud-internal-sdk)                   │
└─────────────────────────────────────────────────────────────┘
```

## HiveHub SDK Integration

### SDK Module Structure

The `hivehub-cloud-internal-sdk` provides:

```rust
pub mod synap {
    pub fn get_user_resources(user_id: &Uuid) -> Result<UserResourcesResponse>;
    pub fn validate_resource(resource_name: &str, user_id: &Uuid, resource_type: ResourceType) -> Result<ResourceValidation>;
    pub fn create_resource(user_id: &Uuid, request: CreateResourceRequest) -> Result<CreateResourceResponse>;
    pub fn update_usage(user_id: &Uuid, request: UpdateUsageRequest) -> Result<UpdateUsageResponse>;
    pub fn check_quota(user_id: &Uuid, request: QuotaCheckRequest) -> Result<QuotaCheckResponse>;
}
```

### Client Wrapper Implementation

```rust
// synap-server/src/hub/client.rs
pub struct HubClient {
    sdk_client: HiveHubCloudClient,
    cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
}

impl HubClient {
    pub async fn get_user_resources(&self, user_id: &Uuid) -> Result<UserResources> {
        // Check cache first (60s TTL)
        if let Some(cached) = self.cache.get(&format!("resources:{}", user_id)) {
            if !cached.is_expired() {
                return Ok(cached.data);
            }
        }
        
        // Fetch from SDK
        let resources = self.sdk_client.synap()
            .get_user_resources(user_id)
            .await?;
        
        // Cache result
        self.cache.insert(format!("resources:{}", user_id), CachedResponse {
            data: resources.clone(),
            expires_at: Instant::now() + Duration::from_secs(60),
        });
        
        Ok(resources)
    }
}
```

## Resource Naming Convention

### User-Scoped Naming

All resources are prefixed with user namespace:

- **Queues**: `user_{user_id}:{queue_name}`
- **Streams**: `user_{user_id}:{stream_name}`
- **KV Keys**: `user_{user_id}:{key_name}`
- **Pub/Sub Topics**: `user_{user_id}:{topic_name}`

### Implementation

```rust
// synap-server/src/hub/naming.rs
pub struct ResourceNaming;

impl ResourceNaming {
    pub fn user_scoped_name(user_id: &Uuid, resource_name: &str) -> String {
        format!("user_{}:{}", user_id, resource_name)
    }
    
    pub fn extract_user_id(scoped_name: &str) -> Option<Uuid> {
        if let Some(stripped) = scoped_name.strip_prefix("user_") {
            if let Some((user_id_str, _)) = stripped.split_once(':') {
                return Uuid::parse_str(user_id_str).ok();
            }
        }
        None
    }
    
    pub fn belongs_to_user(scoped_name: &str, user_id: &Uuid) -> bool {
        Self::extract_user_id(scoped_name)
            .map(|id| id == *user_id)
            .unwrap_or(false)
    }
}
```

## SaaS Security Restrictions

### TTL Enforcement

In Hub mode (SaaS), TTL is mandatory for all resources:

```rust
pub struct HubSaaSRestrictions;

impl HubSaaSRestrictions {
    /// TTL máximo permitido (7 dias)
    pub const MAX_TTL_SECONDS: u64 = 604_800;
    
    /// TTL padrão quando não especificado (24 horas)
    pub const DEFAULT_TTL_SECONDS: u64 = 86_400;
    
    /// TTL mínimo (5 minutos)
    pub const MIN_TTL_SECONDS: u64 = 300;
}
```

### Size Limits

```rust
impl HubSaaSRestrictions {
    /// Tamanho máximo de mensagem individual (1MB)
    pub const MAX_MESSAGE_SIZE_BYTES: usize = 1_048_576;
    
    /// Tamanho máximo de valor no KV (512KB)
    pub const MAX_KV_VALUE_SIZE_BYTES: usize = 524_288;
    
    /// Tamanho máximo de key name (256 chars)
    pub const MAX_KEY_NAME_LENGTH: usize = 256;
    
    /// Máximo de mensagens por batch (100)
    pub const MAX_BATCH_SIZE: usize = 100;
}
```

### Rate Limiting

Per-user rate limiting (not per-IP):

```rust
pub struct UserRateLimiter {
    limiters: Arc<RwLock<HashMap<Uuid, TokenBucket>>>,
}

impl UserRateLimiter {
    pub async fn check(&self, user_id: &Uuid) -> Result<()> {
        // Get or create rate limiter for user
        // Check if request is allowed
        // Return 429 if exceeded
    }
}
```

### Plan-Based Restrictions

```rust
pub struct PlanRestrictions {
    pub ttl_max_secs: u64,
    pub ttl_default_secs: u64,
    pub max_message_size_bytes: usize,
    pub max_requests_per_second: u32,
    pub max_batch_size: usize,
    pub operation_timeout_secs: u64,
}

impl PlanRestrictions {
    pub fn for_plan(plan: SubscriptionPlan) -> Self {
        match plan {
            SubscriptionPlan::Free => Self {
                ttl_max_secs: 86_400,        // 24 horas
                ttl_default_secs: 3_600,     // 1 hora
                max_message_size_bytes: 256_000,  // 256KB
                max_requests_per_second: 10,
                max_batch_size: 10,
                operation_timeout_secs: 10,
            },
            SubscriptionPlan::Pro => Self {
                ttl_max_secs: 604_800,       // 7 dias
                ttl_default_secs: 86_400,    // 24 horas
                max_message_size_bytes: 1_048_576,  // 1MB
                max_requests_per_second: 100,
                max_batch_size: 100,
                operation_timeout_secs: 30,
            },
            SubscriptionPlan::Enterprise => Self {
                ttl_max_secs: 2_592_000,     // 30 dias
                ttl_default_secs: 604_800,   // 7 dias
                max_message_size_bytes: 10_485_760,  // 10MB
                max_requests_per_second: 1000,
                max_batch_size: 1000,
                operation_timeout_secs: 60,
            },
        }
    }
}
```

## Usage Tracking

### Background Reporter

```rust
// synap-server/src/hub/usage.rs
pub struct UsageReporter {
    hub_client: Arc<HubClient>,
    metrics: Arc<RwLock<HashMap<Uuid, UserMetrics>>>,
    interval: Duration,
}

impl UsageReporter {
    pub async fn run(&self) {
        let mut interval = tokio::time::interval(self.interval);
        
        loop {
            interval.tick().await;
            
            // Aggregate metrics per user
            let metrics = self.metrics.read().await;
            for (user_id, user_metrics) in metrics.iter() {
                // Send to Hub via SDK
                self.hub_client.sdk_client.synap()
                    .update_usage(user_id, &UpdateUsageRequest {
                        resource_type: ResourceType::Queue,
                        resource_name: format!("user_{}:*", user_id),
                        message_count: Some(user_metrics.queue_messages),
                        storage_bytes: Some(user_metrics.storage_bytes),
                    })
                    .await
                    .ok(); // Don't fail on usage reporting errors
            }
        }
    }
}
```

## Authentication Flow

### Hub Access Key Validation

```rust
// synap-server/src/auth/hub_auth.rs
pub struct HubAuthMiddleware {
    hub_client: Arc<HubClient>,
    key_cache: Arc<RwLock<HashMap<String, CachedKey>>>,
}

impl HubAuthMiddleware {
    pub async fn authenticate(&self, access_key: &str) -> Result<UserContext> {
        // Check cache first
        if let Some(cached) = self.key_cache.read().await.get(access_key) {
            if !cached.is_expired() {
                return Ok(cached.user_context.clone());
            }
        }
        
        // Validate via Hub SDK (access key validation endpoint)
        let validation = self.hub_client.validate_access_key(access_key).await?;
        
        let user_ctx = UserContext {
            user_id: validation.user_id,
            permissions: validation.permissions,
            plan: validation.plan,
        };
        
        // Cache for 60 seconds
        self.key_cache.write().await.insert(
            access_key.to_string(),
            CachedKey {
                user_context: user_ctx.clone(),
                expires_at: Instant::now() + Duration::from_secs(60),
            }
        );
        
        Ok(user_ctx)
    }
}
```

## Module Structure

```
synap-server/src/
├── hub/
│   ├── mod.rs              # Module exports
│   ├── client.rs           # HubClient wrapper around SDK
│   ├── naming.rs           # Resource naming utilities
│   ├── quota.rs            # Quota checking and enforcement
│   ├── usage.rs            # UsageReporter background task
│   ├── restrictions.rs     # SaaS restrictions (TTL, size limits)
│   └── rate_limiter.rs     # Per-user rate limiting
├── auth/
│   └── hub_auth.rs         # Hub authentication middleware
└── migration/
    └── hub_migration.rs    # Migration tool for existing resources
```

## Configuration

```yaml
hub:
  enabled: true  # false = standalone mode
  api_url: "${HIVEHUB_API_URL}"
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  usage_report_interval: 300  # seconds (5 minutes)
  cache_ttl: 60  # seconds for SDK response caching
  restrictions:
    enabled: true  # Apply SaaS restrictions
    ttl_required: true
    max_message_size_bytes: 1048576
    max_requests_per_second: 100
```

