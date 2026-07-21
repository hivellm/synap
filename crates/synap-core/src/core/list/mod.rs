//! List data structure implementation for Synap
//!
//! Provides Redis-compatible list operations (LPUSH, RPUSH, LPOP, RPOP, LRANGE, etc.)
//! Storage: VecDeque for O(1) push/pop at both ends
//!
//! # Performance Targets
//! - LPUSH/RPUSH: <100µs p99 latency
//! - LPOP/RPOP: <100µs p99 latency
//! - LRANGE (100 items): <500µs p99 latency
//! - BLPOP (no wait): <100µs p99 latency
//!
//! # Architecture
//! ```text
//! ListStore
//!   ├─ 64 shards (Arc<RwLock<HashMap<key, ListValue>>>)
//!   └─ TTL applies to entire list
//! ```

use super::error::{Result, SynapError};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Packed-encoding thresholds — mirror Redis's `list-max-listpack-size`
/// defaults: past either bound, the list upgrades to the `Deque` representation.
const MAX_PACKED_ENTRIES: usize = 128;
const MAX_PACKED_ELEM: usize = 64;

/// Storage representation for a list (phase13 contiguous encoding).
///
/// Small lists are stored **packed**: `[u32 LE len][bytes]…` entries in one
/// contiguous buffer — Redis's listpack analogue. Pushing appends into a hot
/// buffer with no per-element heap allocation and dense cache behaviour. Past
/// the thresholds the list upgrades (once, one-way — like listpack → quicklist)
/// to the `VecDeque<Vec<u8>>` representation, and the pre-existing complex-op
/// logic (LSET/LTRIM/LREM/LINSERT) always runs on `Deque` via a lazy upgrade,
/// so it did not need to be rewritten for two encodings.
#[derive(Debug, Clone)]
enum ListRepr {
    Packed { buf: Vec<u8>, count: usize },
    Deque(VecDeque<Vec<u8>>),
}

impl ListRepr {
    /// Iterate the packed entries as byte slices.
    fn packed_iter(buf: &[u8]) -> impl Iterator<Item = &[u8]> {
        let mut pos = 0usize;
        std::iter::from_fn(move || {
            if pos + 4 > buf.len() {
                return None;
            }
            let len =
                u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
            let start = pos + 4;
            pos = start + len;
            buf.get(start..start + len)
        })
    }

    fn append_entry(buf: &mut Vec<u8>, value: &[u8]) {
        buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
        buf.extend_from_slice(value);
    }
}

/// Serialize the representation as the logical element sequence — byte-for-byte
/// the same encoding `VecDeque<Vec<u8>>` produced, so snapshots written before
/// the packed encoding still load and new snapshots stay readable.
mod repr_as_seq {
    use super::ListRepr;
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(repr: &ListRepr, s: S) -> Result<S::Ok, S::Error> {
        match repr {
            ListRepr::Packed { buf, count } => {
                let mut seq = s.serialize_seq(Some(*count))?;
                for e in ListRepr::packed_iter(buf) {
                    seq.serialize_element(e)?;
                }
                seq.end()
            }
            ListRepr::Deque(d) => {
                let mut seq = s.serialize_seq(Some(d.len()))?;
                for e in d {
                    seq.serialize_element(e)?;
                }
                seq.end()
            }
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<ListRepr, D::Error> {
        let elements = Vec::<Vec<u8>>::deserialize(d)?;
        Ok(super::ListValue::repr_from_elements(elements))
    }
}

/// List value stored in a single key
/// Contains ordered elements with push/pop at both ends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListValue {
    /// Ordered elements (front = left, back = right)
    #[serde(rename = "elements", with = "repr_as_seq")]
    repr: ListRepr,
    /// TTL for entire list
    pub ttl_secs: Option<u64>,
    /// Creation timestamp
    pub created_at: u64,
    /// Last modification timestamp
    pub updated_at: u64,
}

impl ListValue {
    /// Create new list value
    pub fn new(ttl_secs: Option<u64>) -> Self {
        let now = Self::current_timestamp();
        Self {
            repr: ListRepr::Packed {
                buf: Vec::new(),
                count: 0,
            },
            ttl_secs,
            created_at: now,
            updated_at: now,
        }
    }

