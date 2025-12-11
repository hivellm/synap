//! Tests for Lua scripting manager

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;
    use synap_sdk::ScriptEvalOptions;

    #[tokio::test]
    async fn test_script_eval() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "script.eval",
                "payload": {
                    "script": "return ARGV[1]",
                    "keys": ["key1"],
                    "args": ["value"],
                    "timeout_ms": 5000
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"result": "value", "sha1": "abc123"}}"#)
            .create_async()
            .await;

        let response = client
            .script()
            .eval::<String>(
                "return ARGV[1]",
                ScriptEvalOptions {
                    keys: vec!["key1".to_string()],
                    args: vec![json!("value")],
                    timeout_ms: Some(5000),
                },
            )
            .await
            .unwrap();

        assert_eq!(response.result, "value");
        assert_eq!(response.sha1, "abc123");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_script_evalsha() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "script.evalsha",
                "payload": {
                    "sha1": "abc123",
                    "keys": [],
                    "args": [],
                    "timeout_ms": null
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"result": 42, "sha1": "abc123"}}"#)
            .create_async()
            .await;

        let response = client
            .script()
            .evalsha::<i32>("abc123", ScriptEvalOptions::default())
            .await
            .unwrap();

        assert_eq!(response.result, 42);
        assert_eq!(response.sha1, "abc123");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_script_load() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "script.load",
                "payload": {"script": "return 1"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"sha1": "def456"}}"#)
            .create_async()
            .await;

        let sha1 = client.script().load("return 1").await.unwrap();
        assert_eq!(sha1, "def456");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_script_exists() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "script.exists",
                "payload": {"hashes": ["abc", "def"]}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"exists": [true, false]}}"#)
            .create_async()
            .await;

        let exists = client.script().exists(&["abc", "def"]).await.unwrap();
        assert_eq!(exists, vec![true, false]);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_script_flush_and_kill() {
        let (client, mut server) = setup_test_client().await;

        let flush_mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "script.flush",
                "payload": {}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"cleared": 3}}"#)
            .create_async()
            .await;

        let kill_mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "script.kill",
                "payload": {}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"terminated": true}}"#)
            .create_async()
            .await;

        let cleared = client.script().flush().await.unwrap();
        let terminated = client.script().kill().await.unwrap();

        assert_eq!(cleared, 3);
        assert!(terminated);

        flush_mock.assert_async().await;
        kill_mock.assert_async().await;
    }
}
