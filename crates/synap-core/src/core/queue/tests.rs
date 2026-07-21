use super::*;

#[tokio::test]
async fn test_queue_publish_consume() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("test_queue", None).await.unwrap();

    // Publish
    let msg_id = manager
        .publish("test_queue", b"Hello".to_vec(), None, None)
        .await
        .unwrap();

    assert!(!msg_id.is_empty());

    // Consume
    let message = manager.consume("test_queue", "consumer1").await.unwrap();
    assert!(message.is_some());
    assert_eq!(*message.unwrap().payload, b"Hello");
}

#[tokio::test]
async fn test_queue_priority() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("priority_queue", None).await.unwrap();

    // Publish with different priorities
    manager
        .publish("priority_queue", b"Low".to_vec(), Some(1), None)
        .await
        .unwrap();
    manager
        .publish("priority_queue", b"High".to_vec(), Some(9), None)
        .await
        .unwrap();
    manager
        .publish("priority_queue", b"Medium".to_vec(), Some(5), None)
        .await
        .unwrap();

    // Consume in priority order
    let msg1 = manager
        .consume("priority_queue", "c1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*msg1.payload, b"High");
    assert_eq!(msg1.priority, 9);

    let msg2 = manager
        .consume("priority_queue", "c1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*msg2.payload, b"Medium");
    assert_eq!(msg2.priority, 5);

    let msg3 = manager
        .consume("priority_queue", "c1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*msg3.payload, b"Low");
    assert_eq!(msg3.priority, 1);
}

#[tokio::test]
async fn test_queue_ack_nack() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("ack_queue", None).await.unwrap();

    // Publish
    let msg_id = manager
        .publish("ack_queue", b"Test".to_vec(), None, None)
        .await
        .unwrap();

    // Consume
    let message = manager.consume("ack_queue", "c1").await.unwrap().unwrap();
    assert_eq!(message.id, msg_id);

    // ACK
    let result = manager.ack("ack_queue", &msg_id).await;
    assert!(result.is_ok());

    // Second ACK should fail
    let result = manager.ack("ack_queue", &msg_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_queue_nack_requeue() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("nack_queue", None).await.unwrap();

    // Publish
    let msg_id = manager
        .publish("nack_queue", b"Test".to_vec(), None, Some(2))
        .await
        .unwrap();

    // Consume
    manager.consume("nack_queue", "c1").await.unwrap();

    // NACK with requeue
    manager.nack("nack_queue", &msg_id, true).await.unwrap();

    // Should be back in queue
    let message = manager.consume("nack_queue", "c1").await.unwrap().unwrap();
    assert_eq!(message.id, msg_id);
    assert_eq!(message.retry_count, 1);
}

#[tokio::test]
async fn test_queue_dead_letter() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("dlq_queue", None).await.unwrap();

    // Publish with max 0 retries (first NACK goes to DLQ)
    let msg_id = manager
        .publish("dlq_queue", b"Test".to_vec(), None, Some(0))
        .await
        .unwrap();

    // Consume
    let message = manager.consume("dlq_queue", "c1").await.unwrap();
    assert!(message.is_some());

    // NACK - should go directly to DLQ (retry_count=0 >= max_retries=0)
    manager.nack("dlq_queue", &msg_id, true).await.unwrap();

    // Message should be in DLQ now, queue should be empty
    let stats = manager.stats("dlq_queue").await.unwrap();
    assert_eq!(stats.depth, 0); // No messages in queue
    assert_eq!(stats.dead_lettered, 1); // One in DLQ
    assert_eq!(stats.nacked, 1); // One NACK

    // Verify queue is empty
    let message = manager.consume("dlq_queue", "c1").await.unwrap();
    assert!(message.is_none());
}

// ==================== DEADLINE SWEEP TESTS (M-017) ====================

fn queue_with_deadline(secs: u64) -> Queue {
    Queue::new(
        "dl".to_string(),
        QueueConfig {
            ack_deadline_secs: secs,
            ..QueueConfig::default()
        },
    )
}

#[test]
fn test_deadline_requeue_expired() {
    // ack_deadline_secs = 0 → deadline == now, so it is immediately due.
    let mut queue = queue_with_deadline(0);
    queue
        .publish(QueueMessage::new(b"job".to_vec(), 5, 3))
        .unwrap();

    let consumed = queue.consume("c1".to_string()).unwrap();
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(queue.messages.len(), 0);

    queue.check_expired_pending();

    // Expired message is requeued: back in `messages`, out of `pending`.
    assert_eq!(queue.pending.len(), 0);
    assert_eq!(queue.messages.len(), 1);
    assert_eq!(queue.messages[0].id, consumed.id);
    assert_eq!(queue.stats.nacked, 1);
}

#[test]
fn test_deadline_skips_acked_stale_entry() {
    let mut queue = queue_with_deadline(0);
    let id = queue
        .publish(QueueMessage::new(b"job".to_vec(), 5, 3))
        .unwrap();

    queue.consume("c1".to_string()).unwrap();
    queue.ack(&id).unwrap();
    // Heap still holds a stale (deadline, id) entry for the acked message.
    assert!(!queue.deadlines.is_empty());

    queue.check_expired_pending();

    // Stale entry is discarded, not requeued (no phantom message reappears).
    assert_eq!(queue.messages.len(), 0);
    assert_eq!(queue.pending.len(), 0);
    assert_eq!(queue.stats.nacked, 0);
    assert!(queue.deadlines.is_empty());
}

#[test]
fn test_deadline_keeps_unexpired() {
    let mut queue = queue_with_deadline(1000);
    queue
        .publish(QueueMessage::new(b"job".to_vec(), 5, 3))
        .unwrap();
    queue.consume("c1".to_string()).unwrap();

    queue.check_expired_pending();

    // Deadline far in the future → message stays in-flight.
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(queue.messages.len(), 0);
    assert_eq!(queue.stats.nacked, 0);
}

// ============ ACTIVE-CONSUMER COUNT TESTS (M-013) ============

#[tokio::test]
async fn test_active_consumer_count_is_honest() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("q", None).await.unwrap();
    for i in 0..2u8 {
        manager.publish("q", vec![i], None, None).await.unwrap();
    }

    // Nobody consuming yet.
    assert_eq!(manager.stats("q").await.unwrap().consumers, 0);

    let m1 = manager.consume("q", "c1").await.unwrap().unwrap();
    assert_eq!(manager.stats("q").await.unwrap().consumers, 1);

    let m2 = manager.consume("q", "c2").await.unwrap().unwrap();
    assert_eq!(manager.stats("q").await.unwrap().consumers, 2);

    // Each ACK releases its consumer.
    manager.ack("q", &m1.id).await.unwrap();
    assert_eq!(manager.stats("q").await.unwrap().consumers, 1);
    manager.ack("q", &m2.id).await.unwrap();
    assert_eq!(manager.stats("q").await.unwrap().consumers, 0);
}

