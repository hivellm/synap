//! Keyspace notifications (Redis `notify-keyspace-events`).
//!
//! When enabled, mutating commands publish two kinds of events through the
//! Pub/Sub router so `PSUBSCRIBE`rs can observe changes:
//!
//! - **Keyspace** events on `__keyspace@<db>__:<key>` with the *event name* as
//!   the payload (e.g. key `foo` set → topic `__keyspace@0__:foo`, payload
//!   `"set"`).
//! - **Keyevent** events on `__keyevent@<db>__:<event>` with the *key* as the
//!   payload (e.g. `__keyevent@0__:set`, payload `"foo"`).
//!
//! Which classes fire is controlled by a flag set parsed from the Redis-style
//! spec string (`K`, `E`, `g`, `$`, `l`, `s`, `h`, `z`, `x`, `e`, `A`). Both a
//! target (`K` keyspace and/or `E` keyevent) and at least one class must be set
//! for any notification to be delivered.

use std::sync::Arc;

use crate::core::pubsub::PubSubRouter;

/// The data-type / operation class an event belongs to. Maps to the Redis
/// per-class flags that gate whether an event is published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventClass {
    /// `g` — generic key ops (`del`, `expire`, `rename`, `persist`).
    Generic,
    /// `$` — string commands.
    String,
    /// `l` — list commands.
    List,
    /// `s` — set commands.
    Set,
    /// `h` — hash commands.
    Hash,
    /// `z` — sorted-set commands.
    SortedSet,
    /// `x` — expired events (key removed because its TTL elapsed).
    Expired,
    /// `e` — evicted events (key removed under memory pressure).
    Evicted,
}

/// Parsed `notify-keyspace-events` flag set. Cheap to copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyspaceEventFlags {
    /// `K` — publish keyspace events (`__keyspace@<db>__:<key>`).
    pub keyspace: bool,
    /// `E` — publish keyevent events (`__keyevent@<db>__:<event>`).
    pub keyevent: bool,
    pub generic: bool,
    pub string: bool,
    pub list: bool,
    pub set: bool,
    pub hash: bool,
    pub sorted_set: bool,
    pub expired: bool,
    pub evicted: bool,
}

impl KeyspaceEventFlags {
    /// Parse a Redis-style spec string. Unknown characters are ignored. `A` is
    /// shorthand for all classes (`g$lshzxe`) but not the `K`/`E` targets.
    ///
    /// Examples: `"KEA"` (everything), `"Kg$"` (keyspace generic+string only),
    /// `"Ex"` (keyevent expired only).
    pub fn parse(spec: &str) -> Self {
        let mut f = Self::default();
        for c in spec.chars() {
            match c {
                'K' => f.keyspace = true,
                'E' => f.keyevent = true,
                'g' => f.generic = true,
                '$' => f.string = true,
                'l' => f.list = true,
                's' => f.set = true,
                'h' => f.hash = true,
                'z' => f.sorted_set = true,
                'x' => f.expired = true,
                'e' => f.evicted = true,
                'A' => {
                    f.generic = true;
                    f.string = true;
                    f.list = true;
                    f.set = true;
                    f.hash = true;
                    f.sorted_set = true;
                    f.expired = true;
                    f.evicted = true;
                }
                _ => {}
            }
        }
        f
    }

    /// True when at least one target (`K`/`E`) and at least one class are set —
    /// i.e. some notification could actually be delivered.
    pub fn is_active(&self) -> bool {
        (self.keyspace || self.keyevent)
            && (self.generic
                || self.string
                || self.list
                || self.set
                || self.hash
                || self.sorted_set
                || self.expired
                || self.evicted)
    }

    /// Whether events of the given class are enabled.
    pub fn class_enabled(&self, class: EventClass) -> bool {
        match class {
            EventClass::Generic => self.generic,
            EventClass::String => self.string,
            EventClass::List => self.list,
            EventClass::Set => self.set,
            EventClass::Hash => self.hash,
            EventClass::SortedSet => self.sorted_set,
            EventClass::Expired => self.expired,
            EventClass::Evicted => self.evicted,
        }
    }
}

