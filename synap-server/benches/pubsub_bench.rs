use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde_json::json;
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::Arc;
use synap_server::core::PubSubRouter;

/// Benchmark: Topic publish throughput
fn bench_pubsub_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("pubsub_publish");

    let router = Arc::new(PubSubRouter::new());

    for message_size_desc in [("small", 64), ("medium", 256), ("large", 1024)].iter() {
        group.throughput(Throughput::Bytes(message_size_desc.1 as u64));

        group.bench_with_input(
            BenchmarkId::new("publish", message_size_desc.0),
            &message_size_desc.1,
            |b, &size| {
                let payload = json!({
                    "data": "x".repeat(size),
                    "timestamp": 1234567890
                });

                b.iter(|| {
                    router
                        .publish("test.topic", black_box(payload.clone()), None)
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Wildcard pattern matching
fn bench_pubsub_wildcards(c: &mut Criterion) {
    let mut group = c.benchmark_group("pubsub_wildcards");

    let router = Arc::new(PubSubRouter::new());

    // Setup wildcard subscriptions
    let _ = router.subscribe(vec!["events.*".to_string()]);
    let _ = router.subscribe(vec!["events.user.#".to_string()]);
    let _ = router.subscribe(vec!["metrics.*.cpu".to_string()]);

    group.bench_function("single_level_wildcard", |b| {
        let payload = json!({"event": "login"});
        b.iter(|| {
            router
                .publish("events.login", black_box(payload.clone()), None)
                .unwrap();
        });
    });

    group.bench_function("multi_level_wildcard", |b| {
        let payload = json!({"event": "user_created"});
        b.iter(|| {
            router
                .publish("events.user.created", black_box(payload.clone()), None)
                .unwrap();
        });
    });

    group.bench_function("nested_wildcards", |b| {
        let payload = json!({"value": 42});
        b.iter(|| {
            router
                .publish("metrics.server1.cpu", black_box(payload.clone()), None)
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: Topic hierarchy navigation
fn bench_pubsub_hierarchy(c: &mut Criterion) {
    let mut group = c.benchmark_group("pubsub_hierarchy");

    let router = Arc::new(PubSubRouter::new());

    // Create hierarchical topic structure
    let topics = vec![
        "app.frontend.components.button",
        "app.frontend.components.input",
        "app.frontend.pages.home",
        "app.backend.api.users",
        "app.backend.api.products",
        "app.backend.database.queries",
    ];

    for topic in topics {
        router.publish(topic, json!({"init": true}), None).unwrap();
    }

    group.bench_function("deep_topic_publish", |b| {
        let payload = json!({"action": "click"});
        b.iter(|| {
            router
                .publish(
                    "app.frontend.components.button.click",
                    black_box(payload.clone()),
                    None,
                )
                .unwrap();
        });
    });

    group.bench_function("shallow_topic_publish", |b| {
        let payload = json!({"event": "test"});
        b.iter(|| {
            router
                .publish("app.event", black_box(payload.clone()), None)
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: Subscribe/unsubscribe operations
fn bench_pubsub_subscription_mgmt(c: &mut Criterion) {
    let mut group = c.benchmark_group("pubsub_subscription");

    group.bench_function("subscribe_single_topic", |b| {
        let router = Arc::new(PubSubRouter::new());

        b.iter(|| {
            router
                .subscribe(black_box(vec!["test.topic".to_string()]))
                .unwrap();
        });
    });

    group.bench_function("subscribe_multiple_topics", |b| {
        let router = Arc::new(PubSubRouter::new());

        b.iter(|| {
            router
                .subscribe(black_box(vec![
                    "topic1".to_string(),
                    "topic2".to_string(),
                    "topic3".to_string(),
                    "topic4".to_string(),
                    "topic5".to_string(),
                ]))
                .unwrap();
        });
    });

    group.bench_function("subscribe_wildcards", |b| {
        let router = Arc::new(PubSubRouter::new());

        b.iter(|| {
            router
                .subscribe(black_box(vec![
                    "events.*".to_string(),
                    "metrics.#".to_string(),
                ]))
                .unwrap();
        });
    });

    group.bench_function("unsubscribe_all", |b| {
        let router = Arc::new(PubSubRouter::new());

        // Pre-create subscribers
        let mut subscriber_ids = vec![];
        for _ in 0..100 {
            let result = router.subscribe(vec!["test.topic".to_string()]).unwrap();
            subscriber_ids.push(result.subscriber_id);
        }

        let mut counter = 0;
        b.iter(|| {
            let sub_id = &subscriber_ids[counter % subscriber_ids.len()];
            counter += 1;
            router.unsubscribe(black_box(sub_id), None).unwrap();
        });
    });

    group.finish();
}

/// Benchmark: Publish with metadata
fn bench_pubsub_with_metadata(c: &mut Criterion) {
    let mut group = c.benchmark_group("pubsub_metadata");

    let router = Arc::new(PubSubRouter::new());

    group.bench_function("no_metadata", |b| {
        let payload = json!({"message": "test"});
        b.iter(|| {
            router
                .publish("test.topic", black_box(payload.clone()), None)
                .unwrap();
        });
    });

    group.bench_function("with_metadata", |b| {
        let payload = json!({"message": "test"});
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "benchmark".to_string());
        metadata.insert("timestamp".to_string(), "2025-10-21".to_string());
        metadata.insert("priority".to_string(), "high".to_string());

        b.iter(|| {
            router
                .publish(
                    "test.topic",
                    black_box(payload.clone()),
                    Some(metadata.clone()),
                )
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark: Topic statistics retrieval
fn bench_pubsub_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("pubsub_stats");

    let router = Arc::new(PubSubRouter::new());

    // Create multiple topics with subscriptions
    for topic_id in 0..100 {
        let topic = format!("topic_{}", topic_id);
        for _ in 0..5 {
            router.subscribe(vec![topic.clone()]).unwrap();
        }
        router.publish(&topic, json!({"init": true}), None).unwrap();
    }

    group.bench_function("global_stats", |b| {
        b.iter(|| {
            let stats = router.get_stats();
            black_box(stats);
        });
    });

    group.bench_function("list_topics", |b| {
        b.iter(|| {
            let topics = router.list_topics();
            black_box(topics);
        });
    });

    group.bench_function("topic_info", |b| {
        b.iter(|| {
            let info = router.get_topic_info("topic_0").unwrap();
            black_box(info);
        });
    });

    group.finish();
}

/// Benchmark: Pattern validation
fn bench_pubsub_pattern_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("pubsub_pattern");

    let router = Arc::new(PubSubRouter::new());

    group.bench_function("exact_topic", |b| {
        b.iter(|| {
            router
                .subscribe(black_box(vec!["exact.topic.name".to_string()]))
                .unwrap();
        });
    });

    group.bench_function("single_wildcard", |b| {
        b.iter(|| {
            router
                .subscribe(black_box(vec!["prefix.*.suffix".to_string()]))
                .unwrap();
        });
    });

    group.bench_function("multi_wildcard", |b| {
        b.iter(|| {
            router
                .subscribe(black_box(vec!["prefix.#".to_string()]))
                .unwrap();
        });
    });

    group.bench_function("complex_pattern", |b| {
        b.iter(|| {
            router
                .subscribe(black_box(vec!["events.*.user.#".to_string()]))
                .unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pubsub_publish,
    bench_pubsub_wildcards,
    bench_pubsub_hierarchy,
    bench_pubsub_subscription_mgmt,
    bench_pubsub_with_metadata,
    bench_pubsub_stats,
    bench_pubsub_pattern_validation
);

criterion_main!(benches);