#[tokio::test]
async fn test_same_consumer_counts_once() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("q", None).await.unwrap();
    for i in 0..2u8 {
        manager.publish("q", vec![i], None, None).await.unwrap();
    }

    let a = manager.consume("q", "c1").await.unwrap().unwrap();
    let b = manager.consume("q", "c1").await.unwrap().unwrap();
    // One distinct consumer holding two in-flight messages.
    assert_eq!(manager.stats("q").await.unwrap().consumers, 1);

    manager.ack("q", &a.id).await.unwrap();
    // Still active: it holds the second message.
    assert_eq!(manager.stats("q").await.unwrap().consumers, 1);
    manager.ack("q", &b.id).await.unwrap();
    assert_eq!(manager.stats("q").await.unwrap().consumers, 0);
}

// ============ PREFETCH / FAIR-DISPATCH TESTS (M-013, phase6f 1.4/1.5) ============

#[tokio::test]
async fn test_queue_prefetch_throttles_and_dispatches_fairly() {
    let cfg = QueueConfig {
        prefetch_limit: 1,
        ..QueueConfig::default()
    };
    let manager = QueueManager::new(cfg);
    manager.create_queue("q", None).await.unwrap();
    for i in 0..3u8 {
        manager.publish("q", vec![i], None, None).await.unwrap();
    }

    // c1 pulls one message and then hits its prefetch limit.
    let m1 = manager.consume("q", "c1").await.unwrap();
    assert!(m1.is_some());
    assert!(
        manager.consume("q", "c1").await.unwrap().is_none(),
        "c1 at prefetch=1 must be throttled until it acks"
    );

    // Fair dispatch: c2 can still pull while c1 is throttled.
    let m2 = manager.consume("q", "c2").await.unwrap();
    assert!(m2.is_some());

    // After c1 acks, it may pull again.
    manager.ack("q", &m1.unwrap().id).await.unwrap();
    assert!(manager.consume("q", "c1").await.unwrap().is_some());

    // Two consumers each hold one in-flight message.
    assert_eq!(manager.stats("q").await.unwrap().consumers, 2);
    assert_eq!(manager.stats("q").await.unwrap().depth, 0);
}

