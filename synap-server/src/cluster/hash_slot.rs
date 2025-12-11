//! Hash Slot Algorithm - CRC16 mod 16384
//!
//! Redis-compatible hash slot calculation using CRC16.

use crate::cluster::types::TOTAL_SLOTS;

/// CRC16 lookup table (Redis-compatible)
const CRC16_TABLE: [u16; 256] = [
    0x0000, 0x1021, 0x2042, 0x3063, 0x4084, 0x50a5, 0x60c6, 0x70e7, 0x8108, 0x9129, 0xa14a, 0xb16b,
    0xc18c, 0xd1ad, 0xe1ce, 0xf1ef, 0x1231, 0x0210, 0x3273, 0x2252, 0x52b5, 0x4294, 0x72f7, 0x62d6,
    0x9339, 0x8318, 0xb37b, 0xa35a, 0xd3bd, 0xc39c, 0xf3ff, 0xe3de, 0x2462, 0x3443, 0x0420, 0x1401,
    0x64e6, 0x74c7, 0x44a4, 0x5485, 0xa56a, 0xb54b, 0x8528, 0x9509, 0xe5ee, 0xf5cf, 0xc5ac, 0xd58d,
    0x3653, 0x2672, 0x1611, 0x0630, 0x76d7, 0x66f6, 0x5695, 0x46b4, 0xb75b, 0xa77a, 0x9719, 0x8738,
    0xf7df, 0xe7fe, 0xd79d, 0xc7bc, 0x48c4, 0x58e5, 0x6886, 0x78a7, 0x0840, 0x1861, 0x2802, 0x3823,
    0xc9cc, 0xd9ed, 0xe98e, 0xf9af, 0x8948, 0x9969, 0xa90a, 0xb92b, 0x5af5, 0x4ad4, 0x7ab7, 0x6a96,
    0x1a71, 0x0a50, 0x3a33, 0x2a12, 0xdbfd, 0xcbdc, 0xfbbf, 0xeb9e, 0x9b79, 0x8b58, 0xbb3b, 0xab1a,
    0x6ca6, 0x7c87, 0x4ce4, 0x5cc5, 0x2c22, 0x3c03, 0x0c60, 0x1c41, 0xedae, 0xfd8f, 0xcdec, 0xddcd,
    0xad2a, 0xbd0b, 0x8d68, 0x9d49, 0x7e97, 0x6eb6, 0x5ed5, 0x4ef4, 0x3e13, 0x2e32, 0x1e51, 0x0e70,
    0xff9f, 0xefbe, 0xdfdd, 0xcffc, 0xbf1b, 0xaf3a, 0x9f59, 0x8f78, 0x9188, 0x81a9, 0xb1ca, 0xa1eb,
    0xd10c, 0xc12d, 0xf14e, 0xe16f, 0x1080, 0x00a1, 0x30c2, 0x20e3, 0x5004, 0x4025, 0x7046, 0x6067,
    0x83b9, 0x9398, 0xa3fb, 0xb3da, 0xc33d, 0xd31c, 0xe37f, 0xf35e, 0x02b1, 0x1290, 0x22f3, 0x32d2,
    0x4235, 0x5214, 0x6277, 0x7256, 0xb5ea, 0xa5cb, 0x95a8, 0x8589, 0xf56e, 0xe54f, 0xd52c, 0xc50d,
    0x34e2, 0x24c3, 0x14a0, 0x0481, 0x7466, 0x6447, 0x5424, 0x4405, 0xa7db, 0xb7fa, 0x8799, 0x97b8,
    0xe75f, 0xf77e, 0xc71d, 0xd73c, 0x26d3, 0x36f2, 0x0691, 0x16b0, 0x6657, 0x7676, 0x4615, 0x5634,
    0xd94c, 0xc96d, 0xf90e, 0xe92f, 0x99c8, 0x89e9, 0xb98a, 0xa9ab, 0x5844, 0x4865, 0x7806, 0x6827,
    0x18c0, 0x08e1, 0x3882, 0x28a3, 0xcb7d, 0xdb5c, 0xeb3f, 0xfb1e, 0x8bf9, 0x9bd8, 0xabbb, 0xbb9a,
    0x4a75, 0x5a54, 0x6a37, 0x7a16, 0x0af1, 0x1ad0, 0x2ab3, 0x3a92, 0xfd2e, 0xed0f, 0xdd6c, 0xcd4d,
    0xbdaa, 0xad8b, 0x9de8, 0x8dc9, 0x7c26, 0x6c07, 0x5c64, 0x4c45, 0x3ca2, 0x2c83, 0x1ce0, 0x0cc1,
    0xef1f, 0xff3e, 0xcf5d, 0xdf7c, 0xaf9b, 0xbfba, 0x8fd9, 0x9ff8, 0x6e17, 0x7e36, 0x4e55, 0x5e74,
    0x2e93, 0x3eb2, 0x0ed1, 0x1ef0,
];

