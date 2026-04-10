//! Mock-based tests for Bitmap operations (no running server required)

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;
    use synap_sdk::bitmap::BitmapOperation;

    #[tokio::test]
    async fn test_bitmap_setbit() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.setbit",
                "payload": {"key": "bm:users", "offset": 42, "value": 1}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"old_value": 0}}"#)
            .create_async()
            .await;

        let old = client.bitmap().setbit("bm:users", 42, 1).await.unwrap();
        assert_eq!(old, 0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_setbit_invalid_value() {
        let (client, _server) = setup_test_client().await;
        let result = client.bitmap().setbit("key", 0, 2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bitmap_getbit() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.getbit",
                "payload": {"key": "bm:users", "offset": 42}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"value": 1}}"#)
            .create_async()
            .await;

        let val = client.bitmap().getbit("bm:users", 42).await.unwrap();
        assert_eq!(val, 1);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_getbit_unset() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.getbit",
                "payload": {"key": "bm:users", "offset": 999}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"value": 0}}"#)
            .create_async()
            .await;

        let val = client.bitmap().getbit("bm:users", 999).await.unwrap();
        assert_eq!(val, 0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_bitcount_full() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.bitcount",
                "payload": {"key": "bm:users"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 15}}"#)
            .create_async()
            .await;

        let count = client
            .bitmap()
            .bitcount("bm:users", None, None)
            .await
            .unwrap();
        assert_eq!(count, 15);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_bitcount_range() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.bitcount",
                "payload": {"key": "bm:users", "start": 0, "end": 10}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 5}}"#)
            .create_async()
            .await;

        let count = client
            .bitmap()
            .bitcount("bm:users", Some(0), Some(10))
            .await
            .unwrap();
        assert_eq!(count, 5);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_bitpos_found() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.bitpos",
                "payload": {"key": "bm:users", "value": 1}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"position": 7}}"#)
            .create_async()
            .await;

        let pos = client
            .bitmap()
            .bitpos("bm:users", 1, None, None)
            .await
            .unwrap();
        assert_eq!(pos, Some(7));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_bitpos_not_found() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.bitpos",
                "payload": {"key": "bm:empty", "value": 1}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let pos = client
            .bitmap()
            .bitpos("bm:empty", 1, None, None)
            .await
            .unwrap();
        assert_eq!(pos, None);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_bitpos_invalid_value() {
        let (client, _server) = setup_test_client().await;
        let result = client.bitmap().bitpos("key", 2, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bitmap_bitop_and() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.bitop",
                "payload": {
                    "destination": "result",
                    "operation": "AND",
                    "source_keys": ["bm1", "bm2"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"length": 64}}"#)
            .create_async()
            .await;

        let len = client
            .bitmap()
            .bitop(BitmapOperation::And, "result", &["bm1", "bm2"])
            .await
            .unwrap();
        assert_eq!(len, 64);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_bitop_or() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.bitop",
                "payload": {
                    "destination": "result",
                    "operation": "OR",
                    "source_keys": ["bm1", "bm2", "bm3"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"length": 128}}"#)
            .create_async()
            .await;

        let len = client
            .bitmap()
            .bitop(BitmapOperation::Or, "result", &["bm1", "bm2", "bm3"])
            .await
            .unwrap();
        assert_eq!(len, 128);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_bitop_not_single_source() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.bitop",
                "payload": {
                    "destination": "result",
                    "operation": "NOT",
                    "source_keys": ["bm1"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"length": 32}}"#)
            .create_async()
            .await;

        let len = client
            .bitmap()
            .bitop(BitmapOperation::Not, "result", &["bm1"])
            .await
            .unwrap();
        assert_eq!(len, 32);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_bitop_not_multiple_sources_error() {
        let (client, _server) = setup_test_client().await;
        let result = client
            .bitmap()
            .bitop(BitmapOperation::Not, "result", &["bm1", "bm2"])
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bitmap_bitop_empty_sources_error() {
        let (client, _server) = setup_test_client().await;
        let empty: &[&str] = &[];
        let result = client
            .bitmap()
            .bitop(BitmapOperation::And, "result", empty)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bitmap_bitfield() {
        let (client, mut server) = setup_test_client().await;

        let operations = vec![
            json!({"operation": "SET", "offset": 0, "width": 8, "signed": false, "value": 42}),
            json!({"operation": "GET", "offset": 0, "width": 8, "signed": false}),
        ];

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.bitfield",
                "payload": {"key": "bf:test"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"results": [0, 42]}}"#)
            .create_async()
            .await;

        let results = client
            .bitmap()
            .bitfield("bf:test", &operations)
            .await
            .unwrap();
        assert_eq!(results, vec![0, 42]);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bitmap_stats() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "bitmap.stats"
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"total_bitmaps": 3, "total_bits": 1024, "setbit_count": 50, "getbit_count": 100, "bitcount_count": 20, "bitop_count": 5, "bitpos_count": 10, "bitfield_count": 2}}"#)
            .create_async()
            .await;

        let stats = client.bitmap().stats().await.unwrap();
        assert_eq!(stats.total_bitmaps, 3);
        assert_eq!(stats.total_bits, 1024);
        assert_eq!(stats.setbit_count, 50);

        mock.assert_async().await;
    }
}
