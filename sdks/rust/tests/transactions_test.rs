//! Tests for transaction manager

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;
    use synap_sdk::{TransactionExecResult, TransactionManager, TransactionOptions};

    #[tokio::test]
    async fn test_transaction_multi() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "transaction.multi",
                "payload": {"client_id": "client-1"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"success": true, "message": "Transaction started"}}"#)
            .create_async()
            .await;

        let response = client
            .transaction()
            .multi(TransactionOptions {
                client_id: Some("client-1".into()),
            })
            .await
            .unwrap();

        assert!(response.success);
        assert_eq!(response.message.as_deref(), Some("Transaction started"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_transaction_exec_success() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "transaction.exec",
                "payload": {"client_id": "client-2"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"results": [1, "OK"]}}"#)
            .create_async()
            .await;

        let result = client
            .transaction()
            .exec(TransactionOptions {
                client_id: Some("client-2".into()),
            })
            .await
            .unwrap();

        match result {
            TransactionExecResult::Success { results } => {
                assert_eq!(results.len(), 2);
                assert_eq!(results[0], json!(1));
                assert_eq!(results[1], json!("OK"));
            }
            _ => panic!("expected success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_transaction_exec_aborted() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "transaction.exec",
                "payload": {}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"aborted": true, "message": "watched key changed"}}"#)
            .create_async()
            .await;

        let result = client
            .transaction()
            .exec(TransactionOptions::default())
            .await
            .unwrap();

        match result {
            TransactionExecResult::Aborted { aborted, message } => {
                assert!(aborted);
                assert_eq!(message.as_deref(), Some("watched key changed"));
            }
            _ => panic!("expected abort"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_transaction_watch_and_unwatch() {
        let (client, mut server) = setup_test_client().await;
        let manager: TransactionManager = client.transaction();

        let watch_mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "transaction.watch",
                "payload": {
                    "client_id": "client-3",
                    "keys": ["key1", "key2"]
                }
            })))
            .with_status(200)
            .with_body(
                r#"{"success": true, "payload": {"success": true, "message": "Keys watched"}}"#,
            )
            .create_async()
            .await;

        let unwatch_mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "transaction.unwatch",
                "payload": {"client_id": "client-3"}
            })))
            .with_status(200)
            .with_body(
                r#"{"success": true, "payload": {"success": true, "message": "Keys unwatched"}}"#,
            )
            .create_async()
            .await;

        let watch_response = manager
            .watch(
                &["key1", "key2"],
                TransactionOptions {
                    client_id: Some("client-3".into()),
                },
            )
            .await
            .unwrap();
        assert!(watch_response.success);

        let unwatch_response = manager
            .unwatch(TransactionOptions {
                client_id: Some("client-3".into()),
            })
            .await
            .unwrap();
        assert!(unwatch_response.success);

        watch_mock.assert_async().await;
        unwatch_mock.assert_async().await;
    }
}