/// Publishes keyspace / keyevent notifications through the Pub/Sub router.
///
/// Constructed only when notifications are enabled; stores hold it as an
/// `Option<Arc<KeyspaceNotifier>>`, so the disabled path is a single `None`
/// check with zero publish overhead.
pub struct KeyspaceNotifier {
    pubsub: Arc<PubSubRouter>,
    flags: KeyspaceEventFlags,
    db: u32,
}

impl KeyspaceNotifier {
    /// Create a notifier bound to `pubsub` with the given `flags`. `db` is the
    /// logical database index embedded in the channel names (Synap is single-DB,
    /// so this is `0` in practice).
    pub fn new(pubsub: Arc<PubSubRouter>, flags: KeyspaceEventFlags, db: u32) -> Self {
        Self { pubsub, flags, db }
    }

    /// Whether any notification could be delivered with the current flags.
    pub fn is_active(&self) -> bool {
        self.flags.is_active()
    }

    /// Publish `event` for `key` in `class`. No-op when the class or both targets
    /// are disabled. Delivery failures (e.g. no subscribers) are ignored — a
    /// notification is best-effort and must never fail the originating command.
    pub fn notify(&self, class: EventClass, event: &str, key: &str) {
        if !self.flags.class_enabled(class) {
            return;
        }
        if self.flags.keyspace {
            let topic = format!("__keyspace@{}__:{}", self.db, key);
            let _ = self
                .pubsub
                .publish(&topic, serde_json::Value::String(event.to_string()), None);
        }
        if self.flags.keyevent {
            let topic = format!("__keyevent@{}__:{}", self.db, event);
            let _ = self
                .pubsub
                .publish(&topic, serde_json::Value::String(key.to_string()), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all_flags() {
        let f = KeyspaceEventFlags::parse("KEA");
        assert!(f.keyspace && f.keyevent);
        assert!(f.generic && f.string && f.list && f.set);
        assert!(f.hash && f.sorted_set && f.expired && f.evicted);
        assert!(f.is_active());
    }

    #[test]
    fn test_parse_subset() {
        let f = KeyspaceEventFlags::parse("Kg$");
        assert!(f.keyspace);
        assert!(!f.keyevent);
        assert!(f.generic && f.string);
        assert!(!f.list && !f.hash);
        assert!(f.class_enabled(EventClass::Generic));
        assert!(!f.class_enabled(EventClass::List));
    }

    #[test]
    fn test_not_active_without_target() {
        // Classes but no K/E target → nothing can be delivered.
        let f = KeyspaceEventFlags::parse("g$l");
        assert!(!f.is_active());
    }

    #[test]
    fn test_not_active_without_class() {
        // Targets but no class → nothing can be delivered.
        let f = KeyspaceEventFlags::parse("KE");
        assert!(!f.is_active());
    }

    #[test]
    fn test_unknown_chars_ignored() {
        let f = KeyspaceEventFlags::parse("K$?#@");
        assert!(f.keyspace && f.string);
        assert!(!f.keyevent);
    }

    #[test]
    fn test_notify_publishes_keyspace_and_keyevent() {
        let pubsub = Arc::new(PubSubRouter::new());
        let notifier = KeyspaceNotifier::new(pubsub.clone(), KeyspaceEventFlags::parse("KEA"), 0);
        // No subscribers, but publish must succeed and bump the published count.
        notifier.notify(EventClass::String, "set", "foo");
        let stats = pubsub.get_stats();
        // One keyspace + one keyevent publish.
        assert_eq!(stats.messages_published, 2);
    }

    #[test]
    fn test_notify_respects_class_gate() {
        let pubsub = Arc::new(PubSubRouter::new());
        // String disabled (only generic).
        let notifier = KeyspaceNotifier::new(pubsub.clone(), KeyspaceEventFlags::parse("KEg"), 0);
        notifier.notify(EventClass::String, "set", "foo");
        assert_eq!(pubsub.get_stats().messages_published, 0);
        notifier.notify(EventClass::Generic, "del", "foo");
        assert_eq!(pubsub.get_stats().messages_published, 2);
    }
}
