/// Integration tests for Kafka-style partitioning and consumer groups
use std::sync::Arc;
use synap_server::{
    AssignmentStrategy, ConsumerGroupConfig, ConsumerGroupManager, PartitionConfig,
    PartitionManager, RetentionPolicy,
};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_kafka_style_publish_consume_with_consumer_group() {
    // Create partition manager with 3 partitions
    let partition_config = PartitionConfig {
        num_partitions: 3,
        replication_factor: 1,
        retention: RetentionPolicy::Messages { max_messages: 100 },
        ..Default::default()
    };

    let partition_manager = Arc::new(PartitionManager::new(partition_config));

    // Create topic
    partition_manager
        .create_topic("orders", None)
        .await
        .expect("Failed to create topic");

    // Publish events with keys
    for i in 0..30 {
        let key = format!("customer-{}", i % 10);
        let data = format!("order-{}", i).into_bytes();

        partition_manager
            .publish("orders", "order.created", Some(key.into_bytes()), data)
            .await
            .expect("Failed to publish");
    }

    // Create consumer group manager
    let cg_config = ConsumerGroupConfig {
        strategy: AssignmentStrategy::RoundRobin,
        session_timeout_secs: 30,
        ..Default::default()
    };

    let cg_manager = Arc::new(ConsumerGroupManager::new(cg_config));

    // Create consumer group
    cg_manager
        .create_group("order-processors", "orders", 3, None)
        .await
        .expect("Failed to create consumer group");

    // Join 3 consumers
    let member1 = cg_manager
        .join_group("order-processors", 30)
        .await
        .expect("Failed to join group");

    let member2 = cg_manager
        .join_group("order-processors", 30)
        .await
        .expect("Failed to join group");

    let member3 = cg_manager
        .join_group("order-processors", 30)
        .await
        .expect("Failed to join group");

    // Trigger rebalance
    cg_manager
        .rebalance_group("order-processors")
        .await
        .expect("Failed to rebalance");

    // Get assignments
    let assignment1 = cg_manager
        .get_assignment("order-processors", &member1.id)
        .await
        .expect("Failed to get assignment");

    let assignment2 = cg_manager
        .get_assignment("order-processors", &member2.id)
        .await
        .expect("Failed to get assignment");

    let assignment3 = cg_manager
        .get_assignment("order-processors", &member3.id)
        .await
        .expect("Failed to get assignment");

    // Each consumer should have 1 partition
    assert_eq!(assignment1.len(), 1);
    assert_eq!(assignment2.len(), 1);
    assert_eq!(assignment3.len(), 1);

    // Total partitions = 3
    let mut all_partitions = assignment1.clone();
    all_partitions.extend(assignment2);
    all_partitions.extend(assignment3);
    all_partitions.sort();
    assert_eq!(all_partitions, vec![0, 1, 2]);

    // Consume from assigned partitions
    let mut total_consumed = 0;

    for partition_id in assignment1 {
        let events = partition_manager
            .consume_partition("orders", partition_id, 0, 100)
            .await
            .expect("Failed to consume");

        total_consumed += events.len();

        // Commit offset
        if let Some(last_event) = events.last() {
            cg_manager
                .commit_offset("order-processors", partition_id, last_event.offset + 1)
                .await
                .expect("Failed to commit offset");
        }
    }

    // Should have consumed some events
    assert!(total_consumed > 0);
    assert!(total_consumed <= 30);
}

#[tokio::test]
async fn test_retention_policy_time_based() {
    let partition_config = PartitionConfig {
        num_partitions: 1,
        retention: RetentionPolicy::Time { retention_secs: 1 },
        ..Default::default()
    };

    let partition_manager = Arc::new(PartitionManager::new(partition_config));

    partition_manager
        .create_topic("temp-events", None)
        .await
        .expect("Failed to create topic");

    // Publish events
    for i in 0..10 {
        partition_manager
            .publish(
                "temp-events",
                "event",
                None,
                format!("data-{}", i).into_bytes(),
            )
            .await
            .expect("Failed to publish");
    }

    // Check stats before retention
    let stats = partition_manager
        .topic_stats("temp-events")
        .await
        .expect("Failed to get stats");

    assert_eq!(stats[0].message_count, 10);

    // Wait for retention to kick in
    sleep(Duration::from_secs(2)).await;

    // Trigger compaction
    partition_manager.compact_all().await;

    // Check stats after retention
    let stats_after = partition_manager
        .topic_stats("temp-events")
        .await
        .expect("Failed to get stats");

    // Events should be removed
    assert_eq!(stats_after[0].message_count, 0);
}

#[tokio::test]
async fn test_retention_policy_size_based() {
    let partition_config = PartitionConfig {
        num_partitions: 1,
        retention: RetentionPolicy::Size { max_bytes: 200 },
        ..Default::default()
    };

    let partition_manager = Arc::new(PartitionManager::new(partition_config));

    partition_manager
        .create_topic("size-limited", None)
        .await
        .expect("Failed to create topic");

    // Publish large events
    for i in 0..20 {
        partition_manager
            .publish(
                "size-limited",
                "event",
                None,
                vec![i as u8; 50], // 50 bytes each
            )
            .await
            .expect("Failed to publish");
    }

    // Trigger compaction
    partition_manager.compact_all().await;

    let stats = partition_manager
        .topic_stats("size-limited")
        .await
        .expect("Failed to get stats");

    // Should respect size limit
    assert!(stats[0].total_bytes <= 200);
}

