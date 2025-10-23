//! Tests for Pub/Sub operations

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
            .mock("POST", "/pubsub/publish")
            .match_body(Matcher::Json(json!({
                "topic": "events.user.login",
                "message": {"user_id": 123},
                "priority": 5,
                "headers": null
            })))
            .with_status(200)
            .with_body(r#"{"delivered_count": 3}"#)
            .create_async()
            .await;

        let count = client
            .pubsub()
            .publish("events.user.login", json!({"user_id": 123}), Some(5), None)
            .await
            .unwrap();
        assert_eq!(count, 3);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pubsub_subscribe() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/pubsub/subscribe")
            .match_body(Matcher::Json(json!({
                "topics": ["events.*", "notifications.#"]
            })))
            .with_status(200)
            .with_body(r#"{"subscription_id": "sub-123"}"#)
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
            .mock("POST", "/pubsub/unsubscribe")
            .match_body(Matcher::Json(json!({"subscription_id": "sub-123"})))
            .with_status(200)
            .with_body(r#"{"success": true}"#)
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
            .mock("GET", "/pubsub/topics")
            .with_status(200)
            .with_body(
                r#"{"topics": ["events.user.login", "events.user.logout", "notifications.email"]}"#,
            )
            .create_async()
            .await;

        let topics = client.pubsub().list_topics().await.unwrap();
        assert_eq!(topics.len(), 3);
        assert!(topics.contains(&"events.user.login".to_string()));

        mock.assert_async().await;
    }
}
