use synap_server::core::{Aggregate, SortedSetStore, ZAddOptions};

// ==================== Set Operations Tests ====================

#[test]
fn test_zinterstore_basic() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    // Create two sets with overlapping members
    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);

    store.zadd("zset2", b"b".to_vec(), 4.0, &opts);
    store.zadd("zset2", b"c".to_vec(), 5.0, &opts);
    store.zadd("zset2", b"d".to_vec(), 6.0, &opts);

    // Intersection should only have b and c
    let count = store.zinterstore("dest", &["zset1", "zset2"], None, Aggregate::Sum);

    assert_eq!(count, 2);
    assert_eq!(store.zcard("dest"), 2);
    assert_eq!(store.zscore("dest", b"b"), Some(6.0)); // 2.0 + 4.0
    assert_eq!(store.zscore("dest", b"c"), Some(8.0)); // 3.0 + 5.0
    assert_eq!(store.zscore("dest", b"a"), None); // Not in intersection
    assert_eq!(store.zscore("dest", b"d"), None); // Not in intersection
}

#[test]
fn test_zinterstore_with_weights() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 10.0, &opts);
    store.zadd("zset2", b"a".to_vec(), 5.0, &opts);

    // With weights [2, 3]: (10 * 2) + (5 * 3) = 35
    let count = store.zinterstore(
        "dest",
        &["zset1", "zset2"],
        Some(&[2.0, 3.0]),
        Aggregate::Sum,
    );

    assert_eq!(count, 1);
    assert_eq!(store.zscore("dest", b"a"), Some(35.0));
}

#[test]
fn test_zinterstore_aggregate_min() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 10.0, &opts);
    store.zadd("zset2", b"a".to_vec(), 5.0, &opts);

    let count = store.zinterstore("dest", &["zset1", "zset2"], None, Aggregate::Min);

    assert_eq!(count, 1);
    assert_eq!(store.zscore("dest", b"a"), Some(5.0)); // Min of 10 and 5
}

#[test]
fn test_zinterstore_aggregate_max() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 10.0, &opts);
    store.zadd("zset2", b"a".to_vec(), 5.0, &opts);

    let count = store.zinterstore("dest", &["zset1", "zset2"], None, Aggregate::Max);

    assert_eq!(count, 1);
    assert_eq!(store.zscore("dest", b"a"), Some(10.0)); // Max of 10 and 5
}

#[test]
fn test_zinterstore_empty_intersection() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset2", b"b".to_vec(), 2.0, &opts);

    let count = store.zinterstore("dest", &["zset1", "zset2"], None, Aggregate::Sum);

    assert_eq!(count, 0); // No common members
    assert_eq!(store.zcard("dest"), 0);
}

#[test]
fn test_zinterstore_nonexistent_set() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);

    let count = store.zinterstore("dest", &["zset1", "nonexistent"], None, Aggregate::Sum);

    assert_eq!(count, 0); // One set doesn't exist
}

#[test]
fn test_zunionstore_basic() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);

    store.zadd("zset2", b"b".to_vec(), 3.0, &opts);
    store.zadd("zset2", b"c".to_vec(), 4.0, &opts);

    let count = store.zunionstore("dest", &["zset1", "zset2"], None, Aggregate::Sum);

    assert_eq!(count, 3);
    assert_eq!(store.zscore("dest", b"a"), Some(1.0)); // Only in zset1
    assert_eq!(store.zscore("dest", b"b"), Some(5.0)); // 2.0 + 3.0
    assert_eq!(store.zscore("dest", b"c"), Some(4.0)); // Only in zset2
}

#[test]
fn test_zunionstore_with_weights() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 10.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 5.0, &opts);

    store.zadd("zset2", b"a".to_vec(), 20.0, &opts);

    // With weights [1, 2]: a = (10 * 1) + (20 * 2) = 50, b = 5 * 1 = 5
    let count = store.zunionstore(
        "dest",
        &["zset1", "zset2"],
        Some(&[1.0, 2.0]),
        Aggregate::Sum,
    );

    assert_eq!(count, 2);
    assert_eq!(store.zscore("dest", b"a"), Some(50.0));
    assert_eq!(store.zscore("dest", b"b"), Some(5.0));
}