    /// Build the right representation for a known element sequence (snapshot
    /// recovery / deserialization).
    fn repr_from_elements(elements: Vec<Vec<u8>>) -> ListRepr {
        let fits = elements.len() <= MAX_PACKED_ENTRIES
            && elements.iter().all(|e| e.len() <= MAX_PACKED_ELEM);
        if fits {
            let mut buf = Vec::new();
            let count = elements.len();
            for e in &elements {
                ListRepr::append_entry(&mut buf, e);
            }
            ListRepr::Packed { buf, count }
        } else {
            ListRepr::Deque(elements.into())
        }
    }

    /// Upgrade to the `Deque` representation (one-way) and return it — the
    /// complex mutators run their existing logic on this.
    fn ensure_deque(&mut self) -> &mut VecDeque<Vec<u8>> {
        if let ListRepr::Packed { buf, .. } = &self.repr {
            let elements: VecDeque<Vec<u8>> =
                ListRepr::packed_iter(buf).map(|e| e.to_vec()).collect();
            self.repr = ListRepr::Deque(elements);
        }
        match &mut self.repr {
            ListRepr::Deque(d) => d,
            ListRepr::Packed { .. } => unreachable!("just upgraded"),
        }
    }

    /// Whether one more element of `elem_len` bytes still fits the packed form.
    fn packed_fits(&self, elem_len: usize) -> bool {
        match &self.repr {
            ListRepr::Packed { count, .. } => {
                *count < MAX_PACKED_ENTRIES && elem_len <= MAX_PACKED_ELEM
            }
            ListRepr::Deque(_) => false,
        }
    }

    /// Total payload bytes across elements (memory accounting).
    pub fn element_bytes(&self) -> usize {
        match &self.repr {
            // Subtract the 4-byte length headers — accounting wants the logical
            // payload (matches the previous per-element sum).
            ListRepr::Packed { buf, count } => buf.len().saturating_sub(4 * count),
            ListRepr::Deque(d) => d.iter().map(Vec::len).sum(),
        }
    }

    /// Consume into the logical element sequence (snapshot recovery).
    pub fn into_elements(self) -> Vec<Vec<u8>> {
        match self.repr {
            ListRepr::Packed { buf, .. } => {
                ListRepr::packed_iter(&buf).map(|e| e.to_vec()).collect()
            }
            ListRepr::Deque(d) => d.into(),
        }
    }

