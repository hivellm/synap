//! Tests for Stream operations

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;

    #[tokio::test]
    async fn test_stream_create_room() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/stream/create")
            .match_body(Matcher::Json(json!({
                "room": "chat-1",
                "max_events": 10000
            })))
            .with_status(200)
            .with_body(r#"{"success": true}"#)
            .create_async()
            .await;

        let result = client.stream().create_room("chat-1", Some(10000)).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_publish() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/stream/publish")
            .match_body(Matcher::Json(json!({
                "room": "chat-1",
                "event": "message",
                "data": {"text": "hello"}
            })))
            .with_status(200)
            .with_body(r#"{"offset": 42}"#)
            .create_async()
            .await;

        let offset = client
            .stream()
            .publish("chat-1", "message", json!({"text": "hello"}))
            .await
            .unwrap();
        assert_eq!(offset, 42);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_consume() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/stream/consume")
            .match_body(Matcher::Json(json!({
                "room": "chat-1",
                "offset": 0,
                "limit": 10
            })))
            .with_status(200)
            .with_body(
                r#"{
                "events": [
                    {
                        "offset": 0,
                        "event": "message",
                        "data": {"text": "hello"},
                        "timestamp": 1234567890
                    }
                ]
            }"#,
            )
            .create_async()
            .await;

        let events: Vec<crate::types::Event> = client
            .stream()
            .consume("chat-1", Some(0), Some(10))
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].offset, 0);
        assert_eq!(events[0].event, "message");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_stats() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/stream/stats")
            .match_body(Matcher::Json(json!({"room": "chat-1"})))
            .with_status(200)
            .with_body(
                r#"{
                "room": "chat-1",
                "max_offset": 100,
                "total_events": 100,
                "created_at": 1234567890,
                "last_activity": 1234567990
            }"#,
            )
            .create_async()
            .await;

        let stats = client.stream().stats("chat-1").await.unwrap();
        assert_eq!(stats.room, "chat-1");
        assert_eq!(stats.total_events, 100);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_list() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/stream/list")
            .with_status(200)
            .with_body(r#"{"rooms": ["chat-1", "chat-2", "notifications"]}"#)
            .create_async()
            .await;

        let rooms = client.stream().list().await.unwrap();
        assert_eq!(rooms.len(), 3);
        assert!(rooms.contains(&"chat-1".to_string()));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_delete() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/stream/delete")
            .match_body(Matcher::Json(json!({"room": "chat-1"})))
            .with_status(200)
            .with_body(r#""chat-1""#) // Server returns room name
            .create_async()
            .await;

        let result = client.stream().delete_room("chat-1").await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }
}
