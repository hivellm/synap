//! Test helper utilities for creating AppState instances

use std::sync::Arc;
use synap_server::core::{HashStore, ListStore, SetStore, SortedSetStore};
use synap_server::monitoring::MonitoringManager;
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

    AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager,
    }
}
