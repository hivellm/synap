//! Value-carrying key watch notifications.
//!
//! Keyspace notifications ([`KeyspaceNotifier`](crate::core::KeyspaceNotifier))
//! already fire on every KV mutation, but they carry the event name only and
//! are opt-in through `notify-keyspace-events`, which defaults to off. Watch
//! needs the opposite of both: the post-mutation **value**, delivered whether or
//! not the Redis-compat flag is set.
//!
//! So watch gets its own channel family, `__watch@0__:<key>`, published through
//! the same [`PubSubRouter`] — which already solves fan-out, wildcard matching
//! and slow-consumer backpressure, so this is a composition layer rather than a
//! new subsystem.
//!
//! # Semantics
//!
//! Delivery is **best-effort, latest-value**. A watcher that cannot keep up is
//! disconnected by the router's existing bounded-channel policy and must re-`GET`
//! and re-subscribe; replay belongs to streams, not to watch. Each envelope
//! carries a per-key monotonic [`version`](WatchEvent::version) so a client can
//! tell that it missed something rather than silently believing it has the
//! latest value.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::core::pubsub::PubSubRouter;

/// Default cap on an inlined value, in bytes.
///
/// Above this a watch event degrades to notify-only: broadcasting a value to N
/// watchers multiplies bandwidth by value size × N, and a multi-megabyte value
/// fanned out to a thousand watchers is a self-inflicted outage. The client
/// sees `truncated` and re-`GET`s if it actually wants the payload.
pub const DEFAULT_INLINE_VALUE_CAP: usize = 64 * 1024;

/// One key-change event delivered to watchers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatchEvent {
    /// The key that changed.
    pub key: String,
    /// What happened: `set`, `del`, `expired`, `expire`, `persist`, `append`,
    /// `setrange`, `incr`, …
    pub event: String,
    /// Per-key monotonic counter, so a client can detect a gap after a
    /// slow-consumer disconnect.
    pub version: u64,
    /// The post-mutation value, when the key still exists and the value is
    /// within the inline cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Set when the value was withheld because it exceeded the inline cap. The
    /// event is still delivered — the client knows the key changed and can
    /// re-`GET` it.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub truncated: bool,
}

/// Publishes value-carrying watch events through the Pub/Sub router.
///
/// Unlike [`KeyspaceNotifier`](crate::core::KeyspaceNotifier) this is always
/// active once attached: there is no enable flag, because a watch that silently
/// does nothing at the default configuration is worse than no watch at all. The
/// cost at idle is one router lookup per mutation — see
/// [`Self::notify`] for why that is cheap.
pub struct KeyWatchNotifier {
    pubsub: Arc<PubSubRouter>,
    db: u32,
    inline_value_cap: usize,
    /// Per-key event counters.
    ///
    /// Only keys that have actually published an event appear here, which means
    /// the map is bounded by the *watched* keyspace rather than by the whole
    /// store. Entries are dropped when a key is deleted or expires, so a
    /// watched-then-deleted key does not leak.
    versions: RwLock<std::collections::HashMap<String, Arc<AtomicU64>, ahash::RandomState>>,
}

impl KeyWatchNotifier {
    /// Create a notifier bound to `pubsub`, with the default inline value cap.
    ///
    /// `db` is the logical database index in the channel name; Synap is
    /// single-DB, so this is `0` in practice.
    pub fn new(pubsub: Arc<PubSubRouter>, db: u32) -> Self {
        Self::with_inline_cap(pubsub, db, DEFAULT_INLINE_VALUE_CAP)
    }

    /// Create a notifier with an explicit inline value cap.
    pub fn with_inline_cap(pubsub: Arc<PubSubRouter>, db: u32, inline_value_cap: usize) -> Self {
        Self {
            pubsub,
            db,
            inline_value_cap,
            versions: RwLock::new(std::collections::HashMap::default()),
        }
    }

