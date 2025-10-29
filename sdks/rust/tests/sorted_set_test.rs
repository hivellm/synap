use synap_sdk::{SynapClient, SynapConfig};

#[tokio::test]
async fn test_sorted_set_basic_operations() {
    // Note: These are interface tests - will fail if server not running
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).unwrap();

    // ZADD - Add members
    let added = client
        .sorted_set()
        .add("leaderboard", "alice", 100.0)
        .await
        .unwrap();
    assert!(added);

    let added = client
        .sorted_set()
        .add("leaderboard", "bob", 200.0)
        .await
        .unwrap();
    assert!(added);

    let added = client
        .sorted_set()
        .add("leaderboard", "charlie", 150.0)
        .await
        .unwrap();
    assert!(added);

    // ZCARD - Get cardinality
    let count = client.sorted_set().card("leaderboard").await.unwrap();
    assert_eq!(count, 3);

    // ZSCORE - Get score
    let score = client
        .sorted_set()
        .score("leaderboard", "alice")
        .await
        .unwrap();
    assert_eq!(score, Some(100.0));

    let score = client
        .sorted_set()
        .score("leaderboard", "bob")
        .await
        .unwrap();
    assert_eq!(score, Some(200.0));

    // ZRANK - Get rank (0-based, lowest score first)
    let rank = client
        .sorted_set()
        .rank("leaderboard", "alice")
        .await
        .unwrap();
    assert_eq!(rank, Some(0)); // Lowest score

    let rank = client
        .sorted_set()
        .rank("leaderboard", "charlie")
        .await
        .unwrap();
    assert_eq!(rank, Some(1)); // Middle score

    let rank = client
        .sorted_set()
        .rank("leaderboard", "bob")
        .await
        .unwrap();
    assert_eq!(rank, Some(2)); // Highest score

    // ZREVRANK - Get reverse rank (highest score first)
    let rev_rank = client
        .sorted_set()
        .rev_rank("leaderboard", "bob")
        .await
        .unwrap();
    assert_eq!(rev_rank, Some(0)); // Highest score = rank 0 in reverse

    // ZREM - Remove members
    let removed = client
        .sorted_set()
        .rem("leaderboard", vec!["charlie".to_string()])
        .await
        .unwrap();
    assert_eq!(removed, 1);

    let count = client.sorted_set().card("leaderboard").await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_sorted_set_range_operations() {
    // Note: These are interface tests - will fail if server not running
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).unwrap();

    // Add test data
    client.sorted_set().add("scores", "a", 1.0).await.unwrap();
    client.sorted_set().add("scores", "b", 2.0).await.unwrap();
    client.sorted_set().add("scores", "c", 3.0).await.unwrap();
    client.sorted_set().add("scores", "d", 4.0).await.unwrap();

    // ZRANGE - Get range by index
    let range = client
        .sorted_set()
        .range("scores", 0, 1, true)
        .await
        .unwrap();
    assert_eq!(range.len(), 2);
    assert_eq!(range[0].member, "a");
    assert_eq!(range[0].score, 1.0);
    assert_eq!(range[1].member, "b");
    assert_eq!(range[1].score, 2.0);

    // ZREVRANGE - Get reverse range
    let rev_range = client
        .sorted_set()
        .rev_range("scores", 0, 1, true)
        .await
        .unwrap();
    assert_eq!(rev_range.len(), 2);
    assert_eq!(rev_range[0].member, "d");
    assert_eq!(rev_range[0].score, 4.0);
    assert_eq!(rev_range[1].member, "c");
    assert_eq!(rev_range[1].score, 3.0);

    // ZRANGEBYSCORE - Get range by score
    let by_score = client
        .sorted_set()
        .range_by_score("scores", 2.0, 3.0, true)
        .await
        .unwrap();
    assert_eq!(by_score.len(), 2); // b and c
    assert_eq!(by_score[0].member, "b");
    assert_eq!(by_score[1].member, "c");

    // ZCOUNT - Count in range
    let count = client.sorted_set().count("scores", 2.0, 4.0).await.unwrap();
    assert_eq!(count, 3); // b, c, d
}

#[tokio::test]
async fn test_sorted_set_increment() {
    // Note: These are interface tests - will fail if server not running
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).unwrap();

    // ZINCRBY - Increment score
    let score = client
        .sorted_set()
        .incr_by("counters", "visits", 1.0)
        .await
        .unwrap();
    assert_eq!(score, 1.0);

    let score = client
        .sorted_set()
        .incr_by("counters", "visits", 2.5)
        .await
        .unwrap();
    assert_eq!(score, 3.5);

    // Verify score
    let score = client
        .sorted_set()
        .score("counters", "visits")
        .await
        .unwrap();
    assert_eq!(score, Some(3.5));
}

