use super::*;

#[test]
fn test_setbit_getbit() {
    let mut bitmap = BitmapValue::new(None);

    // Set bit 0 to 1
    assert_eq!(bitmap.setbit(0, 1).unwrap(), 0);
    assert_eq!(bitmap.getbit(0), 1);

    // Set bit 7 to 1 (same byte)
    assert_eq!(bitmap.setbit(7, 1).unwrap(), 0);
    assert_eq!(bitmap.getbit(7), 1);

    // Set bit 0 back to 0
    assert_eq!(bitmap.setbit(0, 0).unwrap(), 1);
    assert_eq!(bitmap.getbit(0), 0);

    // Set bit 8 to 1 (next byte)
    assert_eq!(bitmap.setbit(8, 1).unwrap(), 0);
    assert_eq!(bitmap.getbit(8), 1);
}

#[test]
fn test_bitcount() {
    let mut bitmap = BitmapValue::new(None);

    // Set bits at positions 0, 2, 4, 6 (all in first byte)
    bitmap.setbit(0, 1).unwrap();
    bitmap.setbit(2, 1).unwrap();
    bitmap.setbit(4, 1).unwrap();
    bitmap.setbit(6, 1).unwrap();

    assert_eq!(bitmap.bitcount(None, None), 4);

    // Set more bits
    bitmap.setbit(8, 1).unwrap();
    bitmap.setbit(10, 1).unwrap();

    assert_eq!(bitmap.bitcount(None, None), 6);
}

#[test]
fn test_bitpos() {
    let mut bitmap = BitmapValue::new(None);

    // Set bit at position 5
    bitmap.setbit(5, 1).unwrap();

    // Find first set bit
    assert_eq!(bitmap.bitpos(1, None, None), Some(5));

    // Find first unset bit (should be 0)
    assert_eq!(bitmap.bitpos(0, None, None), Some(0));

    // Find first set bit starting from position 6
    assert_eq!(bitmap.bitpos(1, Some(6), None), None);
}

#[test]
fn test_bitop_and() {
    let store = BitmapStore::new();

    // Create two bitmaps
    store.setbit("bitmap1", 0, 1).unwrap();
    store.setbit("bitmap1", 1, 1).unwrap();
    store.setbit("bitmap1", 2, 1).unwrap();

    store.setbit("bitmap2", 1, 1).unwrap();
    store.setbit("bitmap2", 2, 1).unwrap();
    store.setbit("bitmap2", 3, 1).unwrap();

    // AND operation
    store
        .bitop(
            BitmapOperation::And,
            "result",
            &["bitmap1".to_string(), "bitmap2".to_string()],
        )
        .unwrap();

    assert_eq!(store.getbit("result", 0).unwrap(), 0);
    assert_eq!(store.getbit("result", 1).unwrap(), 1);
    assert_eq!(store.getbit("result", 2).unwrap(), 1);
    assert_eq!(store.getbit("result", 3).unwrap(), 0);
}

#[test]
fn test_bitop_or() {
    let store = BitmapStore::new();

    store.setbit("bitmap1", 0, 1).unwrap();
    store.setbit("bitmap1", 1, 1).unwrap();

    store.setbit("bitmap2", 1, 1).unwrap();
    store.setbit("bitmap2", 2, 1).unwrap();

    store
        .bitop(
            BitmapOperation::Or,
            "result",
            &["bitmap1".to_string(), "bitmap2".to_string()],
        )
        .unwrap();

    assert_eq!(store.getbit("result", 0).unwrap(), 1);
    assert_eq!(store.getbit("result", 1).unwrap(), 1);
    assert_eq!(store.getbit("result", 2).unwrap(), 1);
    assert_eq!(store.getbit("result", 3).unwrap(), 0);
}

