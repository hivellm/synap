//! UMICP Tool Discovery for Synap
//!
//! Implements the DiscoverableService trait to expose all 8 MCP tools
//! via UMICP v0.2.3 tool discovery protocol

use serde_json::json;
use umicp_core::{DiscoverableService, OperationSchema, ServerInfo};

/// Synap Discovery Service
/// Exposes all MCP tools as UMICP-discoverable operations
pub struct SynapDiscoveryService;

impl DiscoverableService for SynapDiscoveryService {
    fn server_info(&self) -> ServerInfo {
        ServerInfo::new("synap-server", env!("CARGO_PKG_VERSION"), "UMICP/2.0")
            .features(vec![
                "key-value-store".to_string(),
                "message-queues".to_string(),
                "event-streams".to_string(),
                "pub-sub".to_string(),
                "kafka-partitioning".to_string(),
                "consumer-groups".to_string(),
                "persistence".to_string(),
                "replication".to_string(),
                "mcp-compatible".to_string(),
            ])
            .operations_count(13)
            .mcp_compatible(true)
            .metadata(json!({
                "description": "Synap - Unified messaging system (KV + Hash + List + Set + Queue + Stream + PubSub) with 13 core operations"
            }))
    }

    fn list_operations(&self) -> Vec<OperationSchema> {
        // Get all MCP tools
        let mcp_tools = crate::server::mcp_tools::get_mcp_tools();

        // Convert MCP Tools to UMICP OperationSchema
        mcp_tools
            .into_iter()
            .map(|tool| {
                let mut schema =
                    OperationSchema::new(tool.name.to_string(), json!(tool.input_schema));

                // Set title if available
                if let Some(title) = tool.title {
                    schema = schema.title(title);
                }

                // Set description if available
                if let Some(description) = tool.description {
                    schema = schema.description(description.to_string());
                }

                // Set output schema if available
                if let Some(output) = tool.output_schema {
                    schema = schema.output_schema(json!(output));
                }

                // Convert MCP annotations to UMICP annotations JSON
                if let Some(annotations) = tool.annotations {
                    let annotations_json = json!({
                        "read_only": annotations.read_only_hint,
                        "idempotent": annotations.idempotent_hint,
                        "destructive": annotations.destructive_hint,
                    });
                    schema = schema.annotations(annotations_json);
                }

                schema
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info() {
        let service = SynapDiscoveryService;
        let info = service.server_info();

        assert_eq!(info.server, "synap-server");
        assert_eq!(info.protocol, "UMICP/2.0");
        assert!(info.features.is_some());
        let features = info.features.unwrap();
        assert!(features.contains(&"key-value-store".to_string()));
    }

    #[test]
    fn test_list_operations() {
        let service = SynapDiscoveryService;
        let operations = service.list_operations();

        // Should have 13 essential operations (3 KV + 3 Hash + 3 List + 3 Set + 1 Queue)
        assert_eq!(
            operations.len(),
            13,
            "Expected 13 operations, got {}",
            operations.len()
        );

        // Check for key operations
        let op_names: Vec<String> = operations.iter().map(|op| op.name.clone()).collect();

        // Verify essential operations are present
        // KV operations (3)
        assert!(op_names.contains(&"synap_kv_get".to_string()));
        assert!(op_names.contains(&"synap_kv_set".to_string()));
        assert!(op_names.contains(&"synap_kv_delete".to_string()));
        // Hash operations (3)
        assert!(op_names.contains(&"synap_hash_set".to_string()));
        assert!(op_names.contains(&"synap_hash_get".to_string()));
        assert!(op_names.contains(&"synap_hash_getall".to_string()));
        // List operations (3)
        assert!(op_names.contains(&"synap_list_push".to_string()));
        assert!(op_names.contains(&"synap_list_pop".to_string()));
        assert!(op_names.contains(&"synap_list_range".to_string()));
        // Set operations (3)
        assert!(op_names.contains(&"synap_set_add".to_string()));
        assert!(op_names.contains(&"synap_set_members".to_string()));
        assert!(op_names.contains(&"synap_set_inter".to_string()));
        // Queue operations (1)
        assert!(op_names.contains(&"synap_queue_publish".to_string()));
    }

    #[test]
    fn test_operation_has_required_fields() {
        let service = SynapDiscoveryService;
        let operations = service.list_operations();

        for op in operations.iter() {
            // Check that operation has a name
            assert!(!op.name.is_empty());

            // Check that input_schema exists
            assert!(op.input_schema.is_object() || op.input_schema.is_null());
        }
    }

    #[test]
    fn test_kv_get_operation() {
        let service = SynapDiscoveryService;
        let operations = service.list_operations();

        let kv_get_op = operations
            .iter()
            .find(|op| op.name == "synap_kv_get")
            .expect("synap_kv_get operation not found");

        // Should have input schema
        assert!(kv_get_op.input_schema.is_object());
        let schema = kv_get_op.input_schema.as_object().unwrap();
        assert!(schema.contains_key("properties"));

        // Verify required fields
        let properties = schema.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("key"));
    }
}
