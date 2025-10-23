//! Comprehensive tests for Pub/Sub operations

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;

    #[tokio::test]
    async fn test_pubsub_publish() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "pubsub.publish",
                "payload": {
                    "topic": "user.created",
                    "data": {"id": 123}
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"delivered_count": 5}}"#)
            .create_async()
            .await;

        let count = client
            .pubsub()
            .publish("user.created", json!({"id": 123}), None, None)
            .await
            .unwrap();
        assert_eq!(count, 5);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pubsub_publish_with_priority() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "pubsub.publish",
                "payload": {
                    "topic": "alerts.critical",
                    "data": {"message": "Server down"},
                    "priority": 9
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"delivered_count": 3}}"#)
            .create_async()
            .await;

        let count = client
            .pubsub()
            .publish(
                "alerts.critical",
                json!({"message": "Server down"}),
                Some(9),
                None,
            )
            .await
            .unwrap();
        assert_eq!(count, 3);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pubsub_subscribe() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "pubsub.subscribe",
                "payload": {
                    "subscriber_id": "sub-123",
                    "topics": ["events.*", "notifications.#"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"subscription_id": "sub-123"}}"#)
            .create_async()
            .await;

        let sub_id = client
            .pubsub()
            .subscribe_topics(
                "sub-123",
                vec!["events.*".to_string(), "notifications.#".to_string()],
            )
            .await
            .unwrap();
        assert_eq!(sub_id, "sub-123");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pubsub_unsubscribe() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "pubsub.unsubscribe",
                "payload": {
                    "subscriber_id": "sub-123",
                    "topics": ["topic.test"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let result = client
            .pubsub()
            .unsubscribe("sub-123", vec!["topic.test".to_string()])
            .await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pubsub_list_topics() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "pubsub.topics",
                "payload": {}
            })))
            .with_status(200)
            .with_body(
                r#"{"success": true, "payload": {"topics": ["user.created", "order.completed"]}}"#,
            )
            .create_async()
            .await;

        let topics = client.pubsub().list_topics().await.unwrap();
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0], "user.created");

        mock.assert_async().await;
    }
}