#[tokio::test]
async fn test_queue_prefetch_zero_is_unlimited() {
    // Default prefetch_limit = 0 preserves unthrottled pull behavior.
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("q", None).await.unwrap();
    for i in 0..5u8 {
        manager.publish("q", vec![i], None, None).await.unwrap();
    }
    for _ in 0..5 {
        assert!(manager.consume("q", "c1").await.unwrap().is_some());
    }
    assert_eq!(manager.stats("q").await.unwrap().consumers, 1);
}

#[tokio::test]
async fn test_queue_stats() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("stats_queue", None).await.unwrap();

    // Publish 5 messages
    for i in 0..5 {
        manager
            .publish("stats_queue", format!("msg{}", i).into_bytes(), None, None)
            .await
            .unwrap();
    }

    let stats = manager.stats("stats_queue").await.unwrap();
    assert_eq!(stats.depth, 5);
    assert_eq!(stats.published, 5);
}

#[tokio::test]
async fn test_queue_purge() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("purge_queue", None).await.unwrap();

    // Add messages
    for i in 0..10 {
        manager
            .publish("purge_queue", format!("msg{}", i).into_bytes(), None, None)
            .await
            .unwrap();
    }

    // Purge
    let count = manager.purge("purge_queue").await.unwrap();
    assert_eq!(count, 10);

    // Verify empty
    let stats = manager.stats("purge_queue").await.unwrap();
    assert_eq!(stats.depth, 0);
}

#[tokio::test]
async fn test_list_queues() {
    let manager = QueueManager::new(QueueConfig::default());

    manager.create_queue("queue1", None).await.unwrap();
    manager.create_queue("queue2", None).await.unwrap();
    manager.create_queue("queue3", None).await.unwrap();

    let queues = manager.list_queues().await.unwrap();
    assert_eq!(queues.len(), 3);
    assert!(queues.contains(&"queue1".to_string()));
    assert!(queues.contains(&"queue2".to_string()));
    assert!(queues.contains(&"queue3".to_string()));
}

#[tokio::test]
async fn test_delete_queue() {
    let manager = QueueManager::new(QueueConfig::default());
    manager.create_queue("temp_queue", None).await.unwrap();

    let deleted = manager.delete_queue("temp_queue").await.unwrap();
    assert!(deleted);

    let deleted = manager.delete_queue("temp_queue").await.unwrap();
    assert!(!deleted);
}

// ==================== CONCURRENCY TESTS ====================
// These tests ensure no duplicate processing when multiple consumers
// are competing for messages