#[test]
fn test_bitfield_get() {
    let mut bitmap = BitmapValue::new(None);

    // Set bits manually to create value 42 (binary: 101010)
    // Using 8 bits at offset 0
    bitmap.setbit(0, 1).unwrap(); // bit 0 = 1
    bitmap.setbit(2, 1).unwrap(); // bit 2 = 1
    bitmap.setbit(4, 1).unwrap(); // bit 4 = 1
    bitmap.setbit(6, 1).unwrap(); // bit 6 = 1

    // Read as unsigned 8-bit value
    let value = bitmap.bitfield_get(0, 8, false).unwrap();
    assert_eq!(value, 0b10101010); // 170 in decimal

    // Read as signed 8-bit value (should be negative if MSB is set)
    let signed_value = bitmap.bitfield_get(0, 8, true).unwrap();
    assert_eq!(signed_value, -86); // 0b10101010 as signed = -86
}

#[test]
fn test_bitfield_set() {
    let mut bitmap = BitmapValue::new(None);

    // Set 8-bit unsigned value 42 at offset 0
    // 42 in binary (little-endian): 01010100
    let old_value = bitmap.bitfield_set(0, 8, false, 42).unwrap();
    assert_eq!(old_value, 0); // Was empty

    // Read back using bitfield_get (same encoding)
    let value = bitmap.bitfield_get(0, 8, false).unwrap();
    assert_eq!(value, 42);

    // Test setting another value
    let old_value2 = bitmap.bitfield_set(0, 8, false, 100).unwrap();
    assert_eq!(old_value2, 42); // Previous value
    let value2 = bitmap.bitfield_get(0, 8, false).unwrap();
    assert_eq!(value2, 100);
}

#[test]
fn test_bitfield_incrby_wrap() {
    let mut bitmap = BitmapValue::new(None);

    // Set 4-bit unsigned value 14 at offset 0
    bitmap.bitfield_set(0, 4, false, 14).unwrap();

    // Increment by 1 (should wrap to 0)
    let new_value = bitmap
        .bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Wrap)
        .unwrap();
    assert_eq!(new_value, 15);

    // Increment by 1 again (should wrap to 0)
    let new_value = bitmap
        .bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Wrap)
        .unwrap();
    assert_eq!(new_value, 0);
}

#[test]
fn test_bitfield_incrby_sat() {
    let mut bitmap = BitmapValue::new(None);

    // Set 4-bit unsigned value 14 at offset 0
    bitmap.bitfield_set(0, 4, false, 14).unwrap();

    // Increment by 1 (should saturate at 15)
    let new_value = bitmap
        .bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Sat)
        .unwrap();
    assert_eq!(new_value, 15);

    // Increment by 1 again (should stay at 15)
    let new_value = bitmap
        .bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Sat)
        .unwrap();
    assert_eq!(new_value, 15);
}

#[test]
fn test_bitfield_incrby_fail() {
    let mut bitmap = BitmapValue::new(None);

    // Set 4-bit unsigned value 15 at offset 0
    bitmap.bitfield_set(0, 4, false, 15).unwrap();

    // Increment by 1 (should fail)
    let result = bitmap.bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Fail);
    assert!(result.is_err());
}

#[test]
fn test_bitfield_store_operations() {
    let store = BitmapStore::new();

    // Execute multiple operations
    let operations = vec![
        BitfieldOperation::Set {
            offset: 0,
            width: 8,
            signed: false,
            value: 100,
        },
        BitfieldOperation::Get {
            offset: 0,
            width: 8,
            signed: false,
        },
        BitfieldOperation::IncrBy {
            offset: 0,
            width: 8,
            signed: false,
            increment: 50,
            overflow: BitfieldOverflow::Wrap,
        },
    ];

    let results = store.bitfield("test", &operations).unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].value, 0); // Old value was 0
    assert_eq!(results[1].value, 100); // Read back 100
    assert_eq!(results[2].value, 150); // Incremented to 150
}

#[test]
fn test_bitfield_signed_values() {
    let mut bitmap = BitmapValue::new(None);

    // Set signed 8-bit value -10
    bitmap.bitfield_set(0, 8, true, -10).unwrap();

    // Read back as signed
    let value = bitmap.bitfield_get(0, 8, true).unwrap();
    assert_eq!(value, -10);

    // Read as unsigned (should be 246)
    let unsigned_value = bitmap.bitfield_get(0, 8, false).unwrap();
    assert_eq!(unsigned_value, 246);
}

