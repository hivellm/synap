//! aarch64 NEON implementations. Only compiled on `target_arch = "aarch64"`.

#![cfg(target_arch = "aarch64")]

use std::arch::aarch64::*;

/// NEON popcount.
///
/// # Safety
/// Caller must have verified `std::arch::is_aarch64_feature_detected!("neon")`.
#[target_feature(enable = "neon")]
pub unsafe fn popcount_neon(bytes: &[u8]) -> u64 {
    let mut acc: u64 = 0;
    let chunks = bytes.chunks_exact(16);
    let tail = chunks.remainder();
    for chunk in chunks {
        // SAFETY: chunk is exactly 16 bytes.
        let v = unsafe { vld1q_u8(chunk.as_ptr()) };
        let cnt = unsafe { vcntq_u8(v) };
        acc += unsafe { vaddlvq_u8(cnt) } as u64;
    }
    acc + super::fallback::popcount_slice(tail)
}

/// NEON bitwise AND.
///
/// # Safety
/// Caller must have verified NEON availability.
#[target_feature(enable = "neon")]
pub unsafe fn bitop_and_neon(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 16 <= len {
        let va = unsafe { vld1q_u8(a.as_ptr().add(i)) };
        let vb = unsafe { vld1q_u8(b.as_ptr().add(i)) };
        let r = unsafe { vandq_u8(va, vb) };
        unsafe { vst1q_u8(out.as_mut_ptr().add(i), r) };
        i += 16;
    }
    while i < len {
        out[i] = a[i] & b[i];
        i += 1;
    }
}

/// NEON bitwise OR.
///
/// # Safety
/// Caller must have verified NEON availability.
#[target_feature(enable = "neon")]
pub unsafe fn bitop_or_neon(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 16 <= len {
        let va = unsafe { vld1q_u8(a.as_ptr().add(i)) };
        let vb = unsafe { vld1q_u8(b.as_ptr().add(i)) };
        let r = unsafe { vorrq_u8(va, vb) };
        unsafe { vst1q_u8(out.as_mut_ptr().add(i), r) };
        i += 16;
    }
    while i < len {
        out[i] = a[i] | b[i];
        i += 1;
    }
}

/// NEON bitwise XOR.
///
/// # Safety
/// Caller must have verified NEON availability.
#[target_feature(enable = "neon")]
pub unsafe fn bitop_xor_neon(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 16 <= len {
        let va = unsafe { vld1q_u8(a.as_ptr().add(i)) };
        let vb = unsafe { vld1q_u8(b.as_ptr().add(i)) };
        let r = unsafe { veorq_u8(va, vb) };
        unsafe { vst1q_u8(out.as_mut_ptr().add(i), r) };
        i += 16;
    }
    while i < len {
        out[i] = a[i] ^ b[i];
        i += 1;
    }
}

/// NEON bitwise NOT.
///
/// # Safety
/// Caller must have verified NEON availability.
#[target_feature(enable = "neon")]
pub unsafe fn bitop_not_neon(a: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 16 <= len {
        let va = unsafe { vld1q_u8(a.as_ptr().add(i)) };
        let r = unsafe { vmvnq_u8(va) };
        unsafe { vst1q_u8(out.as_mut_ptr().add(i), r) };
        i += 16;
    }
    while i < len {
        out[i] = !a[i];
        i += 1;
    }
}

/// NEON element-wise unsigned max.
///
/// # Safety
/// Caller must have verified NEON availability.
#[target_feature(enable = "neon")]
pub unsafe fn max_reduce_u8_neon(dest: &mut [u8], src: &[u8]) {
    let len = dest.len().min(src.len());
    let mut i = 0;
    while i + 16 <= len {
        let vd = unsafe { vld1q_u8(dest.as_ptr().add(i)) };
        let vs = unsafe { vld1q_u8(src.as_ptr().add(i)) };
        let r = unsafe { vmaxq_u8(vd, vs) };
        unsafe { vst1q_u8(dest.as_mut_ptr().add(i), r) };
        i += 16;
    }
    while i < len {
        if src[i] > dest[i] {
            dest[i] = src[i];
        }
        i += 1;
    }
}
