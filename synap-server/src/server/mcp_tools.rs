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
