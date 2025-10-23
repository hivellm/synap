//! Tests for KV Store operations

#[cfg(test)]
mod tests {
    use crate::tests::helpers::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;

    #[tokio::test]
    async fn test_kv_set() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/kv/set")
            .match_body(Matcher::Json(json!({
                "key": "test_key",
                "value": "test_value",
                "ttl": null
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"success": true, "key": "test_key"}"#)
            .create_async()
            .await;

        let result = client.kv().set("test_key", "test_value", None).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_set_with_ttl() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/kv/set")
            .match_body(Matcher::Json(json!({
                "key": "session",
                "value": "token123",
                "ttl": 3600
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "key": "session"}"#)
            .create_async()
            .await;

        let result = client.kv().set("session", "token123", Some(3600)).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_get_found() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("GET", "/kv/get/test_key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#""\"test_value\""#) // Double-encoded string
            .create_async()
            .await;

        let result: Option<String> = client.kv().get("test_key").await.unwrap();
        assert_eq!(result, Some("test_value".to_string()));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_get_not_found() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("GET", "/kv/get/nonexistent")
            .with_status(200)
            .with_body(r#"{"error": "Key not found"}"#)
            .create_async()
            .await;

        let result: Option<String> = client.kv().get("nonexistent").await.unwrap();
        assert_eq!(result, None);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_delete() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("DELETE", "/kv/del/test_key")
            .with_status(200)
            .with_body(r#"{"deleted": true, "key": "test_key"}"#)
            .create_async()
            .await;

        let result = client.kv().delete("test_key").await.unwrap();
        assert!(result);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_incr() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/kv/incr")
            .match_body(Matcher::Json(json!({"key": "counter"})))
            .with_status(200)
            .with_body(r#"{"value": 1}"#)
            .create_async()
            .await;

        let result = client.kv().incr("counter").await.unwrap();
        assert_eq!(result, 1);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_decr() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/kv/decr")
            .match_body(Matcher::Json(json!({"key": "counter"})))
            .with_status(200)
            .with_body(r#"{"value": -1}"#)
            .create_async()
            .await;

        let result = client.kv().decr("counter").await.unwrap();
        assert_eq!(result, -1);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_stats() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("GET", "/kv/stats")
            .with_status(200)
            .with_body(
                r#"{
                "total_keys": 100,
                "total_memory_bytes": 1024,
                "hit_rate": 0.95
            }"#,
            )
            .create_async()
            .await;

        let stats = client.kv().stats().await.unwrap();
        assert_eq!(stats.total_keys, 100);
        assert_eq!(stats.total_memory_bytes, 1024);
        assert_eq!(stats.hit_rate, 0.95);

        mock.assert_async().await;
    }
}