    /// Get current Unix timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Check if list has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_secs {
            let now = Self::current_timestamp();
            now >= self.created_at + ttl
        } else {
            false
        }
    }

    /// Get number of elements
    pub fn len(&self) -> usize {
        match &self.repr {
            ListRepr::Packed { count, .. } => *count,
            ListRepr::Deque(d) => d.len(),
        }
    }

    /// Check if list is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Push element to left (front)
    pub fn lpush(&mut self, value: Vec<u8>) {
        self.updated_at = Self::current_timestamp();
        if self.packed_fits(value.len())
            && let ListRepr::Packed { buf, count } = &mut self.repr
        {
            // Prepend: splice the entry at the front. O(len(buf)) memmove —
            // bounded by the packed thresholds, cache-friendly.
            let mut entry = Vec::with_capacity(4 + value.len());
            ListRepr::append_entry(&mut entry, &value);
            buf.splice(0..0, entry);
            *count += 1;
            return;
        }
        self.ensure_deque().push_front(value);
    }

    /// Push element to right (back)
    pub fn rpush(&mut self, value: Vec<u8>) {
        self.updated_at = Self::current_timestamp();
        if self.packed_fits(value.len())
            && let ListRepr::Packed { buf, count } = &mut self.repr
        {
            // Append into the hot contiguous buffer — no per-element alloc.
            ListRepr::append_entry(buf, &value);
            *count += 1;
            return;
        }
        self.ensure_deque().push_back(value);
    }

    /// Pop element from left (front)
    pub fn lpop(&mut self) -> Option<Vec<u8>> {
        self.updated_at = Self::current_timestamp();
        match &mut self.repr {
            ListRepr::Packed { buf, count } => {
                let first = ListRepr::packed_iter(buf).next()?.to_vec();
                buf.drain(..4 + first.len());
                *count -= 1;
                Some(first)
            }
            ListRepr::Deque(d) => d.pop_front(),
        }
    }

    /// Pop element from right (back)
    pub fn rpop(&mut self) -> Option<Vec<u8>> {
        self.updated_at = Self::current_timestamp();
        match &mut self.repr {
            ListRepr::Packed { buf, count } => {
                // Find the last entry's start (forward scan — bounded by the
                // packed entry cap).
                let mut pos = 0usize;
                let mut last_start = None;
                while pos + 4 <= buf.len() {
                    let len =
                        u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]])
                            as usize;
                    last_start = Some(pos);
                    pos += 4 + len;
                }
                let start = last_start?;
                let value = buf[start + 4..].to_vec();
                buf.truncate(start);
                *count -= 1;
                Some(value)
            }
            ListRepr::Deque(d) => d.pop_back(),
        }
    }

    /// Get range of elements (0-indexed, inclusive)
    /// Supports negative indices (-1 = last element)
    pub fn lrange(&self, start: i64, stop: i64) -> Vec<Vec<u8>> {
        let len = self.len() as i64;
        if len == 0 {
            return Vec::new();
        }

        // Normalize negative indices
        let start = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len - 1)
        };

        let stop = if stop < 0 {
            (len + stop).max(-1)
        } else {
            stop.min(len - 1)
        };

        if start > stop || start >= len {
            return Vec::new();
        }

        let skip = start as usize;
        let take = (stop - start + 1) as usize;
        match &self.repr {
            ListRepr::Packed { buf, .. } => ListRepr::packed_iter(buf)
                .skip(skip)
                .take(take)
                .map(|e| e.to_vec())
                .collect(),
            ListRepr::Deque(d) => d.iter().skip(skip).take(take).cloned().collect(),
        }
    }

    /// Get element at index (0-indexed)
    /// Supports negative indices (-1 = last element)
    pub fn lindex(&self, index: i64) -> Option<Vec<u8>> {
        let len = self.len() as i64;
        let idx = if index < 0 {
            (len + index).max(0)
        } else {
            index
        };

        if idx >= len || idx < 0 {
            None
        } else {
            match &self.repr {
                ListRepr::Packed { buf, .. } => ListRepr::packed_iter(buf)
                    .nth(idx as usize)
                    .map(<[u8]>::to_vec),
                ListRepr::Deque(d) => d.get(idx as usize).cloned(),
            }
        }
    }

    /// Set element at index
    pub fn lset(&mut self, index: i64, value: Vec<u8>) -> Result<()> {
        let len = self.len() as i64;
        let idx = if index < 0 {
            (len + index).max(0)
        } else {
            index
        };

        if idx >= len || idx < 0 {
            return Err(SynapError::IndexOutOfRange);
        }

        self.updated_at = Self::current_timestamp();
        // Rare mutator: run on the Deque representation (lazy one-way upgrade).
        self.ensure_deque()[idx as usize] = value;
        Ok(())
    }

    /// Trim list to keep only elements in range [start, stop]
    pub fn ltrim(&mut self, start: i64, stop: i64) {
        let len = self.len() as i64;
        if len == 0 {
            return;
        }

        // Normalize indices
        let start = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len - 1)
        };

        let stop = if stop < 0 {
            (len + stop).max(-1)
        } else {
            stop.min(len - 1)
        };

        // Rare mutator: run on the Deque representation (lazy one-way upgrade).
        let d = self.ensure_deque();

        if start > stop || start >= len {
            d.clear();
            return;
        }

        self.updated_at = Self::current_timestamp();
        let d = self.ensure_deque();

        // Remove elements after stop+1
        let keep_len = (stop + 1) as usize;
        d.truncate(keep_len);

        // Remove elements before start
        for _ in 0..start {
            d.pop_front();
        }
    }

    /// Remove count occurrences of value
    /// count > 0: remove from head to tail
    /// count < 0: remove from tail to head
    /// count = 0: remove all occurrences
    pub fn lrem(&mut self, count: i64, value: &[u8]) -> usize {
        self.updated_at = Self::current_timestamp();
        let mut removed = 0;
        // Rare mutator: run on the Deque representation (lazy one-way upgrade).
        let d = self.ensure_deque();

        if count == 0 {
            // Remove all occurrences
            d.retain(|elem| elem != value);
            removed = d.len();
        } else if count > 0 {
            // Remove from head to tail
            let mut i = 0;
            while i < d.len() && removed < count as usize {
                if d[i] == value {
                    d.remove(i);
                    removed += 1;
                } else {
                    i += 1;
                }
            }
        } else {
            // Remove from tail to head
            let mut i = d.len();
            let target = count.unsigned_abs() as usize;
            while i > 0 && removed < target {
                i -= 1;
                if d[i] == value {
                    d.remove(i);
                    removed += 1;
                }
            }
        }

        removed
    }

    /// Insert value before or after pivot
    pub fn linsert(&mut self, before: bool, pivot: &[u8], value: Vec<u8>) -> Result<usize> {
        // Rare mutator: run on the Deque representation (lazy one-way upgrade).
        let d = self.ensure_deque();
        if let Some(pos) = d.iter().position(|elem| elem == pivot) {
            let insert_pos = if before { pos } else { pos + 1 };
            d.insert(insert_pos, value);
            self.updated_at = Self::current_timestamp();
            Ok(self.len())
        } else {
            Err(SynapError::NotFound)
        }
    }

    /// Find position of value (first occurrence, 0-indexed)
    pub fn lpos(&self, value: &[u8]) -> Option<usize> {
        match &self.repr {
            ListRepr::Packed { buf, .. } => ListRepr::packed_iter(buf).position(|e| e == value),
            ListRepr::Deque(d) => d.iter().position(|elem| elem == value),
        }
    }
}

