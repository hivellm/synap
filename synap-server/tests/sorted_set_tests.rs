use synap_server::core::{SortedSetStore, SortedSetValue, ZAddOptions};

// ==================== SortedSetValue Tests ====================

#[test]
fn test_zadd_single_member() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    let (added, _) = zset.zadd(b"alice".to_vec(), 100.0, &opts);

    assert_eq!(added, 1);
    assert_eq!(zset.zcard(), 1);
    assert_eq!(zset.zscore(b"alice"), Some(100.0));
}

#[test]
fn test_zadd_multiple_members() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.0, &opts);
    zset.zadd(b"bob".to_vec(), 200.0, &opts);
    zset.zadd(b"charlie".to_vec(), 150.0, &opts);

    assert_eq!(zset.zcard(), 3);
    assert_eq!(zset.zscore(b"alice"), Some(100.0));
    assert_eq!(zset.zscore(b"bob"), Some(200.0));
    assert_eq!(zset.zscore(b"charlie"), Some(150.0));
}

#[test]
fn test_zadd_update_score() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.0, &opts);
    let (added, _) = zset.zadd(b"alice".to_vec(), 200.0, &opts);

    assert_eq!(added, 0); // Not added, updated
    assert_eq!(zset.zcard(), 1);
    assert_eq!(zset.zscore(b"alice"), Some(200.0));
}

#[test]
fn test_zadd_nx_option() {
    let mut zset = SortedSetValue::new();
    let opts_default = ZAddOptions::default();
    let opts_nx = ZAddOptions {
        nx: true,
        ..Default::default()
    };

    zset.zadd(b"alice".to_vec(), 100.0, &opts_default);
    let (added, _) = zset.zadd(b"alice".to_vec(), 200.0, &opts_nx);

    assert_eq!(added, 0); // NX prevents update
    assert_eq!(zset.zscore(b"alice"), Some(100.0)); // Score unchanged

    // NX allows adding new members
    let (added, _) = zset.zadd(b"bob".to_vec(), 150.0, &opts_nx);
    assert_eq!(added, 1);
}

#[test]
fn test_zadd_xx_option() {
    let mut zset = SortedSetValue::new();
    let opts_default = ZAddOptions::default();
    let opts_xx = ZAddOptions {
        xx: true,
        ..Default::default()
    };

    // XX prevents adding new members
    let (added, _) = zset.zadd(b"alice".to_vec(), 100.0, &opts_xx);
    assert_eq!(added, 0);
    assert_eq!(zset.zcard(), 0);

    // Add normally, then XX allows update
    zset.zadd(b"alice".to_vec(), 100.0, &opts_default);
    let (added, _) = zset.zadd(b"alice".to_vec(), 200.0, &opts_xx);
    assert_eq!(added, 0);
    assert_eq!(zset.zscore(b"alice"), Some(200.0));
}

#[test]
fn test_zadd_gt_option() {
    let mut zset = SortedSetValue::new();
    let opts_default = ZAddOptions::default();
    let opts_gt = ZAddOptions {
        gt: true,
        ..Default::default()
    };

    zset.zadd(b"alice".to_vec(), 100.0, &opts_default);

    // GT prevents downgrade
    let (added, _) = zset.zadd(b"alice".to_vec(), 50.0, &opts_gt);
    assert_eq!(added, 0);
    assert_eq!(zset.zscore(b"alice"), Some(100.0));

    // GT allows upgrade
    let (added, _) = zset.zadd(b"alice".to_vec(), 150.0, &opts_gt);
    assert_eq!(added, 0);
    assert_eq!(zset.zscore(b"alice"), Some(150.0));
}

#[test]
fn test_zadd_lt_option() {
    let mut zset = SortedSetValue::new();
    let opts_default = ZAddOptions::default();
    let opts_lt = ZAddOptions {
        lt: true,
        ..Default::default()
    };

    zset.zadd(b"alice".to_vec(), 100.0, &opts_default);

    // LT prevents upgrade
    let (added, _) = zset.zadd(b"alice".to_vec(), 150.0, &opts_lt);
    assert_eq!(added, 0);
    assert_eq!(zset.zscore(b"alice"), Some(100.0));

    // LT allows downgrade
    let (added, _) = zset.zadd(b"alice".to_vec(), 50.0, &opts_lt);
    assert_eq!(added, 0);
    assert_eq!(zset.zscore(b"alice"), Some(50.0));
}

