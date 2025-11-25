//! Tests for HyperLogLog manager

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;

    #[tokio::test]
    async fn test_pfadd() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "hyperloglog.pfadd",
                "payload": {
                    "key": "unique-users",
                    "elements": [
                        [117, 115, 101, 114, 58, 49],
                        [117, 115, 101, 114, 58, 50]
                    ]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"added": 2}}"#)
            .create_async()
            .await;

        let added = client
            .hyperloglog()
            .pfadd("unique-users", ["user:1", "user:2"])
            .await
            .unwrap();

        assert_eq!(added, 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pfcount() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "hyperloglog.pfcount",
                "payload": {"key": "unique-users"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 123}}"#)
            .create_async()
            .await;

        let count = client.hyperloglog().pfcount("unique-users").await.unwrap();
        assert_eq!(count, 123);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pfmerge() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "hyperloglog.pfmerge",
                "payload": {
                    "destination": "dest",
                    "sources": ["hll:1", "hll:2"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 456}}"#)
            .create_async()
            .await;

        let count = client
            .hyperloglog()
            .pfmerge("dest", &["hll:1", "hll:2"])
            .await
            .unwrap();
        assert_eq!(count, 456);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stats() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "hyperloglog.stats",
                "payload": {}
            })))
            .with_status(200)
            .with_body(
                r#"{"success": true, "payload": {
                "total_hlls": 2,
                "pfadd_count": 10,
                "pfcount_count": 5,
                "pfmerge_count": 3,
                "total_cardinality": 1000
            }}"#,
            )
            .create_async()
            .await;

        let stats = client.hyperloglog().stats().await.unwrap();
        assert_eq!(stats.total_hlls, 2);
        assert_eq!(stats.pfadd_count, 10);
        assert_eq!(stats.total_cardinality, 1000);

        mock.assert_async().await;
    }
}