mod store;
pub use store::{ListStats, ListStore};

#[cfg(test)]
mod tests {
    use super::*;

    /// Packed → Deque upgrade at the entry-count threshold keeps order and ops.
    #[test]
    fn test_packed_upgrade_on_entry_count() {
        let mut lv = ListValue::new(None);
        for i in 0..(MAX_PACKED_ENTRIES + 10) {
            lv.rpush(format!("e{i}").into_bytes());
        }
        assert!(
            matches!(lv.repr, ListRepr::Deque(_)),
            "must upgrade past cap"
        );
        assert_eq!(lv.len(), MAX_PACKED_ENTRIES + 10);
        assert_eq!(lv.lindex(0), Some(b"e0".to_vec()));
        assert_eq!(
            lv.lindex(-1),
            Some(format!("e{}", MAX_PACKED_ENTRIES + 9).into_bytes())
        );
        assert_eq!(lv.lpop(), Some(b"e0".to_vec()));
        assert_eq!(
            lv.rpop(),
            Some(format!("e{}", MAX_PACKED_ENTRIES + 9).into_bytes())
        );
    }

    /// A single oversized element upgrades immediately.
    #[test]
    fn test_packed_upgrade_on_large_element() {
        let mut lv = ListValue::new(None);
        lv.rpush(vec![b'x'; MAX_PACKED_ELEM + 1]);
        assert!(matches!(lv.repr, ListRepr::Deque(_)));
        assert_eq!(lv.len(), 1);
    }

    /// Packed push/pop from both ends behaves exactly like the deque form.
    #[test]
    fn test_packed_ops_match_semantics() {
        let mut lv = ListValue::new(None);
        lv.rpush(b"b".to_vec());
        lv.lpush(b"a".to_vec());
        lv.rpush(b"c".to_vec());
        assert!(matches!(lv.repr, ListRepr::Packed { .. }));
        assert_eq!(
            lv.lrange(0, -1),
            vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
        );
        assert_eq!(lv.lpos(b"b"), Some(1));
        assert_eq!(lv.rpop(), Some(b"c".to_vec()));
        assert_eq!(lv.lpop(), Some(b"a".to_vec()));
        assert_eq!(lv.len(), 1);
        // Complex mutator on a packed list lazily upgrades then works.
        lv.lset(0, b"z".to_vec()).unwrap();
        assert_eq!(lv.lindex(0), Some(b"z".to_vec()));
    }

