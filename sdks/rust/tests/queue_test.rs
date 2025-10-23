//! Comprehensive tests for Queue operations

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;

    #[tokio::test]
    async fn test_queue_create() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.create",
                "payload": {"queue_name": "test_queue"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let result = client.queue().create_queue("test_queue", None, None).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_create_with_options() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.create",
                "payload": {
                    "queue_name": "custom_queue",
                    "max_depth": 1000,
                    "ack_deadline_secs": 30
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let result = client
            .queue()
            .create_queue("custom_queue", Some(1000), Some(30))
            .await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_publish() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.publish",
                "payload": {
                    "queue_name": "test_queue",
                    "priority": 9,
                    "max_retries": 3
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"message_id": "msg-123"}}"#)
            .create_async()
            .await;

        let msg_id = client
            .queue()
            .publish("test_queue", b"test", Some(9), Some(3))
            .await
            .unwrap();
        assert_eq!(msg_id, "msg-123");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_consume() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.consume",
                "payload": {
                    "queue_name": "test_queue",
                    "consumer_id": "worker-1"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"id": "msg-123", "payload": [116,101,115,116], "priority": 5, "retry_count": 0, "max_retries": 3, "deadline": null}}"#)
            .create_async()
            .await;

        let message = client
            .queue()
            .consume("test_queue", "worker-1")
            .await
            .unwrap();
        assert!(message.is_some());
        let msg = message.unwrap();
        assert_eq!(msg.id, "msg-123");
        assert_eq!(msg.priority, 5);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_consume_empty() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.consume",
                "payload": {"queue_name": "empty_queue", "consumer_id": "worker-1"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": null}"#)
            .create_async()
            .await;

        let message = client
            .queue()
            .consume("empty_queue", "worker-1")
            .await
            .unwrap();
        assert!(message.is_none());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_ack() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.ack",
                "payload": {"queue_name": "test_queue", "message_id": "msg-123"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let result = client.queue().ack("test_queue", "msg-123").await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_nack() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.nack",
                "payload": {"queue_name": "test_queue", "message_id": "msg-123"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let result = client.queue().nack("test_queue", "msg-123").await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.stats",
                "payload": {"queue_name": "test_queue"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"depth": 10, "pending": 5, "max_depth": 1000, "total_published": 100, "total_consumed": 90, "total_acked": 85, "total_nacked": 5, "dlq_count": 2}}"#)
            .create_async()
            .await;

        let stats = client.queue().stats("test_queue").await.unwrap();
        assert_eq!(stats.depth, 10);
        assert_eq!(stats.total_published, 100);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_list() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.list",
                "payload": {}
            })))
            .with_status(200)
            .with_body(
                r#"{"success": true, "payload": {"queues": ["queue1", "queue2", "queue3"]}}"#,
            )
            .create_async()
            .await;

        let queues = client.queue().list().await.unwrap();
        assert_eq!(queues.len(), 3);
        assert_eq!(queues[0], "queue1");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_queue_delete() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "queue.delete",
                "payload": {"queue_name": "test_queue"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let result = client.queue().delete_queue("test_queue").await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }
}
