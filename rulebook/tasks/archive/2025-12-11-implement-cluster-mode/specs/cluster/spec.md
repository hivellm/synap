# Cluster Mode Specification

## ADDED Requirements

### Requirement: HiveHub SDK Integration
The system SHALL integrate with the `hivehub-cloud-internal-sdk` Rust crate to communicate with HiveHub.Cloud API for quota management, user information, and access key validation.

#### Scenario: SDK Client Initialization
Given cluster mode is enabled in configuration
When the server starts
Then the system SHALL initialize the HiveHub SDK client with service API key from configuration
And the SDK client SHALL be configured with base URL and timeout settings
And initialization failures SHALL prevent server startup with clear error messages

#### Scenario: SDK Dependency Management
Given the project uses Cargo for dependency management
When cluster mode is implemented
Then `hivehub-cloud-internal-sdk` SHALL be added as a dependency in `synap-server/Cargo.toml`
And the SDK version SHALL be specified to ensure compatibility

### Requirement: Multi-Tenant Data Isolation
The system SHALL isolate all data by user namespace when cluster mode is enabled, ensuring complete data segregation between users.

#### Scenario: Namespace Prefixing
Given cluster mode is enabled
When a user performs any data operation (KV set, queue publish, stream append)
Then all keys SHALL be prefixed with user namespace format `user:{user_id}:{original_key}`
And the user_id SHALL be extracted from the authenticated request context
And namespace prefixing SHALL apply to KV store, queues, streams, pub/sub topics, and all data structures

#### Scenario: Data Access Isolation
Given cluster mode is enabled
When a user requests data (GET, SCAN, LIST operations)
Then the system SHALL only return data belonging to that user's namespace
And users SHALL NOT be able to access data from other users' namespaces
And SCAN operations SHALL automatically filter by user namespace prefix

### Requirement: Quota Management via SDK
The system SHALL use the HiveHub SDK to query and enforce user quotas for storage limits and monthly usage.

#### Scenario: Quota Checking Before Write
Given cluster mode is enabled
When a user attempts to write data (SET, queue publish, stream append)
Then the system SHALL query user quota via SDK `get_user_resources()` or quota endpoints
And the system SHALL check if storage quota allows the operation
And if quota is exceeded, the system SHALL return quota exceeded error
And the operation SHALL be rejected before any data is written

#### Scenario: Usage Updates
Given cluster mode is enabled
When a user successfully writes or deletes data
Then the system SHALL update usage metrics via SDK `update_usage()` method
And usage updates SHALL include storage_bytes, message_count, or other relevant metrics
And usage updates SHALL be sent asynchronously to avoid blocking operations

### Requirement: Access Key Validation via SDK
The system SHALL validate access keys using the HiveHub SDK to verify ownership and permissions.

#### Scenario: Access Key Validation
Given cluster mode is enabled
When a request includes an access key (API key or Bearer token)
Then the system SHALL validate the key via SDK access key validation endpoints
And the system SHALL extract user_id and permissions from validated key
And invalid or expired keys SHALL result in 401 Unauthorized response

#### Scenario: Permission Checking
Given an access key is validated
When a user attempts to perform an operation
Then the system SHALL check if the key's permissions allow the operation
And function-level permissions SHALL be enforced (MCP-only, admin, read-only, full-access)
And operations not allowed by key permissions SHALL return 403 Forbidden

### Requirement: Mandatory Authentication in Cluster Mode
The system SHALL require authentication for all endpoints when cluster mode is enabled, with no anonymous access allowed.

#### Scenario: REST API Authentication
Given cluster mode is enabled
When a request is made to any REST API endpoint
Then authentication SHALL be required (API key or Basic Auth)
And unauthenticated requests SHALL return 401 Unauthorized
And authentication SHALL be validated before processing any request

#### Scenario: MCP Protocol Authentication
Given cluster mode is enabled
When a request is made to any MCP endpoint
Then authentication SHALL be required
And unauthenticated MCP requests SHALL return 401 Unauthorized
And MCP tools SHALL only be accessible with valid access keys

#### Scenario: WebSocket Authentication
Given cluster mode is enabled
When a WebSocket connection is established
Then authentication SHALL be required during connection handshake
And unauthenticated WebSocket connections SHALL be rejected
And authenticated connections SHALL maintain user context for all messages

### Requirement: Rate Limiting Per User
The system SHALL enforce rate limits per user (not just per IP) when cluster mode is enabled, using limits fetched from HiveHub.

#### Scenario: Per-User Rate Limiting
Given cluster mode is enabled
When a user makes requests
Then rate limiting SHALL be applied per user_id (extracted from auth context)
And rate limits SHALL be fetched from HiveHub via SDK if available
And rate limit exceeded SHALL return 429 Too Many Requests
And rate limit headers SHALL be included in responses

### Requirement: Configuration for Cluster Mode
The system SHALL provide configuration options to enable cluster mode and configure HiveHub SDK connection.

#### Scenario: Cluster Mode Configuration
Given a configuration file
When cluster mode is configured
Then `cluster_mode: true` SHALL enable cluster mode
And `hivehub.base_url` SHALL specify HiveHub API base URL
And `hivehub.service_api_key` SHALL specify service API key for SDK authentication
And `hivehub.timeout` SHALL specify SDK request timeout
And configuration SHALL support environment variables for sensitive values

#### Scenario: Standalone Mode Compatibility
Given cluster mode is disabled (default)
When the server operates
Then all existing functionality SHALL work without changes
And no namespace prefixing SHALL occur
And authentication SHALL be optional (based on existing auth configuration)
And backward compatibility SHALL be maintained

