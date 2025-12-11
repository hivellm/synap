# HiveHub.Cloud Integration Specification (Synap)

## ADDED Requirements

### Requirement: Hub Authentication
The system SHALL authenticate all user requests through HiveHub.Cloud access keys.

#### Scenario: Valid Hub access key
Given a request includes valid Hub-issued access key
When the request is processed
Then the system SHALL validate key with Hub and extract user_id

#### Scenario: Invalid access key
Given a request includes invalid access key
When the request is processed
Then the system SHALL return 401 Unauthorized

### Requirement: User-Scoped Resource Isolation
The system SHALL isolate all resources by user with naming convention.

#### Scenario: Create user-scoped queue
Given a user creates queue "tasks"
When the queue is created
Then the system SHALL name it "user_{user_id}:tasks" and enforce ownership

#### Scenario: Create user-scoped stream
Given a user creates stream "events"
When the stream is created
Then the system SHALL name it "user_{user_id}:events" and enforce ownership

#### Scenario: Create user-scoped key
Given a user sets key "config:app"
When the operation is executed
Then the system SHALL namespace it as "user_{user_id}:config:app"

#### Scenario: List user resources
Given a user requests their resources
When the request is processed
Then the system SHALL return only resources owned by that user

#### Scenario: Prevent cross-user access
Given a user attempts to access another user's resource
When the request is processed
Then the system SHALL return 404 Not Found

### Requirement: Quota Enforcement
The system SHALL enforce quotas for resources via Hub API.

#### Scenario: Create queue within quota
Given a user is within queue limit
When they create a queue
Then the system SHALL validate with Hub and create queue

#### Scenario: Exceed quota
Given a user has reached queue limit
When they attempt to create queue
Then the system SHALL return 429 Too Many Requests

#### Scenario: Exceed storage quota
Given a user has reached storage limit
When they attempt to add data
Then the system SHALL return 429 Too Many Requests

### Requirement: Usage Reporting
The system SHALL report usage metrics to Hub for billing.

#### Scenario: Report queue usage
Given messages are published to queue
When the operation completes
Then the system SHALL report message count and storage to Hub

#### Scenario: Report stream usage
Given messages are published to stream
When the operation completes
Then the system SHALL report partition and storage to Hub

#### Scenario: Report KV usage
Given key-value pairs are set
When the operation completes
Then the system SHALL report storage usage to Hub

#### Scenario: Periodic sync
Given the system is running
When usage interval elapses
Then the system SHALL sync all usage metrics to Hub

### Requirement: MCP Integration
The system SHALL integrate with Hub's MCP gateway for resource access.

#### Scenario: MCP queue operation
Given an MCP request includes user key
When queue operation is requested
Then the system SHALL filter queues to user's resources only

#### Scenario: MCP stream operation
Given an MCP request includes user key
When stream operation is requested
Then the system SHALL filter streams to user's resources only

#### Scenario: MCP KV operation
Given an MCP request includes user key
When KV operation is requested
Then the system SHALL scope keys to user's namespace only

### Requirement: Cluster Mode
The system SHALL support distributed operation with user resource routing.

#### Scenario: Cross-node request
Given a request routes to different node
When processed with user context
Then the system SHALL maintain user isolation and route correctly

