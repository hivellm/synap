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

MCP tools respect Synap's authentication system:

```rust
// With API key in state
let state = AppState {
    // ... setup with auth enabled
};

// Tools will use configured authentication
let result = handle_mcp_tool(request, state).await?;
```

**Environment Variables**:
- `SYNAP_API_KEY` - API key for authentication
- `SYNAP_URL` - Server URL (default: http://localhost:15500)

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