#[tokio::test]
async fn test_consumer_group_rebalancing() {
    let partition_manager = Arc::new(PartitionManager::new(PartitionConfig {
        num_partitions: 6,
        ..Default::default()
    }));

    partition_manager
        .create_topic("rebalance-test", None)
        .await
        .expect("Failed to create topic");

    let cg_manager = Arc::new(ConsumerGroupManager::new(
        ConsumerGroupConfig::default(),
    ));

    cg_manager
        .create_group("rebalance-group", "rebalance-test", 6, None)
        .await
        .expect("Failed to create group");

    // Join 2 members
    let member1 = cg_manager
        .join_group("rebalance-group", 30)
        .await
        .expect("Failed to join");

    let member2 = cg_manager
        .join_group("rebalance-group", 30)
        .await
        .expect("Failed to join");

    cg_manager
        .rebalance_group("rebalance-group")
        .await
        .expect("Failed to rebalance");

    let assignment1 = cg_manager
        .get_assignment("rebalance-group", &member1.id)
        .await
        .expect("Failed to get assignment");

    let assignment2 = cg_manager
        .get_assignment("rebalance-group", &member2.id)
        .await
        .expect("Failed to get assignment");

    // Each should have 3 partitions
    assert_eq!(assignment1.len(), 3);
    assert_eq!(assignment2.len(), 3);

    // Member 2 leaves
    cg_manager
        .leave_group("rebalance-group", &member2.id)
        .await
        .expect("Failed to leave");

    cg_manager
        .rebalance_group("rebalance-group")
        .await
        .expect("Failed to rebalance");

    let new_assignment1 = cg_manager
        .get_assignment("rebalance-group", &member1.id)
        .await
        .expect("Failed to get assignment");

    // Member 1 should now have all 6 partitions
    assert_eq!(new_assignment1.len(), 6);
}

#[tokio::test]
async fn test_partition_key_routing_consistency() {
    let partition_manager = Arc::new(PartitionManager::new(PartitionConfig {
        num_partitions: 3,
        ..Default::default()
    }));

    partition_manager
        .create_topic("user-events", None)
        .await
        .expect("Failed to create topic");

    // Publish multiple events with same key
    let user_key = "user-123".to_string();
    let mut partition_ids = Vec::new();

    for i in 0..10 {
        let (partition_id, _) = partition_manager
            .publish(
                "user-events",
                "event",
                Some(user_key.clone().into_bytes()),
                format!("event-{}", i).into_bytes(),
            )
            .await
            .expect("Failed to publish");

        partition_ids.push(partition_id);
    }

    // All events with same key should go to same partition
    let first_partition = partition_ids[0];
    for partition_id in partition_ids {
        assert_eq!(partition_id, first_partition);
    }
}

#[tokio::test]
async fn test_combined_retention_policy() {
    let partition_config = PartitionConfig {
        num_partitions: 1,
        retention: RetentionPolicy::Combined {
            retention_secs: Some(3600),
            max_bytes: Some(500),
            max_messages: Some(10),
        },
        ..Default::default()
    };

    let partition_manager = Arc::new(PartitionManager::new(partition_config));

    partition_manager
        .create_topic("combined", None)
        .await
        .expect("Failed to create topic");

    // Publish 20 messages
    for i in 0..20 {
        partition_manager
            .publish("combined", "event", None, vec![i; 20])
            .await
            .expect("Failed to publish");
    }

    // Trigger compaction
    partition_manager.compact_all().await;

    let stats = partition_manager
        .topic_stats("combined")
        .await
        .expect("Failed to get stats");

    // Should be limited by max_messages=10 (smallest limit)
    assert_eq!(stats[0].message_count, 10);
}

#[tokio::test]
async fn test_multiple_consumer_groups_same_topic() {
    let partition_manager = Arc::new(PartitionManager::new(PartitionConfig {
        num_partitions: 3,
        ..Default::default()
    }));

    partition_manager
        .create_topic("shared-topic", None)
        .await
        .expect("Failed to create topic");

    // Publish events
    for i in 0..15 {
        partition_manager
            .publish("shared-topic", "event", None, format!("data-{}", i).into_bytes())
            .await
            .expect("Failed to publish");
    }

    let cg_manager = Arc::new(ConsumerGroupManager::new(
        ConsumerGroupConfig::default(),
    ));

    // Create two consumer groups for same topic
    cg_manager
        .create_group("group-a", "shared-topic", 3, None)
        .await
        .expect("Failed to create group A");

    cg_manager
        .create_group("group-b", "shared-topic", 3, None)
        .await
        .expect("Failed to create group B");

    // Join members to both groups
    let member_a = cg_manager
        .join_group("group-a", 30)
        .await
        .expect("Failed to join group A");

    let member_b = cg_manager
        .join_group("group-b", 30)
        .await
        .expect("Failed to join group B");

    cg_manager
        .rebalance_group("group-a")
        .await
        .expect("Failed to rebalance A");

    cg_manager
        .rebalance_group("group-b")
        .await
        .expect("Failed to rebalance B");

    // Both should get all partitions (independent consumption)
    let assignment_a = cg_manager
        .get_assignment("group-a", &member_a.id)
        .await
        .expect("Failed to get assignment A");

    let assignment_b = cg_manager
        .get_assignment("group-b", &member_b.id)
        .await
        .expect("Failed to get assignment B");

    assert_eq!(assignment_a.len(), 3);
    assert_eq!(assignment_b.len(), 3);
}