#[test]
fn test_bitfield_cross_byte_boundary() {
    let mut bitmap = BitmapValue::new(None);

    // Set 16-bit value at offset 4 (crosses byte boundary)
    bitmap.bitfield_set(4, 16, false, 0x1234).unwrap();

    // Read back
    let value = bitmap.bitfield_get(4, 16, false).unwrap();
    assert_eq!(value, 0x1234);
}

#[test]
fn test_bitfield_different_widths() {
    let mut bitmap = BitmapValue::new(None);

    // Test 4-bit unsigned
    bitmap.bitfield_set(0, 4, false, 15).unwrap();
    assert_eq!(bitmap.bitfield_get(0, 4, false).unwrap(), 15);

    // Test 12-bit unsigned
    bitmap.bitfield_set(4, 12, false, 4095).unwrap();
    assert_eq!(bitmap.bitfield_get(4, 12, false).unwrap(), 4095);

    // Test 24-bit unsigned
    bitmap.bitfield_set(16, 24, false, 16777215).unwrap();
    assert_eq!(bitmap.bitfield_get(16, 24, false).unwrap(), 16777215);
}

#[test]
fn test_bitfield_signed_negative() {
    let mut bitmap = BitmapValue::new(None);

    // Test 8-bit signed negative value
    bitmap.bitfield_set(0, 8, true, -10).unwrap();
    assert_eq!(bitmap.bitfield_get(0, 8, true).unwrap(), -10);

    // Test 16-bit signed negative value
    bitmap.bitfield_set(8, 16, true, -1000).unwrap();
    assert_eq!(bitmap.bitfield_get(8, 16, true).unwrap(), -1000);

    // Test 4-bit signed negative value
    bitmap.bitfield_set(24, 4, true, -8).unwrap();
    assert_eq!(bitmap.bitfield_get(24, 4, true).unwrap(), -8);
}

#[test]
fn test_bitfield_incrby_negative_increment() {
    let mut bitmap = BitmapValue::new(None);

    // Set initial value
    bitmap.bitfield_set(0, 8, false, 100).unwrap();

    // Decrement by 50
    let new_value = bitmap
        .bitfield_incrby(0, 8, false, -50, BitfieldOverflow::Wrap)
        .unwrap();
    assert_eq!(new_value, 50);

    // Decrement by 100 (should wrap)
    let new_value = bitmap
        .bitfield_incrby(0, 8, false, -100, BitfieldOverflow::Wrap)
        .unwrap();
    assert_eq!(new_value, 206); // 50 - 100 wraps to 206 (256 - 50)
}

#[test]
fn test_bitfield_incrby_signed_wrap() {
    let mut bitmap = BitmapValue::new(None);

    // Set signed 8-bit value to 127 (max positive)
    bitmap.bitfield_set(0, 8, true, 127).unwrap();

    // Increment by 1 (should wrap to -128)
    let new_value = bitmap
        .bitfield_incrby(0, 8, true, 1, BitfieldOverflow::Wrap)
        .unwrap();
    assert_eq!(new_value, -128);

    // Increment by 1 again (should wrap to -127)
    let new_value = bitmap
        .bitfield_incrby(0, 8, true, 1, BitfieldOverflow::Wrap)
        .unwrap();
    assert_eq!(new_value, -127);
}

#[test]
fn test_bitfield_incrby_signed_sat() {
    let mut bitmap = BitmapValue::new(None);

    // Set signed 8-bit value to 127 (max positive)
    bitmap.bitfield_set(0, 8, true, 127).unwrap();

    // Increment by 1 (should saturate at 127)
    let new_value = bitmap
        .bitfield_incrby(0, 8, true, 1, BitfieldOverflow::Sat)
        .unwrap();
    assert_eq!(new_value, 127);

    // Set to -128 (min negative)
    bitmap.bitfield_set(0, 8, true, -128).unwrap();

    // Decrement by 1 (should saturate at -128)
    let new_value = bitmap
        .bitfield_incrby(0, 8, true, -1, BitfieldOverflow::Sat)
        .unwrap();
    assert_eq!(new_value, -128);
}

