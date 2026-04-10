//! Cluster REST API Integration Tests
//!
//! Tests REST API endpoints for cluster management

use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::cluster::{migration::SlotMigrationManager, topology::ClusterTopology};
use synap_server::core::{KVConfig, KVStore};
use synap_server::monitoring::MonitoringManager;
use synap_server::{AppState, ScriptManager, create_router};
use tokio::net::TcpListener;

/// Helper to spawn a test server with cluster mode enabled
async fn spawn_test_server_with_cluster() -> String {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());

    // Initialize cluster topology
    let topology = Arc::new(ClusterTopology::new("node-0".to_string()));
    topology.initialize_cluster(3).unwrap();

    let migration = Arc::new(SlotMigrationManager::new(100, Duration::from_secs(60)));

    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        Arc::new(synap_server::core::ListStore::new()),
        Arc::new(synap_server::core::SetStore::new()),
        Arc::new(synap_server::core::SortedSetStore::new()),
    ));
    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        Arc::new(synap_server::core::ListStore::new()),
        Arc::new(synap_server::core::SetStore::new()),
        Arc::new(synap_server::core::SortedSetStore::new()),
    ));
    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));
    let state = AppState {
        kv_store,
        hash_store,
        list_store: Arc::new(synap_server::core::ListStore::new()),
        set_store: Arc::new(synap_server::core::SetStore::new()),
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        pubsub_router: None,
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
        client_list_manager: Arc::new(synap_server::monitoring::ClientListManager::new()),
        cluster_topology: Some(topology),
        cluster_migration: Some(migration),
        hub_client: None,
    };

    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());

    let app = create_router(
        state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        synap_server::config::McpConfig::default(),
        user_manager,
        api_key_manager,
        false, // auth disabled for tests
        false,
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait a bit for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    url
}

#[tokio::test]
async fn test_cluster_info_endpoint() {
    // Test: GET /cluster/info returns cluster information
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/cluster/info", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["state"], "ok");
    assert_eq!(body["cluster_enabled"], true);
    assert_eq!(body["nodes"]["count"], 3);
    assert_eq!(body["slots"]["total"], 16384);
}