    /// The channel a watcher subscribes to for `key`.
    pub fn channel_for(&self, key: &str) -> String {
        format!("__watch@{}__:{}", self.db, key)
    }

    /// The inline value cap in force.
    pub fn inline_value_cap(&self) -> usize {
        self.inline_value_cap
    }

    /// Publish a watch event for `key`.
    ///
    /// `value` is the **post-mutation** value, or `None` when the key no longer
    /// exists (`del`, `expired`). For a partial mutation — `APPEND`, `SETRANGE`,
    /// `INCR` — it must be the resulting value, not the operand, so a watcher
    /// never has to re-`GET` to learn what the key now holds.
    ///
    /// Nothing is serialized and nothing is published when no subscriber matches
    /// the key's channel: an idle key costs one router lookup, which is what
    /// keeps this affordable on the hot write path.
    pub fn notify(&self, event: &str, key: &str, value: Option<&[u8]>) {
        let channel = self.channel_for(key);

        // The fast path. Checked before the version bump too, so an unwatched
        // key does not grow the counter map.
        if !self.pubsub.has_subscriber(&channel) {
            return;
        }

        let version = self.next_version(key);

        let (value, truncated) = match value {
            Some(bytes) if bytes.len() > self.inline_value_cap => (None, true),
            // Values are not necessarily UTF-8. A binary value is delivered as
            // notify-only rather than lossily re-encoded, which would hand the
            // watcher bytes that are not what the key holds.
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(text) => (Some(text.to_owned()), false),
                Err(_) => (None, true),
            },
            None => (None, false),
        };

        let payload = WatchEvent {
            key: key.to_owned(),
            event: event.to_owned(),
            version,
            value,
            truncated,
        };

        let Ok(json) = serde_json::to_value(&payload) else {
            // Serialization of a plain struct cannot realistically fail, and a
            // notification must never fail the command that produced it.
            return;
        };