#[test]
fn test_bitfield_multiple_operations() {
    let store = BitmapStore::new();

    // Execute complex sequence of operations
    let operations = vec![
        // Set multiple fields
        BitfieldOperation::Set {
            offset: 0,
            width: 8,
            signed: false,
            value: 100,
        },
        BitfieldOperation::Set {
            offset: 8,
            width: 8,
            signed: false,
            value: 200,
        },
        BitfieldOperation::Set {
            offset: 16,
            width: 8,
            signed: false,
            value: 50,
        },
        // Read them back
        BitfieldOperation::Get {
            offset: 0,
            width: 8,
            signed: false,
        },
        BitfieldOperation::Get {
            offset: 8,
            width: 8,
            signed: false,
        },
        BitfieldOperation::Get {
            offset: 16,
            width: 8,
            signed: false,
        },
        // Increment middle field
        BitfieldOperation::IncrBy {
            offset: 8,
            width: 8,
            signed: false,
            increment: 50,
            overflow: BitfieldOverflow::Wrap,
        },
    ];

    let results = store.bitfield("multi", &operations).unwrap();
    assert_eq!(results.len(), 7);
    assert_eq!(results[0].value, 0); // Old value at offset 0
    assert_eq!(results[1].value, 0); // Old value at offset 8
    assert_eq!(results[2].value, 0); // Old value at offset 16
    assert_eq!(results[3].value, 100); // Read back offset 0
    assert_eq!(results[4].value, 200); // Read back offset 8
    assert_eq!(results[5].value, 50); // Read back offset 16
    assert_eq!(results[6].value, 250); // Incremented offset 8
}

#[test]
fn test_bitfield_overlapping_fields() {
    let mut bitmap = BitmapValue::new(None);

    // Set 8-bit value at offset 0
    bitmap.bitfield_set(0, 8, false, 0xAA).unwrap(); // 10101010

    // Set 4-bit value at offset 4 (overlaps with first field)
    bitmap.bitfield_set(4, 4, false, 0xF).unwrap(); // 1111

    // Read back 8-bit value (should be modified)
    let value = bitmap.bitfield_get(0, 8, false).unwrap();
    assert_eq!(value, 0xFA); // 11111010
}

#[test]
fn test_bitfield_large_offset() {
    let mut bitmap = BitmapValue::new(None);

    // Set value at large offset (beyond initial capacity)
    bitmap.bitfield_set(1000, 8, false, 42).unwrap();

    // Read back
    let value = bitmap.bitfield_get(1000, 8, false).unwrap();
    assert_eq!(value, 42);

    // Verify earlier bits are still 0
    let value_before = bitmap.bitfield_get(0, 8, false).unwrap();
    assert_eq!(value_before, 0);
}

#[test]
fn test_bitfield_64bit_value() {
    let mut bitmap = BitmapValue::new(None);

    // Set 64-bit unsigned value
    bitmap
        .bitfield_set(0, 64, false, 0x1234567890ABCDEF)
        .unwrap();

    // Read back
    let value = bitmap.bitfield_get(0, 64, false).unwrap();
    assert_eq!(value, 0x1234567890ABCDEF);
}

#[test]
fn test_bitfield_incrby_fail_unsigned() {
    let mut bitmap = BitmapValue::new(None);

    // Set 4-bit unsigned value to max (15)
    bitmap.bitfield_set(0, 4, false, 15).unwrap();

    // Try to increment (should fail)
    let result = bitmap.bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Fail);
    assert!(result.is_err());

    // Try to increment by large value (should fail)
    let result = bitmap.bitfield_incrby(0, 4, false, 100, BitfieldOverflow::Fail);
    assert!(result.is_err());
}

