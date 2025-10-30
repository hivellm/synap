mod test_helper;

use reqwest::{Client, StatusCode};
use serde_json::json;
use std::time::Duration;
use synap_server::{AppState, create_router};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let state: AppState = test_helper::create_test_app_state();

    let app = create_router(
        state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        synap_server::config::McpConfig::default(),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    base_url
}

#[tokio::test]
async fn test_script_eval_returns_result() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return tonumber(ARGV[1]) + tonumber(ARGV[2])",
            "args": ["2", "3"],
        }))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["result"], json!(5));
    assert!(body["sha1"].as_str().unwrap().len() == 40);
}

#[tokio::test]
async fn test_script_evalsha_uses_cache() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let load_res = client
        .post(format!("{}/script/load", base_url))
        .json(&json!({
            "script": "return #KEYS",
        }))
        .send()
        .await
        .unwrap();

    let load_body: serde_json::Value = load_res.json().await.unwrap();
    let sha = load_body["sha1"].as_str().unwrap().to_string();

    let evalsha_res = client
        .post(format!("{}/script/evalsha", base_url))
        .json(&json!({
            "sha1": sha,
            "keys": ["a", "b", "c"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = evalsha_res.json().await.unwrap();
    assert_eq!(body["result"], json!(3));
}

#[tokio::test]
async fn test_script_redis_call_bridge() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "redis.call('set', KEYS[1], ARGV[1]); return redis.call('get', KEYS[1])",
            "keys": ["scripting:key"],
            "args": ["lua-value"],
        }))
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!("lua-value"));

    let stored: serde_json::Value = client
        .get(format!("{}/kv/get/{}", base_url, "scripting:key"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stored, json!("lua-value"));
}

#[tokio::test]
async fn test_script_exists_and_flush() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let load_res = client
        .post(format!("{}/script/load", base_url))
        .json(&json!({
            "script": "return ARGV[1]",
        }))
        .send()
        .await
        .unwrap();
    let load_body: serde_json::Value = load_res.json().await.unwrap();
    let sha = load_body["sha1"].as_str().unwrap();

    let exists_res = client
        .post(format!("{}/script/exists", base_url))
        .json(&json!({
            "hashes": [sha],
        }))
        .send()
        .await
        .unwrap();
    let exists_body: serde_json::Value = exists_res.json().await.unwrap();
    assert_eq!(exists_body["exists"], json!([true]));

    let flush_res = client
        .post(format!("{}/script/flush", base_url))
        .send()
        .await
        .unwrap();
    assert!(flush_res.status().is_success());

    let exists_after = client
        .post(format!("{}/script/exists", base_url))
        .json(&json!({
            "hashes": [sha],
        }))
        .send()
        .await
        .unwrap();
    let exists_after_body: serde_json::Value = exists_after.json().await.unwrap();
    assert_eq!(exists_after_body["exists"], json!([false]));
}

#[tokio::test]
async fn test_script_disables_dangerous_functions() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return load == nil and require == nil and collectgarbage == nil",
        }))
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(true));
}

#[tokio::test]
async fn test_script_blocked_load_invocation() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return load('return 42')()",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = res.json().await.unwrap();
    let message = body["error"].as_str().unwrap();
    assert!(message.contains("attempt to call a nil value"));
}

#[tokio::test]
async fn test_script_hash_commands() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let hset_res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return redis.call('hset', KEYS[1], ARGV[1], ARGV[2])",
            "keys": ["hash:test"],
            "args": ["field", "value"],
        }))
        .send()
        .await
        .unwrap();
    let hset_body: serde_json::Value = hset_res.json().await.unwrap();
    assert_eq!(hset_body["result"], json!(1));

    let hget_res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return redis.call('hget', KEYS[1], ARGV[1])",
            "keys": ["hash:test"],
            "args": ["field"],
        }))
        .send()
        .await
        .unwrap();
    let hget_body: serde_json::Value = hget_res.json().await.unwrap();
    assert_eq!(hget_body["result"], json!("value"));
}

