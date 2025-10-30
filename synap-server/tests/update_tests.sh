#!/bin/bash
# Script to add monitoring field to all test AppState instances

for file in *.rs; do
    if grep -q "partition_manager: None," "$file" && ! grep -q "monitoring:" "$file"; then
        # Get store variable names
        kv_var=$(grep "kv_store.*=" "$file" | head -1 | sed 's/.*let \([^ ]*\).*/\1/')
        hash_var=$(grep "hash_store.*=" "$file" | head -1 | sed 's/.*let \([^ ]*\).*/\1/')
        
        # If not found, use defaults
        [[ -z "$kv_var" ]] && kv_var="kv_store"
        [[ -z "$hash_var" ]] && hash_var="hash_store"
        
        # Add monitoring field after partition_manager
        sed -i "/partition_manager: None,/a\        monitoring: Arc::new(synap_server::monitoring::MonitoringManager::new(\n            ${kv_var}.clone(),\n            ${hash_var}.clone(),\n            list_store.clone(),\n            set_store.clone(),\n            sorted_set_store.clone(),\n        ))," "$file"
        echo "Updated $file"
    fi
done