    /// Serde round-trip: packed and deque forms serialize as the same logical
    /// sequence (snapshot compatibility) and reload correctly.
    #[test]
    fn test_repr_serde_roundtrip() {
        let mut packed = ListValue::new(None);
        packed.rpush(b"one".to_vec());
        packed.rpush(b"two".to_vec());
        let bytes = serde_json::to_vec(&packed).unwrap();
        let back: ListValue = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.lrange(0, -1), vec![b"one".to_vec(), b"two".to_vec()]);

        let mut big = ListValue::new(None);
        for i in 0..(MAX_PACKED_ENTRIES + 5) {
            big.rpush(format!("v{i}").into_bytes());
        }
        let bytes = serde_json::to_vec(&big).unwrap();
        let back: ListValue = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.len(), MAX_PACKED_ENTRIES + 5);
        assert_eq!(back.lindex(3), Some(b"v3".to_vec()));
    }

    #[test]
    fn test_lpush_rpush() {
        let store = ListStore::new();

        // Test LPUSH
        let result = store.lpush("mylist", vec![b"world".to_vec()], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        let result = store.lpush("mylist", vec![b"hello".to_vec()], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        // Test RPUSH
        let result = store.rpush("mylist", vec![b"!".to_vec()], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);

        // Verify order: ["hello", "world", "!"]
        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(
            range,
            vec![b"hello".to_vec(), b"world".to_vec(), b"!".to_vec()]
        );
    }

    #[test]
    fn test_lpop_rpop() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()],
                false,
            )
            .unwrap();

        // Test LPOP
        let result = store.lpop("mylist", Some(1)).unwrap();
        assert_eq!(result, vec![b"one".to_vec()]);

        // Test RPOP
        let result = store.rpop("mylist", Some(1)).unwrap();
        assert_eq!(result, vec![b"three".to_vec()]);

        // Only "two" should remain
        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range, vec![b"two".to_vec()]);
    }

    #[test]
    fn test_lrange() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![
                    b"0".to_vec(),
                    b"1".to_vec(),
                    b"2".to_vec(),
                    b"3".to_vec(),
                    b"4".to_vec(),
                ],
                false,
            )
            .unwrap();

        // Test positive indices
        let range = store.lrange("mylist", 1, 3).unwrap();
        assert_eq!(range, vec![b"1".to_vec(), b"2".to_vec(), b"3".to_vec()]);

        // Test negative indices
        let range = store.lrange("mylist", -2, -1).unwrap();
        assert_eq!(range, vec![b"3".to_vec(), b"4".to_vec()]);

        // Test entire range
        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range.len(), 5);
    }

    #[test]
    fn test_lindex() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"zero".to_vec(), b"one".to_vec(), b"two".to_vec()],
                false,
            )
            .unwrap();

        // Test positive index
        let value = store.lindex("mylist", 1).unwrap();
        assert_eq!(value, b"one".to_vec());

        // Test negative index
        let value = store.lindex("mylist", -1).unwrap();
        assert_eq!(value, b"two".to_vec());

        // Test out of range
        let result = store.lindex("mylist", 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_lset() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"zero".to_vec(), b"one".to_vec(), b"two".to_vec()],
                false,
            )
            .unwrap();

        // Test set at index
        let result = store.lset("mylist", 1, b"ONE".to_vec());
        assert!(result.is_ok());

        let value = store.lindex("mylist", 1).unwrap();
        assert_eq!(value, b"ONE".to_vec());

        // Test set with negative index
        let result = store.lset("mylist", -1, b"TWO".to_vec());
        assert!(result.is_ok());

        let value = store.lindex("mylist", -1).unwrap();
        assert_eq!(value, b"TWO".to_vec());
    }

    #[test]
    fn test_ltrim() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![
                    b"0".to_vec(),
                    b"1".to_vec(),
                    b"2".to_vec(),
                    b"3".to_vec(),
                    b"4".to_vec(),
                ],
                false,
            )
            .unwrap();

        // Trim to keep only [1, 3]
        store.ltrim("mylist", 1, 3).unwrap();

        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range, vec![b"1".to_vec(), b"2".to_vec(), b"3".to_vec()]);
    }

    #[test]
    fn test_lrem() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![
                    b"a".to_vec(),
                    b"b".to_vec(),
                    b"a".to_vec(),
                    b"c".to_vec(),
                    b"a".to_vec(),
                ],
                false,
            )
            .unwrap();

        // Remove 2 occurrences of "a" from head
        let removed = store.lrem("mylist", 2, b"a".to_vec()).unwrap();
        assert_eq!(removed, 2);

        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range, vec![b"b".to_vec(), b"c".to_vec(), b"a".to_vec()]);
    }

    #[test]
    fn test_linsert() {
        let store = ListStore::new();
        store
            .rpush("mylist", vec![b"a".to_vec(), b"c".to_vec()], false)
            .unwrap();

        // Insert before "c"
        let len = store
            .linsert("mylist", true, b"c".to_vec(), b"b".to_vec())
            .unwrap();
        assert_eq!(len, 3);

        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn test_lpos() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                false,
            )
            .unwrap();

        let pos = store.lpos("mylist", b"b".to_vec()).unwrap();
        assert_eq!(pos, Some(1));

        let pos = store.lpos("mylist", b"z".to_vec()).unwrap();
        assert_eq!(pos, None);
    }

    #[test]
    fn test_rpoplpush() {
        let store = ListStore::new();
        store
            .rpush(
                "source",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                false,
            )
            .unwrap();

        let value = store.rpoplpush("source", "dest").unwrap();
        assert_eq!(value, b"c".to_vec());

        let source_range = store.lrange("source", 0, -1).unwrap();
        assert_eq!(source_range, vec![b"a".to_vec(), b"b".to_vec()]);

        let dest_range = store.lrange("dest", 0, -1).unwrap();
        assert_eq!(dest_range, vec![b"c".to_vec()]);
    }

    #[test]
    fn test_llen() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                false,
            )
            .unwrap();

        let len = store.llen("mylist").unwrap();
        assert_eq!(len, 3);
    }

    #[test]
    fn test_lpushx_exists() {
        let store = ListStore::new();

        // LPUSHX on non-existent key should return 0
        let result = store.lpush("mylist", vec![b"a".to_vec()], true).unwrap();
        assert_eq!(result, 0);

        // Create the list
        store.lpush("mylist", vec![b"a".to_vec()], false).unwrap();

        // Now LPUSHX should work
        let result = store.lpush("mylist", vec![b"b".to_vec()], true).unwrap();
        assert_eq!(result, 2);
    }

    #[tokio::test]
    async fn test_blpop_immediate() {
        let store = ListStore::new();
        store
            .rpush("mylist", vec![b"value".to_vec()], false)
            .unwrap();

        let (key, value) = store
            .blpop(vec!["mylist".to_string()], Some(1))
            .await
            .unwrap();
        assert_eq!(key, "mylist");
        assert_eq!(value, b"value".to_vec());
    }

    #[tokio::test]
    async fn test_blpop_timeout() {
        let store = ListStore::new();

        let result = store.blpop(vec!["mylist".to_string()], Some(1)).await;
        assert!(matches!(result, Err(SynapError::Timeout)));
    }

    #[tokio::test]
    async fn test_brpoplpush_immediate() {
        let store = ListStore::new();
        store
            .rpush("source", vec![b"value".to_vec()], false)
            .unwrap();

        let value = store.brpoplpush("source", "dest", Some(1)).await.unwrap();
        assert_eq!(value, b"value".to_vec());

        let dest_range = store.lrange("dest", 0, -1).unwrap();
        assert_eq!(dest_range, vec![b"value".to_vec()]);
    }

    #[tokio::test]
    async fn test_brpoplpush_timeout() {
        let store = ListStore::new();

        let result = store.brpoplpush("source", "dest", Some(1)).await;
        assert!(matches!(result, Err(SynapError::Timeout)));
    }

    #[test]
    fn test_index_set_and_range_edge_cases() {
        let store = ListStore::new();
        store
            .rpush(
                "l",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec(), b"d".to_vec()],
                false,
            )
            .unwrap();

        // Negative and out-of-range indices.
        assert_eq!(store.lindex("l", -1).unwrap(), b"d".to_vec());
        assert!(store.lindex("l", 99).is_err());
        assert_eq!(
            store.lrange("l", -2, -1).unwrap(),
            vec![b"c".to_vec(), b"d".to_vec()]
        );
        // start > stop yields an empty range.
        assert!(store.lrange("l", 3, 1).unwrap().is_empty());

        store.lset("l", 0, b"A".to_vec()).unwrap();
        assert_eq!(store.lindex("l", 0).unwrap(), b"A".to_vec());
        assert!(store.lset("l", 99, b"z".to_vec()).is_err());

        assert_eq!(store.llen("l").unwrap(), 4);
        assert_eq!(store.lpos("l", b"c".to_vec()).unwrap(), Some(2));
        assert_eq!(store.lpos("l", b"zzz".to_vec()).unwrap(), None);
    }

    #[test]
    fn test_trim_rem_insert() {
        let store = ListStore::new();
        store
            .rpush(
                "l",
                vec![
                    b"a".to_vec(),
                    b"b".to_vec(),
                    b"a".to_vec(),
                    b"c".to_vec(),
                    b"a".to_vec(),
                ],
                false,
            )
            .unwrap();

        // Remove 2 occurrences of "a" from head.
        assert_eq!(store.lrem("l", 2, b"a".to_vec()).unwrap(), 2);
        // Remove 1 occurrence from tail.
        assert_eq!(store.lrem("l", -1, b"a".to_vec()).unwrap(), 1);
        assert_eq!(
            store.lrange("l", 0, -1).unwrap(),
            vec![b"b".to_vec(), b"c".to_vec()]
        );

        store
            .linsert("l", true, b"c".to_vec(), b"x".to_vec())
            .unwrap();
        assert_eq!(
            store.lrange("l", 0, -1).unwrap(),
            vec![b"b".to_vec(), b"x".to_vec(), b"c".to_vec()]
        );

        store.ltrim("l", 1, 2).unwrap();
        assert_eq!(
            store.lrange("l", 0, -1).unwrap(),
            vec![b"x".to_vec(), b"c".to_vec()]
        );
    }

    #[test]
    fn test_pop_count_and_only_if_exists() {
        let store = ListStore::new();
        // only_if_exists on a missing key does nothing.
        assert_eq!(
            store.lpush("missing", vec![b"a".to_vec()], true).unwrap(),
            0
        );
        assert!(!store.exists("missing"));

        store
            .rpush(
                "l",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                false,
            )
            .unwrap();
        assert_eq!(
            store.lpop("l", Some(2)).unwrap(),
            vec![b"a".to_vec(), b"b".to_vec()]
        );
        assert_eq!(store.rpop("l", Some(5)).unwrap(), vec![b"c".to_vec()]);
        // Empty list is removed.
        assert!(!store.exists("l"));
    }

    #[test]
    fn test_stats_delete_and_memory() {
        let store = ListStore::new();
        store.rpush("a", vec![b"1".to_vec()], false).unwrap();
        store
            .rpush("b", vec![b"2".to_vec(), b"3".to_vec()], false)
            .unwrap();

        let stats = store.stats();
        assert_eq!(stats.total_lists, 2);
        assert_eq!(stats.total_elements, 3);
        store.refresh_memory();

        assert!(store.delete("a").unwrap());
        assert!(!store.delete("a").unwrap());
        assert!(store.exists("b"));
    }
}
