use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::monitoring::ClientListManager;
use synap_server::{AppState, KVConfig, KVStore, ScriptManager, create_router};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let list_store = Arc::new(synap_server::core::ListStore::new());
    let set_store = Arc::new(synap_server::core::SetStore::new());
    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());

    let monitoring = Arc::new(synap_server::monitoring::MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let script_manager = Arc::new(ScriptManager::default());
    let client_list_manager = Arc::new(ClientListManager::new());
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));
    let state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
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
        client_list_manager,
        cluster_topology: None,
        cluster_migration: None,
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
        false,
        false,
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

#[tokio::test]
async fn test_string_append_creates_and_appends() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/kv/string:append/append", base_url))
        .json(&json!({"value": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let stored_value: String = client
        .get(format!("{}/kv/get/string:append", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(stored_value.contains("hello"));
    assert_eq!(body["length"].as_u64().unwrap(), stored_value.len() as u64);

    let res = client
        .post(format!("{}/kv/string:append/append", base_url))
        .json(&json!({"value": " world"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let stored_value: String = client
        .get(format!("{}/kv/get/string:append", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(stored_value.contains("hello"));
    assert!(stored_value.contains(" world"));
    assert_eq!(body["length"].as_u64().unwrap(), stored_value.len() as u64);

    let res = client
        .get(format!("{}/kv/string:append/strlen", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["length"].as_u64().unwrap(), stored_value.len() as u64);
}

#[tokio::test]
async fn test_string_getrange_positive_and_negative_indices() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({"key": "string:getrange", "value": "foobarbaz"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let stored_value: String = client
        .get(format!("{}/kv/get/string:getrange", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let compute_expected = |value: &str, start: isize, end: isize| -> String {
        let bytes = value.as_bytes();
        let len = bytes.len() as isize;
        let start_idx = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len)
        } as usize;
        let end_idx = if end < 0 {
            (len + end + 1).max(0)
        } else {
            (end + 1).min(len)
        } as usize;

        if start_idx >= end_idx || start_idx >= bytes.len() {
            return String::new();
        }

        String::from_utf8(bytes[start_idx..end_idx.min(bytes.len())].to_vec()).unwrap()
    };

    let res = client
        .get(format!(
            "{}/kv/string:getrange/getrange?start={}&end={}",
            base_url, 0, 2
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let value: serde_json::Value = res.json().await.unwrap();
    let expected = compute_expected(&stored_value, 0, 2);
    assert_eq!(value.as_str().unwrap(), expected);

    let res = client
        .get(format!(
            "{}/kv/string:getrange/getrange?start={}&end={}",
            base_url, -3, -1
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let value: serde_json::Value = res.json().await.unwrap();
    let expected = compute_expected(&stored_value, -3, -1);
    assert_eq!(value.as_str().unwrap(), expected);
}

#[tokio::test]
async fn test_string_setrange_overwrites_existing_value() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({"key": "string:setrange", "value": "hello world"}))
        .send()
        .await
        .unwrap();

    let res = client
        .post(format!("{}/kv/string:setrange/setrange", base_url))
        .json(&json!({"offset": 6, "value": "Synap"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();

    let value_str: String = client
        .get(format!("{}/kv/get/string:setrange", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(value_str.contains("hello"));
    assert!(value_str.contains("Synap"));
    assert_eq!(body["length"].as_u64().unwrap(), value_str.len() as u64);
}

#[tokio::test]
async fn test_string_getset_returns_previous_and_sets_new_value() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({"key": "string:getset", "value": "first"}))
        .send()
        .await
        .unwrap();

    let res = client
        .post(format!("{}/kv/string:getset/getset", base_url))
        .json(&json!({"value": "second"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"first\"");

    let res = client
        .post(format!("{}/kv/string:getset-new/getset", base_url))
        .json(&json!({"value": "initial"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let value_null: serde_json::Value = res.json().await.unwrap();
    assert!(value_null.is_null());

    let value_new: String = client
        .get(format!("{}/kv/get/string:getset-new", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(value_new, "\"initial\"");

    let res = client
        .get(format!("{}/kv/get/string:getset", base_url))
        .send()
        .await
        .unwrap();
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"second\"");
}

#[tokio::test]
async fn test_string_msetnx_sets_multiple_keys_when_absent() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/kv/msetnx", base_url))
        .json(&json!({
            "pairs": [
                ["string:msetnx:one", "one"],
                ["string:msetnx:two", "two"]
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);

    let res = client
        .get(format!("{}/kv/get/string:msetnx:one", base_url))
        .send()
        .await
        .unwrap();
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"one\"");

    let res = client
        .get(format!("{}/kv/get/string:msetnx:two", base_url))
        .send()
        .await
        .unwrap();
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"two\"");
}

#[tokio::test]
async fn test_string_msetnx_fails_when_any_key_exists() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({"key": "string:msetnx:existing", "value": "value"}))
        .send()
        .await
        .unwrap();

    let res = client
        .post(format!("{}/kv/msetnx", base_url))
        .json(&json!({
            "pairs": [
                ["string:msetnx:existing", "new"],
                ["string:msetnx:new", "other"]
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], false);

    let res = client
        .get(format!("{}/kv/get/string:msetnx:new", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body.as_object().unwrap().get("error").is_some());
}

#[tokio::test]
async fn test_string_msetnx_with_object_format() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test MSETNX with object format {key, value}
    let res = client
        .post(format!("{}/kv/msetnx", base_url))
        .json(&json!({
            "pairs": [
                {"key": "string:msetnx:obj:one", "value": "one"},
                {"key": "string:msetnx:obj:two", "value": "two"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);

    // Verify values were set
    let res = client
        .get(format!("{}/kv/get/string:msetnx:obj:one", base_url))
        .send()
        .await
        .unwrap();
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"one\"");

    let res = client
        .get(format!("{}/kv/get/string:msetnx:obj:two", base_url))
        .send()
        .await
        .unwrap();
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"two\"");
}

#[tokio::test]
async fn test_string_msetnx_backward_compatibility_with_tuple_format() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test MSETNX with tuple format (backward compatibility)
    let res = client
        .post(format!("{}/kv/msetnx", base_url))
        .json(&json!({
            "pairs": [
                ["string:msetnx:tuple:one", "one"],
                ["string:msetnx:tuple:two", "two"]
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);

    // Verify values were set
    let res = client
        .get(format!("{}/kv/get/string:msetnx:tuple:one", base_url))
        .send()
        .await
        .unwrap();
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"one\"");
}

#[tokio::test]
async fn test_string_strlen_returns_zero_for_missing_key() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .get(format!("{}/kv/string:missing/strlen", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["length"], 0);
}

#[tokio::test]
async fn test_string_setrange_creates_new_key_with_padding() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/kv/string:padding/setrange", base_url))
        .json(&json!({"offset": 3, "value": "xyz"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let value = client
        .get(format!("{}/kv/get/string:padding", base_url))
        .send()
        .await
        .unwrap()
        .json::<String>()
        .await
        .unwrap();
    let zero_prefix = value.chars().take(3).filter(|c| *c == '\u{0000}').count();
    assert_eq!(zero_prefix, 3);
    assert!(value.contains("xyz"));
    assert_eq!(body["length"].as_u64().unwrap(), value.len() as u64);
}
