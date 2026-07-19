# Proposal: HiveHub.Cloud Integration

## Why

Synap needs to integrate with HiveHub.Cloud to operate as a managed multi-tenant messaging and storage service. Currently, Synap operates in standalone mode without multi-tenant isolation, local authentication only, no quota management, and no usage tracking. This integration enables Synap to be deployed as a shared SaaS service where multiple users can securely share the same server instance with complete data isolation, centralized authentication via HiveHub access keys, quota management based on subscription plans, and automatic usage tracking for billing purposes.

## What Changes

This task will implement comprehensive HiveHub.Cloud integration:

1. **Internal SDK Integration**: Add `hivehub-cloud-internal-sdk` dependency and create wrapper module
2. **Authentication Layer**: Hub access key validation middleware with UserContext propagation
3. **User-Scoped Naming**: Implement `user_{user_id}:{resource_name}` naming convention for all resources
4. **Multi-Tenancy**: Complete user isolation for queues, streams, KV stores, pub/sub topics
5. **Quota Management**: Resource limits enforcement via Hub SDK before operations
6. **Usage Tracking**: Background reporting task that syncs usage metrics every 5 minutes
7. **Cluster Support**: User-aware routing and distributed quota management
8. **Migration Tool**: CLI tool to migrate existing resources to user-scoped naming

**Configuration**:
```yaml
hub:
  enabled: true  # false = standalone mode
  api_url: "${HIVEHUB_API_URL}"
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  usage_report_interval: 300  # seconds
```

## Impact

- **Affected specs**: 
  - `docs/specs/HUB_INTEGRATION.md` - New specification for Hub integration
  - `docs/specs/QUOTA_MANAGEMENT.md` - New specification for quota system
  - `docs/specs/ACCESS_KEYS.md` - New specification for access key system
  - `docs/AUTHENTICATION.md` - Extend with Hub authentication details

- **Affected code**: 
  - `synap-server/Cargo.toml` - Add `hivehub-cloud-internal-sdk` dependency
  - `synap-server/src/hub/` - New module (client, quota, usage, naming)
  - `synap-server/src/auth/hub_auth.rs` - Hub authentication middleware
  - `synap-server/src/migration/hub_migration.rs` - Migration tool
  - `synap-server/src/core/{queue,stream,kv_store}.rs` - Add namespace isolation
  - `synap-server/src/auth/middleware.rs` - Extend for Hub auth
  - `synap-server/src/cluster/topology.rs` - User-aware routing
  - `synap-server/src/config.rs` - Add Hub configuration

- **Breaking change**: YES (requires resource migration or standalone mode)

- **User benefit**: 
  - Enables Synap as managed SaaS service
  - Multi-tenant isolation with complete data segregation
  - Centralized authentication and access control
  - Quota enforcement based on subscription plans
  - Automatic usage tracking for billing
  - Standalone mode preserved for self-hosted deployments
