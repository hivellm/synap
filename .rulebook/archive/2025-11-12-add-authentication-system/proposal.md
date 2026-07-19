# Proposal: Authentication & Authorization System

## Overview

Implement a comprehensive authentication and authorization system for Synap to secure access to all resources and operations. The system will support user management, API keys, fine-grained permissions, and protection for REST APIs, SDKs, and MCP.

## Motivation

Currently, Synap has basic authentication infrastructure but lacks:
- Root user management
- Comprehensive permission system
- REST API protection
- SDK authentication support
- MCP authentication
- Temporary API keys
- Key revocation

This proposal addresses production security requirements and enables fine-grained access control similar to RabbitMQ's permission model.

## Goals

1. **Root User Management**: Configurable root user at startup (Docker: root/root)
2. **User Management**: Create, grant access, revoke access, delete users
3. **API Key Management**: Generate, revoke, temporary keys with expiration
4. **Permission System**: Resource-based permissions (queue, stream, etc.)
5. **REST Protection**: All REST endpoints protected with authentication
6. **SDK Support**: All SDKs support authentication
7. **MCP Authentication**: MCP server validates authentication

## Design

### Root User

- Configurable via environment variables or config file
- Default Docker credentials: `root/root`
- Full permissions (all actions on all resources)
- Can be disabled after initial setup (security best practice)
- Cannot be deleted (protected user)

### User Management

- CRUD operations for users
- Role assignment/removal
- Enable/disable accounts
- Password management

### API Keys

- Generate keys with optional expiration
- Temporary keys (TTL support)
- Revoke keys (immediate invalidation)
- Key metadata (created_at, expires_at, last_used)

### Permission Model

RabbitMQ-style resource-based permissions:

- **Queue**: read, write, configure, delete
- **Stream/Chatroom**: read, write, configure, delete
- **Data Structures**: read, write, delete (KV, Hash, List, Set, SortedSet)
- **Pub/Sub**: publish, subscribe
- **Admin**: user management, system config

Wildcard support: `queue:*`, `stream:*`, etc.

### Authentication Methods

1. **Basic Auth**: Username/password
2. **Bearer Token**: API key in Authorization header
3. **Query Parameter**: `api_key` for GET requests

### REST API Protection

- Authentication middleware for all routes
- Permission checking middleware
- 401 Unauthorized for missing/invalid credentials
- 403 Forbidden for insufficient permissions

### SDK Support

All SDKs must support:
- API key authentication
- Basic Auth
- Automatic token handling

### MCP Authentication

- API key validation
- User context propagation
- Permission checks in MCP tools

## Implementation Plan

### Phase 1: Core Authentication (Week 1-2)
- Root user management
- User CRUD operations
- Basic permission checking

### Phase 2: API Key Management (Week 2-3)
- Key generation with expiration
- Key revocation
- Key metadata tracking

### Phase 3: Permission System (Week 3-4)
- Resource-based permissions
- Permission checking in handlers
- Wildcard support

### Phase 4: REST API Protection (Week 4-5)
- Authentication middleware
- Permission middleware
- Auth endpoints

### Phase 5: SDK Updates (Week 5-6)
- Add auth to all SDKs
- Update examples
- Documentation

### Phase 6: MCP Authentication (Week 6)
- MCP auth middleware
- Permission checks
- Examples

### Phase 7: Docker & Configuration (Week 6-7)
- Environment variables
- Config updates
- Documentation

## Configuration

```yaml
auth:
  enabled: true
  require_auth: true
  root:
    username: "root"
    password: "root"
    enabled: true
  default_key_ttl: 3600
```

## Security Considerations

- Password hashing (bcrypt, cost 12+)
- Secure API key generation
- Key storage (hashed)
- Rate limiting on auth endpoints
- IP filtering (optional)
- Audit logging
- Secure password requirements

## Backward Compatibility

- Authentication disabled by default
- Existing deployments continue to work
- Migration guide provided

## Success Criteria

- All REST endpoints protected
- SDKs support authentication
- MCP validates authentication
- Permission system functional
- Root user manageable
- API keys with expiration/revocation
- Comprehensive test coverage

