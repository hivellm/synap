//! x86_64 SSE2 / AVX2 implementations of the SIMD primitives.
//!
//! These functions are only compiled on `target_arch = "x86_64"`. Each
//! `#[target_feature]` entry point is `unsafe` and MUST only be called
//! after a successful `is_x86_feature_detected!` check — the public
//! dispatch in `super::mod` enforces that.

#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::*;

/// AVX2 popcount using the Harley–Seal style byte shuffle.
///
/// # Safety
/// Caller MUST have verified `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
pub unsafe fn popcount_avx2(bytes: &[u8]) -> u64 {
    let lookup = _mm256_setr_epi8(
        0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3,
        3, 4,
    );
    let low_mask = _mm256_set1_epi8(0x0f);
    let mut acc = _mm256_setzero_si256();
    let chunks = bytes.chunks_exact(32);
    let tail = chunks.remainder();
    for chunk in chunks {
        // SAFETY: chunk is exactly 32 bytes; loadu does not require alignment.
        let v = unsafe { _mm256_loadu_si256(chunk.as_ptr() as *const __m256i) };
        let lo = _mm256_and_si256(v, low_mask);
        let hi = _mm256_and_si256(_mm256_srli_epi16(v, 4), low_mask);
        let cnt_lo = _mm256_shuffle_epi8(lookup, lo);
        let cnt_hi = _mm256_shuffle_epi8(lookup, hi);
        let sum = _mm256_add_epi8(cnt_lo, cnt_hi);
        acc = _mm256_add_epi64(acc, _mm256_sad_epu8(sum, _mm256_setzero_si256()));
    }

    // Horizontal sum of 4 x u64 lanes.
    let mut tmp = [0u64; 4];
    // SAFETY: tmp is 32 bytes, unaligned store is fine.
    unsafe { _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc) };
    let mut total = tmp[0] + tmp[1] + tmp[2] + tmp[3];
    total += super::fallback::popcount_slice(tail);
    total
}

/// AVX2 bitwise AND — writes `a & b` into `out`, sized to `min(a.len(), b.len())`.
///
/// # Safety
/// Caller must have verified `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
pub unsafe fn bitop_and_avx2(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 32 <= len {
        // SAFETY: bounds checked by the `while` condition.
        // SAFETY: 32 bytes remain at offset i in a, b and out.
        let va = unsafe { _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i) };
        let vb = unsafe { _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i) };
        let r = _mm256_and_si256(va, vb);
        // SAFETY: 32 bytes remain at offset i in out.
        unsafe { _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, r) };
        i += 32;
    }
    while i < len {
        out[i] = a[i] & b[i];
        i += 1;
    }
}

/// AVX2 bitwise OR over the common prefix of `a` and `b`. Tail bytes from
/// the longer input are NOT copied; the caller handles that.
///
/// # Safety
/// Caller must have verified `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
pub unsafe fn bitop_or_avx2(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 32 <= len {
        // SAFETY: 32 bytes remain in a, b, out at offset i.
        let va = unsafe { _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i) };
        let vb = unsafe { _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i) };
        let r = _mm256_or_si256(va, vb);
        // SAFETY: 32 bytes remain in out at offset i.
        unsafe { _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, r) };
        i += 32;
    }
    while i < len {
        out[i] = a[i] | b[i];
        i += 1;
    }
}

/// AVX2 bitwise XOR over the common prefix of `a` and `b`.
///
/// # Safety
/// Caller must have verified `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
pub unsafe fn bitop_xor_avx2(a: &[u8], b: &[u8], out: &mut [u8]) {
    let len = out.len();
    let mut i = 0;
    while i + 32 <= len {
        // SAFETY: 32 bytes remain in a, b, out at offset i.
        let va = unsafe { _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i) };
        let vb = unsafe { _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i) };
        let r = _mm256_xor_si256(va, vb);
        // SAFETY: 32 bytes remain in out at offset i.
        unsafe { _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, r) };
        i += 32;
    }
    while i < len {
        out[i] = a[i] ^ b[i];
        i += 1;
    }
}

/// AVX2 bitwise NOT.
///
/// # Safety
/// Caller must have verified `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
pub unsafe fn bitop_not_avx2(a: &[u8], out: &mut [u8]) {
    let mask = _mm256_set1_epi8(-1);
    let len = out.len();
    let mut i = 0;
    while i + 32 <= len {
        // SAFETY: 32 bytes remain in a at offset i.
        let va = unsafe { _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i) };
        let r = _mm256_xor_si256(va, mask);
        // SAFETY: 32 bytes remain in out at offset i.
        unsafe { _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, r) };
        i += 32;
    }
    while i < len {
        out[i] = !a[i];
        i += 1;
    }
}

/// AVX2 element-wise unsigned max, writing into `dest` over the common
/// prefix with `src`. Tail bytes are NOT appended; the caller handles that.
///
/// # Safety
/// Caller must have verified `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
pub unsafe fn max_reduce_u8_avx2(dest: &mut [u8], src: &[u8]) {
    let len = dest.len().min(src.len());
    let mut i = 0;
    while i + 32 <= len {
        // SAFETY: 32 bytes remain in dest, src at offset i.
        let vd = unsafe { _mm256_loadu_si256(dest.as_ptr().add(i) as *const __m256i) };
        let vs = unsafe { _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i) };
        let r = _mm256_max_epu8(vd, vs);
        // SAFETY: 32 bytes remain in dest at offset i.
        unsafe { _mm256_storeu_si256(dest.as_mut_ptr().add(i) as *mut __m256i, r) };
        i += 32;
    }
    while i < len {
        if src[i] > dest[i] {
            dest[i] = src[i];
        }
        i += 1;
    }
}
