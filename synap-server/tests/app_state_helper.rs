//! Helper function to create AppState for tests

use std::sync::Arc;
use synap_server::core::{HashStore, ListStore, SetStore, SortedSetStore};
use synap_server::monitoring::MonitoringManager;
use synap_server::{AppState, KVConfig, KVStore};

/// Create a default AppState for testing
pub fn create_test_app_state_with_stores(
    kv_store: Arc<KVStore>,
    hash_store: Arc<HashStore>,
    list_store: Arc<ListStore>,
    set_store: Arc<SetStore>,
    sorted_set_store: Arc<SortedSetStore>,
) -> AppState {
    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

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
    }
}
