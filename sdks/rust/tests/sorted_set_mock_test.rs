//! Mock-based tests for Sorted Set operations (no running server required)

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;

    #[tokio::test]
    async fn test_sorted_set_zadd() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zadd",
                "payload": {
                    "key": "leaderboard",
                    "member": "player1",
                    "score": 100.0
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"added": 1}}"#)
            .create_async()
            .await;

        let added = client
            .sorted_set()
            .add("leaderboard", "player1", 100.0)
            .await
            .unwrap();
        assert!(added);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zadd_existing() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zadd",
                "payload": {
                    "key": "leaderboard",
                    "member": "player1",
                    "score": 200.0
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"added": 0}}"#)
            .create_async()
            .await;

        let added = client
            .sorted_set()
            .add("leaderboard", "player1", 200.0)
            .await
            .unwrap();
        assert!(!added);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zadd_multiple() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zadd",
                "payload": {
                    "key": "leaderboard"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"added": 3}}"#)
            .create_async()
            .await;

        let members = vec![
            synap_sdk::sorted_set::ScoredMember {
                member: "a".into(),
                score: 1.0,
            },
            synap_sdk::sorted_set::ScoredMember {
                member: "b".into(),
                score: 2.0,
            },
            synap_sdk::sorted_set::ScoredMember {
                member: "c".into(),
                score: 3.0,
            },
        ];
        let count = client
            .sorted_set()
            .add_multiple("leaderboard", members)
            .await
            .unwrap();
        assert_eq!(count, 3);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zrem() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zrem",
                "payload": {
                    "key": "leaderboard",
                    "members": ["player1", "player2"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"removed": 2}}"#)
            .create_async()
            .await;

        let removed = client
            .sorted_set()
            .rem("leaderboard", vec!["player1".into(), "player2".into()])
            .await
            .unwrap();
        assert_eq!(removed, 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zscore() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zscore",
                "payload": {
                    "key": "leaderboard",
                    "member": "player1"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"score": 150.5}}"#)
            .create_async()
            .await;

        let score = client
            .sorted_set()
            .score("leaderboard", "player1")
            .await
            .unwrap();
        assert_eq!(score, Some(150.5));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zscore_missing() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zscore",
                "payload": {
                    "key": "leaderboard",
                    "member": "unknown"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        let score = client
            .sorted_set()
            .score("leaderboard", "unknown")
            .await
            .unwrap();
        assert_eq!(score, None);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zcard() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zcard",
                "payload": {"key": "leaderboard"}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 42}}"#)
            .create_async()
            .await;

        let count = client.sorted_set().card("leaderboard").await.unwrap();
        assert_eq!(count, 42);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zincrby() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zincrby",
                "payload": {
                    "key": "leaderboard",
                    "member": "player1",
                    "increment": 50.0
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"score": 250.0}}"#)
            .create_async()
            .await;

        let new_score = client
            .sorted_set()
            .incr_by("leaderboard", "player1", 50.0)
            .await
            .unwrap();
        assert_eq!(new_score, 250.0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zrange() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zrange",
                "payload": {
                    "key": "leaderboard",
                    "start": 0,
                    "stop": 2,
                    "withscores": true
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"members": [{"member": "a", "score": 1.0}, {"member": "b", "score": 2.0}, {"member": "c", "score": 3.0}]}}"#)
            .create_async()
            .await;

        let members = client
            .sorted_set()
            .range("leaderboard", 0, 2, true)
            .await
            .unwrap();
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].member, "a");
        assert_eq!(members[0].score, 1.0);
        assert_eq!(members[2].member, "c");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zrevrange() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zrevrange",
                "payload": {
                    "key": "leaderboard",
                    "start": 0,
                    "stop": 1,
                    "withscores": true
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"members": [{"member": "c", "score": 3.0}, {"member": "b", "score": 2.0}]}}"#)
            .create_async()
            .await;

        let members = client
            .sorted_set()
            .rev_range("leaderboard", 0, 1, true)
            .await
            .unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].member, "c");
        assert_eq!(members[0].score, 3.0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zrank() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zrank",
                "payload": {
                    "key": "leaderboard",
                    "member": "player1"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"rank": 5}}"#)
            .create_async()
            .await;

        let rank = client
            .sorted_set()
            .rank("leaderboard", "player1")
            .await
            .unwrap();
        assert_eq!(rank, Some(5));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zrevrank() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zrevrank",
                "payload": {
                    "key": "leaderboard",
                    "member": "player1"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"rank": 0}}"#)
            .create_async()
            .await;

        let rank = client
            .sorted_set()
            .rev_rank("leaderboard", "player1")
            .await
            .unwrap();
        assert_eq!(rank, Some(0));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zcount() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zcount",
                "payload": {
                    "key": "leaderboard",
                    "min": 10.0,
                    "max": 100.0
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 7}}"#)
            .create_async()
            .await;

        let count = client
            .sorted_set()
            .count("leaderboard", 10.0, 100.0)
            .await
            .unwrap();
        assert_eq!(count, 7);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zrangebyscore() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zrangebyscore",
                "payload": {
                    "key": "leaderboard",
                    "min": 0.0,
                    "max": 50.0,
                    "withscores": true
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"members": [{"member": "a", "score": 10.0}, {"member": "b", "score": 20.0}]}}"#)
            .create_async()
            .await;

        let members = client
            .sorted_set()
            .range_by_score("leaderboard", 0.0, 50.0, true)
            .await
            .unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].score, 10.0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zpopmin() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zpopmin",
                "payload": {"key": "leaderboard", "count": 2}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"members": [{"member": "a", "score": 1.0}, {"member": "b", "score": 2.0}]}}"#)
            .create_async()
            .await;

        let popped = client.sorted_set().pop_min("leaderboard", 2).await.unwrap();
        assert_eq!(popped.len(), 2);
        assert_eq!(popped[0].member, "a");
        assert_eq!(popped[0].score, 1.0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zpopmax() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zpopmax",
                "payload": {"key": "leaderboard", "count": 1}
            })))
            .with_status(200)
            .with_body(
                r#"{"success": true, "payload": {"members": [{"member": "top", "score": 999.0}]}}"#,
            )
            .create_async()
            .await;

        let popped = client.sorted_set().pop_max("leaderboard", 1).await.unwrap();
        assert_eq!(popped.len(), 1);
        assert_eq!(popped[0].member, "top");
        assert_eq!(popped[0].score, 999.0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zremrangebyrank() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zremrangebyrank",
                "payload": {"key": "leaderboard", "start": 0, "stop": 4}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"removed": 5}}"#)
            .create_async()
            .await;

        let removed = client
            .sorted_set()
            .rem_range_by_rank("leaderboard", 0, 4)
            .await
            .unwrap();
        assert_eq!(removed, 5);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zremrangebyscore() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zremrangebyscore",
                "payload": {"key": "leaderboard", "min": 0.0, "max": 10.0}
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"removed": 3}}"#)
            .create_async()
            .await;

        let removed = client
            .sorted_set()
            .rem_range_by_score("leaderboard", 0.0, 10.0)
            .await
            .unwrap();
        assert_eq!(removed, 3);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zinterstore() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zinterstore",
                "payload": {
                    "destination": "result",
                    "keys": ["set1", "set2"],
                    "aggregate": "sum"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 5}}"#)
            .create_async()
            .await;

        let count = client
            .sorted_set()
            .inter_store("result", vec!["set1", "set2"], None, "sum")
            .await
            .unwrap();
        assert_eq!(count, 5);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zunionstore() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zunionstore",
                "payload": {
                    "destination": "result",
                    "keys": ["set1", "set2"],
                    "aggregate": "max"
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 8}}"#)
            .create_async()
            .await;

        let count = client
            .sorted_set()
            .union_store("result", vec!["set1", "set2"], None, "max")
            .await
            .unwrap();
        assert_eq!(count, 8);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_zdiffstore() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.zdiffstore",
                "payload": {
                    "destination": "result",
                    "keys": ["set1", "set2"]
                }
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"count": 2}}"#)
            .create_async()
            .await;

        let count = client
            .sorted_set()
            .diff_store("result", vec!["set1", "set2"])
            .await
            .unwrap();
        assert_eq!(count, 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_sorted_set_stats() {
        let (client, mut server) = setup_test_client().await;

        let mock = server
            .mock("POST", "/api/v1/command")
            .match_body(Matcher::PartialJson(json!({
                "command": "sortedset.stats"
            })))
            .with_status(200)
            .with_body(r#"{"success": true, "payload": {"total_keys": 5, "total_members": 100, "avg_members_per_key": 20.0, "memory_bytes": 4096}}"#)
            .create_async()
            .await;

        let stats = client.sorted_set().stats().await.unwrap();
        assert_eq!(stats.total_keys, 5);
        assert_eq!(stats.total_members, 100);
        assert_eq!(stats.avg_members_per_key, 20.0);

        mock.assert_async().await;
    }
}