#[test]
fn test_zadd_ch_option() {
    let mut zset = SortedSetValue::new();
    let opts_ch = ZAddOptions {
        ch: true,
        ..Default::default()
    };

    // CH returns changed count
    let (added, changed) = zset.zadd(b"alice".to_vec(), 100.0, &opts_ch);
    assert_eq!(added, 0);
    assert_eq!(changed, 1); // New element counts as changed

    // Update with same score
    let (added, changed) = zset.zadd(b"alice".to_vec(), 100.0, &opts_ch);
    assert_eq!(added, 0);
    assert_eq!(changed, 0); // No change

    // Update with different score
    let (added, changed) = zset.zadd(b"alice".to_vec(), 200.0, &opts_ch);
    assert_eq!(added, 0);
    assert_eq!(changed, 1); // Changed
}

#[test]
fn test_zadd_incr_option() {
    let mut zset = SortedSetValue::new();
    let opts_incr = ZAddOptions {
        incr: true,
        ..Default::default()
    };

    // Start from 0, increment by 100
    zset.zadd(b"alice".to_vec(), 100.0, &opts_incr);
    // Increment by 50
    zset.zadd(b"alice".to_vec(), 50.0, &opts_incr);

    assert_eq!(zset.zscore(b"alice"), Some(150.0));
}

#[test]
fn test_zrem_single() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.0, &opts);
    zset.zadd(b"bob".to_vec(), 200.0, &opts);

    let removed = zset.zrem(&[b"alice".to_vec()]);

    assert_eq!(removed, 1);
    assert_eq!(zset.zcard(), 1);
    assert_eq!(zset.zscore(b"alice"), None);
    assert_eq!(zset.zscore(b"bob"), Some(200.0));
}

#[test]
fn test_zrem_multiple() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.0, &opts);
    zset.zadd(b"bob".to_vec(), 200.0, &opts);
    zset.zadd(b"charlie".to_vec(), 150.0, &opts);

    let removed = zset.zrem(&[b"alice".to_vec(), b"charlie".to_vec()]);

    assert_eq!(removed, 2);
    assert_eq!(zset.zcard(), 1);
    assert_eq!(zset.zscore(b"bob"), Some(200.0));
}

#[test]
fn test_zrem_nonexistent() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.0, &opts);

    let removed = zset.zrem(&[b"bob".to_vec(), b"charlie".to_vec()]);

    assert_eq!(removed, 0);
    assert_eq!(zset.zcard(), 1);
}

#[test]
fn test_zscore() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.5, &opts);

    assert_eq!(zset.zscore(b"alice"), Some(100.5));
    assert_eq!(zset.zscore(b"bob"), None);
}

#[test]
fn test_zincrby() {
    let mut zset = SortedSetValue::new();

    // Increment non-existing member (starts at 0)
    let score = zset.zincrby(b"alice".to_vec(), 10.5);
    assert_eq!(score, 10.5);

    // Increment existing member
    let score = zset.zincrby(b"alice".to_vec(), 5.25);
    assert_eq!(score, 15.75);

    // Negative increment (decrement)
    let score = zset.zincrby(b"alice".to_vec(), -3.5);
    assert_eq!(score, 12.25);
}

#[test]
fn test_zcard() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    assert_eq!(zset.zcard(), 0);

    zset.zadd(b"alice".to_vec(), 100.0, &opts);
    assert_eq!(zset.zcard(), 1);

    zset.zadd(b"bob".to_vec(), 200.0, &opts);
    assert_eq!(zset.zcard(), 2);

    zset.zrem(&[b"alice".to_vec()]);
    assert_eq!(zset.zcard(), 1);
}

