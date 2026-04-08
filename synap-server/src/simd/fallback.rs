//! Portable scalar fallback implementations for the SIMD module.
//!
//! These are 100% safe Rust and rely on LLVM auto-vectorisation. They are
//! used on every target when the `simd` feature is disabled, and as the
//! last resort on targets without a dedicated fast path.

/// Count set bits in `bytes` using 64-bit chunks (`u64::count_ones`).
#[inline]
pub fn popcount_slice(bytes: &[u8]) -> u64 {
    let mut count: u64 = 0;
    let chunks = bytes.chunks_exact(8);
    let remainder = chunks.remainder();
    for chunk in chunks {
        let v = u64::from_ne_bytes(chunk.try_into().unwrap());
        count += v.count_ones() as u64;
    }
    for &b in remainder {
        count += b.count_ones() as u64;
    }
    count
}

/// Count registers that are non-zero. Used by HyperLogLog small-range
/// correction (which needs the number of empty registers).
#[inline]
pub fn count_nonzero(bytes: &[u8]) -> u64 {
    let mut count: u64 = 0;
    for &b in bytes {
        if b != 0 {
            count += 1;
        }
    }
    count
}

/// Bitwise AND, writing `a & b` into a new `Vec<u8>` sized to `min(a, b)`.
#[inline]
pub fn bitop_and(a: &[u8], b: &[u8]) -> Vec<u8> {
    let len = a.len().min(b.len());
    let mut out = vec![0u8; len];
    for i in 0..len {
        out[i] = a[i] & b[i];
    }
    out
}

/// Bitwise OR, writing `a | b` into a new `Vec<u8>` sized to `max(a, b)`.
#[inline]
pub fn bitop_or(a: &[u8], b: &[u8]) -> Vec<u8> {
    let len = a.len().max(b.len());
    let mut out = vec![0u8; len];
    let min = a.len().min(b.len());
    for i in 0..min {
        out[i] = a[i] | b[i];
    }
    if a.len() > b.len() {
        out[min..].copy_from_slice(&a[min..]);
    } else {
        out[min..].copy_from_slice(&b[min..]);
    }
    out
}

/// Bitwise XOR, writing `a ^ b` into a new `Vec<u8>` sized to `max(a, b)`.
#[inline]
pub fn bitop_xor(a: &[u8], b: &[u8]) -> Vec<u8> {
    let len = a.len().max(b.len());
    let mut out = vec![0u8; len];
    let min = a.len().min(b.len());
    for i in 0..min {
        out[i] = a[i] ^ b[i];
    }
    if a.len() > b.len() {
        out[min..].copy_from_slice(&a[min..]);
    } else {
        out[min..].copy_from_slice(&b[min..]);
    }
    out
}

/// Bitwise NOT of `a` into a new buffer.
#[inline]
pub fn bitop_not(a: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; a.len()];
    for i in 0..a.len() {
        out[i] = !a[i];
    }
    out
}

/// Element-wise maximum of two byte slices, writing into `dest`.
/// If `src` is longer than `dest`, the extra tail is appended.
#[inline]
pub fn max_reduce_u8(dest: &mut Vec<u8>, src: &[u8]) {
    let min = dest.len().min(src.len());
    for i in 0..min {
        if src[i] > dest[i] {
            dest[i] = src[i];
        }
    }
    if src.len() > dest.len() {
        dest.extend_from_slice(&src[dest.len()..]);
    }
}

/// Find the first byte offset whose bit `value` (0 or 1) is set.
#[inline]
pub fn bitpos(bytes: &[u8], value: u8) -> Option<usize> {
    let target = if value == 0 { 0xFF } else { 0x00 };
    for (i, &b) in bytes.iter().enumerate() {
        if b != target {
            let byte = if value == 0 { !b } else { b };
            let bit_from_msb = byte.leading_zeros() as usize;
            return Some(i * 8 + bit_from_msb);
        }
    }
    None
}
