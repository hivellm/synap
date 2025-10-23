//! Comprehensive tests for KV Store operations

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;

    #[tokio::test]
    async fn test_kv_set() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.set",
                "payload": {
                    "key": "test_key",
                    "value": "test_value",
                    "ttl": null
                }
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"success": true, "payload": {}}"#)
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
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.set",
                "payload": {
                    "key": "session",
                    "value": "token123",
                    "ttl": 3600
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
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
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.get",
                "payload": {"key": "test_key"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": "test_value"}"#)
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
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.get",
                "payload": {"key": "nonexistent"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": null}"#)
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
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.del",
                "payload": {"key": "test_key"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"deleted": true}}"#)
            .create_async()
            .await;

        let result = client.kv().delete("test_key").await.unwrap();
        assert_eq!(result, true);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_exists() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.exists",
                "payload": {"key": "test_key"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"exists": true}}"#)
            .create_async()
            .await;

        let result = client.kv().exists("test_key").await.unwrap();
        assert_eq!(result, true);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_incr() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.incr",
                "payload": {"key": "counter"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"value": 1}}"#)
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
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.decr",
                "payload": {"key": "counter"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"value": -1}}"#)
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
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.stats",
                "payload": {}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"total_keys": 100, "total_memory_bytes": 1024, "hit_rate": 0.95}}"#)
            .create_async()
            .await;

        let stats = client.kv().stats().await.unwrap();
        assert_eq!(stats.total_keys, 100);
        assert_eq!(stats.total_memory_bytes, 1024);
        assert_eq!(stats.hit_rate, 0.95);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_kv_keys() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.keys",
                "payload": {"prefix": "user:"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"keys": ["user:1", "user:2", "user:3"]}}"#)
            .create_async()
            .await;

        let keys = client.kv().keys("user:").await.unwrap();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0], "user:1");

        mock.assert_async().await;
    }
}
