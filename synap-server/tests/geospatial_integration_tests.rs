//! Integration tests for Geospatial operations

use axum::http::StatusCode;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use synap_server::config::{McpConfig, RateLimitConfig, ServerConfig};
use synap_server::core::{
    GeospatialStore, HashStore, HyperLogLogStore, ListStore, SetStore, SortedSetStore,
    TransactionManager,
};
use synap_server::monitoring::MonitoringManager;
use synap_server::server::router::create_router;
use synap_server::{AppState, KVStore, ScriptManager};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let transaction_manager = Arc::new(TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let script_manager = Arc::new(ScriptManager::default());
    let hyperloglog_store = Arc::new(HyperLogLogStore::new());
    let bitmap_store = Arc::new(synap_server::core::BitmapStore::new());
    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));

    let state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store,
        bitmap_store,
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager,
    };

    let app = create_router(
        state,
        RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        McpConfig::default(),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    url
}

// ==================== REST API Integration Tests ====================

#[tokio::test]
async fn test_geospatial_geoadd_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // GEOADD - Add locations
    let response = client
        .post(format!("{}/geospatial/cities/geoadd", base_url))
        .json(&serde_json::json!({
            "locations": [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                {"lat": 40.7128, "lon": -74.0060, "member": "New York"},
                {"lat": 34.0522, "lon": -118.2437, "member": "Los Angeles"}
            ],
            "nx": false,
            "xx": false,
            "ch": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["added"], 3);
}

#[tokio::test]
async fn test_geospatial_geodist_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add two locations first
    client
        .post(format!("{}/geospatial/cities/geoadd", base_url))
        .json(&serde_json::json!({
            "locations": [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                {"lat": 40.7128, "lon": -74.0060, "member": "New York"}
            ]
        }))
        .send()
        .await
        .unwrap();

    // GEODIST - Calculate distance
    let response = client
        .get(format!(
            "{}/geospatial/cities/geodist/San Francisco/New York?unit=m",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["distance"].as_f64().unwrap() > 0.0);
    assert_eq!(body["unit"], "m");
}

#[tokio::test]
async fn test_geospatial_georadius_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add locations
    client
        .post(format!("{}/geospatial/cities/geoadd", base_url))
        .json(&serde_json::json!({
            "locations": [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"},
                {"lat": 34.0522, "lon": -118.2437, "member": "Los Angeles"}
            ]
        }))
        .send()
        .await
        .unwrap();

    // GEORADIUS - Find cities within 100km of San Francisco
    let response = client
        .get(format!(
            "{}/geospatial/cities/georadius?lat=37.7749&lon=-122.4194&radius=100&unit=km&withdist=true&count=10",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["results"].as_array().unwrap().len() >= 2); // Should find SF and Oakland
}

#[tokio::test]
async fn test_geospatial_georadiusbymember_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add locations
    client
        .post(format!("{}/geospatial/cities/geoadd", base_url))
        .json(&serde_json::json!({
            "locations": [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"},
                {"lat": 34.0522, "lon": -118.2437, "member": "Los Angeles"}
            ]
        }))
        .send()
        .await
        .unwrap();

    // GEORADIUSBYMEMBER - Find cities within 100km of San Francisco
    let response = client
        .get(format!(
            "{}/geospatial/cities/georadiusbymember/San Francisco?radius=100&unit=km&withdist=true&count=10",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["results"].as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn test_geospatial_geopos_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add location
    client
        .post(format!("{}/geospatial/cities/geoadd", base_url))
        .json(&serde_json::json!({
            "locations": [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"}
            ]
        }))
        .send()
        .await
        .unwrap();

    // GEOPOS - Get coordinates
    let response = client
        .post(format!("{}/geospatial/cities/geopos", base_url))
        .json(&serde_json::json!({
            "members": ["San Francisco"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    let coords = body["coordinates"][0].as_object().unwrap();
    assert!((coords["lat"].as_f64().unwrap() - 37.7749).abs() < 0.001);
    assert!((coords["lon"].as_f64().unwrap() - -122.4194).abs() < 0.001);
}

#[tokio::test]
async fn test_geospatial_geohash_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add location
    client
        .post(format!("{}/geospatial/cities/geoadd", base_url))
        .json(&serde_json::json!({
            "locations": [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"}
            ]
        }))
        .send()
        .await
        .unwrap();

    // GEOHASH - Get geohash string
    let response = client
        .post(format!("{}/geospatial/cities/geohash", base_url))
        .json(&serde_json::json!({
            "members": ["San Francisco"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    let geohash = body["geohashes"][0].as_str().unwrap();
    assert_eq!(geohash.len(), 11); // Redis uses 11-character geohash
    assert!(!geohash.is_empty());
}

#[tokio::test]
async fn test_geospatial_stats_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add some locations
    client
        .post(format!("{}/geospatial/cities/geoadd", base_url))
        .json(&serde_json::json!({
            "locations": [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                {"lat": 40.7128, "lon": -74.0060, "member": "New York"}
            ]
        }))
        .send()
        .await
        .unwrap();

    // STATS - Get statistics
    let response = client
        .get(format!("{}/geospatial/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["total_keys"].as_u64().unwrap() >= 1);
    assert!(body["total_locations"].as_u64().unwrap() >= 2);
}

// ==================== StreamableHTTP Integration Tests ====================

#[tokio::test]
async fn test_geospatial_geoadd_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geoadd",
            "request_id": "test-123",
            "payload": {
                "key": "cities",
                "locations": [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 40.7128, "lon": -74.0060, "member": "New York"}
                ],
                "nx": false,
                "xx": false,
                "ch": false
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["payload"]["added"], 2);
}

#[tokio::test]
async fn test_geospatial_geodist_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add locations first
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geoadd",
            "request_id": "test-1",
            "payload": {
                "key": "cities",
                "locations": [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 40.7128, "lon": -74.0060, "member": "New York"}
                ]
            }
        }))
        .send()
        .await
        .unwrap();

    // GEODIST
    let response = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geodist",
            "request_id": "test-2",
            "payload": {
                "key": "cities",
                "member1": "San Francisco",
                "member2": "New York",
                "unit": "km"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    let distance = body["payload"]["distance"].as_f64().unwrap();
    assert!(distance > 0.0);
    assert!(distance < 10000.0); // Should be around 4000km
}

#[tokio::test]
async fn test_geospatial_georadius_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add locations
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geoadd",
            "request_id": "test-1",
            "payload": {
                "key": "cities",
                "locations": [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"},
                    {"lat": 34.0522, "lon": -118.2437, "member": "Los Angeles"}
                ]
            }
        }))
        .send()
        .await
        .unwrap();

    // GEORADIUS
    let response = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.georadius",
            "request_id": "test-2",
            "payload": {
                "key": "cities",
                "center_lat": 37.7749,
                "center_lon": -122.4194,
                "radius": 100.0,
                "unit": "km",
                "with_dist": true,
                "with_coord": false,
                "count": 10,
                "sort": "ASC"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    let results = body["payload"]["results"].as_array().unwrap();
    assert!(results.len() >= 2);
}

#[tokio::test]
async fn test_geospatial_georadiusbymember_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add locations
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geoadd",
            "request_id": "test-1",
            "payload": {
                "key": "cities",
                "locations": [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"}
                ]
            }
        }))
        .send()
        .await
        .unwrap();

    // GEORADIUSBYMEMBER
    let response = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.georadiusbymember",
            "request_id": "test-2",
            "payload": {
                "key": "cities",
                "member": "San Francisco",
                "radius": 50.0,
                "unit": "km",
                "with_dist": true,
                "with_coord": true,
                "count": 10
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    let results = body["payload"]["results"].as_array().unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_geospatial_geopos_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add location
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geoadd",
            "request_id": "test-1",
            "payload": {
                "key": "cities",
                "locations": [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"}
                ]
            }
        }))
        .send()
        .await
        .unwrap();

    // GEOPOS
    let response = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geopos",
            "request_id": "test-2",
            "payload": {
                "key": "cities",
                "members": ["San Francisco"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    let coords = body["payload"]["coordinates"][0].as_object().unwrap();
    assert!((coords["lat"].as_f64().unwrap() - 37.7749).abs() < 0.001);
}

#[tokio::test]
async fn test_geospatial_geohash_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add location
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geoadd",
            "request_id": "test-1",
            "payload": {
                "key": "cities",
                "locations": [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"}
                ]
            }
        }))
        .send()
        .await
        .unwrap();

    // GEOHASH
    let response = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geohash",
            "request_id": "test-2",
            "payload": {
                "key": "cities",
                "members": ["San Francisco"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    let geohash = body["payload"]["geohashes"][0].as_str().unwrap();
    assert_eq!(geohash.len(), 11);
}

#[tokio::test]
async fn test_geospatial_stats_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add locations
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geoadd",
            "request_id": "test-1",
            "payload": {
                "key": "cities",
                "locations": [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 40.7128, "lon": -74.0060, "member": "New York"}
                ]
            }
        }))
        .send()
        .await
        .unwrap();

    // STATS
    let response = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.stats",
            "request_id": "test-2",
            "payload": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    assert!(body["payload"]["total_keys"].as_u64().unwrap() >= 1);
    assert!(body["payload"]["geoadd_count"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_geospatial_geodist_not_found() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // GEODIST with non-existent member
    let response = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&serde_json::json!({
            "command": "geospatial.geodist",
            "request_id": "test-1",
            "payload": {
                "key": "cities",
                "member1": "Unknown",
                "member2": "AlsoUnknown",
                "unit": "m"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    // Should return error or null distance
    assert!(!body["success"].as_bool().unwrap_or(true) || body["payload"]["distance"].is_null());
}