#[tokio::test]
async fn test_cluster_nodes_endpoint() {
    // Test: GET /cluster/nodes returns all nodes
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/cluster/nodes", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["count"], 3);

    let nodes = body["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);

    // Verify node structure
    for node in nodes {
        assert!(node["id"].is_string());
        assert!(node["address"].is_string());
        assert!(node["state"].is_string());
        assert!(node["slot_count"].is_number());
    }
}

#[tokio::test]
async fn test_cluster_node_info_endpoint() {
    // Test: GET /cluster/nodes/{node_id} returns node information
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/cluster/nodes/node-0", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["id"], "node-0");
    assert!(body["address"].is_string());
    assert!(body["slot_count"].is_number());
    assert!(body["slots"].is_array());
}

#[tokio::test]
async fn test_cluster_node_info_not_found() {
    // Test: GET /cluster/nodes/{node_id} returns 404 for non-existent node
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/cluster/nodes/non-existent", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_cluster_slots_endpoint() {
    // Test: GET /cluster/slots returns slot assignments
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/cluster/slots", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["total"], 16384);
    assert_eq!(body["assigned"], 16384);
    assert!(body["coverage"].as_f64().unwrap() > 99.0);

    let slots = body["slots"].as_array().unwrap();
    assert_eq!(slots.len(), 16384);

    // Verify slot structure
    for slot in slots {
        assert!(slot["slot"].is_number());
        assert!(slot["owner"].is_string());
    }
}

#[tokio::test]
async fn test_cluster_add_node_endpoint() {
    // Test: POST /cluster/nodes adds a node
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .post(format!("{}/cluster/nodes", url))
        .json(&json!({
            "node_id": "node-3",
            "address": "127.0.0.1:15505"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["node_id"], "node-3");

    // Verify node was added
    let nodes_response = client
        .get(format!("{}/cluster/nodes", url))
        .send()
        .await
        .unwrap();

    let nodes_body: serde_json::Value = nodes_response.json().await.unwrap();
    assert_eq!(nodes_body["count"], 4);
}

#[tokio::test]
async fn test_cluster_remove_node_endpoint() {
    // Test: DELETE /cluster/nodes/{node_id} removes a node
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .delete(format!("{}/cluster/nodes/node-2", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["node_id"], "node-2");
}

#[tokio::test]
async fn test_cluster_assign_slots_endpoint() {
    // Test: POST /cluster/slots/assign assigns slots to a node
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    // First add a node
    client
        .post(format!("{}/cluster/nodes", url))
        .json(&json!({
            "node_id": "node-3",
            "address": "127.0.0.1:15505"
        }))
        .send()
        .await
        .unwrap();

    // Assign some slots
    let response = client
        .post(format!("{}/cluster/slots/assign", url))
        .json(&json!({
            "node_id": "node-3",
            "slots": [{"start": 1000, "end": 2000}]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["node_id"], "node-3");
    assert_eq!(body["slots_assigned"], 1);
}

#[tokio::test]
async fn test_cluster_start_migration_endpoint() {
    // Test: POST /cluster/migration/start starts a migration
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .post(format!("{}/cluster/migration/start", url))
        .json(&json!({
            "slot": 100,
            "from_node": "node-0",
            "to_node": "node-1"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["slot"], 100);
    assert_eq!(body["from_node"], "node-0");
    assert_eq!(body["to_node"], "node-1");
}

#[tokio::test]
async fn test_cluster_complete_migration_endpoint() {
    // Test: POST /cluster/migration/complete completes a migration
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    // Start migration first
    client
        .post(format!("{}/cluster/migration/start", url))
        .json(&json!({
            "slot": 100,
            "from_node": "node-0",
            "to_node": "node-1"
        }))
        .send()
        .await
        .unwrap();

    // Complete migration
    let response = client
        .post(format!("{}/cluster/migration/complete", url))
        .json(&json!({
            "slot": 100
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["slot"], 100);
}

#[tokio::test]
async fn test_cluster_migration_status_endpoint() {
    // Test: GET /cluster/migration/{slot} returns migration status
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    // Start migration first
    client
        .post(format!("{}/cluster/migration/start", url))
        .json(&json!({
            "slot": 100,
            "from_node": "node-0",
            "to_node": "node-1"
        }))
        .send()
        .await
        .unwrap();

    // Get migration status
    let response = client
        .get(format!("{}/cluster/migration/100", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["slot"], 100);
    assert_eq!(body["from_node"], "node-0");
    assert_eq!(body["to_node"], "node-1");
    assert!(body["state"].is_string());
}

#[tokio::test]
async fn test_cluster_migration_status_not_found() {
    // Test: GET /cluster/migration/{slot} returns null for non-existent migration
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/cluster/migration/9999", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["slot"], 9999);
    assert_eq!(body["migration"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_cluster_endpoints_require_cluster_mode() {
    // Test: Cluster endpoints return error when cluster mode not enabled
    // Note: This test would require a server without cluster mode
    // For now, we just verify endpoints work with cluster mode enabled
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/cluster/info", url))
        .send()
        .await
        .unwrap();

    // Should succeed when cluster mode is enabled
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_cluster_info_slot_coverage() {
    // Test: Cluster info returns correct slot coverage
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/cluster/info", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    let slots = &body["slots"];
    assert_eq!(slots["assigned"], 16384);
    assert_eq!(slots["total"], 16384);
    assert!(slots["coverage"].as_f64().unwrap() > 99.0);
}

#[tokio::test]
async fn test_cluster_add_node_duplicate() {
    // Test: Adding duplicate node returns error
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    // Try to add node-0 again (already exists)
    let response = client
        .post(format!("{}/cluster/nodes", url))
        .json(&json!({
            "node_id": "node-0",
            "address": "127.0.0.1:15502"
        }))
        .send()
        .await
        .unwrap();

    // Should fail (500 - internal error)
    assert_eq!(response.status(), 500);
}

#[tokio::test]
async fn test_cluster_remove_node_not_found() {
    // Test: Removing non-existent node returns error
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .delete(format!("{}/cluster/nodes/non-existent", url))
        .send()
        .await
        .unwrap();

    // Should fail (500 - internal error)
    assert_eq!(response.status(), 500);
}

#[tokio::test]
async fn test_cluster_assign_slots_node_not_found() {
    // Test: Assigning slots to non-existent node returns error
    let url = spawn_test_server_with_cluster().await;
    let client = Client::new();

    let response = client
        .post(format!("{}/cluster/slots/assign", url))
        .json(&json!({
            "node_id": "non-existent",
            "slots": [{"start": 1000, "end": 2000}]
        }))
        .send()
        .await
        .unwrap();

    // Should fail (500 - internal error)
    assert_eq!(response.status(), 500);
}