#[tokio::test]
async fn test_script_list_commands() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let lpush_res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return redis.call('lpush', KEYS[1], 'a', 'b', 'c')",
            "keys": ["list:test"],
        }))
        .send()
        .await
        .unwrap();
    let lpush_body: serde_json::Value = lpush_res.json().await.unwrap();
    assert_eq!(lpush_body["result"], json!(3));

    let lpop_res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return redis.call('lpop', KEYS[1], 2)",
            "keys": ["list:test"],
        }))
        .send()
        .await
        .unwrap();
    let lpop_body: serde_json::Value = lpop_res.json().await.unwrap();
    assert_eq!(lpop_body["result"], json!(["c", "b"]));
}

#[tokio::test]
async fn test_script_set_commands() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let sadd_res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return redis.call('sadd', KEYS[1], ARGV[1], ARGV[2])",
            "keys": ["set:test"],
            "args": ["alpha", "beta"],
        }))
        .send()
        .await
        .unwrap();
    let sadd_body: serde_json::Value = sadd_res.json().await.unwrap();
    assert_eq!(sadd_body["result"], json!(2));

    let scard_res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return redis.call('scard', KEYS[1])",
            "keys": ["set:test"],
        }))
        .send()
        .await
        .unwrap();
    let scard_body: serde_json::Value = scard_res.json().await.unwrap();
    assert_eq!(scard_body["result"], json!(2));

    let sismember_res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "redis.call('srem', KEYS[1], ARGV[1]); return redis.call('sismember', KEYS[1], ARGV[1])",
            "keys": ["set:test"],
            "args": ["alpha"],
        }))
        .send()
        .await
        .unwrap();
    let sismember_body: serde_json::Value = sismember_res.json().await.unwrap();
    assert_eq!(sismember_body["result"], json!(0));
}

#[tokio::test]
async fn test_script_expire_and_ttl() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let eval_res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('set', KEYS[1], '42');
                redis.call('expire', KEYS[1], tonumber(ARGV[1]));
                local ttl = redis.call('ttl', KEYS[1]);
                redis.call('persist', KEYS[1]);
                return ttl
            ",
            "keys": ["ttl:test"],
            "args": ["10"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = eval_res.json().await.unwrap();
    let ttl_value = body["result"].as_i64().unwrap();
    assert!(ttl_value >= 0 && ttl_value <= 10);

    let ttl_after = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "return redis.call('ttl', KEYS[1])",
            "keys": ["ttl:test"],
        }))
        .send()
        .await
        .unwrap();
    let ttl_after_body: serde_json::Value = ttl_after.json().await.unwrap();
    assert_eq!(ttl_after_body["result"], json!(-1));
}

#[tokio::test]
async fn test_script_sortedset_basic() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], ARGV[1], ARGV[2], ARGV[3], ARGV[4], ARGV[5], ARGV[6]);
                return redis.call('zrange', KEYS[1], 0, -1)
            ",
            "keys": ["leaders"],
            "args": ["10", "alice", "20", "bob", "15", "carol"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["alice", "carol", "bob"]));
}

#[tokio::test]
async fn test_script_sortedset_withscores() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 5, 'eve', 8, 'mallory');
                return redis.call('zrange', KEYS[1], 0, -1, 'WITHSCORES')
            ",
            "keys": ["scores"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["eve", "5", "mallory", "8"]));
}

#[tokio::test]
async fn test_script_zincrby_and_rank() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'p1');
                redis.call('zadd', KEYS[1], 2, 'p2');
                local new_score = redis.call('zincrby', KEYS[1], 5, 'p1');
                local rank = redis.call('zrank', KEYS[1], 'p2');
                return {new_score, rank}
            ",
            "keys": ["players"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["6", 0]));
}

#[tokio::test]
async fn test_script_zpopmin_returns_pairs() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'low', 3, 'mid', 5, 'high');
                return redis.call('zpopmin', KEYS[1], 2)
            ",
            "keys": ["queue"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["low", "1", "mid", "3"]));
}

#[tokio::test]
async fn test_script_sortedset_zcard_and_zscore() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], tonumber(ARGV[1]), ARGV[2]);
                local card = redis.call('zcard', KEYS[1]);
                local score = redis.call('zscore', KEYS[1], ARGV[2]);
                return {card, score}
            ",
            "keys": ["sorted:zcard"],
            "args": ["10", "alpha"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!([1, "10"]));
}