#[test]
fn test_zrange_basic() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);
    zset.zadd(b"c".to_vec(), 3.0, &opts);
    zset.zadd(b"d".to_vec(), 4.0, &opts);

    let range = zset.zrange(0, 2, true);

    assert_eq!(range.len(), 3);
    assert_eq!(range[0].member, b"a");
    assert_eq!(range[0].score, 1.0);
    assert_eq!(range[1].member, b"b");
    assert_eq!(range[1].score, 2.0);
    assert_eq!(range[2].member, b"c");
    assert_eq!(range[2].score, 3.0);
}

#[test]
fn test_zrange_negative_indices() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);
    zset.zadd(b"c".to_vec(), 3.0, &opts);
    zset.zadd(b"d".to_vec(), 4.0, &opts);

    // Last 2 elements
    let range = zset.zrange(-2, -1, true);

    assert_eq!(range.len(), 2);
    assert_eq!(range[0].member, b"c");
    assert_eq!(range[1].member, b"d");
}

#[test]
fn test_zrange_all() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);
    zset.zadd(b"c".to_vec(), 3.0, &opts);

    let range = zset.zrange(0, -1, true);

    assert_eq!(range.len(), 3);
}

#[test]
fn test_zrevrange() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);
    zset.zadd(b"c".to_vec(), 3.0, &opts);

    let range = zset.zrevrange(0, 1, true);

    assert_eq!(range.len(), 2);
    assert_eq!(range[0].member, b"c");
    assert_eq!(range[0].score, 3.0);
    assert_eq!(range[1].member, b"b");
    assert_eq!(range[1].score, 2.0);
}

#[test]
fn test_zrank() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.0, &opts);
    zset.zadd(b"bob".to_vec(), 200.0, &opts);
    zset.zadd(b"charlie".to_vec(), 150.0, &opts);

    assert_eq!(zset.zrank(b"alice"), Some(0));
    assert_eq!(zset.zrank(b"charlie"), Some(1));
    assert_eq!(zset.zrank(b"bob"), Some(2));
    assert_eq!(zset.zrank(b"dave"), None);
}

#[test]
fn test_zrevrank() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.0, &opts);
    zset.zadd(b"bob".to_vec(), 200.0, &opts);
    zset.zadd(b"charlie".to_vec(), 150.0, &opts);

    assert_eq!(zset.zrevrank(b"bob"), Some(0));
    assert_eq!(zset.zrevrank(b"charlie"), Some(1));
    assert_eq!(zset.zrevrank(b"alice"), Some(2));
    assert_eq!(zset.zrevrank(b"dave"), None);
}

#[test]
fn test_zcount() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);
    zset.zadd(b"c".to_vec(), 3.0, &opts);
    zset.zadd(b"d".to_vec(), 4.0, &opts);
    zset.zadd(b"e".to_vec(), 5.0, &opts);

    assert_eq!(zset.zcount(2.0, 4.0), 3); // b, c, d
    assert_eq!(zset.zcount(1.0, 5.0), 5); // all
    assert_eq!(zset.zcount(3.5, 4.5), 1); // only d
    assert_eq!(zset.zcount(10.0, 20.0), 0); // none
}

#[test]
fn test_zpopmin() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);
    zset.zadd(b"c".to_vec(), 3.0, &opts);

    let popped = zset.zpopmin(2);

    assert_eq!(popped.len(), 2);
    assert_eq!(popped[0].member, b"a");
    assert_eq!(popped[0].score, 1.0);
    assert_eq!(popped[1].member, b"b");
    assert_eq!(popped[1].score, 2.0);
    assert_eq!(zset.zcard(), 1);
    assert_eq!(zset.zscore(b"c"), Some(3.0));
}

#[test]
fn test_zpopmin_more_than_available() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);

    let popped = zset.zpopmin(5);

    assert_eq!(popped.len(), 2); // Only 2 available
    assert_eq!(zset.zcard(), 0);
}

#[test]
fn test_zpopmax() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);
    zset.zadd(b"c".to_vec(), 3.0, &opts);

    let popped = zset.zpopmax(2);

    assert_eq!(popped.len(), 2);
    assert_eq!(popped[0].member, b"c");
    assert_eq!(popped[0].score, 3.0);
    assert_eq!(popped[1].member, b"b");
    assert_eq!(popped[1].score, 2.0);
    assert_eq!(zset.zcard(), 1);
    assert_eq!(zset.zscore(b"a"), Some(1.0));
}

