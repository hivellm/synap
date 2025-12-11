## 1. HiveHub SDK Integration Phase
- [x] 1.1 Add `hivehub-cloud-internal-sdk` dependency to `synap-server/Cargo.toml`
- [x] 1.2 Create HiveHub client wrapper module (`synap-server/src/hub/client.rs`)
- [x] 1.3 Initialize SDK client with service API key from configuration
- [x] 1.4 Implement quota fetching using `client.synap().get_user_resources()` and quota endpoints
- [x] 1.5 Implement rate limit configuration fetching via SDK
- [x] 1.6 Implement access key validation using SDK `validate_resource()` methods
- [x] 1.7 Add caching layer for SDK responses (quota, user info)
- [x] 1.8 Add configuration for HiveHub service API key and base URL
- [x] 1.9 Handle SDK errors and convert to Synap errors

## 2. Access Key System Phase
- [x] 2.1 Extend API key manager to support HiveHub-generated keys
- [x] 2.2 Implement function-level permission checking
- [x] 2.3 Add permission sets (MCP-only, admin, read-only, full-access)
- [x] 2.4 Create access key validation middleware
- [x] 2.5 Implement key-to-user mapping cache
- [x] 2.6 Add key metadata storage (user_id, permissions, created_at)

## 3. Multi-Tenant Data Isolation Phase
- [x] 3.1 Create namespace manager for user isolation
- [x] 3.2 Modify KV store to prefix all keys with user namespace
- [x] 3.3 Modify queue system to isolate queues by user namespace
- [x] 3.4 Modify stream system to isolate streams by user namespace
- [x] 3.5 Modify pub/sub system to isolate topics by user namespace
- [x] 3.6 Update all data structure operations (Hash, List, Set, etc.) for namespace isolation
- [x] 3.7 Ensure SCAN operations only return keys for authenticated user

## 4. Quota Management Phase
- [x] 4.1 Create quota tracker per user (storage usage, monthly limits)
- [x] 4.2 Implement quota checking before write operations using SDK `get_user_resources()` or quota endpoints
- [x] 4.3 Add quota exceeded error responses
- [x] 4.4 Implement quota usage tracking (increment on write, decrement on delete)
- [x] 4.5 Update usage via SDK `update_usage()` method after operations
- [x] 4.6 Add periodic quota sync with HiveHub API via SDK
- [x] 4.7 Create quota statistics endpoint per user

## 5. Rate Limiting Enhancement Phase
- [x] 5.1 Extend rate limiter to support per-user limits (not just per-IP)
- [x] 5.2 Fetch user-specific rate limits from HiveHub via SDK (if available in user info/quota)
- [x] 5.3 Implement rate limit buckets per user
- [x] 5.4 Add rate limit headers to responses
- [x] 5.5 Update rate limit middleware to use user ID from auth context

## 6. Mandatory Authentication Phase
- [x] 6.1 Add cluster mode check to authentication middleware
- [x] 6.2 Enforce authentication on all REST API endpoints in cluster mode
- [x] 6.3 Enforce authentication on all MCP endpoints in cluster mode
- [x] 6.4 Enforce authentication on all WebSocket connections in cluster mode
- [x] 6.5 Remove anonymous access option in cluster mode
- [x] 6.6 Add proper error messages for unauthenticated requests
- [x] 6.7 Update all route handlers to extract user_id from auth context

## 7. Configuration Phase
- [x] 7.1 Add `cluster_mode` boolean flag to config
- [x] 7.2 Add HiveHub SDK configuration section (base_url, service_api_key, timeout)
- [x] 7.3 Add cluster mode validation on startup (verify SDK client can connect)
- [x] 7.4 Ensure backward compatibility with standalone mode
- [x] 7.5 Add configuration examples for cluster mode
- [x] 7.6 Add environment variable support for HiveHub service API key

## 8. Testing Phase
- [x] 8.1 Write unit tests for HiveHub SDK client wrapper (mock SDK responses)
- [x] 8.2 Write unit tests for namespace isolation
- [x] 8.3 Write unit tests for quota enforcement (using mocked SDK)
- [x] 8.4 Write unit tests for access key permissions
- [ ] 8.5 Write integration tests for multi-tenant isolation (DEFERRED - requires Hub API)
- [ ] 8.6 Write integration tests for quota limits (DEFERRED - requires Hub API)
- [ ] 8.7 Write integration tests for mandatory authentication (DEFERRED - requires Hub API)
- [x] 8.8 Test backward compatibility with standalone mode
- [x] 8.9 Verify test coverage ≥ 80%

## 9. Documentation Phase
- [x] 9.1 Create `docs/specs/CLUSTER_MODE.md` specification (HUB_INTEGRATION.md)
- [x] 9.2 Create `docs/specs/QUOTA_MANAGEMENT.md` specification
- [x] 9.3 Create `docs/specs/ACCESS_KEYS.md` specification
- [x] 9.4 Update `docs/AUTHENTICATION.md` with cluster mode details
- [x] 9.5 Create user guide for cluster mode configuration
- [x] 9.6 Update CHANGELOG.md
- [x] 9.7 Add API documentation for quota endpoints
