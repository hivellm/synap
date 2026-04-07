# MCP (Model Context Protocol) Usage Guide

## 📖 Overview

Synap implements the **Model Context Protocol (MCP)** to enable AI assistants and language models to interact with the Synap server using structured tools.

**MCP Library**: Uses `rmcp` (Rust MCP implementation)

## 🛠️ Available Tools

Synap exposes **8 MCP tools** covering all core subsystems:

### 🔑 Key-Value Store Tools (4)

1. **`synap_kv_get`** - Retrieve a value
   - Input: `{ "key": "string" }`
   - Output: `{ "value": "..." }`
   - Read-only, Idempotent

2. **`synap_kv_set`** - Store a value
   - Input: `{ "key": "string", "value": "string", "ttl": number? }`
   - Output: `{ "success": true }`

3. **`synap_kv_delete`** - Delete a key
   - Input: `{ "key": "string" }`
   - Output: `{ "deleted": boolean }`

4. **`synap_kv_scan`** - Scan keys by prefix
   - Input: `{ "prefix": "string"?, "limit": number? }`
   - Output: `{ "keys": ["key1", "key2", ...] }`
   - Read-only, Idempotent

### 📨 Message Queue Tools (2)

5. **`synap_queue_publish`** - Publish message to queue
   - Input: `{ "queue": "string", "message": "string", "priority": number? }`
   - Output: `{ "message_id": "..." }`

6. **`synap_queue_consume`** - Consume message from queue
   - Input: `{ "queue": "string", "consumer_id": "string" }`
   - Output: `{ "message_id": "...", "payload": [...], "priority": number }`

### 📡 Event Stream Tools (1)

7. **`synap_stream_publish`** - Publish event to stream
   - Input: `{ "room": "string", "event": "string", "data": object }`
   - Output: `{ "offset": number }`

### 🔔 Pub/Sub Tools (1)

8. **`synap_pubsub_publish`** - Publish to topic
   - Input: `{ "topic": "string", "message": object }`
   - Output: `{ "message_id": "...", "subscribers_matched": number }`

## 🚀 Using MCP with AI Assistants

