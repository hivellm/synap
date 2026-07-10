## 1. Implementation
- [ ] 1.1 Add a cluster section to ServerConfig (enabled, node id/address, seeds, slots)
- [ ] 1.2 Build ClusterTopology + SlotMigrationManager from config in main.rs when enabled
- [ ] 1.3 Pass topology/migration into AppState and KVStore (new_with_cluster) instead of None
- [ ] 1.4 Surface cluster mode in startup logs and INFO cluster
- [ ] 1.5 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (cluster config)
- [ ] 2.2 Write tests covering the new behavior (config → topology/migration wired)
- [ ] 2.3 Run tests and confirm they pass