#[tokio::test]
async fn test_sorted_set_pop_operations() {
    // Note: These are interface tests - will fail if server not running
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).unwrap();

    // Add test data
    client.sorted_set().add("tasks", "low", 1.0).await.unwrap();
    client
        .sorted_set()
        .add("tasks", "medium", 5.0)
        .await
        .unwrap();
    client
        .sorted_set()
        .add("tasks", "high", 10.0)
        .await
        .unwrap();

    // ZPOPMIN - Pop lowest scored
    let popped = client.sorted_set().pop_min("tasks", 1).await.unwrap();
    assert_eq!(popped.len(), 1);
    assert_eq!(popped[0].member, "low");
    assert_eq!(popped[0].score, 1.0);

    // ZPOPMAX - Pop highest scored
    let popped = client.sorted_set().pop_max("tasks", 1).await.unwrap();
    assert_eq!(popped.len(), 1);
    assert_eq!(popped[0].member, "high");
    assert_eq!(popped[0].score, 10.0);

    // Should have 1 member left
    let count = client.sorted_set().card("tasks").await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_sorted_set_remove_range() {
    // Note: These are interface tests - will fail if server not running
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).unwrap();

    // Add test data
    for i in 0..10 {
        client
            .sorted_set()
            .add("numbers", &format!("n{}", i), i as f64)
            .await
            .unwrap();
    }

    // ZREMRANGEBYRANK - Remove by rank range
    let removed = client
        .sorted_set()
        .rem_range_by_rank("numbers", 0, 2)
        .await
        .unwrap();
    assert_eq!(removed, 3); // Removed n0, n1, n2

    // Should have 7 left
    let count = client.sorted_set().card("numbers").await.unwrap();
    assert_eq!(count, 7);

    // ZREMRANGEBYSCORE - Remove by score range
    let removed = client
        .sorted_set()
        .rem_range_by_score("numbers", 5.0, 7.0)
        .await
        .unwrap();
    assert_eq!(removed, 3); // Removed n5, n6, n7

    // Should have 4 left
    let count = client.sorted_set().card("numbers").await.unwrap();
    assert_eq!(count, 4);
}

#[tokio::test]
async fn test_sorted_set_set_operations() {
    // Note: These are interface tests - will fail if server not running
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).unwrap();

    // Create two sorted sets
    client.sorted_set().add("zset1", "a", 1.0).await.unwrap();
    client.sorted_set().add("zset1", "b", 2.0).await.unwrap();
    client.sorted_set().add("zset1", "c", 3.0).await.unwrap();

    client.sorted_set().add("zset2", "b", 10.0).await.unwrap();
    client.sorted_set().add("zset2", "c", 20.0).await.unwrap();
    client.sorted_set().add("zset2", "d", 30.0).await.unwrap();

    // ZINTERSTORE - Intersection (members in both sets)
    let count = client
        .sorted_set()
        .inter_store("inter", vec!["zset1", "zset2"], None, "sum")
        .await
        .unwrap();
    assert_eq!(count, 2); // b and c are in both

    // ZUNIONSTORE - Union (members in either set)
    let count = client
        .sorted_set()
        .union_store("union", vec!["zset1", "zset2"], None, "sum")
        .await
        .unwrap();
    assert_eq!(count, 4); // a, b, c, d

    // ZDIFFSTORE - Difference (in first but not in second)
    let count = client
        .sorted_set()
        .diff_store("diff", vec!["zset1", "zset2"])
        .await
        .unwrap();
    assert_eq!(count, 1); // Only 'a' is in zset1 but not in zset2
}

#[tokio::test]
async fn test_sorted_set_stats() {
    // Note: These are interface tests - will fail if server not running
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).unwrap();

    // Add some data
    client.sorted_set().add("zset1", "a", 1.0).await.unwrap();
    client.sorted_set().add("zset1", "b", 2.0).await.unwrap();
    client.sorted_set().add("zset2", "c", 3.0).await.unwrap();

    // Get statistics
    let stats = client.sorted_set().stats().await.unwrap();
    assert!(stats.total_keys >= 2);
    assert!(stats.total_members >= 3);
}