#[tokio::test]
async fn test_script_sortedset_zrangebyscore_plain() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one', 2, 'two', 3, 'three', 5, 'five');
                return redis.call('zrangebyscore', KEYS[1], 2, 3.5)
            ",
            "keys": ["sorted:zrangebyscore"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["two", "three"]));
}

#[tokio::test]
async fn test_script_sortedset_zrem_and_remaining() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one', 2, 'two', 3, 'three');
                local removed = redis.call('zrem', KEYS[1], 'two');
                local card = redis.call('zcard', KEYS[1]);
                return {removed, card}
            ",
            "keys": ["sorted:zrem"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!([1, 2]));
}

#[tokio::test]
async fn test_script_sortedset_zcount() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one', 2, 'two', 3, 'three');
                return redis.call('zcount', KEYS[1], 1, 2)
            ",
            "keys": ["sorted:zcount"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(2));
}

#[tokio::test]
async fn test_script_sortedset_zrank_nil() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one');
                return redis.call('zrank', KEYS[1], 'missing')
            ",
            "keys": ["sorted:zrank:nil"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_script_sortedset_zrevrange_withscores() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one', 4, 'four', 2, 'two');
                return redis.call('zrevrange', KEYS[1], 0, -1, 'WITHSCORES')
            ",
            "keys": ["sorted:zrevrange"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["four", "4", "two", "2", "one", "1"]));
}

#[tokio::test]
async fn test_script_sortedset_zremrangebyrank() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one', 2, 'two', 3, 'three');
                return redis.call('zremrangebyrank', KEYS[1], 0, 1)
            ",
            "keys": ["sorted:zremrangebyrank"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(2));
}

#[tokio::test]
async fn test_script_sortedset_zremrangebyscore() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one', 2, 'two', 3, 'three');
                return redis.call('zremrangebyscore', KEYS[1], 2, 3)
            ",
            "keys": ["sorted:zremrangebyscore"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(2));
}

#[tokio::test]
async fn test_script_sortedset_zmscore() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 5, 'five');
                return redis.call('zmscore', KEYS[1], 'five', 'missing')
            ",
            "keys": ["sorted:zmscore"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["5"]));
}

#[tokio::test]
async fn test_script_sortedset_zpopmax() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'low', 9, 'high');
                return redis.call('zpopmax', KEYS[1], 1)
            ",
            "keys": ["sorted:zpopmax"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["high", "9"]));
}

#[tokio::test]
async fn test_script_sortedset_zrange_empty() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one');
                redis.call('zpopmax', KEYS[1], 1);
                return redis.call('zrange', KEYS[1], 0, -1)
            ",
            "keys": ["sorted:zrange:empty"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_script_sortedset_zrange_withscores_after_updates() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 2, 'item');
                redis.call('zincrby', KEYS[1], 3, 'item');
                return redis.call('zrange', KEYS[1], 0, -1, 'WITHSCORES')
            ",
            "keys": ["sorted:zrange:withscores"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["item", "5"]));
}

#[tokio::test]
async fn test_script_sortedset_zrangebyscore_withscores() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 2, 'two', 4, 'four', 6, 'six');
                return redis.call('zrangebyscore', KEYS[1], 2, 5, 'WITHSCORES')
            ",
            "keys": ["sorted:zrangebyscore:withscores"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["two", "2", "four", "4"]));
}

#[tokio::test]
async fn test_script_sortedset_zcard_zero_after_removals() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one');
                redis.call('zrem', KEYS[1], 'one');
                return redis.call('zcard', KEYS[1])
            ",
            "keys": ["sorted:zcard:zero"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(0));
}

#[tokio::test]
async fn test_script_sortedset_zcount_zero_range() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 1, 'one', 2, 'two');
                return redis.call('zcount', KEYS[1], 5, 10)
            ",
            "keys": ["sorted:zcount:zero"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(0));
}

#[tokio::test]
async fn test_script_sortedset_zmscore_mixed_results() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .post(format!("{}/script/eval", base_url))
        .json(&json!({
            "script": "\
                redis.call('zadd', KEYS[1], 7, 'seven', 9, 'nine');
                return redis.call('zmscore', KEYS[1], 'nine', 'missing', 'seven')
            ",
            "keys": ["sorted:zmscore:mixed"],
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"], json!(["9", null, "7"]));
}