#[test]
fn test_bitfield_incrby_fail_signed() {
    let mut bitmap = BitmapValue::new(None);

    // Set 4-bit signed value to max (7)
    bitmap.bitfield_set(0, 4, true, 7).unwrap();

    // Try to increment (should fail)
    let result = bitmap.bitfield_incrby(0, 4, true, 1, BitfieldOverflow::Fail);
    assert!(result.is_err());

    // Set to min (-8)
    bitmap.bitfield_set(0, 4, true, -8).unwrap();

    // Try to decrement (should fail)
    let result = bitmap.bitfield_incrby(0, 4, true, -1, BitfieldOverflow::Fail);
    assert!(result.is_err());
}

#[test]
fn test_bitfield_get_unset_bits() {
    let bitmap = BitmapValue::new(None);

    // Read from empty bitmap (should return 0)
    let value = bitmap.bitfield_get(0, 8, false).unwrap();
    assert_eq!(value, 0);

    // Read from offset beyond bitmap size
    let value = bitmap.bitfield_get(1000, 8, false).unwrap();
    assert_eq!(value, 0);
}

#[test]
fn test_bitfield_partial_read() {
    let mut bitmap = BitmapValue::new(None);

    // Set 8-bit value
    bitmap.bitfield_set(0, 8, false, 0xFF).unwrap();

    // Read only 4 bits (should get lower 4 bits)
    let value = bitmap.bitfield_get(0, 4, false).unwrap();
    assert_eq!(value, 0xF); // Lower 4 bits of 0xFF

    // Read upper 4 bits
    let value = bitmap.bitfield_get(4, 4, false).unwrap();
    assert_eq!(value, 0xF); // Upper 4 bits of 0xFF
}

#[test]
fn test_bitfield_store_expiration() {
    let store = BitmapStore::new();

    // Create bitmap with operations
    let operations = vec![BitfieldOperation::Set {
        offset: 0,
        width: 8,
        signed: false,
        value: 42,
    }];

    let results = store.bitfield("expire_test", &operations).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].value, 0);

    // Read back
    let read_ops = vec![BitfieldOperation::Get {
        offset: 0,
        width: 8,
        signed: false,
    }];
    let results = store.bitfield("expire_test", &read_ops).unwrap();
    assert_eq!(results[0].value, 42);
}

#[test]
fn test_bitfield_invalid_width() {
    let mut bitmap = BitmapValue::new(None);

    // Width 0 should fail
    let result = bitmap.bitfield_set(0, 0, false, 42);
    assert!(result.is_err());

    // Width > 64 should fail
    let result = bitmap.bitfield_set(0, 65, false, 42);
    assert!(result.is_err());

    // Width 0 for GET should fail
    let result = bitmap.bitfield_get(0, 0, false);
    assert!(result.is_err());
}

#[test]
fn test_bitfield_signed_unsigned_interop() {
    let mut bitmap = BitmapValue::new(None);

    // Set as unsigned 8-bit value 200
    bitmap.bitfield_set(0, 8, false, 200).unwrap();

    // Read as signed (should be negative)
    let signed_value = bitmap.bitfield_get(0, 8, true).unwrap();
    assert_eq!(signed_value, -56); // 200 as signed 8-bit is -56

    // Set as signed negative value
    bitmap.bitfield_set(8, 8, true, -10).unwrap();

    // Read as unsigned (should be 246)
    let unsigned_value = bitmap.bitfield_get(8, 8, false).unwrap();
    assert_eq!(unsigned_value, 246); // -10 as unsigned 8-bit is 246
}

#[test]
fn test_bitfield_32bit_values() {
    let mut bitmap = BitmapValue::new(None);

    // Test 32-bit unsigned
    bitmap.bitfield_set(0, 32, false, 0xFFFFFFFF).unwrap();
    assert_eq!(bitmap.bitfield_get(0, 32, false).unwrap(), 0xFFFFFFFF);

    // Test 32-bit signed positive
    bitmap.bitfield_set(32, 32, true, 2147483647).unwrap();
    assert_eq!(bitmap.bitfield_get(32, 32, true).unwrap(), 2147483647);

    // Test 32-bit signed negative
    bitmap.bitfield_set(64, 32, true, -2147483648).unwrap();
    assert_eq!(bitmap.bitfield_get(64, 32, true).unwrap(), -2147483648);
}