#[test]
fn test_ttl_support() {
    let mut zset = SortedSetValue::with_ttl(3600); // 1 hour

    assert!(!zset.is_expired());
    assert!(zset.ttl().unwrap() > 3500);
    assert!(zset.ttl().unwrap() <= 3600);

    zset.persist();
    assert_eq!(zset.ttl(), None);
}

#[test]
fn test_sorted_order_maintained() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    // Add in random order
    zset.zadd(b"e".to_vec(), 5.0, &opts);
    zset.zadd(b"a".to_vec(), 1.0, &opts);
    zset.zadd(b"d".to_vec(), 4.0, &opts);
    zset.zadd(b"b".to_vec(), 2.0, &opts);
    zset.zadd(b"c".to_vec(), 3.0, &opts);

    // Verify sorted order
    let range = zset.zrange(0, -1, true);
    assert_eq!(range.len(), 5);
    assert_eq!(range[0].member, b"a");
    assert_eq!(range[1].member, b"b");
    assert_eq!(range[2].member, b"c");
    assert_eq!(range[3].member, b"d");
    assert_eq!(range[4].member, b"e");
}

#[test]
fn test_same_score_different_members() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"alice".to_vec(), 100.0, &opts);
    zset.zadd(b"bob".to_vec(), 100.0, &opts);
    zset.zadd(b"charlie".to_vec(), 100.0, &opts);

    assert_eq!(zset.zcard(), 3);
    let range = zset.zrange(0, -1, true);
    assert_eq!(range.len(), 3);

    // All should have same score
    for member in &range {
        assert_eq!(member.score, 100.0);
    }
}

#[test]
fn test_negative_scores() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), -10.0, &opts);
    zset.zadd(b"b".to_vec(), -5.0, &opts);
    zset.zadd(b"c".to_vec(), 0.0, &opts);
    zset.zadd(b"d".to_vec(), 5.0, &opts);

    let range = zset.zrange(0, -1, true);
    assert_eq!(range[0].score, -10.0);
    assert_eq!(range[1].score, -5.0);
    assert_eq!(range[2].score, 0.0);
    assert_eq!(range[3].score, 5.0);
}

#[test]
fn test_float_precision() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    zset.zadd(b"a".to_vec(), 1.123456789, &opts);

    assert_eq!(zset.zscore(b"a"), Some(1.123456789));
}

// ==================== SortedSetStore Tests ====================

#[test]
fn test_store_basic_operations() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"alice".to_vec(), 100.0, &opts);
    store.zadd("zset1", b"bob".to_vec(), 200.0, &opts);

    assert_eq!(store.zcard("zset1"), 2);
    assert_eq!(store.zscore("zset1", b"alice"), Some(100.0));
    assert_eq!(store.zscore("zset1", b"bob"), Some(200.0));
}

#[test]
fn test_store_multiple_sets() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset2", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset3", b"c".to_vec(), 3.0, &opts);

    assert_eq!(store.zcard("zset1"), 1);
    assert_eq!(store.zcard("zset2"), 1);
    assert_eq!(store.zcard("zset3"), 1);
}

#[test]
fn test_store_zincrby() {
    let store = SortedSetStore::new();

    let score = store.zincrby("zset1", b"counter".to_vec(), 10.0);
    assert_eq!(score, 10.0);

    let score = store.zincrby("zset1", b"counter".to_vec(), 5.0);
    assert_eq!(score, 15.0);
}

#[test]
fn test_store_range_operations() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);

    let range = store.zrange("zset1", 0, 1, true);
    assert_eq!(range.len(), 2);
    assert_eq!(range[0].member, b"a");
    assert_eq!(range[1].member, b"b");

    let revrange = store.zrevrange("zset1", 0, 1, true);
    assert_eq!(revrange.len(), 2);
    assert_eq!(revrange[0].member, b"c");
    assert_eq!(revrange[1].member, b"b");
}

