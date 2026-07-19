//! KV watch fan-out benchmark.
//!
//! Validates the "efficient for many watchers" claim by measuring the
//! publish→deliver cost of [`KeyWatchNotifier::notify`] as the number of
//! watchers on one key — and on a shared wildcard — grows, plus the idle cost
//! when nobody is watching and the cost under a stalled (slow-consumer)
//! watcher.

use std::hint::black_box;
use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use synap_core::core::{KeyWatchNotifier, PubSubRouter};
use tokio::sync::mpsc;

/// Register `n` drained watchers on `channel` and return the notifier + router.
/// The receivers are spawned onto a runtime that empties them, so delivery
/// measures the real send path rather than filling bounded buffers.
fn setup_watchers(
    rt: &tokio::runtime::Runtime,
    channel: &str,
    n: usize,
) -> (Arc<PubSubRouter>, KeyWatchNotifier) {
    let router = Arc::new(PubSubRouter::new());
    let notifier = KeyWatchNotifier::new(Arc::clone(&router), 0);

    for _ in 0..n {
        let result = router
            .subscribe(vec![channel.to_string()])
            .expect("subscribe succeeds");
        let (tx, mut rx) = mpsc::channel(1024);
        router.register_connection(result.subscriber_id, tx);
        // A drainer per watcher, so the bounded channel never backs up.
        rt.spawn(async move { while rx.recv().await.is_some() {} });
    }

    (router, notifier)
}

/// Fan-out to N exact-key watchers.
fn bench_watch_fanout_exact(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime builds");
    let mut group = c.benchmark_group("kv_watch_fanout_exact");

    for &n in &[1usize, 10, 100, 1000] {
        let (_router, notifier) = setup_watchers(&rt, "__watch@0__:hot", n);
        // Let the drainer tasks start.
        std::thread::sleep(std::time::Duration::from_millis(20));

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                notifier.notify("set", black_box("hot"), black_box(Some(b"value")));
            });
        });
    }

    group.finish();
}

/// Fan-out to N watchers sharing one wildcard subscription (`user:*`), the
/// cache-invalidation shape.
fn bench_watch_fanout_wildcard(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime builds");
    let mut group = c.benchmark_group("kv_watch_fanout_wildcard");

    for &n in &[1usize, 10, 100, 1000] {
        let (_router, notifier) = setup_watchers(&rt, "__watch@0__:user:*", n);
        std::thread::sleep(std::time::Duration::from_millis(20));

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                notifier.notify("set", black_box("user:42"), black_box(Some(b"value")));
            });
        });
    }

    group.finish();
}

/// The idle path: no watcher matches, so `notify` is one router lookup and
/// returns before building an envelope or bumping a version counter.
fn bench_watch_idle(c: &mut Criterion) {
    let router = Arc::new(PubSubRouter::new());
    let notifier = KeyWatchNotifier::new(router, 0);

    c.bench_function("kv_watch_idle_unwatched", |b| {
        b.iter(|| {
            notifier.notify("set", black_box("cold"), black_box(Some(b"value")));
        });
    });
}

/// One stalled watcher (its bounded buffer is never drained) alongside `n`
/// healthy ones. Measures that a slow consumer does not slow delivery to the
/// rest — the router drops it rather than blocking the publish path.
fn bench_watch_stalled_consumer(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime builds");
    let mut group = c.benchmark_group("kv_watch_stalled_consumer");

    for &n in &[10usize, 100] {
        let router = Arc::new(PubSubRouter::new());
        let notifier = KeyWatchNotifier::new(Arc::clone(&router), 0);

        // Healthy, drained watchers.
        for _ in 0..n {
            let result = router
                .subscribe(vec!["__watch@0__:hot".to_string()])
                .expect("subscribe succeeds");
            let (tx, mut rx) = mpsc::channel(1024);
            router.register_connection(result.subscriber_id, tx);
            rt.spawn(async move { while rx.recv().await.is_some() {} });
        }

        // One stalled watcher: a capacity-1 channel that is never read, so it
        // fills immediately and every later publish finds it full.
        let stalled = router
            .subscribe(vec!["__watch@0__:hot".to_string()])
            .expect("subscribe succeeds");
        let (stalled_tx, _stalled_rx) = mpsc::channel(1);
        router.register_connection(stalled.subscriber_id, stalled_tx);

        std::thread::sleep(std::time::Duration::from_millis(20));

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                notifier.notify("set", black_box("hot"), black_box(Some(b"value")));
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_watch_fanout_exact,
    bench_watch_fanout_wildcard,
    bench_watch_idle,
    bench_watch_stalled_consumer,
);
criterion_main!(benches);