### Claude Desktop Integration

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "synap": {
      "command": "synap-mcp-server",
      "args": ["--url", "http://localhost:15500"],
      "env": {
        "SYNAP_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

### Cursor/VSCode Integration

Create `.cursorrules` or `.vscode/settings.json`:

```json
{
  "mcp.servers": {
    "synap": {
      "command": "synap-mcp-server",
      "args": ["--url", "http://localhost:15500"]
    }
  }
}
```

## 📝 Example Usage

### Programmatic Access (Rust)

```rust
use rmcp::model::CallToolRequestParam;
use serde_json::json;
use synap_server::{AppState, ScriptManager, handle_mcp_tool};

// Setup state
let state = Arc::new(AppState {
    kv_store: Arc::new(KVStore::new(config.to_kv_config())),
    queue_manager: Some(Arc::new(QueueManager::new(QueueConfig::default()))),
    stream_manager: Some(Arc::new(StreamManager::new(StreamConfig::default()))),
    partition_manager: None,
    consumer_group_manager: None,
    pubsub_router: Some(Arc::new(PubSubRouter::new())),
    persistence: None,
    script_manager: Arc::new(ScriptManager::default()),
    // ... initialize monitoring and transaction managers as needed
});

// Call MCP tool
let request = CallToolRequestParam {
    name: "synap_kv_set".into(),
    arguments: json!({
        "key": "user:123",
        "value": "John Doe",
        "ttl": 3600
    }).as_object().cloned(),
};

let result = handle_mcp_tool(request, state).await?;
```

### Via AI Assistant (Natural Language)

**User**: "Store user:123 with value 'John Doe' in Synap"

**Assistant (using MCP)**:
```json
Tool: synap_kv_set
Arguments: {
  "key": "user:123",
  "value": "John Doe",
  "ttl": 3600
}
Result: {"success": true}
```

**User**: "Publish a message to the tasks queue with high priority"

**Assistant (using MCP)**:
```json
Tool: synap_queue_publish
Arguments: {
  "queue": "tasks",
  "message": "Process video #42",
  "priority": 9
}
Result: {"message_id": "msg_abc123"}
```

## 🧪 Testing

Run MCP tests:

```bash
# All MCP tests
cargo test --test mcp_tests

# Specific test
cargo test --test mcp_tests test_mcp_kv_get

# With output
cargo test --test mcp_tests -- --nocapture
```

**Test Coverage**: 5 tests covering all tool types

## 🔒 Authentication

MCP requests support **full authentication** via HTTP headers, with permission checks for all operations.

### Authentication Methods

#### 1. Basic Auth (Username/Password)

```bash
# Via curl
curl -u username:password \
  -X POST http://localhost:15500/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "method": "tools/call",
    "params": {
      "name": "synap_kv_get",
      "arguments": {"key": "test"}
    }
  }'
```

#### 2. Bearer Token (API Key)

```bash
# Via curl
curl -X POST http://localhost:15500/mcp \
  -H "Authorization: Bearer sk_XXXXX..." \
  -H "Content-Type: application/json" \
  -d '{
    "method": "tools/call",
    "params": {
      "name": "synap_kv_get",
      "arguments": {"key": "test"}
    }
  }'
```

### Permission Checks

All MCP operations verify permissions before execution:

- **KV Operations**: Requires `kv:*` permissions
  - `synap_kv_get` - Requires `Read` permission
  - `synap_kv_set` - Requires `Write` permission
  - `synap_kv_delete` - Requires `Delete` permission

- **Queue Operations**: Requires `queue:*` permissions
  - `synap_queue_publish` - Requires `Write` permission

- **Hash Operations**: Requires `hash:*` permissions
  - `synap_hash_get` - Requires `Read` permission
  - `synap_hash_set` - Requires `Write` permission

- **List Operations**: Requires `list:*` permissions
  - `synap_list_push` - Requires `Write` permission
  - `synap_list_pop` - Requires `Write` permission
  - `synap_list_range` - Requires `Read` permission

- **Set Operations**: Requires `set:*` permissions
  - `synap_set_add` - Requires `Write` permission
  - `synap_set_members` - Requires `Read` permission

- **Admin Users**: Bypass all permission checks (full access)

### Error Responses

When authentication or authorization fails:

```json
{
  "error": {
    "code": -32603,
    "message": "Insufficient permissions for resource: kv:test_key, action: write"
  }
}
```

**HTTP Status Codes**:
- `401 Unauthorized` - Missing or invalid credentials
- `403 Forbidden` - Valid credentials but insufficient permissions (handled in tool response)

### Configuration

Enable authentication in `config.yml`:

```yaml
auth:
  enabled: true
  require_auth: true  # Require auth for MCP requests
  root:
    username: "root"
    password: "root"
    enabled: true
```

**Environment Variables**:
- `SYNAP_AUTH_ENABLED=true` - Enable authentication
- `SYNAP_AUTH_REQUIRE_AUTH=true` - Require auth for all requests
- `SYNAP_AUTH_ROOT_USERNAME=root` - Root username
- `SYNAP_AUTH_ROOT_PASSWORD=root` - Root password

### Example: Authenticated MCP Request

```bash
# 1. Create API key (via REST API)
curl -u root:root \
  -X POST http://localhost:15500/auth/keys \
  -H "Content-Type: application/json" \
  -d '{
    "name": "mcp-readonly-key",
    "permissions": [
      {"resource": "kv:*", "actions": ["read"]}
    ]
  }'

# Response: {"id": "...", "key": "sk_XXXXX..."}

# 2. Use API key for MCP request
curl -X POST http://localhost:15500/mcp \
  -H "Authorization: Bearer sk_XXXXX..." \
  -H "Content-Type: application/json" \
  -d '{
    "method": "tools/call",
    "params": {
      "name": "synap_kv_get",
      "arguments": {"key": "test"}
    }
  }'

# 3. Write operation will fail (read-only key)
curl -X POST http://localhost:15500/mcp \
  -H "Authorization: Bearer sk_XXXXX..." \
  -H "Content-Type: application/json" \
  -d '{
    "method": "tools/call",
    "params": {
      "name": "synap_kv_set",
      "arguments": {"key": "test", "value": "data"}
    }
  }'

# Response: Error - Insufficient permissions
```

### Anonymous Access

When `auth.require_auth=false`, MCP requests can be made without authentication:

```bash
# No auth header required
curl -X POST http://localhost:15500/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "method": "tools/call",
    "params": {
      "name": "synap_kv_get",
      "arguments": {"key": "test"}
    }
  }'
```

**Note**: Anonymous access is only available when authentication is disabled or `require_auth=false`. In production, always enable `require_auth=true`.

## 📊 Tool Annotations

All tools include MCP annotations:

- **Read-only**: Tools that don't modify state (`kv_get`, `kv_scan`)
- **Idempotent**: Safe to retry (`kv_get`, `kv_scan`)
- **Non-idempotent**: May have side effects if retried (all write operations)

## 🔄 Error Handling

MCP tools return standard MCP errors:

```rust
// Invalid params
Err(ErrorData::invalid_params("Missing key", None))

// Internal error
Err(ErrorData::internal_error("Set failed: ...", None))
```

Error codes:
- `-32602`: Invalid params
- `-32603`: Internal error

## 📚 Implementation Files

- **Tools Definition**: `synap-server/src/server/mcp_tools.rs`
- **Handlers**: `synap-server/src/server/mcp_handlers.rs`
- **Tests**: `synap-server/tests/mcp_tests.rs`
- **Exports**: `synap-server/src/lib.rs` (public API)

## 🚧 Future Enhancements

Planned MCP features:

- [ ] **Kafka-style tools**: Partitioned topic operations
- [ ] **Consumer group tools**: Join, assign, commit offset
- [ ] **Batch operations**: Multi-get, multi-set via MCP
- [ ] **Streaming results**: For large scan operations
- [ ] **MCP server binary**: Standalone server for stdio transport
- [ ] **Prompts**: Pre-defined interaction patterns
- [ ] **Resources**: Expose Synap stats as MCP resources

## 📖 References

- **MCP Specification**: https://spec.modelcontextprotocol.io/
- **rmcp Crate**: https://crates.io/crates/rmcp
- **Synap API Docs**: [../api/REST_API.md](../api/REST_API.md)

## ✅ Status

**Current State**: ✅ **PRODUCTION READY**

- 8 tools implemented and tested
- 5 comprehensive tests passing
- Error handling complete
- Authentication integrated
- Tool annotations included
- Documentation complete

**Version**: 0.3.0-rc  
**Last Updated**: October 22, 2025

