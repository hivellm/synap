use rmcp::model::{Tool, ToolAnnotations};
use serde_json::json;
use std::borrow::Cow;

pub fn get_mcp_tools() -> Vec<Tool> {
    vec![
        // KV Store Tools
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
        Tool {
            name: Cow::Borrowed("synap_kv_scan"),
            title: Some("Scan Keys by Prefix".to_string()),
            description: Some(Cow::Borrowed(
                "Scan keys by prefix pattern for efficient bulk retrieval",
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prefix": {"type": "string", "description": "Prefix to match"},
                    "limit": {"type": "integer", "description": "Maximum keys to return", "default": 100}
                }
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(true).idempotent(true)),
        },
        // Hash Tools
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
        Tool {
            name: Cow::Borrowed("synap_hash_del"),
            title: Some("Delete Hash Fields".to_string()),
            description: Some(Cow::Borrowed("Delete one or more fields from a hash")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Hash key"},
                    "fields": {"type": "array", "items": {"type": "string"}, "description": "Field names to delete"}
                },
                "required": ["key", "fields"]
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
            name: Cow::Borrowed("synap_hash_incrby"),
            title: Some("Increment Hash Field".to_string()),
            description: Some(Cow::Borrowed("Atomically increment a hash field by an integer")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Hash key"},
                    "field": {"type": "string", "description": "Field name"},
                    "increment": {"type": "integer", "description": "Amount to increment (can be negative)"}
                },
                "required": ["key", "field", "increment"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        // List Tools
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
        Tool {
            name: Cow::Borrowed("synap_list_len"),
            title: Some("Get List Length".to_string()),
            description: Some(Cow::Borrowed("Get the number of elements in a list (LLEN)")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "List key"}
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
            name: Cow::Borrowed("synap_list_rpoplpush"),
            title: Some("Atomic List Move".to_string()),
            description: Some(Cow::Borrowed("Atomically pop from source list and push to destination list (RPOPLPUSH)")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "Source list key"},
                    "destination": {"type": "string", "description": "Destination list key"}
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
        // Queue Tools
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
        Tool {
            name: Cow::Borrowed("synap_queue_consume"),
            title: Some("Consume from Queue".to_string()),
            description: Some(Cow::Borrowed("Consume a message from a queue")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "queue": {"type": "string"},
                    "consumer_id": {"type": "string"}
                },
                "required": ["queue", "consumer_id"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        // Stream Tools
        Tool {
            name: Cow::Borrowed("synap_stream_publish"),
            title: Some("Publish to Stream".to_string()),
            description: Some(Cow::Borrowed("Publish an event to a stream room")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "room": {"type": "string"},
                    "event": {"type": "string"},
                    "data": {"type": "object"}
                },
                "required": ["room", "event", "data"]
            })
            .as_object()
            .unwrap()
            .clone()
            .into(),
            output_schema: None,
            icons: None,
            annotations: Some(ToolAnnotations::new().read_only(false)),
        },
        // Pub/Sub Tools
        Tool {
            name: Cow::Borrowed("synap_pubsub_publish"),
            title: Some("Publish to Topic".to_string()),
            description: Some(Cow::Borrowed("Publish a message to a pub/sub topic")),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "topic": {"type": "string"},
                    "message": {"type": "object"}
                },
                "required": ["topic", "message"]
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