#[test]
fn test_store_rank_operations() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"low".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"mid".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"high".to_vec(), 3.0, &opts);

    assert_eq!(store.zrank("zset1", b"low"), Some(0));
    assert_eq!(store.zrank("zset1", b"mid"), Some(1));
    assert_eq!(store.zrank("zset1", b"high"), Some(2));

    assert_eq!(store.zrevrank("zset1", b"high"), Some(0));
    assert_eq!(store.zrevrank("zset1", b"mid"), Some(1));
    assert_eq!(store.zrevrank("zset1", b"low"), Some(2));
}

#[test]
fn test_store_pop_operations() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset1", b"c".to_vec(), 3.0, &opts);

    let min = store.zpopmin("zset1", 1);
    assert_eq!(min.len(), 1);
    assert_eq!(min[0].member, b"a");
    assert_eq!(store.zcard("zset1"), 2);

    let max = store.zpopmax("zset1", 1);
    assert_eq!(max.len(), 1);
    assert_eq!(max[0].member, b"c");
    assert_eq!(store.zcard("zset1"), 1);
}

#[test]
fn test_store_delete() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    assert_eq!(store.zcard("zset1"), 1);

    let deleted = store.delete("zset1");
    assert!(deleted);
    assert_eq!(store.zcard("zset1"), 0);

    let deleted_again = store.delete("zset1");
    assert!(!deleted_again);
}

#[test]
fn test_store_stats() {
    let store = SortedSetStore::new();
    let opts = ZAddOptions::default();

    store.zadd("zset1", b"a".to_vec(), 1.0, &opts);
    store.zadd("zset1", b"b".to_vec(), 2.0, &opts);
    store.zadd("zset2", b"c".to_vec(), 3.0, &opts);

    let stats = store.stats();
    assert_eq!(stats.total_keys, 2);
    assert_eq!(stats.total_members, 3);
    assert!(stats.avg_members_per_key > 0.0);
}

#[test]
fn test_store_nonexistent_key() {
    let store = SortedSetStore::new();

    assert_eq!(store.zcard("nonexistent"), 0);
    assert_eq!(store.zscore("nonexistent", b"member"), None);
    assert_eq!(store.zrank("nonexistent", b"member"), None);
    assert_eq!(store.zcount("nonexistent", 0.0, 100.0), 0);

    let range = store.zrange("nonexistent", 0, -1, true);
    assert_eq!(range.len(), 0);
}

#[test]
fn test_large_sorted_set() {
    let mut zset = SortedSetValue::new();
    let opts = ZAddOptions::default();

    // Add 1000 members
    for i in 0..1000 {
        let member = format!("member{}", i).into_bytes();
        zset.zadd(member, i as f64, &opts);
    }

    assert_eq!(zset.zcard(), 1000);

    // Verify first and last
    assert_eq!(zset.zrank(b"member0"), Some(0));
    assert_eq!(zset.zrank(b"member999"), Some(999));

    // Range query
    let range = zset.zrange(100, 199, true);
    assert_eq!(range.len(), 100);
    assert_eq!(range[0].score, 100.0);
    assert_eq!(range[99].score, 199.0);
}

#[test]
fn test_concurrent_access_simulation() {
    let store = std::sync::Arc::new(SortedSetStore::new());
    let opts = ZAddOptions::default();

    // Simulate concurrent writes
    for i in 0..100 {
        let member = format!("member{}", i).into_bytes();
        store.zadd("concurrent", member, i as f64, &opts);
    }

    assert_eq!(store.zcard("concurrent"), 100);
}

#[test]
fn test_edge_case_empty_zset() {
    let mut zset = SortedSetValue::new();

    assert_eq!(zset.zcard(), 0);
    assert_eq!(zset.zscore(b"any"), None);
    assert_eq!(zset.zrank(b"any"), None);
    assert_eq!(zset.zcount(0.0, 100.0), 0);

    let range = zset.zrange(0, -1, true);
    assert_eq!(range.len(), 0);

    let popped = zset.zpopmin(10);
    assert_eq!(popped.len(), 0);
}