#[test]
fn test_zunionstore_aggregate_min() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 10.0, &opts);
    store.zadd("zset2", b"a".to_vec(), 5.0, &opts);

    let count = store.zunionstore("dest", &["zset1", "zset2"], None, Aggregate::Min);

    assert_eq!(count, 1);
    assert_eq!(store.zscore("dest", b"a"), Some(5.0));
}

#[test]
fn test_zunionstore_aggregate_max() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 10.0, &opts);
    store.zadd("zset2", b"a".to_vec(), 5.0, &opts);

    let count = store.zunionstore("dest", &["zset1", "zset2"], None, Aggregate::Max);

    assert_eq!(count, 1);
    assert_eq!(store.zscore("dest", b"a"), Some(10.0));
}

#[test]
fn test_zunionstore_single_set() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);

    let count = store.zunionstore("dest", &["zset1"], None, Aggregate::Sum);

    assert_eq!(count, 2);
    assert_eq!(store.zscore("dest", b"a"), Some(1.0));
    assert_eq!(store.zscore("dest", b"b"), Some(2.0));
}

#[test]
fn test_zdiffstore_basic() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);

    store.zadd("zset2", b"b".to_vec(), 4.0, &opts);
    store.zadd("zset2", b"d".to_vec(), 5.0, &opts);

    // zset1 - zset2 = {a, c} (removes b which is in zset2)
    let count = store.zdiffstore("dest", &["zset1", "zset2"]);

    assert_eq!(count, 2);
    assert_eq!(store.zscore("dest", b"a"), Some(1.0));
    assert_eq!(store.zscore("dest", b"c"), Some(3.0));
    assert_eq!(store.zscore("dest", b"b"), None); // Removed
    assert_eq!(store.zscore("dest", b"d"), None); // Not in first set
}

#[test]
fn test_zdiffstore_multiple_subtract() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);
    store.zadd("zset1", b"d".to_vec(), 4.0, &opts);

    store.zadd("zset2", b"b".to_vec(), 5.0, &opts);
    store.zadd("zset3", b"d".to_vec(), 6.0, &opts);

    // zset1 - zset2 - zset3 = {a, c}
    let count = store.zdiffstore("dest", &["zset1", "zset2", "zset3"]);

    assert_eq!(count, 2);
    assert_eq!(store.zscore("dest", b"a"), Some(1.0));
    assert_eq!(store.zscore("dest", b"c"), Some(3.0));
}

#[test]
fn test_zdiffstore_first_set_empty() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset2", b"a".to_vec(), 1.0, &opts);

    let count = store.zdiffstore("dest", &["nonexistent", "zset2"]);

    assert_eq!(count, 0);
}

#[test]
fn test_zdiffstore_no_overlap() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset2", b"b".to_vec(), 2.0, &opts);

    // No overlap, all of zset1 remains
    let count = store.zdiffstore("dest", &["zset1", "zset2"]);

    assert_eq!(count, 1);
    assert_eq!(store.zscore("dest", b"a"), Some(1.0));
}

#[test]
fn test_zrangebyscore() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);
    store.zadd("zset1", b"d".to_vec(), 4.0, &opts);
    store.zadd("zset1", b"e".to_vec(), 5.0, &opts);

    let range = store.zrangebyscore("zset1", 2.0, 4.0, true);

    assert_eq!(range.len(), 3); // b, c, d
    assert_eq!(range[0].member, b"b");
    assert_eq!(range[0].score, 2.0);
    assert_eq!(range[1].member, b"c");
    assert_eq!(range[2].member, b"d");
}

