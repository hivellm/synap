# Proposal: Implement Cluster Mode with HiveHub Integration

## Why

The Synap server currently operates in standalone mode where each instance manages its own data without multi-tenant isolation. To enable shared infrastructure deployment through HiveHub, the system needs to support cluster mode where multiple users can share the same server instance with complete data isolation, quota management, and access control. This mode is essential for providing Synap as a managed service where the HiveHub platform controls user quotas, rate limits, and access permissions. Without cluster mode, each user would require a dedicated server instance, making it economically unfeasible for a shared service model.

## What Changes

This task will implement a comprehensive cluster mode that transforms Synap into a multi-tenant system:

1. **HiveHub API Integration**
   - Integration with `hivehub-cloud-internal-sdk` Rust crate (provided by HiveHub Cloud project)
   - Use SDK for all HiveHub API communication (quota queries, user info, access key validation)
   - Quota management (storage limits, monthly usage tracking via SDK)
   - Rate limit configuration per user (fetched via SDK)
   - Access key management and validation (via SDK endpoints)

2. **Multi-Tenant Data Isolation**
   - Namespace-based data segmentation by user ID
   - All keys prefixed with user namespace (e.g., `user:{user_id}:{key}`)
   - Complete isolation of KV store, queues, streams, pub/sub per user
   - Per-user statistics and monitoring

3. **Access Key System**
   - HiveHub-generated access keys with granular permissions
   - Function-level access control (MCP-only keys, admin keys, read-only keys)
   - Key-to-user mapping and validation
   - Automatic key rotation and expiration support

4. **Mandatory Authentication**
   - All REST API endpoints require authentication in cluster mode
   - All MCP endpoints require authentication
   - All WebSocket connections require authentication
   - No anonymous access allowed in cluster mode
   - Proper error handling for unauthenticated requests

5. **Quota Enforcement**
   - Storage quota limits per user (monthly limits)
   - Real-time quota tracking and enforcement
   - Rate limiting per user (not just per IP)
   - Quota exceeded error responses

6. **Configuration**
   - New `cluster_mode` configuration flag
   - HiveHub API endpoint configuration
   - Cluster mode vs standalone mode operation
   - Backward compatibility with existing standalone deployments

## Impact

- **Affected specs**: 
  - `docs/specs/AUTHENTICATION.md` - Extend with cluster mode auth requirements
  - `docs/specs/CLUSTER_MODE.md` - New specification for cluster mode
  - `docs/specs/QUOTA_MANAGEMENT.md` - New specification for quota system
  - `docs/specs/ACCESS_KEYS.md` - New specification for access key system

- **Affected code**: 
  - `synap-server/Cargo.toml` - Add `hivehub-cloud-internal-sdk` dependency
  - `synap-server/src/cluster/` - New module for cluster mode (uses SDK)
  - `synap-server/src/cluster/hivehub_client.rs` - Wrapper around SDK client
  - `synap-server/src/auth/` - Extend authentication for cluster mode
  - `synap-server/src/core/kv_store.rs` - Add namespace prefixing
  - `synap-server/src/core/queue.rs` - Add namespace isolation
  - `synap-server/src/core/stream.rs` - Add namespace isolation
  - `synap-server/src/core/pubsub.rs` - Add namespace isolation
  - `synap-server/src/server/router.rs` - Enforce authentication in cluster mode
  - `synap-server/src/config.rs` - Add cluster mode configuration

- **Breaking change**: NO (cluster mode is opt-in via configuration)

- **User benefit**: 
  - Enables shared infrastructure deployment
  - Cost-effective multi-tenant operation
  - Centralized quota and access management through HiveHub
  - Secure data isolation between users
  - Granular access control for different use cases (MCP, admin, read-only)
