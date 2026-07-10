#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::cluster::config::*;
    use crate::cluster::failover::*;
    use crate::cluster::hash_slot::*;
    use crate::cluster::migration::*;
    use crate::cluster::raft::*;
    use crate::cluster::topology::*;
    use crate::cluster::types::*;
    use std::time::Duration;

    #[test]
    fn test_hash_slot_basic() {
        let slot1 = hash_slot("user:1001");
        let slot2 = hash_slot("user:1002");

        assert!(slot1 < TOTAL_SLOTS);
        assert!(slot2 < TOTAL_SLOTS);
    }

    #[test]
    fn test_hash_tag() {
        let slot1 = hash_slot("user:{1001}:profile");
        let slot2 = hash_slot("user:{1001}:settings");
        assert_eq!(slot1, slot2);
    }

    #[test]
    fn test_hash_slot_consistency() {
        let key = "test:key:12345";
        let slot1 = hash_slot(key);
        let slot2 = hash_slot(key);
        assert_eq!(slot1, slot2);
    }

    #[test]
    fn test_hash_slot_distribution() {
        let mut slots = std::collections::HashSet::new();
        for i in 0..1000 {
            let key = format!("key:{}", i);
            slots.insert(hash_slot(&key));
        }
        assert!(slots.len() > 100);
    }

    #[test]
    fn test_hash_slot_wrapper() {
        let slot = HashSlot::from_key("user:1001");
        assert!(slot.value() < TOTAL_SLOTS);

        let slot2 = HashSlot::new(5000);
        assert_eq!(slot2.value(), 5000);
    }

    #[test]
    fn test_hash_slot_edge_cases() {
        // Empty string
        let slot = hash_slot("");
        assert!(slot < TOTAL_SLOTS);

        // Very long key
        let long_key = "a".repeat(1000);
        let slot = hash_slot(&long_key);
        assert!(slot < TOTAL_SLOTS);

        // Special characters
        let slot = hash_slot("key!@#$%^&*()");
        assert!(slot < TOTAL_SLOTS);
    }

    #[test]
    fn test_hash_tag_edge_cases() {
        // Multiple tags (should use first)
        let slot1 = hash_slot("{tag1}{tag2}");
        let slot2 = hash_slot("{tag1}");
        assert_eq!(slot1, slot2);

        // Empty tag
        let slot = hash_slot("{}");
        assert!(slot < TOTAL_SLOTS);

        // Tag at end
        let slot1 = hash_slot("key{tag}");
        let slot2 = hash_slot("{tag}");
        assert_eq!(slot1, slot2);
    }

    #[test]
    fn test_topology_add_node() {
        let topology = ClusterTopology::new("node-0".to_string());

        let node = ClusterNode {
            id: "node-1".to_string(),
            address: "127.0.0.1:15502".parse().unwrap(),
            state: ClusterState::Connected,
            slots: Vec::new(),
            master_id: None,
            replica_ids: Vec::new(),
            last_ping: 0,
            flags: NodeFlags::default(),
        };

        assert!(topology.add_node(node).is_ok());
        assert!(topology.get_node("node-1").is_ok());
    }

    #[test]
    fn test_topology_remove_node() {
        let topology = ClusterTopology::new("node-0".to_string());

        let node = ClusterNode {
            id: "node-1".to_string(),
            address: "127.0.0.1:15502".parse().unwrap(),
            state: ClusterState::Connected,
            slots: vec![SlotRange::new(0, 100)],
            master_id: None,
            replica_ids: Vec::new(),
            last_ping: 0,
            flags: NodeFlags::default(),
        };

        topology.add_node(node).unwrap();
        topology
            .assign_slots("node-1", vec![SlotRange::new(0, 100)])
            .unwrap();

        assert!(topology.remove_node("node-1").is_ok());
        assert!(topology.get_node("node-1").is_err());
    }

    #[test]
    fn test_topology_assign_slots() {
        let topology = ClusterTopology::new("node-0".to_string());

        let node = ClusterNode {
            id: "node-1".to_string(),
            address: "127.0.0.1:15502".parse().unwrap(),
            state: ClusterState::Connected,
            slots: Vec::new(),
            master_id: None,
            replica_ids: Vec::new(),
            last_ping: 0,
            flags: NodeFlags::default(),
        };

        topology.add_node(node).unwrap();

        let slot_range = SlotRange::new(0, 8191);
        assert!(topology.assign_slots("node-1", vec![slot_range]).is_ok());

        assert_eq!(topology.get_slot_owner(0).unwrap(), "node-1");
        assert_eq!(topology.get_slot_owner(8191).unwrap(), "node-1");
    }

    #[test]
    fn test_topology_update_state() {
        let topology = ClusterTopology::new("node-0".to_string());

        let node = ClusterNode {
            id: "node-1".to_string(),
            address: "127.0.0.1:15502".parse().unwrap(),
            state: ClusterState::Connected,
            slots: Vec::new(),
            master_id: None,
            replica_ids: Vec::new(),
            last_ping: 0,
            flags: NodeFlags::default(),
        };

        topology.add_node(node).unwrap();
        assert!(
            topology
                .update_node_state("node-1", ClusterState::Offline)
                .is_ok()
        );

        let node = topology.get_node("node-1").unwrap();
        assert_eq!(node.state, ClusterState::Offline);
    }

    #[test]
    fn test_topology_initialize_cluster() {
        let topology = ClusterTopology::new("node-0".to_string());
        assert!(topology.initialize_cluster(3).is_ok());

        assert_eq!(topology.get_all_nodes().len(), 3);
        assert!(topology.has_full_coverage());
    }

    #[test]
    fn test_topology_slot_coverage() {
        let topology = ClusterTopology::new("node-0".to_string());
        assert!(!topology.has_full_coverage());
        assert_eq!(topology.slot_coverage(), 0.0);

        topology.initialize_cluster(3).unwrap();
        assert!(topology.has_full_coverage());
        assert_eq!(topology.slot_coverage(), 100.0);
    }

    #[test]
    fn test_topology_get_slot_owner() {
        let topology = ClusterTopology::new("node-0".to_string());
        topology.initialize_cluster(2).unwrap();

        // First node should own slots 0-8191
        assert_eq!(topology.get_slot_owner(0).unwrap(), "node-0");
        assert_eq!(topology.get_slot_owner(8191).unwrap(), "node-0");

        // Second node should own slots 8192-16383
        assert_eq!(topology.get_slot_owner(8192).unwrap(), "node-1");
        assert_eq!(topology.get_slot_owner(16383).unwrap(), "node-1");
    }

    #[test]
    fn test_topology_duplicate_node() {
        let topology = ClusterTopology::new("node-0".to_string());

        let node = ClusterNode {
            id: "node-1".to_string(),
            address: "127.0.0.1:15502".parse().unwrap(),
            state: ClusterState::Connected,
            slots: Vec::new(),
            master_id: None,
            replica_ids: Vec::new(),
            last_ping: 0,
            flags: NodeFlags::default(),
        };

        topology.add_node(node.clone()).unwrap();
        assert!(topology.add_node(node).is_err());
    }

    #[test]
    #[should_panic]
    fn test_topology_invalid_slot_range() {
        // Invalid slot range (end >= TOTAL_SLOTS) - should panic in SlotRange::new
        let _invalid_range = SlotRange::new(16380, TOTAL_SLOTS);
    }

    #[test]
    fn test_topology_get_all_nodes() {
        let topology = ClusterTopology::new("node-0".to_string());
        topology.initialize_cluster(3).unwrap();

        let nodes = topology.get_all_nodes();
        assert_eq!(nodes.len(), 3);
    }

    #[test]
    fn test_topology_my_node_id() {
        let topology = ClusterTopology::new("my-node".to_string());
        assert_eq!(topology.my_node_id(), "my-node");
    }

    #[test]
    fn test_slot_range() {
        let range = SlotRange::new(0, 100);
        assert!(range.contains(50));
        assert!(range.contains(0));
        assert!(range.contains(100));
        assert!(!range.contains(101));
        assert_eq!(range.count(), 101);
    }

    #[test]
    fn test_slot_range_edge_cases() {
        // Single slot range
        let range = SlotRange::new(100, 100);
        assert!(range.contains(100));
        assert_eq!(range.count(), 1);

        // Full range
        let range = SlotRange::new(0, TOTAL_SLOTS - 1);
        assert_eq!(range.count(), TOTAL_SLOTS);
    }

    #[test]
    fn test_node_info_from_cluster_node() {
        let node = ClusterNode {
            id: "node-1".to_string(),
            address: "127.0.0.1:15502".parse().unwrap(),
            state: ClusterState::Connected,
            slots: vec![SlotRange::new(0, 100), SlotRange::new(200, 300)],
            master_id: None,
            replica_ids: Vec::new(),
            last_ping: 0,
            flags: NodeFlags::default(),
        };

        let info = NodeInfo::from(&node);
        assert_eq!(info.id, "node-1");
        assert_eq!(info.slot_count, 202); // 101 + 101 slots
    }

    #[tokio::test]
    async fn test_raft_node_creation() {
        let node = RaftNode::new(
            "node-1".to_string(),
            Duration::from_millis(1000),
            Duration::from_millis(100),
        );

        assert_eq!(node.state(), RaftState::Follower);
        assert_eq!(node.current_term(), 0);
        assert!(!node.is_leader());
    }

    #[tokio::test]
    async fn test_raft_vote() {
        let node = RaftNode::new(
            "node-1".to_string(),
            Duration::from_millis(1000),
            Duration::from_millis(100),
        );

        // First vote should succeed
        assert!(node.request_vote("node-2", 1).unwrap());

        // Vote for different candidate should fail
        assert!(!node.request_vote("node-3", 1).unwrap());

        // Vote in new term should succeed
        assert!(node.request_vote("node-3", 2).unwrap());
    }

    #[tokio::test]
    async fn test_raft_heartbeat() {
        let node = RaftNode::new(
            "node-1".to_string(),
            Duration::from_millis(1000),
            Duration::from_millis(100),
        );

        assert_eq!(node.state(), RaftState::Follower);
        assert!(node.receive_heartbeat("leader-1", 1).is_ok());
    }

    #[tokio::test]
    async fn test_failover_manager() {
        let failover = ClusterFailover::new(Duration::from_secs(5));

        assert!(!failover.is_failing_over("node-1"));

        // Test failure detection
        assert!(failover.detect_failure("node-1").is_ok());
    }

    #[tokio::test]
    async fn test_failover_promote_replica() {
        let failover = ClusterFailover::new(Duration::from_secs(5));

        assert!(
            failover
                .promote_replica("failed-node", "replica-node")
                .is_ok()
        );
        assert!(failover.is_failing_over("failed-node"));
    }

    #[tokio::test]
    async fn test_failover_complete() {
        let failover = ClusterFailover::new(Duration::from_secs(5));

        failover
            .promote_replica("failed-node", "replica-node")
            .unwrap();
        assert!(failover.complete_failover("failed-node").is_ok());
    }

    #[tokio::test]
    async fn test_migration_start() {
        let migration = SlotMigrationManager::new(100, Duration::from_secs(60));

        assert!(
            migration
                .start_migration(100, "node-1".to_string(), "node-2".to_string())
                .is_ok()
        );
        // Migration state is updated asynchronously, so check status instead
        let status = migration.get_migration(100);
        assert!(status.is_some());
    }

    #[tokio::test]
    async fn test_migration_duplicate() {
        let migration = SlotMigrationManager::new(100, Duration::from_secs(60));

        migration
            .start_migration(100, "node-1".to_string(), "node-2".to_string())
            .unwrap();
        assert!(
            migration
                .start_migration(100, "node-1".to_string(), "node-3".to_string())
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_migration_cancel() {
        let migration = SlotMigrationManager::new(100, Duration::from_secs(60));

        migration
            .start_migration(100, "node-1".to_string(), "node-2".to_string())
            .unwrap();
        assert!(migration.cancel_migration(100).is_ok());
    }

    #[tokio::test]
    async fn test_migration_complete() {
        let migration = SlotMigrationManager::new(100, Duration::from_secs(60));

        migration
            .start_migration(100, "node-1".to_string(), "node-2".to_string())
            .unwrap();
        assert!(migration.complete_migration(100).is_ok());
    }

    #[tokio::test]
    async fn test_migration_get_status() {
        let migration = SlotMigrationManager::new(100, Duration::from_secs(60));

        migration
            .start_migration(100, "node-1".to_string(), "node-2".to_string())
            .unwrap();

        let status = migration.get_migration(100);
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.slot, 100);
        assert_eq!(status.from_node, "node-1");
        assert_eq!(status.to_node, "node-2");
    }

    #[test]
    fn test_cluster_node_flags() {
        let flags = NodeFlags {
            is_master: true,
            is_myself: true,
            ..Default::default()
        };

        assert!(flags.is_master);
        assert!(flags.is_myself);
        assert!(!flags.is_replica);
    }

    #[test]
    fn test_cluster_state_variants() {
        assert_eq!(ClusterState::Starting, ClusterState::Starting);
        assert_ne!(ClusterState::Starting, ClusterState::Connected);
    }

    #[test]
    fn test_slot_assignment() {
        let assignment = SlotAssignment {
            node_id: "node-1".to_string(),
            slot: 100,
            migrating_to: None,
            importing_from: None,
        };

        assert_eq!(assignment.node_id, "node-1");
        assert_eq!(assignment.slot, 100);
    }

    #[test]
    fn test_cluster_command_variants() {
        let ping = ClusterCommand::Ping {
            node_id: "node-1".to_string(),
            timestamp: 1000,
        };

        match ping {
            ClusterCommand::Ping { node_id, .. } => assert_eq!(node_id, "node-1"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_cluster_config_defaults() {
        let config = ClusterConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.cluster_port, 15502);
        assert_eq!(config.node_timeout_ms, 5000);
    }

    #[test]
    fn test_cluster_config_durations() {
        let config = ClusterConfig::default();

        assert_eq!(config.node_timeout(), Duration::from_millis(5000));
        assert_eq!(config.migration_timeout(), Duration::from_secs(60));
        assert_eq!(config.raft_election_timeout(), Duration::from_millis(1000));
    }

    #[test]
    fn test_cluster_config_from_env() {
        let config = ClusterConfig::from_env();
        // Should not panic
        assert!(!config.enabled);
    }

    #[test]
    fn test_topology_from_config_owns_all_slots() {
        let mut config = ClusterConfig {
            enabled: true,
            node_id: Some("n1".to_string()),
            ..Default::default()
        };
        config.node_address = "127.0.0.1:16000".parse().unwrap();

        let topology = ClusterTopology::from_config(&config).unwrap();
        assert_eq!(topology.my_node_id(), "n1");
        // A single-node cluster owns every slot → full coverage, self-routed.
        assert!(topology.has_full_coverage());
        assert_eq!(topology.get_slot_owner(0).unwrap(), "n1");
        assert_eq!(topology.get_slot_owner(TOTAL_SLOTS - 1).unwrap(), "n1");
        assert_eq!(topology.get_all_nodes().len(), 1);
    }

    #[test]
    fn test_topology_from_config_derives_node_id() {
        let config = ClusterConfig {
            enabled: true,
            node_id: None,
            node_address: "127.0.0.1:16001".parse().unwrap(),
            ..Default::default()
        };
        let topology = ClusterTopology::from_config(&config).unwrap();
        // Node id derived from the address when not set.
        assert_eq!(topology.my_node_id(), "node-127.0.0.1:16001");
    }

    #[test]
    fn test_cluster_config_partial_deserialize_uses_defaults() {
        // A partial/legacy cluster block (only `enabled`, no node_address) must
        // still deserialize via per-field serde defaults (issue #232 regression).
        let cfg: ClusterConfig = serde_json::from_str(r#"{"enabled": false}"#).unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.node_address.to_string(), "127.0.0.1:15502");
        assert_eq!(cfg.cluster_port, 15502);
        assert!(cfg.require_full_coverage);
        assert_eq!(cfg.migration_batch_size, 100);

        // The legacy `seeds` key aliases to seed_nodes.
        let cfg2: ClusterConfig =
            serde_json::from_str(r#"{"seeds": ["127.0.0.1:16005"]}"#).unwrap();
        assert_eq!(cfg2.seed_nodes.len(), 1);
    }

    #[test]
    fn test_cluster_config_from_getter_overlays_vars() {
        // Map-backed getter (no global env mutation) exercises from_env's parsing.
        let vars: std::collections::HashMap<&str, &str> = [
            ("SYNAP_CLUSTER_ENABLED", "true"),
            ("SYNAP_CLUSTER_NODE_ID", "n7"),
            ("SYNAP_CLUSTER_NODE_ADDRESS", "10.0.0.5:7000"),
            ("SYNAP_CLUSTER_SEEDS", "127.0.0.1:7001, 127.0.0.1:7002"),
            ("SYNAP_CLUSTER_MIGRATION_BATCH_SIZE", "250"),
            ("SYNAP_CLUSTER_RAFT_ELECTION_TIMEOUT_MS", "bad-number"),
        ]
        .into_iter()
        .collect();

        let cfg = ClusterConfig::from_getter(|k| vars.get(k).map(|s| s.to_string()));
        assert!(cfg.enabled);
        assert_eq!(cfg.node_id.as_deref(), Some("n7"));
        assert_eq!(cfg.node_address.to_string(), "10.0.0.5:7000");
        assert_eq!(cfg.seed_nodes.len(), 2);
        assert_eq!(cfg.migration_batch_size, 250);
        // Invalid value falls back to the default (1000), not a panic.
        assert_eq!(cfg.raft_election_timeout_ms, 1000);

        // Empty getter → all defaults, cluster disabled.
        assert!(!ClusterConfig::from_getter(|_| None).enabled);
    }

    #[tokio::test]
    async fn test_migration_cancel_rolls_back_source_intact() {
        use crate::core::{KVConfig, KVStore};
        let kv = std::sync::Arc::new(KVStore::new(KVConfig::default()));
        kv.set("mkey", b"v".to_vec(), None).await.unwrap();
        let slot = crate::cluster::hash_slot("mkey");

        let mgr =
            SlotMigrationManager::new_with_kv_store(100, Duration::from_secs(60), Some(kv.clone()));
        mgr.start_migration(slot, "src".to_string(), "dst".to_string())
            .unwrap();
        assert!(mgr.cancel_migration(slot).is_ok());

        // Let the worker process the cancel/rollback.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let m = mgr.get_migration(slot).expect("migration record");
        assert_eq!(m.state, MigrationState::Failed);
        assert_eq!(m.keys_migrated, 0); // rolled back
        // Non-destructive copy model: the source keeps its key after rollback.
        assert_eq!(kv.get("mkey").await.unwrap(), Some(b"v".to_vec()));
    }
}