/// Calculate CRC16 checksum (Redis-compatible)
fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        let idx = ((crc >> 8) ^ u16::from(byte)) as usize;
        crc = (crc << 8) ^ CRC16_TABLE[idx];
    }
    crc
}

/// Extract hash tag from key (Redis-compatible)
///
/// Hash tags allow multiple keys to be stored on the same node.
/// Format: `{tag}key` or `key{tag}` - only the tag is hashed.
fn extract_hash_tag(key: &str) -> Option<&str> {
    // Look for {tag} pattern
    if let Some(start) = key.find('{') {
        if let Some(end) = key[start + 1..].find('}') {
            let tag = &key[start + 1..start + 1 + end];
            if !tag.is_empty() {
                return Some(tag);
            }
        }
    }
    None
}

/// Calculate hash slot for a key (CRC16 mod 16384)
///
/// # Arguments
/// * `key` - The key to hash
///
/// # Returns
/// Hash slot number (0-16383)
///
/// # Example
/// ```
/// use synap_server::cluster::hash_slot::hash_slot;
///
/// let slot = hash_slot("user:1001");
/// assert!(slot < 16384);
///
/// // Hash tags ensure same slot
/// let slot1 = hash_slot("user:{1001}:profile");
/// let slot2 = hash_slot("user:{1001}:settings");
/// assert_eq!(slot1, slot2);
/// ```
pub fn hash_slot(key: &str) -> u16 {
    // Extract hash tag if present
    let hash_key = extract_hash_tag(key).unwrap_or(key);
    let crc = crc16(hash_key.as_bytes());
    crc % TOTAL_SLOTS
}

/// Hash slot type wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HashSlot(u16);

impl HashSlot {
    /// Create a new hash slot (validates range)
    pub fn new(slot: u16) -> Self {
        assert!(slot < TOTAL_SLOTS, "Slot must be < 16384");
        Self(slot)
    }

    /// Get the slot number
    pub fn value(&self) -> u16 {
        self.0
    }

    /// Calculate hash slot from key
    pub fn from_key(key: &str) -> Self {
        Self(hash_slot(key))
    }
}

impl From<u16> for HashSlot {
    fn from(slot: u16) -> Self {
        Self::new(slot)
    }
}

impl From<HashSlot> for u16 {
    fn from(slot: HashSlot) -> Self {
        slot.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_slot_basic() {
        let slot1 = hash_slot("user:1001");
        let slot2 = hash_slot("user:1002");

        // Should be valid slot numbers
        assert!(slot1 < TOTAL_SLOTS);
        assert!(slot2 < TOTAL_SLOTS);

        // Different keys should (usually) have different slots
        // (not guaranteed, but very likely)
    }

    #[test]
    fn test_hash_tag() {
        // Hash tags ensure same slot
        let slot1 = hash_slot("user:{1001}:profile");
        let slot2 = hash_slot("user:{1001}:settings");
        let slot3 = hash_slot("{1001}");

        assert_eq!(slot1, slot2);
        assert_eq!(slot1, slot3);

        // Different tags = different slots
        let slot4 = hash_slot("user:{1002}:profile");
        assert_ne!(slot1, slot4);
    }

    #[test]
    fn test_hash_slot_consistency() {
        // Same key should always produce same slot
        let key = "test:key:12345";
        let slot1 = hash_slot(key);
        let slot2 = hash_slot(key);
        assert_eq!(slot1, slot2);
    }

    #[test]
    fn test_hash_slot_distribution() {
        // Test that slots are reasonably distributed
        let mut slots = std::collections::HashSet::new();
        for i in 0..1000 {
            let key = format!("key:{}", i);
            slots.insert(hash_slot(&key));
        }

        // Should have good distribution (at least 100 unique slots)
        assert!(slots.len() > 100);
    }

    #[test]
    fn test_crc16() {
        // Test CRC16 calculation
        let data = b"test";
        let crc = crc16(data);
        assert!(crc > 0);

        // Same input = same output
        assert_eq!(crc16(data), crc);
    }

    #[test]
    fn test_hash_slot_wrapper() {
        let slot = HashSlot::from_key("user:1001");
        assert!(slot.value() < TOTAL_SLOTS);

        let slot2 = HashSlot::new(5000);
        assert_eq!(slot2.value(), 5000);
    }
}
