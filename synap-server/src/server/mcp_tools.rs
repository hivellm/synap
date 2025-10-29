use rmcp::model::{Tool, ToolAnnotations};
use serde_json::json;
use std::borrow::Cow;

use crate::config::McpConfig;

/// Get MCP tools based on configuration
/// Tools can be selectively enabled/disabled to stay within Cursor's MCP tool limits
pub fn get_mcp_tools(config: &McpConfig) -> Vec<Tool> {
    let mut tools = Vec::new();

    // KV Store Tools (9 total: 3 essential + 3 string extensions + 3 key management)
    if config.enable_kv_tools {
        tools.extend(get_kv_tools());
    }

    // Hash Tools (3)
    if config.enable_hash_tools {
        tools.extend(get_hash_tools());
    }

    // List Tools (3)
    if config.enable_list_tools {
        tools.extend(get_list_tools());
    }

    // Set Tools (3)
    if config.enable_set_tools {
        tools.extend(get_set_tools());
    }

    // Queue Tools (1)
    if config.enable_queue_tools {
        tools.extend(get_queue_tools());
    }

    // Sorted Set Tools (3)
    if config.enable_sortedset_tools {
        tools.extend(get_sortedset_tools());
    }

    tools
}

fn get_kv_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: Cow::Borrowed("synap_kv_get"),
            title: Some("Get Key-Value".to_string()),
            description: Some(Cow::Borrowed(
                "Retrieve a value from the key-value store (returns string by default)",
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The key to retrieve"
                    },
                    "type": {
                        "type": "string",
                        "description": "Return type: 'string' (default) or 'bytes'",
                        "enum": ["string", "bytes"],
                        "default": "string"
                    }
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        Tool {
            name: Cow::Borrowed("synap_kv_set"),
            title: Some("Set Key-Value".to_string()),
            description: Some(Cow::Borrowed("Store a value in the key-value store")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Key to store"},
                    "value": {"type": "string", "description": "Value to store"},
                    "ttl": {"type": "integer", "description": "Time to live in seconds (optional)"}
                },
                "required": ["key", "value"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        Tool {
            name: Cow::Borrowed("synap_kv_delete"),
            title: Some("Delete Key".to_string()),
            description: Some(Cow::Borrowed("Delete a key from the store")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string"}
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        // String Extension tools (3)
        Tool {
            name: Cow::Borrowed("synap_kv_append"),
            title: Some("Append to String".to_string()),
            description: Some(Cow::Borrowed(
                "Append bytes to an existing value, or create new key with value if it doesn't exist",
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The key to append to"
                    },
                    "value": {
                        "type": "string",
                        "description": "The value to append"
                    }
                },
                "required": ["key", "value"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        Tool {
            name: Cow::Borrowed("synap_kv_getrange"),
            title: Some("Get String Range".to_string()),
            description: Some(Cow::Borrowed(
                "Get substring using Redis-style negative indices. start and end are inclusive. Negative indices count from the end (-1 = last byte)",
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The key to get range from"
                    },
                    "start": {
                        "type": "integer",
                        "description": "Start index (inclusive). Negative indices count from end"
                    },
                    "end": {
                        "type": "integer",
                        "description": "End index (inclusive). Negative indices count from end"
                    }
                },
                "required": ["key", "start", "end"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        Tool {
            name: Cow::Borrowed("synap_kv_strlen"),
            title: Some("Get String Length".to_string()),
            description: Some(Cow::Borrowed("Get the length of the string value in bytes")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The key to get length for"
                    }
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        // Key Management tools (3)
        Tool {
            name: Cow::Borrowed("synap_key_type"),
            title: Some("Get Key Type".to_string()),
            description: Some(Cow::Borrowed(
                "Get the type of a key across all stores (string, hash, list, set, zset, none)",
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The key to get type for"
                    }
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        Tool {
            name: Cow::Borrowed("synap_key_exists"),
            title: Some("Check Key Exists".to_string()),
            description: Some(Cow::Borrowed(
                "Check if a key exists in any store (KV, Hash, List, Set, SortedSet)",
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The key to check"
                    }
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        Tool {
            name: Cow::Borrowed("synap_key_rename"),
            title: Some("Rename Key".to_string()),
            description: Some(Cow::Borrowed(
                "Rename a key atomically, overwriting destination if it exists. Works across all data types.",
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source key name"
                    },
                    "destination": {
                        "type": "string",
                        "description": "Destination key name"
                    }
                },
                "required": ["source", "destination"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
    ]
}

fn get_hash_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: Cow::Borrowed("synap_hash_set"),
            title: Some("Set Hash Field".to_string()),
            description: Some(Cow::Borrowed("Set a field value in a hash")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Hash key"},
                    "field": {"type": "string", "description": "Field name"},
                    "value": {"description": "Value to store (any JSON type)"}
                },
                "required": ["key", "field", "value"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        Tool {
            name: Cow::Borrowed("synap_hash_get"),
            title: Some("Get Hash Field".to_string()),
            description: Some(Cow::Borrowed("Retrieve a field value from a hash")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Hash key"},
                    "field": {"type": "string", "description": "Field name"}
                },
                "required": ["key", "field"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        Tool {
            name: Cow::Borrowed("synap_hash_getall"),
            title: Some("Get All Hash Fields".to_string()),
            description: Some(Cow::Borrowed("Retrieve all field-value pairs from a hash")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Hash key"}
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
    ]
}

fn get_list_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: Cow::Borrowed("synap_list_push"),
            title: Some("Push to List".to_string()),
            description: Some(Cow::Borrowed("Push element(s) to left (LPUSH) or right (RPUSH) of a list")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "List key"},
                    "values": {"type": "array", "items": {}, "description": "Values to push (any JSON types)"},
                    "direction": {"type": "string", "enum": ["left", "right"], "default": "right", "description": "Push to left (front) or right (back)"},
                    "only_if_exists": {"type": "boolean", "default": false, "description": "Only push if list already exists (LPUSHX/RPUSHX)"}
                },
                "required": ["key", "values"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        Tool {
            name: Cow::Borrowed("synap_list_pop"),
            title: Some("Pop from List".to_string()),
            description: Some(Cow::Borrowed("Pop element(s) from left (LPOP) or right (RPOP) of a list")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "List key"},
                    "direction": {"type": "string", "enum": ["left", "right"], "default": "left", "description": "Pop from left (front) or right (back)"},
                    "count": {"type": "integer", "minimum": 1, "default": 1, "description": "Number of elements to pop"}
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        Tool {
            name: Cow::Borrowed("synap_list_range"),
            title: Some("Get List Range".to_string()),
            description: Some(Cow::Borrowed("Get a range of elements from a list (LRANGE)")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "List key"},
                    "start": {"type": "integer", "default": 0, "description": "Start index (supports negative indices)"},
                    "stop": {"type": "integer", "default": -1, "description": "Stop index (supports negative indices, -1 = last element)"}
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
    ]
}

fn get_set_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: Cow::Borrowed("synap_set_add"),
            title: Some("Add Set Members".to_string()),
            description: Some(Cow::Borrowed("Add one or more members to a set")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Set key"},
                    "members": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Members to add"
                    }
                },
                "required": ["key", "members"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        Tool {
            name: Cow::Borrowed("synap_set_members"),
            title: Some("Get Set Members".to_string()),
            description: Some(Cow::Borrowed("Get all members of a set")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Set key"}
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        Tool {
            name: Cow::Borrowed("synap_set_inter"),
            title: Some("Set Intersection".to_string()),
            description: Some(Cow::Borrowed("Compute intersection of multiple sets")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "keys": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Set keys to intersect"
                    }
                },
                "required": ["keys"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
    ]
}

fn get_queue_tools() -> Vec<Tool> {
    vec![Tool {
        name: Cow::Borrowed("synap_queue_publish"),
        title: Some("Publish to Queue".to_string()),
        description: Some(Cow::Borrowed("Publish a message to a queue")),
        input_schema: json!({
            "type": "object",
            "properties": {
                "queue": {"type": "string"},
                "message": {"type": "string"},
                "priority": {"type": "integer", "minimum": 0, "maximum": 9, "default": 5}
            },
            "required": ["queue", "message"]
        })
        .as_object()
        .unwrap()
        .clone()
        .into(),
        output_schema: None,
        icons: None,
        annotations: Some(ToolAnnotations::new().read_only(false)),
    }]
}

fn get_sortedset_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: Cow::Borrowed("synap_sortedset_zadd"),
            title: Some("Add to Sorted Set".to_string()),
            description: Some(Cow::Borrowed("Add member with score to sorted set")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Sorted set key"},
                    "member": {"type": "string", "description": "Member to add"},
                    "score": {"type": "number", "description": "Score value"}
                },
                "required": ["key", "member", "score"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        Tool {
            name: Cow::Borrowed("synap_sortedset_zrange"),
            title: Some("Get Sorted Set Range".to_string()),
            description: Some(Cow::Borrowed("Get range of members by rank (0-based index)")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Sorted set key"},
                    "start": {"type": "integer", "default": 0, "description": "Start index (supports negative)"},
                    "stop": {"type": "integer", "default": -1, "description": "Stop index (supports negative, -1 = last)"},
                    "withscores": {"type": "boolean", "default": true, "description": "Include scores in output"}
                },
                "required": ["key"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        Tool {
            name: Cow::Borrowed("synap_sortedset_zrank"),
            title: Some("Get Sorted Set Rank".to_string()),
            description: Some(Cow::Borrowed("Get rank of member in sorted set (0-based, lowest score = rank 0)")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Sorted set key"},
                    "member": {"type": "string", "description": "Member to find rank for"}
                },
                "required": ["key", "member"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
    ]
}

// MCP Tools Configuration
//
// Available tool categories (can be enabled/disabled in config.yml):
// - KV Tools (3): synap_kv_get, synap_kv_set, synap_kv_delete
// - Hash Tools (3): synap_hash_get, synap_hash_set, synap_hash_getall
// - List Tools (3): synap_list_push, synap_list_pop, synap_list_range
// - Set Tools (3): synap_set_add, synap_set_members, synap_set_inter
// - Queue Tools (1): synap_queue_publish
// - Sorted Set Tools (3): synap_sortedset_zadd, synap_sortedset_zrange, synap_sortedset_zrank
//
// Default enabled (4 tools): KV (3) + Queue (1)
// Maximum tools: 16 (if all categories enabled)
//
// Cursor MCP Limit Considerations:
// - Cursor has a limit on the number of MCP tools it can handle efficiently
// - Default configuration keeps only essential tools enabled (KV + Queue)
// - Additional tools can be enabled as needed in config.yml under mcp section
//
// Note: All functionality is also available via REST API and StreamableHTTP,
// so disabling MCP tools doesn't reduce available features