#[tokio::test]
async fn test_concurrent_consumers_no_duplicates() {
    use std::collections::HashSet;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let manager = Arc::new(QueueManager::new(QueueConfig::default()));
    manager
        .create_queue("concurrent_queue", None)
        .await
        .unwrap();

    // Publish 100 messages
    let num_messages = 100;
    for i in 0..num_messages {
        manager
            .publish(
                "concurrent_queue",
                format!("msg-{}", i).into_bytes(),
                None,
                None,
            )
            .await
            .unwrap();
    }

    // Track consumed messages
    let consumed = Arc::new(Mutex::new(HashSet::new()));
    let mut handles = vec![];

    // Spawn 10 concurrent consumers
    for consumer_id in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let consumed_clone = Arc::clone(&consumed);

        let handle = tokio::spawn(async move {
            let consumer_name = format!("consumer-{}", consumer_id);

            // Each consumer tries to consume messages
            loop {
                match manager_clone
                    .consume("concurrent_queue", &consumer_name)
                    .await
                {
                    Ok(Some(msg)) => {
                        let mut set = consumed_clone.lock().await;
                        let message_content = String::from_utf8_lossy(&msg.payload).to_string();

                        // Check for duplicates - this should NEVER happen
                        assert!(
                            !set.contains(&message_content),
                            "DUPLICATE MESSAGE DETECTED: {} consumed by {}",
                            message_content,
                            consumer_name
                        );

                        set.insert(message_content.clone());
                        drop(set); // Release lock

                        // ACK the message
                        manager_clone
                            .ack("concurrent_queue", &msg.id)
                            .await
                            .unwrap();
                    }
                    Ok(None) => {
                        // Queue empty, we're done
                        break;
                    }
                    Err(e) => {
                        panic!("Unexpected error: {}", e);
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all consumers to finish
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all messages were consumed exactly once
    let final_consumed = consumed.lock().await;
    assert_eq!(
        final_consumed.len(),
        num_messages,
        "Expected {} messages, got {}",
        num_messages,
        final_consumed.len()
    );
}

#[tokio::test]
async fn test_high_concurrency_stress_test() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let manager = Arc::new(QueueManager::new(QueueConfig::default()));
    manager.create_queue("stress_queue", None).await.unwrap();

    // Publish 1000 messages
    let num_messages = 1000;
    for i in 0..num_messages {
        manager
            .publish(
                "stress_queue",
                format!("msg-{:04}", i).into_bytes(),
                None,
                None,
            )
            .await
            .unwrap();
    }

    // Counter for consumed messages
    let consumed_count = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Spawn 50 concurrent consumers (high contention)
    for consumer_id in 0..50 {
        let manager_clone = Arc::clone(&manager);
        let counter_clone = Arc::clone(&consumed_count);

        let handle = tokio::spawn(async move {
            let consumer_name = format!("consumer-{}", consumer_id);
            let mut local_count = 0;

            loop {
                match manager_clone.consume("stress_queue", &consumer_name).await {
                    Ok(Some(msg)) => {
                        local_count += 1;
                        counter_clone.fetch_add(1, Ordering::SeqCst);

                        // ACK immediately
                        manager_clone.ack("stress_queue", &msg.id).await.unwrap();
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(e) => {
                        panic!("Unexpected error in consumer {}: {}", consumer_name, e);
                    }
                }
            }

            local_count
        });

        handles.push(handle);
    }

    // Wait for all consumers and collect individual counts
    let mut total_from_consumers = 0;
    for handle in handles {
        let count = handle.await.unwrap();
        total_from_consumers += count;
    }

    // Verify counts match
    let final_count = consumed_count.load(Ordering::SeqCst);
    assert_eq!(
        final_count, num_messages,
        "Expected {} consumed messages, got {}",
        num_messages, final_count
    );
    assert_eq!(
        total_from_consumers, num_messages,
        "Sum of individual consumer counts ({}) doesn't match total ({})",
        total_from_consumers, num_messages
    );
}

#[tokio::test]
async fn test_concurrent_publish_and_consume() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let manager = Arc::new(QueueManager::new(QueueConfig::default()));
    manager.create_queue("pubsub_queue", None).await.unwrap();

    let published_count = Arc::new(AtomicUsize::new(0));
    let consumed_count = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];

    // Spawn 5 publishers
    for publisher_id in 0..5 {
        let manager_clone = Arc::clone(&manager);
        let counter_clone = Arc::clone(&published_count);

        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let payload = format!("pub-{}-msg-{}", publisher_id, i).into_bytes();
                manager_clone
                    .publish("pubsub_queue", payload, None, None)
                    .await
                    .unwrap();
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        handles.push(handle);
    }

    // Spawn 10 consumers (running concurrently with publishers)
    for consumer_id in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let counter_clone = Arc::clone(&consumed_count);

        let handle = tokio::spawn(async move {
            let consumer_name = format!("consumer-{}", consumer_id);

            // Keep consuming until we don't get messages for a while
            let mut empty_attempts = 0;
            while empty_attempts < 10 {
                match manager_clone.consume("pubsub_queue", &consumer_name).await {
                    Ok(Some(msg)) => {
                        counter_clone.fetch_add(1, Ordering::SeqCst);
                        manager_clone.ack("pubsub_queue", &msg.id).await.unwrap();
                        empty_attempts = 0; // Reset
                    }
                    Ok(None) => {
                        empty_attempts += 1;
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Err(e) => {
                        panic!("Unexpected error: {}", e);
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Give a bit of time for final processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let published = published_count.load(Ordering::SeqCst);
    let consumed = consumed_count.load(Ordering::SeqCst);

    assert_eq!(published, 500, "Expected 500 published messages");
    assert_eq!(consumed, 500, "Expected all 500 messages to be consumed");
}

#[tokio::test]
async fn test_no_message_loss_under_contention() {
    use std::collections::HashSet;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let manager = Arc::new(QueueManager::new(QueueConfig::default()));
    manager.create_queue("no_loss_queue", None).await.unwrap();

    // Publish 500 uniquely identifiable messages
    let num_messages = 500;
    let mut expected_messages = HashSet::new();

    for i in 0..num_messages {
        let msg_id = format!("unique-msg-{:05}", i);
        expected_messages.insert(msg_id.clone());
        manager
            .publish("no_loss_queue", msg_id.into_bytes(), None, None)
            .await
            .unwrap();
    }

    // Track received messages
    let received = Arc::new(Mutex::new(HashSet::new()));
    let mut handles = vec![];

    // Spawn 20 consumers with aggressive competition
    for consumer_id in 0..20 {
        let manager_clone = Arc::clone(&manager);
        let received_clone = Arc::clone(&received);

        let handle = tokio::spawn(async move {
            let consumer_name = format!("consumer-{}", consumer_id);

            loop {
                match manager_clone.consume("no_loss_queue", &consumer_name).await {
                    Ok(Some(msg)) => {
                        let msg_content = String::from_utf8_lossy(&msg.payload).to_string();

                        let mut set = received_clone.lock().await;

                        // Detect duplicates
                        if set.contains(&msg_content) {
                            panic!("DUPLICATE: Message '{}' consumed twice!", msg_content);
                        }

                        set.insert(msg_content);
                        drop(set);

                        // ACK
                        manager_clone.ack("no_loss_queue", &msg.id).await.unwrap();
                    }
                    Ok(None) => break,
                    Err(e) => panic!("Error: {}", e),
                }
            }
        });

        handles.push(handle);
    }

    // Wait for completion
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all messages received exactly once
    let final_received = received.lock().await;

    assert_eq!(
        final_received.len(),
        num_messages,
        "Expected {} messages, received {}",
        num_messages,
        final_received.len()
    );

    // Verify we got exactly the messages we sent
    for expected in &expected_messages {
        assert!(
            final_received.contains(expected),
            "Message '{}' was never received!",
            expected
        );
    }
}

#[tokio::test]
async fn test_priority_with_concurrent_consumers() {
    use std::sync::Arc;

    let manager = Arc::new(QueueManager::new(QueueConfig::default()));
    manager
        .create_queue("priority_concurrent", None)
        .await
        .unwrap();

    // Publish messages with different priorities
    for i in 0..30 {
        let priority = match i % 3 {
            0 => 9, // High
            1 => 5, // Medium
            _ => 1, // Low
        };
        manager
            .publish(
                "priority_concurrent",
                format!("msg-{}", i).into_bytes(),
                Some(priority),
                None,
            )
            .await
            .unwrap();
    }

    let mut handles = vec![];
    let manager_clone = Arc::clone(&manager);

    // Spawn 5 concurrent consumers
    for consumer_id in 0..5 {
        let manager_clone2 = Arc::clone(&manager_clone);

        let handle = tokio::spawn(async move {
            let consumer_name = format!("consumer-{}", consumer_id);
            let mut consumed = vec![];

            loop {
                match manager_clone2
                    .consume("priority_concurrent", &consumer_name)
                    .await
                {
                    Ok(Some(msg)) => {
                        consumed.push((
                            msg.priority,
                            String::from_utf8_lossy(&msg.payload).to_string(),
                        ));
                        manager_clone2
                            .ack("priority_concurrent", &msg.id)
                            .await
                            .unwrap();
                    }
                    Ok(None) => break,
                    Err(e) => panic!("Error: {}", e),
                }
            }

            consumed
        });

        handles.push(handle);
    }

    // Collect all consumed messages
    let mut all_consumed = vec![];
    for handle in handles {
        let mut consumed = handle.await.unwrap();
        all_consumed.append(&mut consumed);
    }

    // Verify all 30 messages were consumed
    assert_eq!(
        all_consumed.len(),
        30,
        "All messages should be consumed exactly once"
    );

    // Verify higher priority messages tend to come first (not strict ordering due to concurrency)
    let high_priority_indices: Vec<usize> = all_consumed
        .iter()
        .enumerate()
        .filter(|(_, (prio, _))| *prio == 9)
        .map(|(idx, _)| idx)
        .collect();

    let low_priority_indices: Vec<usize> = all_consumed
        .iter()
        .enumerate()
        .filter(|(_, (prio, _))| *prio == 1)
        .map(|(idx, _)| idx)
        .collect();

    // On average, high priority should come before low priority
    if !high_priority_indices.is_empty() && !low_priority_indices.is_empty() {
        let avg_high: f64 =
            high_priority_indices.iter().sum::<usize>() as f64 / high_priority_indices.len() as f64;
        let avg_low: f64 =
            low_priority_indices.iter().sum::<usize>() as f64 / low_priority_indices.len() as f64;

        assert!(
            avg_high < avg_low,
            "High priority messages should generally come before low priority (avg high: {}, avg low: {})",
            avg_high,
            avg_low
        );
    }
}
