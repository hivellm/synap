//! Tests for Queue operations

#[cfg(test)]
mod tests {
    use crate::tests::helpers::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;

    #[tokio::test]
    async fn test_queue_create() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/queue/tasks")
            .match_body(Matcher::Json(json!({
                "max_depth": 10000,
                "ack_deadline_secs": 30
            })))
            .with_status(200)
            .with_body(r#"{"success": true}"#)
            .create_async()
            .await;

        let result = client
            .queue()
            .create_queue("tasks", Some(10000), Some(30))
            .await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_publish() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/queue/tasks/publish")
            .match_body(Matcher::Json(json!({
                "payload": [104, 101, 108, 108, 111], // "hello"
                "priority": 9,
                "max_retries": 3
            })))
            .with_status(200)
            .with_body(r#"{"message_id": "msg-123"}"#)
            .create_async()
            .await;

        let msg_id = client
            .queue()
            .publish("tasks", b"hello", Some(9), Some(3))
            .await
            .unwrap();
        assert_eq!(msg_id, "msg-123");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_consume() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("GET", "/queue/tasks/consume/worker-1")
            .with_status(200)
            .with_body(
                r#"{
                "id": "msg-123",
                "payload": [104, 101, 108, 108, 111],
                "priority": 9,
                "retry_count": 0,
                "max_retries": 3,
                "deadline": 1234567890
            }"#,
            )
            .create_async()
            .await;

        let message = client.queue().consume("tasks", "worker-1").await.unwrap();
        assert!(message.is_some());

        let msg = message.unwrap();
        assert_eq!(msg.id, "msg-123");
        assert_eq!(msg.priority, 9);
        assert_eq!(msg.payload, b"hello");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_consume_empty() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("GET", "/queue/tasks/consume/worker-1")
            .with_status(200)
            .with_body("null")
            .create_async()
            .await;

        let message = client.queue().consume("tasks", "worker-1").await.unwrap();
        assert!(message.is_none());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_ack() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/queue/tasks/ack")
            .match_body(Matcher::Json(json!({"message_id": "msg-123"})))
            .with_status(200)
            .with_body(r#"{"success": true}"#)
            .create_async()
            .await;

        let result = client.queue().ack("tasks", "msg-123").await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_nack() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/queue/tasks/nack")
            .match_body(Matcher::Json(json!({"message_id": "msg-123"})))
            .with_status(200)
            .with_body(r#"{"success": true}"#)
            .create_async()
            .await;

        let result = client.queue().nack("tasks", "msg-123").await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("GET", "/queue/tasks/stats")
            .with_status(200)
            .with_body(
                r#"{
                "depth": 10,
                "pending": 5,
                "max_depth": 10000,
                "total_published": 100,
                "total_consumed": 90,
                "total_acked": 85,
                "total_nacked": 5,
                "dlq_count": 0
            }"#,
            )
            .create_async()
            .await;

        let stats = client.queue().stats("tasks").await.unwrap();
        assert_eq!(stats.depth, 10);
        assert_eq!(stats.pending, 5);
        assert_eq!(stats.total_published, 100);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_list() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("GET", "/queue/list")
            .with_status(200)
            .with_body(r#"{"queues": ["tasks", "jobs", "notifications"]}"#)
            .create_async()
            .await;

        let queues = client.queue().list().await.unwrap();
        assert_eq!(queues.len(), 3);
        assert!(queues.contains(&"tasks".to_string()));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_delete() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("DELETE", "/queue/tasks")
            .with_status(200)
            .with_body(r#"{"success": true}"#)
            .create_async()
            .await;

        let result = client.queue().delete_queue("tasks").await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }
}