#[test]
fn test_zremrangebyrank() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);
    store.zadd("zset1", b"d".to_vec(), 4.0, &opts);

    // Remove ranks 1-2 (b and c)
    let removed = store.zremrangebyrank("zset1", 1, 2);

    assert_eq!(removed, 2);
    assert_eq!(store.zcard("zset1"), 2);
    assert_eq!(store.zscore("zset1", b"a"), Some(1.0));
    assert_eq!(store.zscore("zset1", b"b"), None);
    assert_eq!(store.zscore("zset1", b"c"), None);
    assert_eq!(store.zscore("zset1", b"d"), Some(4.0));
}

#[test]
fn test_zremrangebyscore() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);
    store.zadd("zset1", b"d".to_vec(), 4.0, &opts);

    // Remove scores 2.0-3.0 (b and c)
    let removed = store.zremrangebyscore("zset1", 2.0, 3.0);

    assert_eq!(removed, 2);
    assert_eq!(store.zcard("zset1"), 2);
    assert_eq!(store.zscore("zset1", b"a"), Some(1.0));
    assert_eq!(store.zscore("zset1", b"b"), None);
    assert_eq!(store.zscore("zset1", b"c"), None);
    assert_eq!(store.zscore("zset1", b"d"), Some(4.0));
}

#[test]
fn test_zmscore() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);

    let scores = store.zmscore(
        "zset1",
        &[
            b"a".to_vec(),
            b"b".to_vec(),
            b"nonexistent".to_vec(),
            b"c".to_vec(),
        ],
    );

    assert_eq!(scores.len(), 4);
    assert_eq!(scores[0], Some(1.0));
    assert_eq!(scores[1], Some(2.0));
    assert_eq!(scores[2], None);
    assert_eq!(scores[3], Some(3.0));
}

#[test]
fn test_complex_weighted_intersection() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    // Leaderboard scenario with multiple game modes
    store.zadd("easy_mode", b"player1".to_vec(), 100.0, &opts);
    store.zadd("easy_mode", b"player2".to_vec(), 150.0, &opts);

    store.zadd("hard_mode", b"player1".to_vec(), 50.0, &opts);
    store.zadd("hard_mode", b"player2".to_vec(), 75.0, &opts);

    // Weight hard mode 2x: intersection with weights [1, 2]
    let count = store.zinterstore(
        "combined",
        &["easy_mode", "hard_mode"],
        Some(&[1.0, 2.0]),
        Aggregate::Sum,
    );

    assert_eq!(count, 2);
    assert_eq!(store.zscore("combined", b"player1"), Some(200.0)); // 100 + (50 * 2)
    assert_eq!(store.zscore("combined", b"player2"), Some(300.0)); // 150 + (75 * 2)

    // Verify ranking
    assert_eq!(store.zrank("combined", b"player1"), Some(0));
    assert_eq!(store.zrank("combined", b"player2"), Some(1));
}

#[test]
fn test_three_way_union() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("set1", b"a".to_vec(), 1.0, &opts);
    store.zadd("set2", b"b".to_vec(), 2.0, &opts);
    store.zadd("set3", b"c".to_vec(), 3.0, &opts);

    let count = store.zunionstore("dest", &["set1", "set2", "set3"], None, Aggregate::Sum);

    assert_eq!(count, 3);
}

#[test]
fn test_set_operations_overwrites_destination() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    // Create destination with existing data
    store.zadd("dest", b"old".to_vec(), 99.0, &opts);
    assert_eq!(store.zcard("dest"), 1);

    // ZINTERSTORE overwrites
    store.zadd("zset1", b"new".to_vec(), 1.0, &opts);
    store.zadd("zset2", b"new".to_vec(), 2.0, &opts);

    store.zinterstore("dest", &["zset1", "zset2"], None, Aggregate::Sum);

    assert_eq!(store.zcard("dest"), 1);
    assert_eq!(store.zscore("dest", b"old"), None); // Old data removed
    assert_eq!(store.zscore("dest", b"new"), Some(3.0));
}
