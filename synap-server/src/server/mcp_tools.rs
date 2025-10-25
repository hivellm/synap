use rmcp::model::{Tool, ToolAnnotations};
use serde_json::json;
use std::borrow::Cow;

/// Get essential MCP tools (reduced set for Cursor compatibility)
/// Only the most commonly used tools are exposed to stay within Cursor's limits
pub fn get_mcp_tools() -> Vec<Tool> {
    vec![
        // KV Store Tools (3 essential)
        Tool {
            name: Cow::Borrowed("synap_kv_get"),
            title: Some("Get Key-Value".to_string()),
            description: Some(Cow::Borrowed("Retrieve a value from the key-value store (returns string by default)")),
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
        // Hash Tools (3 essential)
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
        // List Tools (3 essential)
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
        // Set Tools (3 essential)
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
        // Queue Tools (1 essential)
        Tool {
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
        },
        // Sorted Set Tools (3 essential)
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

// Total MCP Tools: 16 (3 KV + 3 Hash + 3 List + 3 Set + 1 Queue + 3 Sorted Set)
//
// Note: Removed 8 less-frequently used tools to meet Cursor's MCP tool limits:
// - synap_kv_scan (use KV get with known keys instead)
// - synap_hash_del (use synap_kv_delete on the hash key)
// - synap_hash_incrby (use hash_set with calculated value)
// - synap_list_len (use list_range and count elements)
// - synap_list_rpoplpush (use list_pop + list_push manually)
// - synap_queue_consume (use REST API for consumption)
// - synap_stream_publish (use REST API for streams)
// - synap_pubsub_publish (use REST API for pub/sub)
//
// All removed functionality is still available via REST API and StreamableHTTP
