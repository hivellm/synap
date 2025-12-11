//! Test helper utilities for creating AppState instances

use std::sync::Arc;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::core::{
    GeospatialStore, HashStore, HyperLogLogStore, ListStore, SetStore, SortedSetStore,
};
use synap_server::monitoring::{ClientListManager, MonitoringManager};
use synap_server::server::router::create_router;
use synap_server::{AppState, KVConfig, KVStore, ScriptManager};

/// Create a default AppState for testing
pub fn create_test_app_state() -> AppState {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
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

    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let script_manager = Arc::new(ScriptManager::default());
    let client_list_manager = Arc::new(ClientListManager::new());
    let hyperloglog_store = Arc::new(HyperLogLogStore::new());
    let bitmap_store = Arc::new(synap_server::core::BitmapStore::new());
    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));

    AppState {
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
        client_list_manager,
        cluster_topology: None,
        cluster_migration: None,
        #[cfg(feature = "hub-integration")]
        hub_client: None,
    }
}

/// Create default UserManager and ApiKeyManager for tests
pub fn create_test_auth_managers() -> (Arc<UserManager>, Arc<ApiKeyManager>) {
    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());
    (user_manager, api_key_manager)
}

/// Create a router for testing with default configuration
#[allow(dead_code)]
pub fn create_test_router(state: AppState) -> axum::Router {
    let (user_manager, api_key_manager) = create_test_auth_managers();
    create_router(
        state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        synap_server::config::McpConfig::default(),
        user_manager,
        api_key_manager,
        false, // auth_enabled
        false, // require_auth
    )
}
