//! Helper function to create AppState for tests

use std::sync::Arc;
use std::time::Duration;
use synap_server::core::{HashStore, HyperLogLogStore, ListStore, SetStore, SortedSetStore};
use synap_server::monitoring::MonitoringManager;
use synap_server::{AppState, KVStore, ScriptManager};

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

    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let script_manager = Arc::new(ScriptManager::new(Duration::from_secs(5)));

    let hyperloglog_store = Arc::new(HyperLogLogStore::new());

    AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store,
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
