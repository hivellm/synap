//! Memory Usage Tracking
//!
//! Track memory usage per key across stores

use crate::core::KeyType;
use serde::Serialize;

/// Memory usage for a key
#[derive(Debug, Serialize)]
pub struct MemoryUsage {
    pub key: String,
    pub bytes: usize,
    pub human: String,
}

impl MemoryUsage {
    /// Calculate memory usage for a key (simplified version)
    /// Takes stores directly to avoid KeyManager dependency issue
    pub async fn calculate_with_stores(
        key_type: crate::core::KeyType,
        key: &str,
        kv_store: &crate::core::KVStore,
        hash_store: &crate::core::HashStore,
        list_store: &crate::core::ListStore,
        set_store: &crate::core::SetStore,
        sorted_set_store: &crate::core::SortedSetStore,
    ) -> Option<MemoryUsage> {
        use crate::core::KeyType;

        let bytes = match key_type {
            KeyType::String => {
                // KV store: value size + key overhead
                if let Ok(Some(value)) = kv_store.get(key).await {
                    value.len() + key.len() + 64 // Rough overhead estimate
                } else {
                    return None;
                }
            }
            KeyType::Hash => {
                // Hash: sum of all field values + overhead
                if let Ok(fields) = hash_store.hgetall(key) {
                    let fields_size: usize = fields.values().map(|v| v.len()).sum();
                    fields_size + (fields.len() * 32) + key.len() + 64
                } else {
                    return None;
                }
            }
            KeyType::List => {
                // List: sum of all elements + overhead
                if let Ok(elements) = list_store.lrange(key, 0, -1) {
                    let elements_size: usize = elements.iter().map(|v| v.len()).sum();
                    elements_size + (elements.len() * 16) + key.len() + 64
                } else {
                    return None;
                }
            }
            KeyType::Set => {
                // Set: sum of all members + overhead
                if let Ok(members) = set_store.smembers(key) {
                    let members_size: usize = members.iter().map(|v: &Vec<u8>| v.len()).sum();
                    members_size + (members.len() * 24) + key.len() + 64
                } else {
                    return None;
                }
            }
            KeyType::SortedSet => {
                // Sorted set: sum of members + scores + overhead
                let members = sorted_set_store.zrange(key, 0, -1, false);
                let members_size: usize = members.iter().map(|m| m.member.len() + 8).sum(); // +8 for score
                members_size + (members.len() * 32) + key.len() + 64
            }
            KeyType::None => return None,
        };

        Some(MemoryUsage {
            key: key.to_string(),
            bytes,
            human: format_bytes(bytes),
        })
    }
}

fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if size.fract() < 0.01 {
        format!("{:.0}{}", size, UNITS[unit_idx])
    } else {
        format!("{:.2}{}", size, UNITS[unit_idx])
    }
}