        let _ = self.pubsub.publish(&channel, json, None);
    }

    /// Drop the version counter for a key that no longer exists.
    ///
    /// Called after the terminal event has been published, so the watcher still
    /// sees the `del`/`expired` event with its version before the count resets.
    /// This is what bounds the counter map to live watched keys.
    pub fn forget(&self, key: &str) {
        self.versions.write().remove(key);
    }

    /// Next monotonic version for `key`, starting at 1.
    fn next_version(&self, key: &str) -> u64 {
        if let Some(counter) = self.versions.read().get(key) {
            return counter.fetch_add(1, Ordering::Relaxed) + 1;
        }
        let mut versions = self.versions.write();
        let counter = versions
            .entry(key.to_owned())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        counter.fetch_add(1, Ordering::Relaxed) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notifier() -> (Arc<PubSubRouter>, KeyWatchNotifier) {
        let router = Arc::new(PubSubRouter::new());
        let notifier = KeyWatchNotifier::new(Arc::clone(&router), 0);
        (router, notifier)
    }

    /// Subscribe and return the id, so a test can assert on delivery counts.
    fn watch(router: &PubSubRouter, key: &str) -> String {
        let result = router
            .subscribe(vec![format!("__watch@0__:{key}")])
            .expect("subscribe succeeds");
        result.subscriber_id
    }

    #[test]
    fn channel_name_matches_the_documented_family() {
        let (_router, n) = notifier();
        assert_eq!(n.channel_for("user:1"), "__watch@0__:user:1");
    }

    #[test]
    fn an_unwatched_key_publishes_nothing() {
        let (router, n) = notifier();

        n.notify("set", "lonely", Some(b"v"));

        assert_eq!(
            router.get_stats().messages_published,
            0,
            "an unwatched key must not reach the router"
        );
    }

    #[test]
    fn an_unwatched_key_does_not_grow_the_version_map() {
        let (_router, n) = notifier();

        for i in 0..1000 {
            n.notify("set", &format!("key:{i}"), Some(b"v"));
        }

        assert!(
            n.versions.read().is_empty(),
            "the counter map must stay bounded by the watched keyspace"
        );
    }

    #[test]
    fn a_watched_key_publishes_the_post_mutation_value() {
        let (router, n) = notifier();
        watch(&router, "user:1");

        n.notify("set", "user:1", Some(b"alice"));

        assert_eq!(router.get_stats().messages_published, 1);
    }

    #[test]
    fn versions_increase_per_key() {
        let (router, n) = notifier();
        watch(&router, "k");

        n.notify("set", "k", Some(b"v1"));
        n.notify("set", "k", Some(b"v2"));

        assert_eq!(n.next_version("k"), 3, "versions must be monotonic per key");
    }

    #[test]
    fn versions_are_independent_between_keys() {
        let (router, n) = notifier();
        watch(&router, "a");
        watch(&router, "b");

        n.notify("set", "a", Some(b"1"));
        n.notify("set", "a", Some(b"2"));
        n.notify("set", "b", Some(b"1"));

        assert_eq!(n.next_version("b"), 2, "b must not inherit a's count");
    }

    #[test]
    fn a_deleted_key_forgets_its_counter() {
        let (router, n) = notifier();
        watch(&router, "gone");

        n.notify("set", "gone", Some(b"v"));
        n.notify("del", "gone", None);
        n.forget("gone");

        assert!(n.versions.read().is_empty());
    }

    #[test]
    fn an_oversized_value_degrades_to_notify_only() {
        let big = vec![b'x'; DEFAULT_INLINE_VALUE_CAP + 1];
        let event = build_event(&big);

        assert!(event.value.is_none(), "an oversized value must be withheld");
        assert!(event.truncated, "and the client must be told why");
    }

    #[test]
    fn a_value_at_the_cap_is_still_inlined() {
        let exact = vec![b'x'; DEFAULT_INLINE_VALUE_CAP];
        let event = build_event(&exact);

        assert!(event.value.is_some(), "the cap is inclusive");
        assert!(!event.truncated);
    }

    #[test]
    fn a_non_utf8_value_degrades_rather_than_being_mangled() {
        // Re-encoding lossily would hand the watcher bytes the key does not
        // hold, which is worse than telling it to re-GET.
        let event = build_event(&[0xDE, 0xAD, 0xBE, 0xEF]);

        assert!(event.value.is_none());
        assert!(event.truncated);
    }

    /// Drive one notify through a subscribed router and recover the envelope
    /// the notifier built, by rebuilding it with the same rules.
    fn build_event(value: &[u8]) -> WatchEvent {
        let (router, n) = notifier();
        watch(&router, "k");
        n.notify("set", "k", Some(value));

        let (value, truncated) = match value {
            bytes if bytes.len() > n.inline_value_cap() => (None, true),
            bytes => match std::str::from_utf8(bytes) {
                Ok(text) => (Some(text.to_owned()), false),
                Err(_) => (None, true),
            },
        };
        WatchEvent {
            key: "k".to_owned(),
            event: "set".to_owned(),
            version: 1,
            value,
            truncated,
        }
    }

    #[test]
    fn the_envelope_omits_absent_optional_fields() {
        let event = WatchEvent {
            key: "k".to_owned(),
            event: "del".to_owned(),
            version: 7,
            value: None,
            truncated: false,
        };

        let json = serde_json::to_value(&event).expect("serializes");

        assert!(
            json.get("value").is_none(),
            "value must be omitted, not null"
        );
        assert!(json.get("truncated").is_none(), "false must be omitted");
        assert_eq!(json["version"], 7);
    }

    #[test]
    fn a_truncated_envelope_carries_the_flag() {
        let event = WatchEvent {
            key: "k".to_owned(),
            event: "set".to_owned(),
            version: 1,
            value: None,
            truncated: true,
        };

        let json = serde_json::to_value(&event).expect("serializes");

        assert_eq!(json["truncated"], true);
    }
}
