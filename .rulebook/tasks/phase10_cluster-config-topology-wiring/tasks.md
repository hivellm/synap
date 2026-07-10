## 1. Implementation
- [x] 1.1 Add a cluster section to ServerConfig (ClusterConfig with per-field serde defaults)
- [x] 1.2 Build ClusterTopology (from_config) + SlotMigrationManager in main.rs when enabled
- [x] 1.3 Pass topology/migration into AppState and KVStore (with_cluster builder) instead of None
- [x] 1.4 Surface cluster mode in startup logs and INFO cluster
- [x] 1.5 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (config.yml cluster + CHANGELOG)
- [x] 2.2 Write tests covering the new behavior (from_config owns all slots, derives id, partial-config deserialize)
- [x] 2.3 Run tests and confirm they pass
